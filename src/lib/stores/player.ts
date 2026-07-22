import { writable, get } from "svelte/store";
import { getAudioEngine } from "$lib/audio/engine";
import { progressApi } from "$lib/api/progress";
import { plexApi } from "$lib/api/plex";
import type { AudiobookSummary, SleepTimerState } from "$lib/types/models";

export const PLAYBACK_RATES = [0.8, 0.9, 1, 1.1, 1.2, 1.25, 1.5, 1.75, 2] as const;

interface PlayerState {
  book: AudiobookSummary | null;
  serverId: string | null;
  playing: boolean;
  positionSec: number;
  durationSec: number;
  rate: number;
  sleep: SleepTimerState;
  error: string | null;
  /** Demo mode: no real stream yet */
  demoMode: boolean;
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
  playing: false,
  positionSec: 0,
  durationSec: 0,
  rate: 1,
  sleep: initialSleep,
  error: null,
  demoMode: false,
};

function createPlayerStore() {
  const store = writable<PlayerState>(initial);
  const { subscribe, update } = store;
  const engine = getAudioEngine();
  let sleepInterval: ReturnType<typeof setInterval> | null = null;
  let progressInterval: ReturnType<typeof setInterval> | null = null;
  let demoInterval: ReturnType<typeof setInterval> | null = null;
  let lastProgressFlush = 0;

  engine.on((event) => {
    if (event === "timeupdate") {
      update((s) => ({
        ...s,
        positionSec: engine.getPosition(),
        durationSec: engine.getDuration() || s.durationSec,
      }));
    }
    if (event === "playing") update((s) => ({ ...s, playing: true }));
    if (event === "paused") update((s) => ({ ...s, playing: false }));
    if (event === "ended") {
      update((s) => ({ ...s, playing: false }));
      void flushProgress("stopped");
    }
    if (event === "durationchange" || event === "loadedmetadata") {
      update((s) => ({ ...s, durationSec: engine.getDuration() }));
    }
    if (event === "error") {
      update((s) => ({ ...s, error: "Playback error", playing: false }));
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
      // local reliability is handled in Rust; network failures are OK
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

  function stopDemoClock() {
    if (demoInterval) {
      clearInterval(demoInterval);
      demoInterval = null;
    }
  }

  function startDemoClock() {
    stopDemoClock();
    demoInterval = setInterval(() => {
      const s = get(store);
      if (!s.playing || !s.demoMode) {
        stopDemoClock();
        return;
      }
      update((st) => {
        const next = st.positionSec + 0.25 * st.rate;
        if (st.durationSec && next >= st.durationSec) {
          stopDemoClock();
          return { ...st, positionSec: st.durationSec, playing: false };
        }
        return { ...st, positionSec: next };
      });
    }, 250);
  }

  function armSleepWatcher() {
    clearSleepTimer();
    sleepInterval = setInterval(() => {
      const s = get(store);
      if (s.sleep.mode !== "duration" || !s.sleep.endsAt) return;
      const remaining = s.sleep.endsAt - Date.now();
      if (remaining <= 0) {
        if (s.demoMode) {
          stopDemoClock();
        } else {
          engine.pause();
        }
        void flushProgress("paused");
        update((st) => ({
          ...st,
          playing: false,
          sleep: { ...st.sleep, mode: "off", endsAt: null },
        }));
        clearSleepTimer();
      }
    }, 500);
  }

  return {
    subscribe,
    async loadBook(serverId: string, book: AudiobookSummary) {
      stopDemoClock();
      update((s) => ({
        ...s,
        book,
        serverId,
        error: null,
        playing: false,
        positionSec: 0,
        durationSec: book.durationMs ? book.durationMs / 1000 : 0,
      }));

      try {
        const stream = await plexApi.getStream(serverId, book.ratingKey);
        const isStub = stream.url.startsWith("plex://stub");
        if (isStub) {
          update((s) => ({
            ...s,
            demoMode: true,
            durationSec: book.durationMs ? book.durationMs / 1000 : 3600,
          }));
        } else {
          update((s) => ({ ...s, demoMode: false }));
          const headers = Object.fromEntries(stream.headers);
          await engine.load(stream.url, headers);
        }

        const progress = await progressApi.get(book.ratingKey);
        if (progress && progress.positionMs > 0) {
          const sec = progress.positionMs / 1000;
          if (!isStub) engine.seek(sec);
          update((s) => ({ ...s, positionSec: sec }));
        }
      } catch (e) {
        update((s) => ({
          ...s,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },
    async toggle() {
      const s = get(store);
      if (!s.book) return;

      if (s.demoMode) {
        if (s.playing) {
          stopDemoClock();
          update((st) => ({ ...st, playing: false }));
          stopProgressLoop();
          void flushProgress("paused");
        } else {
          update((st) => ({ ...st, playing: true }));
          startDemoClock();
          startProgressLoop();
          void flushProgress("playing");
        }
        return;
      }

      if (engine.isPaused()) {
        await engine.play();
        engine.setRate(s.rate);
        startProgressLoop();
        void flushProgress("playing");
      } else {
        engine.pause();
        stopProgressLoop();
        void flushProgress("paused");
      }
    },
    seek(seconds: number) {
      const s = get(store);
      const clamped = Math.max(0, Math.min(seconds, s.durationSec || seconds));
      if (s.demoMode) {
        update((st) => ({ ...st, positionSec: clamped }));
      } else {
        engine.seek(clamped);
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
      this.seek(Math.max(0, s.positionSec + deltaSec));
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
