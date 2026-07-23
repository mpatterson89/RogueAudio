/**
 * Cache for book-view detail (summary, chapters, art metadata).
 * Mirrors the library cache: persist across restarts; only re-hit Plex on force refresh.
 */
import { plexApi } from "$lib/api/plex";
import { downloadsApi } from "$lib/api/downloads";
import { convertFileSrc } from "@tauri-apps/api/core";
import { hydrateDetailCovers } from "$lib/covers";
import type { BookDetail } from "$lib/types/models";
import type { LocalPlayback } from "$lib/types/downloads";

const CACHE_KEY = "rogueaudio.bookDetailCache";
const CACHE_VERSION = 1;
/** Max titles kept in localStorage (LRU by last access). */
const MAX_ENTRIES = 80;

interface CacheEntry {
  serverId: string;
  ratingKey: string;
  detail: BookDetail;
  fetchedAt: number;
  /** Bumped on each successful read so LRU can drop cold titles. */
  lastAccessAt: number;
}

interface PersistedBlob {
  version: number;
  entries: CacheEntry[];
}

function cacheKey(serverId: string, ratingKey: string): string {
  return `${serverId}::${ratingKey}`;
}

const mem = new Map<string, CacheEntry>();

function loadDisk(): void {
  if (typeof localStorage === "undefined") return;
  if (mem.size > 0) return;
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (!raw) return;
    const blob = JSON.parse(raw) as PersistedBlob;
    if (blob?.version !== CACHE_VERSION || !Array.isArray(blob.entries)) return;
    for (const e of blob.entries) {
      if (e?.serverId && e?.ratingKey && e?.detail) {
        mem.set(cacheKey(e.serverId, e.ratingKey), e);
      }
    }
  } catch {
    /* ignore corrupt cache */
  }
}

function persist(): void {
  if (typeof localStorage === "undefined") return;
  try {
    // LRU: keep most recently accessed
    const entries = Array.from(mem.values())
      .sort((a, b) => b.lastAccessAt - a.lastAccessAt)
      .slice(0, MAX_ENTRIES);
    // Drop cold keys from mem to match disk
    if (entries.length < mem.size) {
      const keep = new Set(entries.map((e) => cacheKey(e.serverId, e.ratingKey)));
      for (const k of mem.keys()) {
        if (!keep.has(k)) mem.delete(k);
      }
    }
    const blob: PersistedBlob = { version: CACHE_VERSION, entries };
    localStorage.setItem(CACHE_KEY, JSON.stringify(blob));
  } catch {
    /* quota / private mode */
  }
}

function touchAndGet(serverId: string, ratingKey: string): BookDetail | null {
  loadDisk();
  const key = cacheKey(serverId, ratingKey);
  const entry = mem.get(key);
  if (!entry) return null;
  entry.lastAccessAt = Date.now();
  mem.set(key, entry);
  // Soft persist access order (cheap enough)
  persist();
  return entry.detail;
}

function put(serverId: string, ratingKey: string, detail: BookDetail): BookDetail {
  loadDisk();
  const now = Date.now();
  const entry: CacheEntry = {
    serverId,
    ratingKey,
    detail,
    fetchedAt: now,
    lastAccessAt: now,
  };
  mem.set(cacheKey(serverId, ratingKey), entry);
  persist();
  return detail;
}

function toPlayableCover(path: string | null | undefined): string | null {
  if (!path) return null;
  if (
    path.startsWith("http://") ||
    path.startsWith("https://") ||
    path.startsWith("asset:") ||
    path.includes("asset.localhost")
  ) {
    return path;
  }
  try {
    return convertFileSrc(path);
  } catch {
    return path;
  }
}

/** Build BookDetail from a completed offline download. */
export function detailFromLocalPlayback(
  ratingKey: string,
  local: LocalPlayback,
): BookDetail {
  const cover = toPlayableCover(local.coverPath ?? null);
  return {
    ratingKey,
    title: local.title,
    author: local.author ?? null,
    summary: local.summary ?? null,
    year: local.year ?? null,
    thumb: cover,
    art: cover,
    durationMs: local.durationMs ?? local.playback.totalDurationMs ?? null,
    libraryKey: local.libraryKey ?? null,
    studio: local.studio ?? null,
    chapters: local.chapters ?? [],
    trackCount: local.trackCount ?? local.playback.tracks?.length ?? 0,
  };
}

export type GetBookDetailOpts = {
  /** Bypass cache and re-fetch from Plex. */
  force?: boolean;
  /**
   * Prefer complete offline download over Plex when available.
   * Default true — reduces API use and enables offline book view.
   */
  preferOffline?: boolean;
};

/**
 * Resolve book detail with cache → offline download → Plex fallback.
 */
export async function getBookDetail(
  serverId: string,
  ratingKey: string,
  opts: GetBookDetailOpts = {},
): Promise<BookDetail> {
  const force = opts.force ?? false;
  const preferOffline = opts.preferOffline ?? true;

  if (!force) {
    const cached = touchAndGet(serverId, ratingKey);
    if (cached) {
      // Refresh cover to local file if we only had a remote URL cached
      const withCover = await hydrateDetailCovers(serverId, cached);
      if (withCover.thumb !== cached.thumb || withCover.art !== cached.art) {
        return put(serverId, ratingKey, withCover);
      }
      return cached;
    }
  }

  if (preferOffline) {
    try {
      const local = await downloadsApi.localPlayback(ratingKey);
      if (local?.playback?.tracks?.length) {
        const detail = detailFromLocalPlayback(ratingKey, local);
        const withCover = await hydrateDetailCovers(serverId, detail);
        return put(serverId, ratingKey, withCover);
      }
    } catch {
      /* not downloaded or API unavailable */
    }
  }

  const detail = await plexApi.getBookDetail(serverId, ratingKey);
  const withCover = await hydrateDetailCovers(serverId, detail);
  return put(serverId, ratingKey, withCover);
}

/** Peek without network or access-order update side effects for UI. */
export function peekBookDetail(
  serverId: string,
  ratingKey: string,
): BookDetail | null {
  loadDisk();
  return mem.get(cacheKey(serverId, ratingKey))?.detail ?? null;
}

/** Insert/overwrite (e.g. after download finishes with full metadata). */
export function seedBookDetail(
  serverId: string,
  ratingKey: string,
  detail: BookDetail,
): void {
  put(serverId, ratingKey, detail);
}

export function invalidateBookDetail(
  serverId?: string,
  ratingKey?: string,
): void {
  loadDisk();
  if (serverId && ratingKey) {
    mem.delete(cacheKey(serverId, ratingKey));
  } else if (serverId) {
    for (const [k, e] of mem) {
      if (e.serverId === serverId) mem.delete(k);
    }
  } else {
    mem.clear();
  }
  persist();
}

export function clearBookDetailCache(): void {
  mem.clear();
  try {
    localStorage.removeItem(CACHE_KEY);
  } catch {
    /* ignore */
  }
}
