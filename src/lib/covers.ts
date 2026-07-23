/**
 * Resolve Plex remote cover URLs to on-disk asset URLs for the webview.
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import { coversApi, type CoverEnsureRequest } from "$lib/api/covers";
import type { AudiobookSummary, BookDetail } from "$lib/types/models";

export function isRemoteMediaUrl(url: string | null | undefined): boolean {
  if (!url) return false;
  return url.startsWith("http://") || url.startsWith("https://");
}

export function isLocalAssetUrl(url: string | null | undefined): boolean {
  if (!url) return false;
  return (
    url.startsWith("asset:") ||
    url.includes("asset.localhost") ||
    url.startsWith("file:")
  );
}

export function pathToAssetUrl(path: string): string {
  try {
    return convertFileSrc(path);
  } catch {
    return path;
  }
}

/** Ensure one cover is on disk; return webview-safe URL or null. */
export async function ensureCoverUrl(
  serverId: string,
  ratingKey: string,
  remoteUrl: string | null | undefined,
): Promise<string | null> {
  if (!remoteUrl) {
    try {
      const local = await coversApi.getLocal(serverId, ratingKey);
      return local ? pathToAssetUrl(local) : null;
    } catch {
      return null;
    }
  }
  if (isLocalAssetUrl(remoteUrl)) return remoteUrl;
  if (!isRemoteMediaUrl(remoteUrl) && remoteUrl.startsWith("/")) {
    // Absolute filesystem path
    return pathToAssetUrl(remoteUrl);
  }
  try {
    const path = await coversApi.ensure(serverId, ratingKey, remoteUrl);
    return pathToAssetUrl(path);
  } catch {
    // Fall back to remote so the UI still shows something online
    return isRemoteMediaUrl(remoteUrl) ? remoteUrl : null;
  }
}

const HYDRATE_CHUNK = 8;

/**
 * Download missing covers and rewrite `thumb` to local asset URLs.
 * Preserves remote URL as fallback if ensure fails.
 * Processes in small chunks to avoid hammering PMS.
 */
export async function hydrateBookCovers(
  serverId: string,
  books: AudiobookSummary[],
): Promise<AudiobookSummary[]> {
  if (!serverId || books.length === 0) return books;

  const byKey = new Map<string, string>(); // ratingKey → asset url

  // 1) Already-local files (fast path, no download)
  const needDownload: CoverEnsureRequest[] = [];
  for (const b of books) {
    if (!b.ratingKey) continue;
    if (b.thumb && isLocalAssetUrl(b.thumb)) {
      byKey.set(b.ratingKey, b.thumb);
      continue;
    }
    try {
      const local = await coversApi.getLocal(serverId, b.ratingKey);
      if (local) {
        byKey.set(b.ratingKey, pathToAssetUrl(local));
        continue;
      }
    } catch {
      /* fall through */
    }
    if (b.thumb && isRemoteMediaUrl(b.thumb)) {
      needDownload.push({
        serverId,
        ratingKey: b.ratingKey,
        remoteUrl: b.thumb,
      });
    }
  }

  // 2) Download missing in chunks
  for (let i = 0; i < needDownload.length; i += HYDRATE_CHUNK) {
    const chunk = needDownload.slice(i, i + HYDRATE_CHUNK);
    try {
      const results = await coversApi.ensureMany(chunk);
      for (const r of results) {
        if (r.path) byKey.set(r.ratingKey, pathToAssetUrl(r.path));
      }
    } catch {
      // Fallback: try one-by-one
      for (const req of chunk) {
        try {
          const path = await coversApi.ensure(
            req.serverId,
            req.ratingKey,
            req.remoteUrl,
          );
          byKey.set(req.ratingKey, pathToAssetUrl(path));
        } catch {
          /* keep remote */
        }
      }
    }
  }

  return books.map((b) => {
    const local = byKey.get(b.ratingKey);
    if (!local || local === b.thumb) return b;
    return { ...b, thumb: local };
  });
}

/** Rewrite BookDetail thumb/art to local cover when possible. */
export async function hydrateDetailCovers(
  serverId: string,
  detail: BookDetail,
): Promise<BookDetail> {
  const remote = detail.thumb || detail.art;
  const local = await ensureCoverUrl(serverId, detail.ratingKey, remote);
  if (!local) return detail;
  return {
    ...detail,
    thumb: local,
    art: local,
  };
}
