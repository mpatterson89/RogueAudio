import { writable, get } from "svelte/store";
import { getAudioEngine } from "$lib/audio/engine";
import { progressApi } from "$lib/api/progress";
import { plexApi } from "$lib/api/plex";
import type {
  AudiobookSummary,
  PlaybackTrack,
  SleepTimerState,
} from "$lib/types/models";

export const PLAYBACK_RATES = [0.8, 0.9, 1, 1.1, 1.2, 1.25, 1.5, 1.75, 2] as const;

interface PlayerState {
  book: AudiobookSummary | null;
  serverId: string | null;
  tracks: PlaybackTrack[];
  trackIndex: number;
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
  fadeSeconds: 15,
};

const initial: PlayerState = {
  book: null,
  serverId: null,
  tracks: [],
  trackIndex: 0,
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

function createPlayerStore() {
  const store = writable<PlayerState>(initial);
  const { subscribe, update } = store;
  const engine = getAudioEngine();

  let sleepInterval: ReturnType<typeof setInterval> | null = null;
  let progressInterval: ReturnType<typeof setInterval> | null = null;
  let lastProgressFlush = 0;
  let loadGen = 0;

  engine.on((event) => {
    const s = get(store);
    // Ignore engine noise while a new source is loading
    if (s.loading || !s.ready || s.tracks.length === 0) return;

    const offsets = trackOffsets(s.tracks);

    if (event === "timeupdate" || event === "durationchange" || event === "loadedmetadata") {
      const trackPos = engine.getPosition();
      const trackDur = engine.getDuration();
      let bookPos = (offsets[s.trackIndex] ?? 0) + trackPos;
      let bookDur = s.durationSec;
      if (trackDur > 0 && !(s.tracks[s.trackIndex]?.durationMs)) {
        bookDur = Math.max(bookDur, bookPos + (trackDur - trackPos));
      }
      update((st) => ({
        ...st,
        positionSec: bookPos,
        durationSec: bookDur || st.durationSec,
      }));
    }

    if (event === "playing") {
      update((st) => ({ ...st, playing: true }));
    }
    if (event === "paused") {
      // Don't clear playing if we intentionally ignore brief pauses during seek
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
    }
  });

  async function flushProgress(state: "playing" | "paused" | "stopped") {
    const s = get(store);
    if (!s.book) return;
    try {
      await progressApi.report({
        ratingKey: s.book.ratingKey,
        state,
        timeMs: Math.floor(s.positionSec * 1000),
        durationMs: s.durationSec ? Math.floor(s.durationSec * 1000) : null,
        speed: s.rate,
      });
      lastProgressFlush = Date.now();
    } catch {
      /* local write is best-effort */
    }
  }

  function startProgressLoop() {
    stopProgressLoop();
    progressInterval = setInterval(() => {
      const s = get(store);
      if (!s.playing || !s.book) return;
      if (Date.now() - lastProgressFlush > 10_000) {
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

  function armSleepWatcher() {
    clearSleepTimer();
    sleepInterval = setInterval(() => {
      const s = get(store);
      if (s.sleep.mode !== "duration" || !s.sleep.endsAt) return;
      if (s.sleep.endsAt - Date.now() <= 0) {
        engine.pause();
        void flushProgress("paused");
        update((st) => ({
          ...st,
          playing: false,
          sleep: { ...st.sleep, mode: "off", endsAt: null },
        }));
        clearSleepTimer();
        stopProgressLoop();
      }
    }, 500);
  }

  async function loadTrackAt(index: number, seekSec = 0, autoplay = false) {
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

    try {
      await engine.load(track.url);
      engine.setRate(s.rate);
      if (seekSec > 0) engine.seek(seekSec);

      const offsets = trackOffsets(s.tracks);
      update((st) => ({
        ...st,
        loading: false,
        ready: true,
        positionSec: (offsets[index] ?? 0) + Math.max(0, seekSec),
      }));

      if (autoplay) {
        // Optimistic UI — don't wait only on media events (can be flaky in webview)
        update((st) => ({ ...st, playing: true }));
        await engine.play();
        startProgressLoop();
        void flushProgress("playing");
      }
    } catch (e) {
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
    // Finished book
    update((st) => ({ ...st, playing: false }));
    stopProgressLoop();
    void flushProgress("stopped");
  }

  return {
    subscribe,

    async loadBook(serverId: string, book: AudiobookSummary, autoplay = true) {
      const gen = ++loadGen;
      engine.pause();
      stopProgressLoop();

      update((s) => ({
        ...s,
        book,
        serverId,
        tracks: [],
        trackIndex: 0,
        positionSec: 0,
        durationSec: book.durationMs ? book.durationMs / 1000 : 0,
        playing: false,
        ready: false,
        loading: true,
        error: null,
      }));

      try {
        const playback = await plexApi.getPlayback(serverId, book.ratingKey);
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

        const durationSec = totalDurationSec(tracks, playback.totalDurationMs);
        update((s) => ({
          ...s,
          tracks,
          durationSec: durationSec || s.durationSec,
        }));

        // Resume from local progress when possible
        let resumeSec = 0;
        try {
          const progress = await progressApi.get(book.ratingKey);
          if (progress && progress.positionMs > 15_000) {
            resumeSec = progress.positionMs / 1000;
          }
        } catch {
          /* ignore */
        }

        if (gen !== loadGen) return;

        // Map book position → track + offset
        const offsets = trackOffsets(tracks);
        let trackIndex = 0;
        let seekInTrack = 0;
        if (resumeSec > 0 && durationSec > 0) {
          for (let i = 0; i < tracks.length; i++) {
            const start = offsets[i] ?? 0;
            const len = (tracks[i].durationMs ?? 0) / 1000;
            const end = len > 0 ? start + len : start + 1e12;
            if (resumeSec < end || i === tracks.length - 1) {
              trackIndex = i;
              seekInTrack = Math.max(0, resumeSec - start);
              break;
            }
          }
        }

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
    },

    async toggle() {
      const s = get(store);
      // Ignore while loading or not ready — avoids race errors on early clicks
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

    async seek(bookSeconds: number) {
      const s = get(store);
      if (!s.ready || s.tracks.length === 0) return;

      const clamped = Math.max(0, Math.min(bookSeconds, s.durationSec || bookSeconds));
      const offsets = trackOffsets(s.tracks);

      let trackIndex = s.trackIndex;
      let seekInTrack = clamped;

      for (let i = 0; i < s.tracks.length; i++) {
        const start = offsets[i] ?? 0;
        const len = (s.tracks[i].durationMs ?? 0) / 1000;
        const end = i === s.tracks.length - 1 ? Number.POSITIVE_INFINITY : start + Math.max(len, 0.001);
        if (clamped >= start && clamped < end) {
          trackIndex = i;
          seekInTrack = clamped - start;
          break;
        }
      }

      if (trackIndex !== s.trackIndex) {
        await loadTrackAt(trackIndex, seekInTrack, s.playing || !engine.isPaused());
      } else {
        engine.seek(seekInTrack);
        update((st) => ({ ...st, positionSec: clamped }));
      }
      void flushProgress(s.playing ? "playing" : "paused");
    },

    setRate(rate: number) {
      engine.setRate(rate);
      update((s) => ({ ...s, rate }));
    },

    setSleepDuration(minutes: number) {
      const endsAt = Date.now() + minutes * 60 * 1000;
      update((s) => ({
        ...s,
        sleep: { ...s.sleep, mode: "duration", minutes, endsAt },
      }));
      armSleepWatcher();
    },

    setSleepEndOfChapter() {
      update((s) => ({
        ...s,
        sleep: { ...s.sleep, mode: "end_of_chapter", endsAt: null },
      }));
    },

    clearSleep() {
      clearSleepTimer();
      update((s) => ({
        ...s,
        sleep: { ...initialSleep },
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
