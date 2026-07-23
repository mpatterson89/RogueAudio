import type { BookChapter } from "$lib/types/models";

/** Active chapter window on the book timeline. */
export interface ChapterWindow {
  index: number;
  title: string;
  /** Book-timeline start (seconds) */
  startSec: number;
  /** Book-timeline end (seconds) */
  endSec: number;
  /** Position within the chapter (seconds) */
  positionSec: number;
  /** Chapter length (seconds) */
  durationSec: number;
  progressPct: number;
}

/**
 * Find the chapter containing `positionMs` and return relative progress.
 * Uses chapter endTimeOffset, next chapter start, or book duration as end.
 */
export function resolveChapterWindow(
  chapters: BookChapter[],
  positionMs: number,
  bookDurationMs?: number | null,
): ChapterWindow | null {
  if (!chapters.length) return null;

  const pos = Math.max(0, positionMs);
  let idx = 0;
  for (let i = 0; i < chapters.length; i++) {
    if (chapters[i].startMs <= pos) idx = i;
    else break;
  }

  const ch = chapters[idx];
  const startMs = ch.startMs;
  const endMs =
    ch.endMs ??
    chapters[idx + 1]?.startMs ??
    (bookDurationMs && bookDurationMs > startMs ? bookDurationMs : startMs + 1);

  const durationMs = Math.max(1, endMs - startMs);
  const withinMs = Math.min(durationMs, Math.max(0, pos - startMs));
  const durationSec = durationMs / 1000;
  const positionSec = withinMs / 1000;

  return {
    index: idx,
    title: ch.title,
    startSec: startMs / 1000,
    endSec: endMs / 1000,
    positionSec,
    durationSec,
    progressPct: Math.min(100, (withinMs / durationMs) * 100),
  };
}
