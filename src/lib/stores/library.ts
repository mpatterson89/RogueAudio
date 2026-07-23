import { writable, get } from "svelte/store";
import { plexApi } from "$lib/api/plex";
import { filterMusicLibraries, pickDefaultLibrary } from "$lib/plex/libraries";
import { hydrateBookCovers } from "$lib/covers";
import type { AudiobookSummary, PlexLibrary, PlexServer } from "$lib/types/models";

const CACHE_KEY = "rogueaudio.libraryCache";
const CACHE_VERSION = 1;

interface LibraryState {
  servers: PlexServer[];
  serverId: string | null;
  /** Music-type libraries only (audiobook sources). */
  libraries: PlexLibrary[];
  libraryKey: string | null;
  /** Books currently displayed (may be search-filtered). */
  books: AudiobookSummary[];
  /** Full unfiltered list for the active server+library (from cache/API). */
  allBooks: AudiobookSummary[];
  query: string;
  loading: boolean;
  error: string | null;
  /** When the active library books were last fetched from Plex. */
  lastFetchedAt: number | null;
}

interface PersistedCache {
  version: number;
  servers: PlexServer[];
  serverId: string | null;
  libraryKey: string | null;
  librariesByServer: Record<string, PlexLibrary[]>;
  booksByKey: Record<string, AudiobookSummary[]>;
  lastFetchedAt: number | null;
}

const initial: LibraryState = {
  servers: [],
  serverId: null,
  libraries: [],
  libraryKey: null,
  books: [],
  allBooks: [],
  query: "",
  loading: false,
  error: null,
  lastFetchedAt: null,
};

function booksCacheKey(serverId: string, libraryKey: string): string {
  return `${serverId}::${libraryKey}`;
}

function filterBooks(all: AudiobookSummary[], query: string): AudiobookSummary[] {
  const q = query.trim().toLowerCase();
  if (!q) return all;
  return all.filter((b) => {
    const title = b.title?.toLowerCase() ?? "";
    const author = b.author?.toLowerCase() ?? "";
    return title.includes(q) || author.includes(q);
  });
}

/** In-memory cache shared for the session (also hydrated from localStorage). */
const mem = {
  servers: null as PlexServer[] | null,
  librariesByServer: new Map<string, PlexLibrary[]>(),
  booksByKey: new Map<string, AudiobookSummary[]>(),
  lastFetchedByKey: new Map<string, number>(),
};

function loadPersisted(): PersistedCache | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const data = JSON.parse(raw) as PersistedCache;
    if (data?.version !== CACHE_VERSION) return null;
    return data;
  } catch {
    return null;
  }
}

function persist() {
  if (typeof localStorage === "undefined") return;
  try {
    const s = get(store);
    const blob: PersistedCache = {
      version: CACHE_VERSION,
      servers: mem.servers ?? s.servers,
      serverId: s.serverId,
      libraryKey: s.libraryKey,
      librariesByServer: Object.fromEntries(mem.librariesByServer),
      booksByKey: Object.fromEntries(mem.booksByKey),
      lastFetchedAt: s.lastFetchedAt,
    };
    localStorage.setItem(CACHE_KEY, JSON.stringify(blob));
  } catch {
    /* quota / private mode */
  }
}

function hydrateFromDisk(): Partial<LibraryState> | null {
  const data = loadPersisted();
  if (!data) return null;

  mem.servers = data.servers ?? null;
  mem.librariesByServer = new Map(Object.entries(data.librariesByServer ?? {}));
  mem.booksByKey = new Map(Object.entries(data.booksByKey ?? {}));

  const serverId = data.serverId;
  const libraryKey = data.libraryKey;
  if (!serverId) {
    return {
      servers: data.servers ?? [],
      serverId: null,
      libraries: [],
      libraryKey: null,
      books: [],
      allBooks: [],
      lastFetchedAt: null,
    };
  }

  const libraries = mem.librariesByServer.get(serverId) ?? [];
  const allBooks =
    libraryKey != null
      ? (mem.booksByKey.get(booksCacheKey(serverId, libraryKey)) ?? [])
      : [];
  const lastFetchedAt =
    libraryKey != null
      ? (mem.lastFetchedByKey.get(booksCacheKey(serverId, libraryKey)) ??
        data.lastFetchedAt)
      : data.lastFetchedAt;

  if (libraryKey && lastFetchedAt) {
    mem.lastFetchedByKey.set(booksCacheKey(serverId, libraryKey), lastFetchedAt);
  }

  return {
    servers: data.servers ?? [],
    serverId,
    libraries,
    libraryKey,
    allBooks,
    books: allBooks,
    lastFetchedAt: lastFetchedAt ?? null,
    query: "",
    loading: false,
    error: null,
  };
}

function createInitialState(): LibraryState {
  const hydrated = hydrateFromDisk();
  return hydrated ? { ...initial, ...hydrated } : { ...initial };
}

const store = writable<LibraryState>(createInitialState());

function createLibraryStore() {
  const { subscribe, update } = store;

  function applyBooks(
    allBooks: AudiobookSummary[],
    query: string,
    extra: Partial<LibraryState> = {},
  ) {
    update((s) => ({
      ...s,
      ...extra,
      allBooks,
      books: filterBooks(allBooks, query),
      query,
      loading: false,
      error: null,
    }));
  }

  function setCachedBooks(
    serverId: string,
    libraryKey: string,
    books: AudiobookSummary[],
    fetchedAt = Date.now(),
  ) {
    const key = booksCacheKey(serverId, libraryKey);
    // Always store remote Plex thumb URLs in the durable cache (not asset://).
    mem.booksByKey.set(key, books);
    mem.lastFetchedByKey.set(key, fetchedAt);
  }

  /**
   * Show books immediately, then rewrite thumbs to on-disk covers in the background.
   * Durable cache keeps remote URLs; UI state gets local asset URLs.
   */
  async function applyBooksWithCovers(
    serverId: string,
    libraryKey: string,
    remoteBooks: AudiobookSummary[],
    query: string,
    extra: Partial<LibraryState> = {},
  ) {
    applyBooks(remoteBooks, query, { ...extra, serverId, libraryKey });
    try {
      const withCovers = await hydrateBookCovers(serverId, remoteBooks);
      const cur = get(store);
      if (cur.serverId !== serverId || cur.libraryKey !== libraryKey) return;
      applyBooks(withCovers, cur.query, { serverId, libraryKey });
    } catch {
      /* remote thumbs already showing */
    }
  }

  return {
    subscribe,

    /**
     * Load only if we have nothing usable for the current user session.
     * Uses memory + localStorage cache — does not hit Plex when data is already loaded.
     */
    async ensureLoaded() {
      const s = get(store);
      if (s.loading) return;

      // Already loaded this selection (including empty library after a successful fetch)
      if (s.servers.length > 0 && s.serverId && s.libraryKey) {
        const key = booksCacheKey(s.serverId, s.libraryKey);
        if (mem.booksByKey.has(key)) {
          const cached = mem.booksByKey.get(key)!;
          // Resolve on-disk covers (no Plex list call)
          await applyBooksWithCovers(s.serverId, s.libraryKey, cached, s.query, {
            lastFetchedAt: mem.lastFetchedByKey.get(key) ?? s.lastFetchedAt,
          });
          return;
        }
      }

      // Servers loaded but no music libraries (valid empty state)
      if (
        s.servers.length > 0 &&
        s.serverId &&
        mem.librariesByServer.has(s.serverId) &&
        s.libraries.length === 0
      ) {
        return;
      }

      // Hydrate books from memory for a known selection
      if (s.serverId && s.libraryKey) {
        const key = booksCacheKey(s.serverId, s.libraryKey);
        if (mem.booksByKey.has(key)) {
          const cached = mem.booksByKey.get(key)!;
          const libs = mem.librariesByServer.get(s.serverId) ?? s.libraries;
          await applyBooksWithCovers(s.serverId, s.libraryKey, cached, s.query, {
            servers: mem.servers ?? s.servers,
            libraries: libs,
            lastFetchedAt: mem.lastFetchedByKey.get(key) ?? s.lastFetchedAt,
          });
          return;
        }
      }

      // Disk may have servers while store is still empty
      if (mem.servers && mem.servers.length > 0 && s.servers.length === 0) {
        const serverId = s.serverId ?? mem.servers[0]?.id ?? null;
        update((st) => ({
          ...st,
          servers: mem.servers!,
          serverId,
        }));
        if (serverId) {
          await this.loadLibraries(serverId, { force: false });
          return;
        }
      }

      // Nothing cached — fetch from Plex
      await this.loadServers({ force: false });
    },

    /**
     * Force re-fetch of servers → libraries → books for the current (or default) selection.
     */
    async refresh() {
      await this.loadServers({ force: true });
    },

    async loadServers(opts: { force?: boolean } = {}) {
      const force = opts.force ?? false;

      if (!force && mem.servers && mem.servers.length > 0) {
        const s = get(store);
        const serverId = s.serverId ?? mem.servers[0]?.id ?? null;
        update((st) => ({
          ...st,
          servers: mem.servers!,
          serverId,
          loading: false,
          error: null,
        }));
        if (serverId) {
          await this.loadLibraries(serverId, { force: false });
        }
        return;
      }

      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const servers = await plexApi.listServers();
        mem.servers = servers;
        const prev = get(store);
        const serverId =
          (prev.serverId && servers.some((x) => x.id === prev.serverId)
            ? prev.serverId
            : null) ??
          servers[0]?.id ??
          null;
        update((s) => ({ ...s, servers, serverId, loading: false }));
        persist();
        if (serverId) {
          await this.loadLibraries(serverId, { force });
        } else {
          update((s) => ({
            ...s,
            libraries: [],
            libraryKey: null,
            books: [],
            allBooks: [],
          }));
        }
      } catch (e) {
        update((s) => ({
          ...s,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },

    async loadLibraries(serverId: string, opts: { force?: boolean } = {}) {
      const force = opts.force ?? false;

      if (!force && mem.librariesByServer.has(serverId)) {
        const libraries = mem.librariesByServer.get(serverId)!;
        const prev = get(store);
        const preferred =
          (prev.libraryKey && libraries.some((l) => l.key === prev.libraryKey)
            ? libraries.find((l) => l.key === prev.libraryKey)
            : null) ?? pickDefaultLibrary(libraries);
        update((s) => ({
          ...s,
          serverId,
          libraries,
          libraryKey: preferred?.key ?? null,
          loading: false,
          error: null,
        }));
        if (preferred) {
          await this.loadBooks(serverId, preferred.key, {
            force: false,
            query: get(store).query,
          });
        } else {
          applyBooks([], "", { serverId, libraries, libraryKey: null });
        }
        return;
      }

      update((s) => ({ ...s, loading: true, error: null, serverId }));
      try {
        const raw = await plexApi.listLibraries(serverId);
        const libraries = filterMusicLibraries(raw);
        mem.librariesByServer.set(serverId, libraries);

        const prev = get(store);
        const preferred =
          (prev.libraryKey &&
          libraries.some((l) => l.key === prev.libraryKey) &&
          !force
            ? libraries.find((l) => l.key === prev.libraryKey)
            : null) ?? pickDefaultLibrary(libraries);

        // On force refresh of a server change, prefer previous key if still valid
        const chosen =
          force && prev.libraryKey && libraries.some((l) => l.key === prev.libraryKey)
            ? libraries.find((l) => l.key === prev.libraryKey)!
            : preferred;

        update((s) => ({
          ...s,
          libraries,
          libraryKey: chosen?.key ?? null,
          loading: false,
        }));
        persist();

        if (chosen) {
          await this.loadBooks(serverId, chosen.key, {
            force,
            query: get(store).query,
          });
        } else {
          applyBooks([], "", { serverId, libraries, libraryKey: null });
          persist();
        }
      } catch (e) {
        update((s) => ({
          ...s,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },

    async loadBooks(
      serverId: string,
      libraryKey: string,
      opts: { force?: boolean; query?: string } = {},
    ) {
      const force = opts.force ?? false;
      const s = get(store);
      const query = opts.query !== undefined ? opts.query : s.query;
      const key = booksCacheKey(serverId, libraryKey);

      if (!force && mem.booksByKey.has(key)) {
        const allBooks = mem.booksByKey.get(key)!;
        await applyBooksWithCovers(serverId, libraryKey, allBooks, query, {
          lastFetchedAt: mem.lastFetchedByKey.get(key) ?? null,
        });
        return;
      }

      update((st) => ({
        ...st,
        loading: true,
        error: null,
        serverId,
        libraryKey,
        query,
        // Avoid flashing the previous section's titles when switching libraries
        ...(force && st.libraryKey !== libraryKey
          ? { books: [], allBooks: [] }
          : {}),
      }));
      try {
        // Always fetch full list; search is client-side on the cache.
        const allBooks = await plexApi.listBooks(serverId, libraryKey);
        const fetchedAt = Date.now();
        setCachedBooks(serverId, libraryKey, allBooks, fetchedAt);
        persist();
        await applyBooksWithCovers(serverId, libraryKey, allBooks, query, {
          lastFetchedAt: fetchedAt,
        });
      } catch (e) {
        update((st) => ({
          ...st,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },

    /** Client-side filter over the cached full library. */
    search(query: string) {
      update((s) => ({
        ...s,
        query,
        books: filterBooks(s.allBooks, query),
      }));
    },

    /**
     * Switch music library — always re-fetch from Plex for the new selection
     * (user expectation: changing selection refreshes that library).
     */
    selectLibrary(libraryKey: string) {
      const s = get(store);
      if (!s.serverId) return;
      // Clear query when switching sections
      update((st) => ({ ...st, query: "" }));
      void this.loadBooks(s.serverId, libraryKey, { force: true, query: "" });
    },

    /**
     * Switch Plex server — always re-fetch libraries + books for the new server.
     */
    selectServer(serverId: string) {
      update((st) => ({ ...st, query: "", libraryKey: null, books: [], allBooks: [] }));
      // Drop library list cache for this server so we re-list sections
      mem.librariesByServer.delete(serverId);
      void this.loadLibraries(serverId, { force: true });
    },

    /** Re-run the most relevant load after a Plex failure (always force). */
    async retry() {
      const s = get(store);
      if (s.serverId && s.libraryKey) {
        await this.loadBooks(s.serverId, s.libraryKey, {
          force: true,
          query: s.query || undefined,
        });
        return;
      }
      if (s.serverId) {
        await this.loadLibraries(s.serverId, { force: true });
        return;
      }
      await this.loadServers({ force: true });
    },

    reset() {
      mem.servers = null;
      mem.librariesByServer.clear();
      mem.booksByKey.clear();
      mem.lastFetchedByKey.clear();
      try {
        localStorage.removeItem(CACHE_KEY);
      } catch {
        /* ignore */
      }
      update(() => ({ ...initial }));
    },
  };
}

export const library = createLibraryStore();
