import { invoke } from "@tauri-apps/api/core";
import type {
  DownloadItem,
  DownloadQueueState,
  LocalPlayback,
} from "$lib/types/downloads";

export const downloadsApi = {
  list: () => invoke<DownloadItem[]>("download_list"),
  get: (ratingKey: string) =>
    invoke<DownloadItem | null>("download_get", { ratingKey }),
  enqueue: (serverId: string, ratingKey: string) =>
    invoke<DownloadItem>("download_enqueue", { serverId, ratingKey }),
  cancel: (ratingKey: string) => invoke<void>("download_cancel", { ratingKey }),
  pauseQueue: () => invoke<DownloadQueueState>("download_pause_queue"),
  resumeQueue: () => invoke<DownloadQueueState>("download_resume_queue"),
  queueState: () => invoke<DownloadQueueState>("download_queue_state"),
  /** Cold-start: heal interrupted jobs; auto-resume if queue was active. */
  restore: () => invoke<DownloadQueueState>("download_restore"),
  remove: (ratingKey: string) => invoke<void>("download_remove", { ratingKey }),
  removeAll: () => invoke<number>("download_remove_all"),
  storageBytes: () => invoke<number>("download_storage_bytes"),
  localPlayback: (ratingKey: string) =>
    invoke<LocalPlayback | null>("download_local_playback", { ratingKey }),
  /** True when offline audio is HTML5-ready (mp3 or already web-safe). */
  playableReady: (ratingKey: string) =>
    invoke<boolean>("download_playable_ready", { ratingKey }),
  /**
   * Build WebKit-safe MP3 sidecars via ffmpeg for AAC/M4B downloads.
   * May take several minutes for long books.
   */
  ensurePlayable: (ratingKey: string) =>
    invoke<DownloadItem>("download_ensure_playable", { ratingKey }),
};
