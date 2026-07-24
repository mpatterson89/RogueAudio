/**
 * Audio engine abstraction.
 * MVP: HTML5 Audio. Later: libmpv / GStreamer via Rust events.
 */

export type AudioEngineEvent =
  | "timeupdate"
  | "ended"
  | "error"
  | "playing"
  | "paused"
  | "loadedmetadata"
  | "durationchange"
  | "seeked";

export type AudioEngineListener = (event: AudioEngineEvent, detail?: unknown) => void;

export interface AudioEngine {
  load(url: string, headers?: Record<string, string>): Promise<void>;
  play(): Promise<void>;
  pause(): void;
  seek(seconds: number): void;
  /** Seek and wait for the element to land (best-effort). */
  seekAndWait(seconds: number, timeoutMs?: number): Promise<void>;
  setRate(rate: number): void;
  getPosition(): number;
  getDuration(): number;
  isPaused(): boolean;
  reset(): void;
  destroy(): void;
  on(listener: AudioEngineListener): () => void;
}

export class Html5AudioEngine implements AudioEngine {
  private audio: HTMLAudioElement;
  private listeners = new Set<AudioEngineListener>();
  private onAudioEvent: (type: AudioEngineEvent) => EventListener;
  private loadToken = 0;

  constructor() {
    this.audio = new Audio();
    this.audio.preload = "auto";
    this.onAudioEvent = (type: AudioEngineEvent) => () => this.emit(type);

    const types: AudioEngineEvent[] = [
      "timeupdate",
      "ended",
      "error",
      "playing",
      "paused",
      "loadedmetadata",
      "durationchange",
      "seeked",
    ];
    for (const t of types) {
      this.audio.addEventListener(t, this.onAudioEvent(t));
    }
  }

  private emit(event: AudioEngineEvent, detail?: unknown) {
    for (const l of this.listeners) l(event, detail);
  }

  /** Tear down current media so the next load is clean. */
  reset(): void {
    this.loadToken++;
    try {
      this.audio.pause();
    } catch {
      /* ignore */
    }
    this.audio.removeAttribute("src");
    this.audio.load();
  }

  async load(url: string, _headers?: Record<string, string>): Promise<void> {
    // Invalidate any in-flight load
    const token = ++this.loadToken;

    const isLocal =
      url.startsWith("asset:") ||
      url.includes("asset.localhost") ||
      url.startsWith("file:") ||
      url.startsWith("blob:") ||
      url.includes("127.0.0.1") ||
      url.startsWith("http://localhost");

    // Fully reset previous source (important when switching chapter streams)
    try {
      this.audio.pause();
    } catch {
      /* ignore */
    }
    this.audio.removeAttribute("src");
    this.audio.load();

    // Large local files + concurrent downloads: avoid aggressive full preload (UI freeze)
    this.audio.preload = isLocal ? "metadata" : "auto";

    await new Promise<void>((resolve, reject) => {
      let settled = false;
      const settle = (fn: () => void) => {
        if (settled || token !== this.loadToken) return;
        settled = true;
        cleanup();
        fn();
      };
      const onReady = () => settle(() => resolve());
      const onError = () =>
        settle(() => {
          const code = this.audio.error?.code;
          const hints: Record<number, string> = {
            1: "aborted",
            2: "network error",
            3: "decode error",
            4: "format/MIME not supported",
          };
          const hint = code ? hints[code] || `code ${code}` : "unknown";
          reject(new Error(`Failed to load audio (${hint})`));
        });
      const cleanup = () => {
        this.audio.removeEventListener("canplay", onReady);
        this.audio.removeEventListener("loadedmetadata", onReady);
        this.audio.removeEventListener("canplaythrough", onReady);
        this.audio.removeEventListener("error", onError);
        clearTimeout(timer);
      };

      const timeoutMs = isLocal ? 20_000 : 45_000;
      const timer = setTimeout(() => {
        // Transcodes can be slow to produce frames; if we have duration, proceed.
        if (!settled && token === this.loadToken && this.audio.src) {
          if (Number.isFinite(this.audio.duration) && this.audio.duration > 0) {
            settle(() => resolve());
          } else {
            settle(() =>
              reject(new Error("Failed to load audio (timeout waiting for stream)")),
            );
          }
        }
      }, timeoutMs);

      this.audio.addEventListener("canplay", onReady);
      this.audio.addEventListener("loadedmetadata", onReady);
      this.audio.addEventListener("canplaythrough", onReady);
      this.audio.addEventListener("error", onError);
      this.audio.src = url;
      this.audio.load();
    });
  }

  async play(): Promise<void> {
    await this.audio.play();
  }

  pause(): void {
    this.audio.pause();
  }

  seek(seconds: number): void {
    if (Number.isFinite(seconds)) {
      try {
        this.audio.currentTime = Math.max(0, seconds);
      } catch {
        /* ignore seek errors on live progressive streams */
      }
    }
  }

  async seekAndWait(seconds: number, timeoutMs = 8000): Promise<void> {
    if (!Number.isFinite(seconds)) return;
    const target = Math.max(0, seconds);
    if (Math.abs(this.audio.currentTime - target) < 0.35) return;

    await new Promise<void>((resolve) => {
      let done = false;
      const finish = () => {
        if (done) return;
        done = true;
        this.audio.removeEventListener("seeked", onSeeked);
        clearTimeout(timer);
        resolve();
      };
      const onSeeked = () => finish();
      const timer = setTimeout(finish, timeoutMs);
      this.audio.addEventListener("seeked", onSeeked);
      try {
        this.audio.currentTime = target;
      } catch {
        finish();
      }
    });
  }

  setRate(rate: number): void {
    this.audio.playbackRate = rate;
  }

  getPosition(): number {
    return this.audio.currentTime || 0;
  }

  getDuration(): number {
    const d = this.audio.duration;
    return Number.isFinite(d) ? d : 0;
  }

  isPaused(): boolean {
    return this.audio.paused;
  }

  destroy(): void {
    this.reset();
    this.listeners.clear();
  }

  on(listener: AudioEngineListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

let shared: AudioEngine | null = null;

export function getAudioEngine(): AudioEngine {
  if (!shared) shared = new Html5AudioEngine();
  return shared;
}
