import type { BookChapter, PlaybackInfo } from "$lib/types/models";

export type DownloadStatus =
  | "queued"
  | "downloading"
  | "complete"
  | "error"
  | "cancelled";

export interface DownloadItem {
  ratingKey: string;
  serverId: string;
  title: string;
  author?: string | null;
  status: DownloadStatus | string;
  /** 0..1 */
  progress: number;
  error?: string | null;
  tracksDone: number;
  trackCount: number;
  bytesDownloaded: number;
  /** Whole-book size estimate (bytes), when known */
  bytesTotal?: number | null;
  durationMs?: number | null;
  /** Absolute filesystem path to cover (use convertFileSrc). */
  coverPath?: string | null;
  downloadedAt?: string | null;
}

export interface LocalPlayback {
  playback: PlaybackInfo;
  chapters: BookChapter[];
  title: string;
  author?: string | null;
}
