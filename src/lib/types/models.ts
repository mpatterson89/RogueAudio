export interface PinAuthStart {
  id: number;
  code: string;
  authUrl: string;
}

export interface AuthStatus {
  authenticated: boolean;
  username?: string | null;
}

export interface PinAuthPoll {
  authorized: boolean;
  status: AuthStatus;
}

export interface PlexConnection {
  uri: string;
  local: boolean;
  relay: boolean;
}

export interface PlexServer {
  id: string;
  name: string;
  product?: string | null;
  provides?: string | null;
  publicAddress?: string | null;
  owned?: boolean;
  connections: PlexConnection[];
}

export interface PlexLibrary {
  key: string;
  title: string;
  libraryType: string;
  agent?: string | null;
}

export interface AudiobookSummary {
  ratingKey: string;
  title: string;
  author?: string | null;
  thumb?: string | null;
  year?: number | null;
  durationMs?: number | null;
  libraryKey?: string | null;
}

export interface StreamInfo {
  url: string;
  headers: [string, string][];
  durationMs?: number | null;
  container?: string | null;
}

export interface ProgressSnapshot {
  ratingKey: string;
  positionMs: number;
  durationMs?: number | null;
  updatedAt: string;
  source: "local" | "plex" | "merged";
}

export interface ProgressReport {
  ratingKey: string;
  state: "playing" | "paused" | "stopped" | "buffering";
  timeMs: number;
  durationMs?: number | null;
  speed: number;
}

export type SleepMode = "off" | "duration" | "end_of_chapter";

export interface SleepTimerState {
  mode: SleepMode;
  /** Minutes when mode === 'duration' */
  minutes: number;
  /** Epoch ms when timer should fire (duration mode) */
  endsAt: number | null;
  fadeSeconds: number;
}
