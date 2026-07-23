import { invoke } from "@tauri-apps/api/core";
import type { ProgressReport, ProgressSnapshot } from "$lib/types/models";

export const progressApi = {
  get: (ratingKey: string) =>
    invoke<ProgressSnapshot | null>("progress_get", { ratingKey }),

  /** Local + Plex viewOffset merge (Continue elsewhere). */
  getMerged: (serverId: string, ratingKey: string) =>
    invoke<ProgressSnapshot>("progress_get_merged", { serverId, ratingKey }),

  report: (
    report: ProgressReport,
    opts?: { serverId?: string | null; syncToPlex?: boolean },
  ) =>
    invoke<ProgressSnapshot>("progress_report", {
      report,
      serverId: opts?.serverId ?? null,
      syncToPlex: opts?.syncToPlex ?? false,
    }),

  clear: (ratingKey: string) => invoke<void>("progress_clear", { ratingKey }),

  syncGetEnabled: (ratingKey: string) =>
    invoke<boolean>("progress_sync_get_enabled", { ratingKey }),

  syncSetEnabled: (ratingKey: string, enabled: boolean) =>
    invoke<boolean>("progress_sync_set_enabled", { ratingKey, enabled }),

  /** Turn on Continue elsewhere and merge local ↔ Plex. */
  syncEnableAndMerge: (serverId: string, ratingKey: string) =>
    invoke<ProgressSnapshot>("progress_sync_enable_and_merge", {
      serverId,
      ratingKey,
    }),
};
