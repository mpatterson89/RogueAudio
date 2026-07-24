import { writable, get, derived } from "svelte/store";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { downloadsApi } from "$lib/api/downloads";
import {
  detailFromLocalPlayback,
  seedBookDetail,
} from "$lib/stores/bookDetail";
import type { DownloadItem, DownloadQueueState } from "$lib/types/downloads";

interface DownloadsState {
  items: DownloadItem[];
  loading: boolean;
  error: string | null;
  /** ratingKeys currently mid-enqueue (before first event) */
  pending: Record<string, boolean>;
  queue: DownloadQueueState;
  /** True after restore() has been attempted this session */
  restored: boolean;
}

const emptyQueue = (): DownloadQueueState => ({
  paused: false,
  order: [],
  activeRatingKey: null,
  estimatedBytes: 0,
  bytesDownloaded: 0,
  bytesRemaining: 0,
  queuedCount: 0,
  activeCount: 0,
});

const initial: DownloadsState = {
  items: [],
  loading: false,
  error: null,
  pending: {},
  queue: emptyQueue(),
  restored: false,
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
  let unlistenProgress: UnlistenFn | null = null;
  let unlistenQueue: UnlistenFn | null = null;
  let started = false;
  let initPromise: Promise<void> | null = null;

  async function ensureListener() {
    if (started || typeof window === "undefined") return;
    started = true;
    try {
      // Throttle progress UI updates — high-frequency events freeze the UI while downloading
      let progressRaf = 0;
      let latestProgress: DownloadItem | null = null;
      const flushProgress = () => {
        progressRaf = 0;
        const item = latestProgress;
        if (!item) return;
        update((s) => {
          const pending = { ...s.pending };
          delete pending[item.ratingKey];
          return {
            ...s,
            items: upsert(s.items, item),
            pending,
          };
        });
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
      };
      unlistenProgress = await listen<DownloadItem>("download-progress", (ev) => {
        latestProgress = ev.payload;
        const terminal =
          ev.payload.status === "complete" ||
          ev.payload.status === "error" ||
          ev.payload.status === "cancelled" ||
          ev.payload.status === "paused";
        if (terminal) {
          if (progressRaf) cancelAnimationFrame(progressRaf);
          progressRaf = 0;
          flushProgress();
          return;
        }
        if (!progressRaf) {
          progressRaf = requestAnimationFrame(flushProgress);
        }
      });
    } catch {
      /* not in Tauri / events unavailable */
    }
    try {
      unlistenQueue = await listen<DownloadQueueState>("download-queue", (ev) => {
        update((s) => ({ ...s, queue: ev.payload }));
      });
    } catch {
      /* ignore */
    }
  }

  async function refreshItems() {
    const items = await downloadsApi.list();
    update((s) => ({ ...s, items, loading: false }));
  }

  async function refreshQueue() {
    try {
      const queue = await downloadsApi.queueState();
      update((s) => ({ ...s, queue }));
    } catch {
      /* older backend / browser */
    }
  }

  return {
    subscribe,

    /**
     * Call once on app start: restore interrupted queue, then list items.
     * Safe to call multiple times (deduped).
     */
    async init() {
      if (initPromise) return initPromise;
      initPromise = (async () => {
        await ensureListener();
        update((s) => ({ ...s, loading: true, error: null }));
        try {
          let queue: DownloadQueueState | null = null;
          try {
            queue = await downloadsApi.restore();
          } catch {
            /* command may fail outside Tauri */
          }
          const items = await downloadsApi.list().catch(() => [] as DownloadItem[]);
          update((s) => ({
            ...s,
            items,
            queue: queue ?? s.queue,
            loading: false,
            restored: true,
          }));
          if (!queue) await refreshQueue();
        } catch (e) {
          update((s) => ({
            ...s,
            loading: false,
            restored: true,
            error: e instanceof Error ? e.message : String(e),
          }));
        }
      })();
      return initPromise;
    },

    async refresh() {
      await ensureListener();
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        await refreshItems();
        await refreshQueue();
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
        await refreshQueue();
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
      await refreshItems();
      await refreshQueue();
    },

    async pauseQueue() {
      const queue = await downloadsApi.pauseQueue();
      update((s) => ({ ...s, queue }));
      await refreshItems();
      return queue;
    },

    async resumeQueue() {
      const queue = await downloadsApi.resumeQueue();
      update((s) => ({ ...s, queue }));
      await refreshItems();
      return queue;
    },

    async remove(ratingKey: string) {
      await downloadsApi.remove(ratingKey);
      update((s) => ({
        ...s,
        items: s.items.filter((i) => i.ratingKey !== ratingKey),
      }));
      await refreshQueue();
    },

    async removeAll() {
      const n = await downloadsApi.removeAll();
      update((s) => ({ ...s, items: [] }));
      await refreshQueue();
      return n;
    },

    getItem(ratingKey: string): DownloadItem | null {
      const s = get(store);
      return s.items.find((i) => i.ratingKey === ratingKey) ?? null;
    },

    destroy() {
      if (unlistenProgress) {
        unlistenProgress();
        unlistenProgress = null;
      }
      if (unlistenQueue) {
        unlistenQueue();
        unlistenQueue = null;
      }
      started = false;
      initPromise = null;
    },
  };
}

export const downloads = createDownloadsStore();

export const downloadsByKey = derived(downloads, ($d) => {
  const map = new Map<string, DownloadItem>();
  for (const item of $d.items) map.set(item.ratingKey, item);
  return map;
});

/** Items currently in the download queue (not complete / cancelled). */
export const queueItems = derived(downloads, ($d) => {
  const order = $d.queue.order;
  const byKey = new Map($d.items.map((i) => [i.ratingKey, i]));
  const ordered: DownloadItem[] = [];
  for (const key of order) {
    const item = byKey.get(key);
    if (item && isInDownloadQueue(item)) ordered.push(item);
  }
  // Fallback: any active-ish items missing from order (heal race)
  for (const item of $d.items) {
    if (isInDownloadQueue(item) && !ordered.some((o) => o.ratingKey === item.ratingKey)) {
      ordered.push(item);
    }
  }
  return ordered;
});

export function isDownloadComplete(item: DownloadItem | null | undefined): boolean {
  return item?.status === "complete";
}

/** Actively transferring or waiting to transfer. */
export function isDownloadActive(item: DownloadItem | null | undefined): boolean {
  return item?.status === "downloading" || item?.status === "queued";
}

/** Still part of the offline queue (includes paused / error). */
export function isInDownloadQueue(item: DownloadItem | null | undefined): boolean {
  const s = item?.status;
  return s === "downloading" || s === "queued" || s === "paused" || s === "error";
}

export function formatBytes(n: number): string {
  if (!n || n < 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export function statusLabel(status: string | undefined): string {
  switch (status) {
    case "queued":
      return "Queued";
    case "downloading":
      return "Downloading";
    case "paused":
      return "Paused";
    case "complete":
      return "Complete";
    case "error":
      return "Error";
    case "cancelled":
      return "Cancelled";
    default:
      return status ?? "Unknown";
  }
}
