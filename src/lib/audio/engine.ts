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
    this.audio.preload = "metadata";
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
    // Note: custom headers on media elements are limited in webview.
    // Real Plex streams typically use token query params; Rust will supply that.
    this.audio.src = url;
    this.audio.load();
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
