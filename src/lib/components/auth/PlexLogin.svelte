<script lang="ts">
  import { auth } from "$lib/stores/auth";
  import { openUrl } from "@tauri-apps/plugin-opener";

  async function openAuthPage() {
    if ($auth.pin?.authUrl) {
      try {
        await openUrl($auth.pin.authUrl);
      } catch {
        // Browser open may fail outside Tauri; user can copy code
      }
    }
  }
</script>

<div class="mx-auto max-w-lg space-y-6">
  <header class="space-y-2">
    <h1 class="text-2xl font-semibold tracking-tight">Connect to Plex</h1>
    <p class="text-sm leading-relaxed text-ra-muted">
      Sign in with a PIN to access your self-hosted audiobook libraries. RogueAudio never
      talks to Audible or any DRM service.
    </p>
  </header>

  {#if $auth.status.authenticated}
    <div class="rounded-2xl border border-ra-border bg-ra-surface p-5">
      <p class="text-sm text-ra-muted">Signed in as</p>
      <p class="mt-1 text-lg font-semibold">{$auth.status.username ?? "Plex user"}</p>
      <button
        type="button"
        class="mt-4 min-h-11 rounded-xl border border-ra-border px-4 text-sm hover:border-ra-danger hover:text-ra-danger"
        onclick={() => auth.logout()}
      >
        Sign out
      </button>
    </div>
  {:else}
    <div class="rounded-2xl border border-ra-border bg-ra-surface p-5 space-y-4">
      {#if $auth.pin}
        <div class="text-center">
          <p class="text-xs uppercase tracking-wide text-ra-muted">Your PIN</p>
          <p class="mt-2 font-mono text-4xl font-bold tracking-[0.3em] text-ra-accent">
            {$auth.pin.code}
          </p>
          <p class="mt-3 text-sm text-ra-muted">
            Enter this code at plex.tv/link
            {#if $auth.polling}
              <span class="ml-1 animate-pulse">· waiting…</span>
            {/if}
          </p>
          <button
            type="button"
            class="mt-4 min-h-11 rounded-xl bg-ra-accent px-5 text-sm font-medium text-white hover:bg-ra-accent-hover"
            onclick={openAuthPage}
          >
            Open Plex authorization
          </button>
        </div>
      {:else}
        <button
          type="button"
          class="min-h-12 w-full rounded-xl bg-ra-accent text-sm font-semibold text-white hover:bg-ra-accent-hover disabled:opacity-50"
          disabled={$auth.loading}
          onclick={() => auth.startPin()}
        >
          {$auth.loading ? "Starting…" : "Sign in with Plex PIN"}
        </button>
      {/if}

      <div class="border-t border-ra-border pt-4">
        <p class="mb-2 text-xs text-ra-muted">
          Development only — simulate a successful login to explore the UI without a live
          Plex account.
        </p>
        <button
          type="button"
          class="min-h-11 w-full rounded-xl border border-dashed border-ra-border text-sm text-ra-muted hover:border-ra-accent hover:text-ra-text"
          onclick={() => auth.devComplete("Dev Listener")}
        >
          Continue with stub auth
        </button>
      </div>
    </div>
  {/if}

  {#if $auth.error}
    <p class="text-sm text-ra-danger">{$auth.error}</p>
  {/if}
</div>
