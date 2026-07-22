import { writable } from "svelte/store";
import { plexApi } from "$lib/api/plex";
import type { AudiobookSummary, PlexLibrary, PlexServer } from "$lib/types/models";

interface LibraryState {
  servers: PlexServer[];
  serverId: string | null;
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
  const { subscribe, update } = writable<LibraryState>(initial);

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
        const libraries = await plexApi.listLibraries(serverId);
        // Prefer a library titled like audiobooks
        const preferred =
          libraries.find((l) => /audio|book/i.test(l.title)) ?? libraries[0] ?? null;
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
      let serverId: string | null = null;
      let libraryKey: string | null = null;
      const unsub = subscribe((s) => {
        serverId = s.serverId;
        libraryKey = s.libraryKey;
      });
      unsub();
      if (serverId && libraryKey) {
        await this.loadBooks(serverId, libraryKey, query);
      }
    },
    selectLibrary(libraryKey: string) {
      let serverId: string | null = null;
      const unsub = subscribe((s) => {
        serverId = s.serverId;
      });
      unsub();
      if (serverId) void this.loadBooks(serverId, libraryKey);
    },
    reset() {
      update(() => ({ ...initial }));
    },
  };
}

export const library = createLibraryStore();
