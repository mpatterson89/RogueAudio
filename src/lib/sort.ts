import type {
  AudiobookSummary,
  AuthorSummary,
  PlexCollection,
  UserCollection,
} from "$lib/types/models";

/** Book list sort (library books, author page, collection detail). */
export type BookSort =
  | "title_asc"
  | "title_desc"
  | "year_desc"
  | "year_asc"
  | "author_asc"
  | "duration_desc";

/** Author list sort. */
export type AuthorSort =
  | "name_asc"
  | "name_desc"
  | "count_desc"
  | "count_asc"
  | "year_desc"
  | "year_asc";

/** Collection list sort (user + plex cards). */
export type CollectionSort =
  | "name_asc"
  | "name_desc"
  | "count_desc"
  | "count_asc"
  | "recent_desc";

export const BOOK_SORT_OPTIONS: { value: BookSort; label: string }[] = [
  { value: "title_asc", label: "Title A–Z" },
  { value: "title_desc", label: "Title Z–A" },
  { value: "year_desc", label: "Release date (newest)" },
  { value: "year_asc", label: "Release date (oldest)" },
  { value: "author_asc", label: "Author A–Z" },
  { value: "duration_desc", label: "Longest first" },
];

export const AUTHOR_SORT_OPTIONS: { value: AuthorSort; label: string }[] = [
  { value: "name_asc", label: "Name A–Z" },
  { value: "name_desc", label: "Name Z–A" },
  { value: "count_desc", label: "Most titles" },
  { value: "count_asc", label: "Fewest titles" },
  { value: "year_desc", label: "Latest release" },
  { value: "year_asc", label: "Earliest release" },
];

export const COLLECTION_SORT_OPTIONS: { value: CollectionSort; label: string }[] =
  [
    { value: "name_asc", label: "Name A–Z" },
    { value: "name_desc", label: "Name Z–A" },
    { value: "count_desc", label: "Most titles" },
    { value: "count_asc", label: "Fewest titles" },
    { value: "recent_desc", label: "Recently updated" },
  ];

function cmpStr(a: string, b: string): number {
  return a.localeCompare(b, undefined, { sensitivity: "base" });
}

function yearOf(b: AudiobookSummary): number {
  return b.year ?? 0;
}

export function sortBooks(
  books: AudiobookSummary[],
  sort: BookSort,
): AudiobookSummary[] {
  const list = books.slice();
  switch (sort) {
    case "title_asc":
      return list.sort((a, b) => cmpStr(a.title, b.title));
    case "title_desc":
      return list.sort((a, b) => cmpStr(b.title, a.title));
    case "year_desc":
      return list.sort((a, b) => yearOf(b) - yearOf(a) || cmpStr(a.title, b.title));
    case "year_asc":
      return list.sort((a, b) => {
        const ya = a.year ?? 99999;
        const yb = b.year ?? 99999;
        return ya - yb || cmpStr(a.title, b.title);
      });
    case "author_asc":
      return list.sort(
        (a, b) =>
          cmpStr(a.author ?? "", b.author ?? "") || cmpStr(a.title, b.title),
      );
    case "duration_desc":
      return list.sort(
        (a, b) =>
          (b.durationMs ?? 0) - (a.durationMs ?? 0) || cmpStr(a.title, b.title),
      );
    default:
      return list;
  }
}

export function sortAuthors(
  authors: AuthorSummary[],
  sort: AuthorSort,
): AuthorSummary[] {
  const list = authors.slice();
  switch (sort) {
    case "name_asc":
      return list.sort((a, b) => cmpStr(a.name, b.name));
    case "name_desc":
      return list.sort((a, b) => cmpStr(b.name, a.name));
    case "count_desc":
      return list.sort(
        (a, b) => b.bookCount - a.bookCount || cmpStr(a.name, b.name),
      );
    case "count_asc":
      return list.sort(
        (a, b) => a.bookCount - b.bookCount || cmpStr(a.name, b.name),
      );
    case "year_desc":
      return list.sort(
        (a, b) =>
          (b.latestYear ?? 0) - (a.latestYear ?? 0) || cmpStr(a.name, b.name),
      );
    case "year_asc": {
      return list.sort((a, b) => {
        const ya = a.earliestYear ?? 99999;
        const yb = b.earliestYear ?? 99999;
        return ya - yb || cmpStr(a.name, b.name);
      });
    }
    default:
      return list;
  }
}

export function sortUserCollections(
  items: UserCollection[],
  sort: CollectionSort,
): UserCollection[] {
  const list = items.slice();
  switch (sort) {
    case "name_asc":
      return list.sort((a, b) => cmpStr(a.name, b.name));
    case "name_desc":
      return list.sort((a, b) => cmpStr(b.name, a.name));
    case "count_desc":
      return list.sort(
        (a, b) =>
          b.ratingKeys.length - a.ratingKeys.length || cmpStr(a.name, b.name),
      );
    case "count_asc":
      return list.sort(
        (a, b) =>
          a.ratingKeys.length - b.ratingKeys.length || cmpStr(a.name, b.name),
      );
    case "recent_desc":
      return list.sort(
        (a, b) =>
          (b.updatedAt || b.createdAt).localeCompare(a.updatedAt || a.createdAt) ||
          cmpStr(a.name, b.name),
      );
    default:
      return list;
  }
}

export function sortPlexCollections(
  items: PlexCollection[],
  sort: CollectionSort,
): PlexCollection[] {
  const list = items.slice();
  switch (sort) {
    case "name_asc":
      return list.sort((a, b) => cmpStr(a.title, b.title));
    case "name_desc":
      return list.sort((a, b) => cmpStr(b.title, a.title));
    case "count_desc":
      return list.sort(
        (a, b) =>
          (b.childCount ?? 0) - (a.childCount ?? 0) || cmpStr(a.title, b.title),
      );
    case "count_asc":
      return list.sort(
        (a, b) =>
          (a.childCount ?? 0) - (b.childCount ?? 0) || cmpStr(a.title, b.title),
      );
    case "recent_desc":
      // Plex list has no updatedAt — fall back to name
      return list.sort((a, b) => cmpStr(a.title, b.title));
    default:
      return list;
  }
}
