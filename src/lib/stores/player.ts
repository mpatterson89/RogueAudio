import { writable, get } from "svelte/store";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getAudioEngine } from "$lib/audio/engine";
import { progressApi } from "$lib/api/progress";
import { plexApi } from "$lib/api/plex";
import { downloadsApi } from "$lib/api/downloads";
import {
  getBookDetail,
  seedBookDetail,
  detailFromLocalPlayback,
} from "$lib/stores/bookDetail";
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
  /**
   * Continue elsewhere: push/pull position with Plex for this book.
   * Per-title preference loaded from disk when a book is loaded.
   */
  progressSyncEnabled: boolean;
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
  progressSyncEnabled: false,
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

/** Local offline files (media server / asset protocol / absolute paths). */
function isLocalStream(url: string): boolean {
  return (
    url.startsWith("asset:") ||
    url.includes("asset.localhost") ||
    url.startsWith("file:") ||
    url.includes("127.0.0.1") ||
    url.startsWith("http://localhost") ||
    url.startsWith("/") // absolute path before convertFileSrc (shouldn't reach engine)
  );
}

/**
 * Progressive transcoder streams often won't HTML5-seek. Prefer baking
 * start offset into the URL.
 *
 * IMPORTANT: Plex universal transcoder `offset` is in **seconds**, not ms.
 * Sending chapter startTimeOffset (ms) as offset seeks past EOF → empty
 * stream → WebKit media error 4.
 *
 * Local offline MP3s skip query patching — seek via HTML5 instead.
 */
function streamUrlAt(url: string, offsetSec: number): string {
  if (isLocalStream(url)) return url;
  const sec = Math.max(0, Math.floor(offsetSec));
  return withQuery(url, {
    session: crypto.randomUUID(),
    offset: sec >= 1 ? String(sec) : null,
  });
}

function toPlayableUrl(pathOrUrl: string): string {
  if (
    pathOrUrl.startsWith("http://") ||
    pathOrUrl.startsWith("https://") ||
    pathOrUrl.startsWith("asset:") ||
    pathOrUrl.startsWith("file:") ||
    pathOrUrl.includes("asset.localhost") ||
    pathOrUrl.includes("127.0.0.1")
  ) {
    // Local media server (http://127.0.0.1:port/d/…) or remote stream
    return pathOrUrl;
  }
  // Absolute filesystem path fallback → asset protocol
  try {
    return convertFileSrc(pathOrUrl);
  } catch {
    return pathOrUrl;
  }
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
      await progressApi.report(
        {
          ratingKey: s.book.ratingKey,
          state,
          timeMs: Math.floor(Math.max(0, s.positionSec) * 1000),
          durationMs: s.durationSec ? Math.floor(s.durationSec * 1000) : null,
          speed: s.rate,
          trackIndex: s.tracks.length ? s.trackIndex : null,
        },
        {
          serverId: s.serverId,
          syncToPlex: s.progressSyncEnabled && !!s.serverId,
        },
      );
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
      // Local often (~5s); Plex timeline is included when Continue elsewhere is on
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

      // End of current / next chapter — book-timeline markers
      if (
        (s.sleep.mode === "end_of_chapter" ||
          s.sleep.mode === "end_of_next_chapter") &&
        s.sleep.chapterEndMs != null
      ) {
        const posMs = s.positionSec * 1000;
        // Small grace so we don't fire immediately if we're already at the boundary
        if (posMs >= s.sleep.chapterEndMs - 250) {
          fireSleepStop();
        }
      }
    }, 400);
  }

  function chapterEndMs(
    chapters: { startMs: number; endMs?: number | null; title: string }[],
    index: number,
    bookDurMs: number | null,
  ): { endMs: number; title: string } {
    const ch = chapters[index];
    const end =
      ch.endMs ??
      chapters[index + 1]?.startMs ??
      bookDurMs ??
      ch.startMs;
    return { endMs: end, title: ch.title };
  }

  /**
   * Resolve where a chapter ends on the book timeline (ms).
   * @param chapterOffset 0 = current chapter, 1 = next chapter, etc.
   * Prefer Plex embedded chapter markers; fall back to track/part boundaries.
   */
  async function resolveChapterEnd(
    chapterOffset = 0,
  ): Promise<{ endMs: number; title: string | null }> {
    const s = get(store);
    const posMs = Math.max(0, s.positionSec * 1000);
    const bookDurMs = s.durationSec > 0 ? Math.floor(s.durationSec * 1000) : null;
    const offset = Math.max(0, Math.floor(chapterOffset));

    function fromChapters(
      chapters: { startMs: number; endMs?: number | null; title: string }[],
      fallbackDur: number | null,
    ): { endMs: number; title: string | null } | null {
      if (chapters.length === 0) return null;

      let currentIdx = chapters.length - 1;
      for (let i = 0; i < chapters.length; i++) {
        const start = chapters[i].startMs;
        const end =
          chapters[i].endMs ??
          chapters[i + 1]?.startMs ??
          fallbackDur ??
          start;
        if (posMs >= start && posMs < end) {
          currentIdx = i;
          break;
        }
        if (posMs < start) {
          currentIdx = Math.max(0, i - 1);
          break;
        }
      }

      const targetIdx = Math.min(currentIdx + offset, chapters.length - 1);
      return chapterEndMs(chapters, targetIdx, fallbackDur);
    }

    // 1) Prefer chapters already on the player, then cached book detail
    if (s.chapters.length > 0) {
      const hit = fromChapters(s.chapters, bookDurMs);
      if (hit) return hit;
    }

    if (s.book && s.serverId) {
      try {
        const detail = await getBookDetail(s.serverId, s.book.ratingKey);
        const chapters = detail.chapters ?? [];
        const hit = fromChapters(
          chapters,
          detail.durationMs ?? bookDurMs,
        );
        if (hit) return hit;
      } catch {
        /* fall through to track boundary */
      }
    }

    // 2) Fallback: end of current / next file/track (multi-part books)
    const offsets = trackOffsets(s.tracks);
    const idx = Math.min(
      s.trackIndex + offset,
      Math.max(0, s.tracks.length - 1),
    );
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
      const local = isLocalStream(track.url);
      // Strategy A: transcoder offset (best for long jumps on progressive MP3)
      // Strategy B: load from start + HTML5 seek (local files always use B)
      let usedOffset = 0;
      if (wantOffset > 0.25 && !local) {
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
        if (wantOffset > 0.25) {
          await engine.seekAndWait(wantOffset, 10_000);
        }
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

    // Load Continue elsewhere preference for this title
    let syncEnabled = false;
    try {
      syncEnabled = await progressApi.syncGetEnabled(book.ratingKey);
    } catch {
      syncEnabled = false;
    }

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
      progressSyncEnabled: syncEnabled,
    }));

    try {
      // Prefer complete offline download; fall back to live Plex streams.
      let tracks: PlaybackTrack[] = [];
      let totalDurationMs: number | null | undefined;
      let chapters: BookChapter[] = [];

      // Offline: prefer local files. AAC/M4B need an MP3 sidecar (ffmpeg) for WebKit.
      let local = await downloadsApi.localPlayback(book.ratingKey).catch(() => null);
      if (local?.playback?.tracks?.length) {
        const ready = await downloadsApi
          .playableReady(book.ratingKey)
          .catch(() => false);
        if (!ready) {
          // AAC/M4B must be converted to MP3 for WebKit — can take several minutes
          update((s) => ({
            ...s,
            error: "Preparing offline audio for playback (one-time)…",
            loading: true,
          }));
          try {
            await downloadsApi.ensurePlayable(book.ratingKey);
            local =
              (await downloadsApi.localPlayback(book.ratingKey).catch(() => null)) ??
              local;
            update((s) => ({ ...s, error: null }));
          } catch (prepErr) {
            console.warn("ensurePlayable failed", prepErr);
            update((s) => ({
              ...s,
              error:
                prepErr instanceof Error
                  ? `Could not prepare offline audio: ${prepErr.message}`
                  : String(prepErr),
            }));
            // Still try local path (media server + m4a) if prepare failed
          }
        }
      }

      if (local?.playback?.tracks?.length) {
        tracks = local.playback.tracks.map((t) => ({
          ...t,
          url: toPlayableUrl(t.url),
        }));
        totalDurationMs = local.playback.totalDurationMs;
        chapters = local.chapters ?? [];
        // Keep book-view cache warm from offline manifest (no Plex needed)
        seedBookDetail(serverId, book.ratingKey, detailFromLocalPlayback(book.ratingKey, local));
      } else {
        const [playback, detail] = await Promise.all([
          plexApi.getPlayback(serverId, book.ratingKey),
          getBookDetail(serverId, book.ratingKey).catch(() => null),
        ]);
        if (gen !== loadGen) return;
        tracks = playback.tracks ?? [];
        totalDurationMs = playback.totalDurationMs ?? detail?.durationMs;
        chapters = detail?.chapters ?? [];
      }

      if (gen !== loadGen) return;

      if (tracks.length === 0) {
        update((s) => ({
          ...s,
          loading: false,
          error: "No playable audio found for this title",
        }));
        return;
      }

      const durationSec = totalDurationSec(tracks, totalDurationMs);
      update((s) => ({
        ...s,
        tracks,
        chapters,
        durationSec: durationSec || s.durationSec,
      }));

      let targetSec = opts.startSec ?? 0;
      if (!opts.ignoreResume && opts.startSec === undefined) {
        try {
          // Continue elsewhere: merge local + Plex; otherwise local only
          const progress = syncEnabled
            ? await progressApi.getMerged(serverId, book.ratingKey)
            : await progressApi.get(book.ratingKey);
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
      await this.setSleepEndOfChapterOffset(0, "end_of_chapter");
    },

    /**
     * Stop at the end of the *next* chapter (skips the rest of the current one
     * and the following chapter). Falls back to next track/part if no markers.
     */
    async setSleepEndOfNextChapter() {
      await this.setSleepEndOfChapterOffset(1, "end_of_next_chapter");
    },

    async setSleepEndOfChapterOffset(
      chapterOffset: number,
      mode: "end_of_chapter" | "end_of_next_chapter",
    ) {
      const s = get(store);
      if (!s.book) return;
      try {
        const { endMs, title } = await resolveChapterEnd(chapterOffset);
        update((st) => ({
          ...st,
          sleep: {
            ...initialSleep,
            mode,
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

    /**
     * Continue elsewhere: enable/disable Plex position sync for the current book.
     * When enabling, merges with Plex and seeks if Plex is ahead.
     */
    async setProgressSyncEnabled(enabled: boolean): Promise<{
      enabled: boolean;
      positionMs: number;
      source: string;
      message: string | null;
    }> {
      const s = get(store);
      if (!s.book || !s.serverId) {
        return {
          enabled: false,
          positionMs: 0,
          source: "local",
          message: "No book loaded",
        };
      }
      const ratingKey = s.book.ratingKey;
      const serverId = s.serverId;

      if (!enabled) {
        await progressApi.syncSetEnabled(ratingKey, false);
        update((st) => ({ ...st, progressSyncEnabled: false }));
        // Final local save without Plex
        void flushProgress(s.playing ? "playing" : "paused");
        return {
          enabled: false,
          positionMs: Math.floor(s.positionSec * 1000),
          source: "local",
          message: "Stopped syncing with Plex",
        };
      }

      // Enable + merge local ↔ Plex
      const beforeMs = Math.floor(Math.max(0, s.positionSec) * 1000);
      // Push current position into local first so merge sees latest from this session
      if (s.ready && beforeMs > 0) {
        await progressApi.report(
          {
            ratingKey,
            state: s.playing ? "playing" : "paused",
            timeMs: beforeMs,
            durationMs: s.durationSec ? Math.floor(s.durationSec * 1000) : null,
            speed: s.rate,
            trackIndex: s.tracks.length ? s.trackIndex : null,
          },
          { serverId, syncToPlex: false },
        );
      }

      const merged = await progressApi.syncEnableAndMerge(serverId, ratingKey);
      update((st) => ({ ...st, progressSyncEnabled: true }));

      const mergedSec = merged.positionMs / 1000;
      let message: string | null = null;
      if (Math.abs(merged.positionMs - beforeMs) > 5_000 && s.ready) {
        await this.seek(mergedSec, s.playing);
        if (merged.positionMs > beforeMs + 5_000) {
          message = "Resumed from Plex";
        } else {
          message = "Updated Plex from this device";
        }
      } else {
        // Ensure Plex has our position
        void flushProgress(s.playing ? "playing" : "paused");
        message = "Syncing with Plex & Plexamp";
      }

      return {
        enabled: true,
        positionMs: merged.positionMs,
        source: merged.source ?? "merged",
        message,
      };
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
