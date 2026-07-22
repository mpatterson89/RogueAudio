<script lang="ts">
  import { auth } from "$lib/stores/auth";
  import { openUrl } from "@tauri-apps/plugin-opener";

  async function openInBrowser(url: string) {
    try {
      await openUrl(url);
    } catch {
      window.open(url, "_blank");
    }
  }

  async function openAuthPage() {
    if ($auth.pin?.authUrl) {
      await openInBrowser($auth.pin.authUrl);
    }
  }

  async function startAndOpen() {
    await auth.startPin();
    // Auto-open the OAuth page — strong PINs are not typed at plex.tv/link
    const url = $auth.pin?.authUrl;
    if (url) await openInBrowser(url);
  }
</script>

<div class="mx-auto max-w-lg space-y-6">
  <header class="space-y-2">
    <h1 class="text-2xl font-semibold tracking-tight">Connect to Plex</h1>
    <p class="text-sm leading-relaxed text-ra-muted">
      Sign in through plex.tv in your browser. RogueAudio only talks to your account and
      self-hosted servers — never Audible or DRM stores.
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
        <div class="space-y-3 text-center">
          <p class="text-sm text-ra-muted">
            A browser window should open for Plex login.
            {#if $auth.polling}
              <span class="animate-pulse">Waiting for approval…</span>
            {/if}
          </p>
          <p class="text-xs text-ra-muted/80">
            If nothing opened, use the button below. Stay signed in on plex.tv, then
            approve <strong class="text-ra-text">RogueAudio</strong>.
          </p>
          <div class="flex flex-col gap-2 sm:flex-row sm:justify-center">
            <button
              type="button"
              class="min-h-11 rounded-xl bg-ra-accent px-5 text-sm font-medium text-white hover:bg-ra-accent-hover"
              onclick={openAuthPage}
            >
              Open Plex login again
            </button>
            <button
              type="button"
              class="min-h-11 rounded-xl border border-ra-border px-5 text-sm text-ra-muted hover:border-ra-accent hover:text-ra-text"
              onclick={startAndOpen}
            >
              Start over
            </button>
          </div>
          <details class="pt-2 text-left text-xs text-ra-muted">
            <summary class="cursor-pointer select-none hover:text-ra-text">
              Technical details
            </summary>
            <p class="mt-2 break-all font-mono text-[11px] opacity-80">
              PIN id: {$auth.pin.id}
            </p>
            <p class="mt-1 break-all font-mono text-[11px] opacity-60">
              {$auth.pin.authUrl}
            </p>
          </details>
        </div>
      {:else}
        <button
          type="button"
          class="min-h-12 w-full rounded-xl bg-ra-accent text-sm font-semibold text-white hover:bg-ra-accent-hover disabled:opacity-50"
          disabled={$auth.loading}
          onclick={startAndOpen}
        >
          {$auth.loading ? "Contacting plex.tv…" : "Sign in with Plex"}
        </button>
        <p class="text-center text-xs text-ra-muted">
          Opens the official Plex authorization page in your browser.
        </p>
      {/if}

      <div class="border-t border-ra-border pt-4">
        <p class="mb-2 text-xs text-ra-muted">
          Development only — explore the UI with sample data (no plex.tv).
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
