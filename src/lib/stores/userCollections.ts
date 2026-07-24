import { writable, get } from "svelte/store";
import { userCollectionsApi } from "$lib/api/userCollections";
import type { UserCollection } from "$lib/types/models";

interface State {
  serverId: string | null;
  libraryKey: string | null;
  items: UserCollection[];
  loading: boolean;
  error: string | null;
}

const initial: State = {
  serverId: null,
  libraryKey: null,
  items: [],
  loading: false,
  error: null,
};

function createStore() {
  const store = writable<State>(initial);
  const { subscribe, update } = store;

  return {
    subscribe,

    async load(serverId: string, libraryKey: string) {
      update((s) => ({
        ...s,
        loading: true,
        error: null,
        serverId,
        libraryKey,
      }));
      try {
        const items = await userCollectionsApi.list(serverId, libraryKey);
        update((s) => ({ ...s, items, loading: false }));
      } catch (e) {
        update((s) => ({
          ...s,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },

    async ensure(serverId: string, libraryKey: string) {
      const s = get(store);
      if (
        s.serverId === serverId &&
        s.libraryKey === libraryKey &&
        (s.items.length > 0 || !s.loading)
      ) {
        // Still refresh if empty and not loading — first visit
        if (s.items.length === 0 && !s.error) {
          await this.load(serverId, libraryKey);
        }
        return;
      }
      await this.load(serverId, libraryKey);
    },

    async create(name: string) {
      const s = get(store);
      if (!s.serverId || !s.libraryKey) throw new Error("No library selected");
      const col = await userCollectionsApi.create(s.serverId, s.libraryKey, name);
      update((st) => ({
        ...st,
        items: [...st.items, col].sort((a, b) => a.name.localeCompare(b.name)),
      }));
      return col;
    },

    async rename(id: string, name: string) {
      const s = get(store);
      if (!s.serverId || !s.libraryKey) throw new Error("No library selected");
      const col = await userCollectionsApi.rename(
        s.serverId,
        s.libraryKey,
        id,
        name,
      );
      update((st) => ({
        ...st,
        items: st.items.map((c) => (c.id === id ? col : c)),
      }));
      return col;
    },

    async remove(id: string) {
      const s = get(store);
      if (!s.serverId || !s.libraryKey) throw new Error("No library selected");
      await userCollectionsApi.delete(s.serverId, s.libraryKey, id);
      update((st) => ({
        ...st,
        items: st.items.filter((c) => c.id !== id),
      }));
    },

    async addBooks(id: string, ratingKeys: string[]) {
      const s = get(store);
      if (!s.serverId || !s.libraryKey) throw new Error("No library selected");
      const col = await userCollectionsApi.addBooks(
        s.serverId,
        s.libraryKey,
        id,
        ratingKeys,
      );
      update((st) => ({
        ...st,
        items: st.items.map((c) => (c.id === id ? col : c)),
      }));
      return col;
    },

    async removeBooks(id: string, ratingKeys: string[]) {
      const s = get(store);
      if (!s.serverId || !s.libraryKey) throw new Error("No library selected");
      const col = await userCollectionsApi.removeBooks(
        s.serverId,
        s.libraryKey,
        id,
        ratingKeys,
      );
      update((st) => ({
        ...st,
        items: st.items.map((c) => (c.id === id ? col : c)),
      }));
      return col;
    },

    reset() {
      update(() => ({ ...initial }));
    },
  };
}

export const userCollections = createStore();
