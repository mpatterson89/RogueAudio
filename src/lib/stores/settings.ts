import { writable } from "svelte/store";

export interface SettingsState {
  /** Reduce motion for accessibility */
  reduceMotion: boolean;
  /** Sleep fade seconds */
  sleepFadeSeconds: number;
  /** Default skip interval */
  skipSeconds: number;
}

const defaultSettings: SettingsState = {
  reduceMotion: false,
  sleepFadeSeconds: 15,
  skipSeconds: 30,
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
  };
}

export const settings = createSettingsStore();
