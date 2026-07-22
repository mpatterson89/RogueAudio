import { invoke } from "@tauri-apps/api/core";
import type { DownloadItem, LocalPlayback } from "$lib/types/downloads";

export const downloadsApi = {
  list: () => invoke<DownloadItem[]>("download_list"),
  get: (ratingKey: string) =>
    invoke<DownloadItem | null>("download_get", { ratingKey }),
  enqueue: (serverId: string, ratingKey: string) =>
    invoke<DownloadItem>("download_enqueue", { serverId, ratingKey }),
  cancel: (ratingKey: string) => invoke<void>("download_cancel", { ratingKey }),
  remove: (ratingKey: string) => invoke<void>("download_remove", { ratingKey }),
  localPlayback: (ratingKey: string) =>
    invoke<LocalPlayback | null>("download_local_playback", { ratingKey }),
};
