import { writable } from "svelte/store";

import type { LibraryViewMode } from "$lib/types/models";
import type { AuthorSort, BookSort, CollectionSort } from "$lib/sort";

export interface SettingsState {
  /** Reduce motion for accessibility */
  reduceMotion: boolean;
  /** Sleep fade seconds */
  sleepFadeSeconds: number;
  /** Default skip interval */
  skipSeconds: number;
  /** Whether the bottom player chrome is visible */
  playerBarVisible: boolean;
  /** Books vs authors on library / collection detail */
  libraryViewMode: LibraryViewMode;
  bookSort: BookSort;
  authorSort: AuthorSort;
  collectionSort: CollectionSort;
}

const defaultSettings: SettingsState = {
  reduceMotion: false,
  sleepFadeSeconds: 15,
  skipSeconds: 30,
  playerBarVisible: true,
  libraryViewMode: "books",
  bookSort: "title_asc",
  authorSort: "name_asc",
  collectionSort: "name_asc",
};

function load(): SettingsState {
  if (typeof localStorage === "undefined") return defaultSettings;
  try {
    const raw = localStorage.getItem("rogueaudio.settings");
    if (!raw) return defaultSettings;
    return { ...defaultSettings, ...JSON.parse(raw) };
  } catch {
    return defaultSettings;
  }
}

function createSettingsStore() {
  const { subscribe, update, set } = writable<SettingsState>(load());

  return {
    subscribe,
    set,
    patch(partial: Partial<SettingsState>) {
      update((s) => {
        const next = { ...s, ...partial };
        try {
          localStorage.setItem("rogueaudio.settings", JSON.stringify(next));
        } catch {
          /* ignore */
        }
        return next;
      });
    },
    togglePlayerBar() {
      update((s) => {
        const next = { ...s, playerBarVisible: !s.playerBarVisible };
        try {
          localStorage.setItem("rogueaudio.settings", JSON.stringify(next));
        } catch {
          /* ignore */
        }
        return next;
      });
    },
  };
}

export const settings = createSettingsStore();
