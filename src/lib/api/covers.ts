import { invoke } from "@tauri-apps/api/core";

export interface CoverEnsureRequest {
  serverId: string;
  ratingKey: string;
  remoteUrl: string;
}

export interface CoverEnsureResult {
  ratingKey: string;
  path?: string | null;
  error?: string | null;
}

export const coversApi = {
  getLocal: (serverId: string, ratingKey: string) =>
    invoke<string | null>("cover_get_local", { serverId, ratingKey }),
  ensure: (serverId: string, ratingKey: string, remoteUrl: string) =>
    invoke<string>("cover_ensure", { serverId, ratingKey, remoteUrl }),
  ensureMany: (requests: CoverEnsureRequest[]) =>
    invoke<CoverEnsureResult[]>("cover_ensure_many", { requests }),
  import: (serverId: string, ratingKey: string, sourcePath: string) =>
    invoke<string>("cover_import", { serverId, ratingKey, sourcePath }),
};
