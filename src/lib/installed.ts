/**
 * Helpers for the library "Installed" filter (offline downloads).
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import type { DownloadItem } from "$lib/types/downloads";
import type { AudiobookSummary } from "$lib/types/models";
import { isDownloadComplete } from "$lib/stores/downloads";

function coverUrl(path: string | null | undefined): string | null {
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

/** Complete offline downloads only. */
export function completeDownloads(items: DownloadItem[]): DownloadItem[] {
  return items.filter(
    (i) =>
      isDownloadComplete(i) ||
      (i.status === "complete" && (i.fileNames?.length ?? 0) > 0),
  );
}

/** Map a download record to a grid-friendly book summary. */
export function downloadToSummary(item: DownloadItem): AudiobookSummary {
  const authors = item.author
    ? item.author
        .split(/\s+&\s+|\s+and\s+|\//i)
        .map((s) => s.trim())
        .filter(Boolean)
    : [];
  return {
    ratingKey: item.ratingKey,
    title: item.title || "Audiobook",
    author: item.author ?? null,
    authors,
    thumb: coverUrl(item.coverPath),
    year: null,
    durationMs: item.durationMs ?? null,
    libraryKey: null,
  };
}

/**
 * Books to show in Installed mode.
 * Prefer full library metadata when available; fill gaps from download list.
 */
export function installedBooks(
  allBooks: AudiobookSummary[],
  downloads: DownloadItem[],
): AudiobookSummary[] {
  const complete = completeDownloads(downloads);
  const byKey = new Map(allBooks.map((b) => [b.ratingKey, b]));
  const out: AudiobookSummary[] = [];

  for (const d of complete) {
    const fromLib = byKey.get(d.ratingKey);
    if (fromLib) {
      out.push({
        ...fromLib,
        // Prefer local cover when we have one
        thumb: coverUrl(d.coverPath) || fromLib.thumb,
      });
    } else {
      out.push(downloadToSummary(d));
    }
  }

  return out;
}

/** Apply search query to a book list (title/author client filter). */
export function filterBooksByQuery(
  books: AudiobookSummary[],
  query: string,
): AudiobookSummary[] {
  const q = query.trim().toLowerCase();
  if (!q) return books;
  return books.filter((b) => {
    const title = b.title?.toLowerCase() ?? "";
    const author = b.author?.toLowerCase() ?? "";
    const authors = (b.authors ?? []).join(" ").toLowerCase();
    return title.includes(q) || author.includes(q) || authors.includes(q);
  });
}

export function serverIdForBook(
  book: AudiobookSummary,
  downloads: DownloadItem[],
  libraryServerId: string | null,
): string | null {
  if (libraryServerId) return libraryServerId;
  return downloads.find((d) => d.ratingKey === book.ratingKey)?.serverId ?? null;
}
