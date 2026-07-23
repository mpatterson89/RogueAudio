import { writable, get, derived } from "svelte/store";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { downloadsApi } from "$lib/api/downloads";
import {
  detailFromLocalPlayback,
  seedBookDetail,
} from "$lib/stores/bookDetail";
import type { DownloadItem } from "$lib/types/downloads";

interface DownloadsState {
  items: DownloadItem[];
  loading: boolean;
  error: string | null;
  /** ratingKeys currently mid-enqueue (before first event) */
  pending: Record<string, boolean>;
}

const initial: DownloadsState = {
  items: [],
  loading: false,
  error: null,
  pending: {},
};

function upsert(items: DownloadItem[], item: DownloadItem): DownloadItem[] {
  const i = items.findIndex((x) => x.ratingKey === item.ratingKey);
  if (i < 0) return [...items, item].sort((a, b) => a.title.localeCompare(b.title));
  const next = items.slice();
  next[i] = item;
  return next;
}

function createDownloadsStore() {
  const store = writable<DownloadsState>(initial);
  const { subscribe, update } = store;
  let unlisten: UnlistenFn | null = null;
  let started = false;

  async function ensureListener() {
    if (started || typeof window === "undefined") return;
    started = true;
    try {
      unlisten = await listen<DownloadItem>("download-progress", (ev) => {
        const item = ev.payload;
        update((s) => {
          const pending = { ...s.pending };
          delete pending[item.ratingKey];
          return {
            ...s,
            items: upsert(s.items, item),
            pending,
          };
        });
        // Warm book-view cache from offline manifest when a download finishes
        if (item.status === "complete" && item.serverId) {
          void downloadsApi
            .localPlayback(item.ratingKey)
            .then((local) => {
              if (!local) return;
              seedBookDetail(
                item.serverId,
                item.ratingKey,
                detailFromLocalPlayback(item.ratingKey, local),
              );
            })
            .catch(() => {
              /* ignore */
            });
        }
      });
    } catch {
      /* not in Tauri / events unavailable */
    }
  }

  return {
    subscribe,

    async refresh() {
      await ensureListener();
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const items = await downloadsApi.list();
        update((s) => ({ ...s, items, loading: false }));
      } catch (e) {
        update((s) => ({
          ...s,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },

    async enqueue(serverId: string, ratingKey: string) {
      await ensureListener();
      update((s) => ({
        ...s,
        pending: { ...s.pending, [ratingKey]: true },
        error: null,
      }));
      try {
        const item = await downloadsApi.enqueue(serverId, ratingKey);
        update((s) => {
          const pending = { ...s.pending };
          delete pending[ratingKey];
          return { ...s, items: upsert(s.items, item), pending };
        });
        return item;
      } catch (e) {
        update((s) => {
          const pending = { ...s.pending };
          delete pending[ratingKey];
          return {
            ...s,
            pending,
            error: e instanceof Error ? e.message : String(e),
          };
        });
        throw e;
      }
    },

    async cancel(ratingKey: string) {
      await downloadsApi.cancel(ratingKey);
    },

    async remove(ratingKey: string) {
      await downloadsApi.remove(ratingKey);
      update((s) => ({
        ...s,
        items: s.items.filter((i) => i.ratingKey !== ratingKey),
      }));
    },

    async removeAll() {
      const n = await downloadsApi.removeAll();
      update((s) => ({ ...s, items: [] }));
      return n;
    },

    getItem(ratingKey: string): DownloadItem | null {
      const s = get(store);
      return s.items.find((i) => i.ratingKey === ratingKey) ?? null;
    },

    destroy() {
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
      started = false;
    },
  };
}

export const downloads = createDownloadsStore();

export const downloadsByKey = derived(downloads, ($d) => {
  const map = new Map<string, DownloadItem>();
  for (const item of $d.items) map.set(item.ratingKey, item);
  return map;
});

export function isDownloadComplete(item: DownloadItem | null | undefined): boolean {
  return item?.status === "complete";
}

export function isDownloadActive(item: DownloadItem | null | undefined): boolean {
  return item?.status === "downloading" || item?.status === "queued";
}

export function formatBytes(n: number): string {
  if (!n || n < 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}
