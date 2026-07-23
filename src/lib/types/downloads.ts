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
  series?: string | null;
  seriesIndex?: number | null;
  status: DownloadStatus | string;
  /** 0..1 */
  progress: number;
  error?: string | null;
  tracksDone: number;
  trackCount: number;
  bytesDownloaded: number;
  /** Whole-book size estimate (bytes), when known */
  bytesTotal?: number | null;
  /** Actual on-disk bytes for this book folder */
  bytesOnDisk?: number;
  durationMs?: number | null;
  /** Absolute filesystem path to cover (use convertFileSrc). */
  coverPath?: string | null;
  downloadedAt?: string | null;
  /** Audio file names on disk */
  fileNames?: string[];
}

export interface LocalPlayback {
  playback: PlaybackInfo;
  chapters: BookChapter[];
  title: string;
  author?: string | null;
  summary?: string | null;
  year?: number | null;
  durationMs?: number | null;
  libraryKey?: string | null;
  studio?: string | null;
  trackCount?: number;
  serverId?: string;
  /** Absolute path to local cover (convertFileSrc on FE). */
  coverPath?: string | null;
}
