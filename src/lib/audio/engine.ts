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
  | "durationchange";

export type AudioEngineListener = (event: AudioEngineEvent, detail?: unknown) => void;

export interface AudioEngine {
  load(url: string, headers?: Record<string, string>): Promise<void>;
  play(): Promise<void>;
  pause(): void;
  seek(seconds: number): void;
  setRate(rate: number): void;
  getPosition(): number;
  getDuration(): number;
  isPaused(): boolean;
  destroy(): void;
  on(listener: AudioEngineListener): () => void;
}

export class Html5AudioEngine implements AudioEngine {
  private audio: HTMLAudioElement;
  private listeners = new Set<AudioEngineListener>();
  private onAudioEvent: (type: AudioEngineEvent) => EventListener;

  constructor() {
    this.audio = new Audio();
    this.audio.preload = "auto";
    // Do not set crossOrigin — PMS often omits CORS headers; playback still works.
    this.onAudioEvent = (type: AudioEngineEvent) => () => this.emit(type);

    const types: AudioEngineEvent[] = [
      "timeupdate",
      "ended",
      "error",
      "playing",
      "paused",
      "loadedmetadata",
      "durationchange",
    ];
    for (const t of types) {
      this.audio.addEventListener(t, this.onAudioEvent(t));
    }
  }

  private emit(event: AudioEngineEvent, detail?: unknown) {
    for (const l of this.listeners) l(event, detail);
  }

  async load(url: string, _headers?: Record<string, string>): Promise<void> {
    // Token / transcoder params are in the query string (set by Rust).
    // Custom headers cannot be set on HTMLAudioElement in a webview.
    await new Promise<void>((resolve, reject) => {
      let settled = false;
      const settle = (fn: () => void) => {
        if (settled) return;
        settled = true;
        cleanup();
        fn();
      };
      const onReady = () => settle(() => resolve());
      const onError = () =>
        settle(() => {
          const code = this.audio.error?.code;
          // 1=aborted 2=network 3=decode 4=src not supported
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
        this.audio.removeEventListener("error", onError);
        clearTimeout(timer);
      };

      // Long audiobooks may take a moment for metadata
      const timer = setTimeout(() => {
        // If network is slow but no error, allow play to proceed after timeout
        // when we at least have a src assigned.
        if (!settled && this.audio.src) {
          settle(() => resolve());
        }
      }, 20_000);

      this.audio.addEventListener("canplay", onReady);
      this.audio.addEventListener("loadedmetadata", onReady);
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
      this.audio.currentTime = Math.max(0, seconds);
    }
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
    this.audio.pause();
    this.audio.removeAttribute("src");
    this.audio.load();
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
