import { invoke } from "@tauri-apps/api/core";
import type { ProgressReport, ProgressSnapshot } from "$lib/types/models";

export const progressApi = {
  get: (ratingKey: string) =>
    invoke<ProgressSnapshot | null>("progress_get", { ratingKey }),
  report: (report: ProgressReport) =>
    invoke<ProgressSnapshot>("progress_report", { report }),
  clear: (ratingKey: string) => invoke<void>("progress_clear", { ratingKey }),
};
