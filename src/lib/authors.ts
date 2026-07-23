import type { AudiobookSummary, AuthorSummary } from "$lib/types/models";

/** Stable key for routes / maps (lowercase, collapsed spaces). */
export function authorKey(name: string): string {
  return name.trim().toLowerCase().replace(/\s+/g, " ");
}

export function bookAuthors(book: AudiobookSummary): string[] {
  if (book.authors && book.authors.length > 0) {
    return book.authors.map((a) => a.trim()).filter(Boolean);
  }
  if (book.author?.trim()) {
    // Fallback split for cached books without authors[]
    return splitAuthorDisplay(book.author);
  }
  return [];
}

function splitAuthorDisplay(raw: string): string[] {
  const s = raw.trim();
  if (!s) return [];
  const lower = s.toLowerCase();
  if (lower.includes(" & ") || lower.includes(" and ") || s.includes("/")) {
    return s
      .split(/\s+&\s+|\s+and\s+|\s+with\s+|\//i)
      .map((p) => p.trim())
      .filter(Boolean);
  }
  return [s];
}

/** Group books under each author (collabs appear in every co-author's list). */
export function groupByAuthor(books: AudiobookSummary[]): AuthorSummary[] {
  const map = new Map<string, { name: string; books: AudiobookSummary[] }>();

  for (const book of books) {
    const names = bookAuthors(book);
    if (names.length === 0) {
      const key = "__unknown__";
      const cur = map.get(key) ?? { name: "Unknown author", books: [] };
      cur.books.push(book);
      map.set(key, cur);
      continue;
    }
    for (const name of names) {
      const key = authorKey(name);
      const cur = map.get(key) ?? { name, books: [] };
      // Prefer longer / better-cased display name
      if (name.length > cur.name.length) cur.name = name;
      cur.books.push(book);
      map.set(key, cur);
    }
  }

  const list: AuthorSummary[] = [];
  for (const [key, v] of map) {
    // Dedupe books by ratingKey within author
    const seen = new Set<string>();
    const unique = v.books.filter((b) => {
      if (seen.has(b.ratingKey)) return false;
      seen.add(b.ratingKey);
      return true;
    });
    const thumbs = unique
      .map((b) => b.thumb)
      .filter((t): t is string => !!t)
      .slice(0, 4);
    const years = unique
      .map((b) => b.year)
      .filter((y): y is number => typeof y === "number" && y > 0);
    list.push({
      key,
      name: v.name,
      bookCount: unique.length,
      thumbs,
      latestYear: years.length ? Math.max(...years) : null,
      earliestYear: years.length ? Math.min(...years) : null,
    });
  }

  // Default alpha; callers may re-sort
  list.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
  return list;
}

export function booksForAuthor(
  books: AudiobookSummary[],
  key: string,
): AudiobookSummary[] {
  const k = authorKey(decodeURIComponent(key));
  if (k === "__unknown__") {
    return books.filter((b) => bookAuthors(b).length === 0);
  }
  return books.filter((b) => bookAuthors(b).some((n) => authorKey(n) === k));
}

export function authorHref(key: string): string {
  return `/author/${encodeURIComponent(key)}`;
}
