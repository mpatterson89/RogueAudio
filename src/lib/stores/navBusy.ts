import { writable, get } from "svelte/store";

interface NavBusyState {
  active: boolean;
  label: string | null;
  /** Nested start() count so parallel work doesn't clear early */
  depth: number;
}

const initial: NavBusyState = {
  active: false,
  label: null,
  depth: 0,
};

const store = writable<NavBusyState>(initial);

/** Programmatic navigation / pre-route busy flag (complements `$navigating`). */
export const navBusy = {
  subscribe: store.subscribe,

  start(label?: string) {
    store.update((s) => ({
      active: true,
      label: label ?? s.label,
      depth: s.depth + 1,
    }));
  },

  stop() {
    store.update((s) => {
      const depth = Math.max(0, s.depth - 1);
      return {
        active: depth > 0,
        label: depth > 0 ? s.label : null,
        depth,
      };
    });
  },

  /** Force clear (e.g. after a failed navigation). */
  reset() {
    store.set({ ...initial });
  },

  get snapshot() {
    return get(store);
  },
};

/** Run async work with the global nav busy indicator. */
export async function runWithNavBusy<T>(
  label: string | undefined,
  fn: () => Promise<T>,
): Promise<T> {
  navBusy.start(label);
  try {
    return await fn();
  } finally {
    navBusy.stop();
  }
}
