import { writable, get } from "svelte/store";
import { plexApi } from "$lib/api/plex";
import { filterMusicLibraries, pickDefaultLibrary } from "$lib/plex/libraries";
import type { AudiobookSummary, PlexLibrary, PlexServer } from "$lib/types/models";

interface LibraryState {
  servers: PlexServer[];
  serverId: string | null;
  /** Music-type libraries only (audiobook sources). */
  libraries: PlexLibrary[];
  libraryKey: string | null;
  books: AudiobookSummary[];
  query: string;
  loading: boolean;
  error: string | null;
}

const initial: LibraryState = {
  servers: [],
  serverId: null,
  libraries: [],
  libraryKey: null,
  books: [],
  query: "",
  loading: false,
  error: null,
};

function createLibraryStore() {
  const store = writable<LibraryState>(initial);
  const { subscribe, update } = store;

  return {
    subscribe,
    async loadServers() {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const servers = await plexApi.listServers();
        const serverId = servers[0]?.id ?? null;
        update((s) => ({ ...s, servers, serverId, loading: false }));
        if (serverId) await this.loadLibraries(serverId);
      } catch (e) {
        update((s) => ({
          ...s,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },
    async loadLibraries(serverId: string) {
      update((s) => ({ ...s, loading: true, error: null, serverId }));
      try {
        const raw = await plexApi.listLibraries(serverId);
        // Music-type only; multiple music libs → UI filter (select).
        const libraries = filterMusicLibraries(raw);
        const preferred = pickDefaultLibrary(libraries);
        update((s) => ({
          ...s,
          libraries,
          libraryKey: preferred?.key ?? null,
          loading: false,
        }));
        if (preferred) await this.loadBooks(serverId, preferred.key);
      } catch (e) {
        update((s) => ({
          ...s,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },
    async loadBooks(serverId: string, libraryKey: string, query?: string) {
      update((s) => ({
        ...s,
        loading: true,
        error: null,
        serverId,
        libraryKey,
        query: query ?? s.query,
      }));
      try {
        const books = await plexApi.listBooks(serverId, libraryKey, query);
        update((s) => ({ ...s, books, loading: false }));
      } catch (e) {
        update((s) => ({
          ...s,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },
    async search(query: string) {
      update((s) => ({ ...s, query }));
      const s = get(store);
      if (s.serverId && s.libraryKey) {
        await this.loadBooks(s.serverId, s.libraryKey, query);
      }
    },
    selectLibrary(libraryKey: string) {
      const s = get(store);
      if (s.serverId) void this.loadBooks(s.serverId, libraryKey);
    },
    selectServer(serverId: string) {
      void this.loadLibraries(serverId);
    },
    /** Re-run the most relevant load after a Plex failure. */
    async retry() {
      const s = get(store);
      if (s.serverId && s.libraryKey) {
        await this.loadBooks(s.serverId, s.libraryKey, s.query || undefined);
        return;
      }
      if (s.serverId) {
        await this.loadLibraries(s.serverId);
        return;
      }
      await this.loadServers();
    },
    reset() {
      update(() => ({ ...initial }));
    },
  };
}

export const library = createLibraryStore();
