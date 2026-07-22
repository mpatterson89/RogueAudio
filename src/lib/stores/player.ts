import { writable, get } from "svelte/store";
import { getAudioEngine } from "$lib/audio/engine";
import { progressApi } from "$lib/api/progress";
import { plexApi } from "$lib/api/plex";
import type {
  AudiobookSummary,
  BookChapter,
  PlaybackTrack,
  SleepTimerState,
} from "$lib/types/models";

export const PLAYBACK_RATES = [0.8, 0.9, 1, 1.1, 1.2, 1.25, 1.5, 1.75, 2] as const;

export interface LoadBookOptions {
  /** Start at this book-level position (seconds). Overrides saved resume. */
  startSec?: number;
  /** When true, ignore local bookmark and use startSec / 0. */
  ignoreResume?: boolean;
  autoplay?: boolean;
}

interface PlayerState {
  book: AudiobookSummary | null;
  serverId: string | null;
  tracks: PlaybackTrack[];
  trackIndex: number;
  /** Chapter markers for the loaded book (empty if unsupported). */
  chapters: BookChapter[];
  /** Book-level position (across tracks), seconds */
  positionSec: number;
  /** Book-level duration, seconds */
  durationSec: number;
  playing: boolean;
  rate: number;
  sleep: SleepTimerState;
  error: string | null;
  ready: boolean;
  loading: boolean;
}

const initialSleep: SleepTimerState = {
  mode: "off",
  minutes: 30,
  endsAt: null,
  chapterEndMs: null,
  chapterTitle: null,
  fadeSeconds: 15,
};

const initial: PlayerState = {
  book: null,
  serverId: null,
  tracks: [],
  trackIndex: 0,
  chapters: [],
  positionSec: 0,
  durationSec: 0,
  playing: false,
  rate: 1,
  sleep: initialSleep,
  error: null,
  ready: false,
  loading: false,
};

function trackOffsets(tracks: PlaybackTrack[]): number[] {
  const offsets: number[] = [];
  let acc = 0;
  for (const t of tracks) {
    offsets.push(acc);
    acc += (t.durationMs ?? 0) / 1000;
  }
  return offsets;
}

function totalDurationSec(tracks: PlaybackTrack[], totalMs?: number | null): number {
  if (totalMs && totalMs > 0) return totalMs / 1000;
  const sum = tracks.reduce((a, t) => a + (t.durationMs ?? 0), 0);
  return sum > 0 ? sum / 1000 : 0;
}

/** Map book-level seconds → track index + offset within that track. */
function mapBookPosition(
  tracks: PlaybackTrack[],
  bookSec: number,
): { trackIndex: number; seekInTrack: number } {
  const offsets = trackOffsets(tracks);
  const clamped = Math.max(0, bookSec);
  for (let i = 0; i < tracks.length; i++) {
    const start = offsets[i] ?? 0;
    const len = (tracks[i].durationMs ?? 0) / 1000;
    const end =
      i === tracks.length - 1
        ? Number.POSITIVE_INFINITY
        : start + Math.max(len, 0.001);
    if (clamped >= start && clamped < end) {
      return { trackIndex: i, seekInTrack: Math.max(0, clamped - start) };
    }
  }
  const last = Math.max(0, tracks.length - 1);
  return { trackIndex: last, seekInTrack: 0 };
}

/**
 * Append/replace query params on a Plex transcoder URL without using the URL
 * class (avoids path-normalization quirks with `/music/:/transcode/...`).
 */
function withQuery(url: string, patch: Record<string, string | null>): string {
  const qIndex = url.indexOf("?");
  const base = qIndex >= 0 ? url.slice(0, qIndex) : url;
  const qs = qIndex >= 0 ? url.slice(qIndex + 1) : "";
  const map = new Map<string, string>();
  if (qs) {
    for (const part of qs.split("&")) {
      if (!part) continue;
      const eq = part.indexOf("=");
      const k = eq >= 0 ? part.slice(0, eq) : part;
      const v = eq >= 0 ? part.slice(eq + 1) : "";
      map.set(k, v);
    }
  }
  for (const [k, v] of Object.entries(patch)) {
    if (v === null) map.delete(k);
    else map.set(k, encodeURIComponent(v));
  }
  const out = Array.from(map.entries())
    .map(([k, v]) => `${k}=${v}`)
    .join("&");
  return `${base}?${out}`;
}

/**
 * Progressive transcoder streams often won't HTML5-seek. Prefer baking
 * start offset into the URL.
 *
 * IMPORTANT: Plex universal transcoder `offset` is in **seconds**, not ms.
 * Sending chapter startTimeOffset (ms) as offset seeks past EOF → empty
 * stream → WebKit media error 4.
 */
function streamUrlAt(url: string, offsetSec: number): string {
  const sec = Math.max(0, Math.floor(offsetSec));
  return withQuery(url, {
    session: crypto.randomUUID(),
    offset: sec >= 1 ? String(sec) : null,
  });
}

function createPlayerStore() {
  const store = writable<PlayerState>(initial);
  const { subscribe, update } = store;
  const engine = getAudioEngine();

  let sleepInterval: ReturnType<typeof setInterval> | null = null;
  let progressInterval: ReturnType<typeof setInterval> | null = null;
  let lastProgressFlush = 0;
  let loadGen = 0;
  /** Offset baked into the currently loaded stream URL (seconds into track). */
  let streamBaseOffsetSec = 0;

  // Persist when tab/window is hidden
  if (typeof window !== "undefined") {
    window.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") {
        const s = get(store);
        if (s.book && s.ready) {
          void flushProgress(s.playing ? "playing" : "paused");
        }
      }
    });
    window.addEventListener("pagehide", () => {
      const s = get(store);
      if (s.book && s.ready) {
        void flushProgress(s.playing ? "playing" : "paused");
      }
    });
  }

  engine.on((event) => {
    const s = get(store);
    if (s.loading || !s.ready || s.tracks.length === 0) return;

    const offsets = trackOffsets(s.tracks);

    if (event === "timeupdate" || event === "durationchange" || event === "loadedmetadata") {
      // Stream may already start mid-track via transcoder offset
      const trackPos = streamBaseOffsetSec + engine.getPosition();
      const bookPos = (offsets[s.trackIndex] ?? 0) + trackPos;
      update((st) => ({
        ...st,
        positionSec: bookPos,
        durationSec: st.durationSec,
      }));
    }

    if (event === "playing") {
      update((st) => ({ ...st, playing: true }));
    }
    if (event === "paused") {
      update((st) => ({ ...st, playing: false }));
    }

    if (event === "ended") {
      void advanceTrack();
    }

    if (event === "error") {
      update((st) => ({
        ...st,
        error: "Playback error — stream may be unreachable from this network",
        playing: false,
        loading: false,
      }));
      stopProgressLoop();
      void flushProgress("paused");
    }
  });

  async function flushProgress(state: "playing" | "paused" | "stopped") {
    const s = get(store);
    if (!s.book) return;
    // Don't wipe bookmarks with a 0 write while still loading
    if (s.loading && s.positionSec <= 0) return;
    try {
      await progressApi.report({
        ratingKey: s.book.ratingKey,
        state,
        timeMs: Math.floor(Math.max(0, s.positionSec) * 1000),
        durationMs: s.durationSec ? Math.floor(s.durationSec * 1000) : null,
        speed: s.rate,
        trackIndex: s.tracks.length ? s.trackIndex : null,
      });
      lastProgressFlush = Date.now();
    } catch {
      /* best-effort */
    }
  }

  function startProgressLoop() {
    stopProgressLoop();
    progressInterval = setInterval(() => {
      const s = get(store);
      if (!s.playing || !s.book) return;
      // Save more often so restarts land near the true position
      if (Date.now() - lastProgressFlush > 5_000) {
        void flushProgress("playing");
      }
    }, 2000);
  }

  function stopProgressLoop() {
    if (progressInterval) {
      clearInterval(progressInterval);
      progressInterval = null;
    }
  }

  function clearSleepTimer() {
    if (sleepInterval) {
      clearInterval(sleepInterval);
      sleepInterval = null;
    }
  }

  function fireSleepStop() {
    engine.pause();
    void flushProgress("paused");
    update((st) => ({
      ...st,
      playing: false,
      sleep: { ...initialSleep },
    }));
    clearSleepTimer();
    stopProgressLoop();
  }

  function armSleepWatcher() {
    clearSleepTimer();
    sleepInterval = setInterval(() => {
      const s = get(store);
      if (s.sleep.mode === "off") return;

      // Wall-clock timer (custom / preset minutes)
      if (s.sleep.mode === "duration" && s.sleep.endsAt != null) {
        if (s.sleep.endsAt - Date.now() <= 0) {
          fireSleepStop();
        }
        return;
      }

      // End of current chapter — uses Plex chapter timestamps on the book timeline
      if (s.sleep.mode === "end_of_chapter" && s.sleep.chapterEndMs != null) {
        const posMs = s.positionSec * 1000;
        // Small grace so we don't fire immediately if we're already at the boundary
        if (posMs >= s.sleep.chapterEndMs - 250) {
          fireSleepStop();
        }
      }
    }, 400);
  }

  /**
   * Resolve where the current chapter ends on the book timeline (ms).
   * Prefer Plex embedded chapter markers; fall back to end of current track/part.
   */
  async function resolveChapterEnd(): Promise<{ endMs: number; title: string | null }> {
    const s = get(store);
    const posMs = Math.max(0, s.positionSec * 1000);
    const bookDurMs = s.durationSec > 0 ? Math.floor(s.durationSec * 1000) : null;

    // 1) Embedded / multi-file chapters from Plex book detail
    if (s.book && s.serverId) {
      try {
        const detail = await plexApi.getBookDetail(s.serverId, s.book.ratingKey);
        const chapters = detail.chapters ?? [];
        if (chapters.length > 0) {
          for (let i = 0; i < chapters.length; i++) {
            const ch = chapters[i];
            const start = ch.startMs;
            const end =
              ch.endMs ??
              chapters[i + 1]?.startMs ??
              detail.durationMs ??
              bookDurMs ??
              start;
            // Current chapter: position is within [start, end)
            // If exactly at end of previous, treat as next chapter's range
            if (posMs >= start && posMs < end) {
              return { endMs: end, title: ch.title };
            }
          }
          // Past last start — use last chapter end / book end
          const last = chapters[chapters.length - 1];
          const end =
            last.endMs ?? detail.durationMs ?? bookDurMs ?? last.startMs;
          return { endMs: end, title: last.title };
        }
      } catch {
        /* fall through to track boundary */
      }
    }

    // 2) Fallback: end of the current file/track (multi-part books)
    const offsets = trackOffsets(s.tracks);
    const idx = s.trackIndex;
    const trackStartMs = Math.floor((offsets[idx] ?? 0) * 1000);
    const trackDur = s.tracks[idx]?.durationMs ?? 0;
    if (trackDur > 0) {
      return {
        endMs: trackStartMs + trackDur,
        title: s.tracks[idx]?.title ?? `Part ${idx + 1}`,
      };
    }

    // 3) Last resort: end of known book duration
    if (bookDurMs && bookDurMs > posMs) {
      return { endMs: bookDurMs, title: null };
    }

    // Nothing useful — stop after a short buffer so the control still does something
    return { endMs: posMs + 60_000, title: null };
  }

  async function loadTrackAt(index: number, seekInTrackSec = 0, autoplay = false) {
    const s = get(store);
    const track = s.tracks[index];
    if (!track) return;

    update((st) => ({
      ...st,
      trackIndex: index,
      loading: true,
      ready: false,
      playing: false,
      error: null,
    }));

    const wantOffset = Math.max(0, seekInTrackSec);
    const offsets = trackOffsets(s.tracks);

    try {
      // Strategy A: transcoder offset (best for long jumps on progressive MP3)
      // Strategy B: load from start + HTML5 seek (fallback)
      let usedOffset = 0;
      if (wantOffset > 0.25) {
        try {
          usedOffset = wantOffset;
          streamBaseOffsetSec = usedOffset;
          await engine.load(streamUrlAt(track.url, usedOffset));
        } catch {
          usedOffset = 0;
          streamBaseOffsetSec = 0;
          await engine.load(streamUrlAt(track.url, 0));
          await engine.seekAndWait(wantOffset, 10_000);
          // After element seek, position is engine time from file start
          streamBaseOffsetSec = 0;
        }
      } else {
        streamBaseOffsetSec = 0;
        await engine.load(streamUrlAt(track.url, 0));
      }

      engine.setRate(s.rate);

      // Transcoder-offset streams report currentTime≈0; element-seek streams report real time.
      const trackPos = streamBaseOffsetSec + engine.getPosition();
      const bookPos = (offsets[index] ?? 0) + trackPos;

      update((st) => ({
        ...st,
        loading: false,
        ready: true,
        positionSec: bookPos,
      }));

      void flushProgress(autoplay ? "playing" : "paused");

      if (autoplay) {
        update((st) => ({ ...st, playing: true }));
        await engine.play();
        startProgressLoop();
        void flushProgress("playing");
      }
    } catch (e) {
      engine.reset();
      update((st) => ({
        ...st,
        loading: false,
        ready: false,
        playing: false,
        error: e instanceof Error ? e.message : String(e),
      }));
    }
  }

  async function advanceTrack() {
    const s = get(store);
    if (s.trackIndex + 1 < s.tracks.length) {
      await loadTrackAt(s.trackIndex + 1, 0, true);
      return;
    }
    update((st) => ({ ...st, playing: false }));
    stopProgressLoop();
    void flushProgress("stopped");
  }

  async function ensurePlayback(
    serverId: string,
    book: AudiobookSummary,
    opts: LoadBookOptions = {},
  ) {
    const autoplay = opts.autoplay ?? true;
    const gen = ++loadGen;
    engine.pause();
    stopProgressLoop();

    update((s) => ({
      ...s,
      book,
      serverId,
      tracks: [],
      trackIndex: 0,
      chapters: [],
      positionSec: opts.startSec ?? 0,
      durationSec: book.durationMs ? book.durationMs / 1000 : 0,
      playing: false,
      ready: false,
      loading: true,
      error: null,
    }));

    try {
      // Playback streams + chapter markers in parallel
      const [playback, detail] = await Promise.all([
        plexApi.getPlayback(serverId, book.ratingKey),
        plexApi.getBookDetail(serverId, book.ratingKey).catch(() => null),
      ]);
      if (gen !== loadGen) return;

      const tracks = playback.tracks ?? [];
      if (tracks.length === 0) {
        update((s) => ({
          ...s,
          loading: false,
          error: "No playable audio found for this title",
        }));
        return;
      }

      const durationSec = totalDurationSec(
        tracks,
        playback.totalDurationMs ?? detail?.durationMs,
      );
      update((s) => ({
        ...s,
        tracks,
        chapters: detail?.chapters ?? [],
        durationSec: durationSec || s.durationSec,
      }));

      let targetSec = opts.startSec ?? 0;
      if (!opts.ignoreResume && opts.startSec === undefined) {
        try {
          const progress = await progressApi.get(book.ratingKey);
          if (progress && progress.positionMs > 5_000) {
            targetSec = progress.positionMs / 1000;
          }
        } catch {
          /* ignore */
        }
      }

      if (gen !== loadGen) return;

      const { trackIndex, seekInTrack } = mapBookPosition(tracks, targetSec);
      await loadTrackAt(trackIndex, seekInTrack, autoplay);
    } catch (e) {
      if (gen !== loadGen) return;
      update((s) => ({
        ...s,
        loading: false,
        ready: false,
        error: e instanceof Error ? e.message : String(e),
      }));
    }
  }

  return {
    subscribe,

    /** Load a book; optionally resume from bookmark (default) or a specific position. */
    async loadBook(
      serverId: string,
      book: AudiobookSummary,
      autoplayOrOpts: boolean | LoadBookOptions = true,
    ) {
      const opts: LoadBookOptions =
        typeof autoplayOrOpts === "boolean"
          ? { autoplay: autoplayOrOpts }
          : autoplayOrOpts;
      await ensurePlayback(serverId, book, opts);
    },

    /**
     * Jump to a book-level position and play (used by chapter list).
     * Always ignores the saved bookmark so the chapter choice wins.
     */
    async playAt(
      serverId: string,
      book: AudiobookSummary,
      positionSec: number,
      autoplay = true,
    ) {
      const s = get(store);
      const sameBook =
        s.book?.ratingKey === book.ratingKey &&
        s.serverId === serverId &&
        s.ready &&
        s.tracks.length > 0;

      if (sameBook) {
        await this.seek(positionSec, autoplay);
        return;
      }

      await ensurePlayback(serverId, book, {
        startSec: positionSec,
        ignoreResume: true,
        autoplay,
      });
    },

    async toggle() {
      const s = get(store);
      if (!s.book || !s.ready || s.loading) return;

      if (!s.playing || engine.isPaused()) {
        try {
          update((st) => ({ ...st, playing: true, error: null }));
          engine.setRate(s.rate);
          await engine.play();
          startProgressLoop();
          void flushProgress("playing");
        } catch (e) {
          update((st) => ({
            ...st,
            playing: false,
            error: e instanceof Error ? e.message : String(e),
          }));
        }
      } else {
        update((st) => ({ ...st, playing: false }));
        engine.pause();
        stopProgressLoop();
        void flushProgress("paused");
      }
    },

    /**
     * Seek to a book-level time. Reloads the transcoder stream at that offset
     * so jumps (including chapter taps) actually land correctly.
     */
    async seek(bookSeconds: number, autoplay?: boolean) {
      const s = get(store);
      if (!s.ready || s.tracks.length === 0 || s.loading) return;

      const clamped = Math.max(0, Math.min(bookSeconds, s.durationSec || bookSeconds));
      const { trackIndex, seekInTrack } = mapBookPosition(s.tracks, clamped);
      const shouldPlay = autoplay ?? (s.playing || !engine.isPaused());

      // Always reload with transcoder offset — HTML5 seek on progressive MP3 is unreliable
      await loadTrackAt(trackIndex, seekInTrack, shouldPlay);
      void flushProgress(shouldPlay ? "playing" : "paused");
    },

    /** Force-save current position (e.g. leaving book view). */
    async saveProgress() {
      const s = get(store);
      if (!s.book) return;
      await flushProgress(s.playing ? "playing" : "paused");
    },

    setRate(rate: number) {
      engine.setRate(rate);
      update((s) => ({ ...s, rate }));
    },

    setSleepDuration(minutes: number) {
      const mins = Math.max(1, Math.floor(minutes));
      const endsAt = Date.now() + mins * 60 * 1000;
      update((s) => ({
        ...s,
        sleep: {
          ...initialSleep,
          mode: "duration",
          minutes: mins,
          endsAt,
          fadeSeconds: s.sleep.fadeSeconds,
        },
      }));
      armSleepWatcher();
    },

    /**
     * Stop when the current Plex chapter ends (timestamp markers), not after a
     * fixed duration. Falls back to end of the current track/part if no markers.
     */
    async setSleepEndOfChapter() {
      const s = get(store);
      if (!s.book) return;
      try {
        const { endMs, title } = await resolveChapterEnd();
        update((st) => ({
          ...st,
          sleep: {
            ...initialSleep,
            mode: "end_of_chapter",
            minutes: 0,
            endsAt: null,
            chapterEndMs: endMs,
            chapterTitle: title,
            fadeSeconds: st.sleep.fadeSeconds,
          },
        }));
        armSleepWatcher();
      } catch (e) {
        update((st) => ({
          ...st,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },

    clearSleep() {
      clearSleepTimer();
      update((s) => ({
        ...s,
        sleep: { ...initialSleep, fadeSeconds: s.sleep.fadeSeconds },
      }));
    },

    skip(deltaSec: number) {
      const s = get(store);
      void this.seek(Math.max(0, s.positionSec + deltaSec));
    },
  };
}

export const player = createPlayerStore();

export function formatTime(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds < 0) return "0:00";
  const s = Math.floor(totalSeconds % 60);
  const m = Math.floor((totalSeconds / 60) % 60);
  const h = Math.floor(totalSeconds / 3600);
  const pad = (n: number) => n.toString().padStart(2, "0");
  if (h > 0) return `${h}:${pad(m)}:${pad(s)}`;
  return `${m}:${pad(s)}`;
}
