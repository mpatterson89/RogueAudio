<script lang="ts">
  import { auth } from "$lib/stores/auth";
  import { openUrl } from "@tauri-apps/plugin-opener";

  let copied = $state(false);

  async function openAuthPage() {
    if ($auth.pin?.authUrl) {
      try {
        await openUrl($auth.pin.authUrl);
      } catch {
        // Outside Tauri or opener failed — user can use plex.tv/link + code
        window.open($auth.pin.authUrl, "_blank");
      }
    }
  }

  async function openLinkPage() {
    const url = "https://plex.tv/link";
    try {
      await openUrl(url);
    } catch {
      window.open(url, "_blank");
    }
  }

  async function copyCode() {
    if (!$auth.pin?.code) return;
    try {
      await navigator.clipboard.writeText($auth.pin.code);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      /* ignore */
    }
  }
</script>

<div class="mx-auto max-w-lg space-y-6">
  <header class="space-y-2">
    <h1 class="text-2xl font-semibold tracking-tight">Connect to Plex</h1>
    <p class="text-sm leading-relaxed text-ra-muted">
      Sign in with a PIN from plex.tv to access your self-hosted audiobook libraries.
      RogueAudio never talks to Audible or any DRM service.
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
    <div class="space-y-4 rounded-2xl border border-ra-border bg-ra-surface p-5">
      {#if $auth.pin}
        <div class="text-center">
          <p class="text-xs uppercase tracking-wide text-ra-muted">Your PIN</p>
          <button
            type="button"
            class="mt-2 font-mono text-4xl font-bold tracking-[0.3em] text-ra-accent"
            onclick={copyCode}
            title="Copy PIN"
          >
            {$auth.pin.code}
          </button>
          <p class="mt-1 text-xs text-ra-muted">
            {copied ? "Copied!" : "Tap PIN to copy"}
          </p>
          <p class="mt-3 text-sm text-ra-muted">
            Enter this code at
            <button
              type="button"
              class="font-medium text-ra-accent underline-offset-2 hover:underline"
              onclick={openLinkPage}
            >
              plex.tv/link
            </button>
            {#if $auth.polling}
              <span class="ml-1 animate-pulse">· waiting for approval…</span>
            {/if}
          </p>
          <div class="mt-4 flex flex-col gap-2 sm:flex-row sm:justify-center">
            <button
              type="button"
              class="min-h-11 rounded-xl bg-ra-accent px-5 text-sm font-medium text-white hover:bg-ra-accent-hover"
              onclick={openAuthPage}
            >
              Open Plex authorization
            </button>
            <button
              type="button"
              class="min-h-11 rounded-xl border border-ra-border px-5 text-sm text-ra-muted hover:border-ra-accent hover:text-ra-text"
              onclick={() => auth.startPin()}
            >
              Get a new PIN
            </button>
          </div>
        </div>
      {:else}
        <button
          type="button"
          class="min-h-12 w-full rounded-xl bg-ra-accent text-sm font-semibold text-white hover:bg-ra-accent-hover disabled:opacity-50"
          disabled={$auth.loading}
          onclick={() => auth.startPin()}
        >
          {$auth.loading ? "Contacting plex.tv…" : "Sign in with Plex PIN"}
        </button>
      {/if}

      <div class="border-t border-ra-border pt-4">
        <p class="mb-2 text-xs text-ra-muted">
          Development only — explore the UI without a live Plex account (stub data).
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
