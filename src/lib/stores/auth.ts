import { writable, derived } from "svelte/store";
import { plexApi } from "$lib/api/plex";
import { library } from "$lib/stores/library";
import type { AuthStatus, PinAuthStart } from "$lib/types/models";

interface AuthState {
  status: AuthStatus;
  pin: PinAuthStart | null;
  loading: boolean;
  error: string | null;
  polling: boolean;
}

const initial: AuthState = {
  status: { authenticated: false, username: null },
  pin: null,
  loading: false,
  error: null,
  polling: false,
};

function createAuthStore() {
  const { subscribe, update, set } = writable<AuthState>(initial);
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
    update((s) => ({ ...s, polling: false }));
  }

  return {
    subscribe,
    async refresh() {
      update((s) => ({ ...s, loading: true, error: null }));
      try {
        const status = await plexApi.authStatus();
        update((s) => ({ ...s, status, loading: false }));
      } catch (e) {
        update((s) => ({
          ...s,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },
    async startPin() {
      stopPolling();
      update((s) => ({ ...s, loading: true, error: null, pin: null }));
      try {
        const pin = await plexApi.startPinAuth();
        update((s) => ({ ...s, pin, loading: false, polling: true }));

        pollTimer = setInterval(async () => {
          try {
            const result = await plexApi.pollPinAuth();
            if (result.authorized) {
              stopPolling();
              update((s) => ({
                ...s,
                status: result.status,
                pin: null,
                polling: false,
              }));
            }
          } catch (e) {
            stopPolling();
            update((s) => ({
              ...s,
              error: e instanceof Error ? e.message : String(e),
              polling: false,
            }));
          }
        }, 2000);
      } catch (e) {
        update((s) => ({
          ...s,
          loading: false,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },
    async devComplete(username?: string) {
      try {
        const status = await plexApi.devCompleteAuth(username);
        stopPolling();
        update((s) => ({ ...s, status, pin: null, error: null }));
      } catch (e) {
        update((s) => ({
          ...s,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },
    async logout() {
      stopPolling();
      try {
        await plexApi.logout();
        library.reset();
        set({ ...initial });
      } catch (e) {
        update((s) => ({
          ...s,
          error: e instanceof Error ? e.message : String(e),
        }));
      }
    },
    stopPolling,
  };
}

export const auth = createAuthStore();
export const isAuthenticated = derived(auth, ($a) => $a.status.authenticated);
