<script lang="ts">
  let {
    title = "Couldn't load from Plex",
    message = "Something went wrong.",
    loading = false,
    retryLabel = "Retry",
    onretry,
  }: {
    title?: string;
    message?: string;
    loading?: boolean;
    retryLabel?: string;
    onretry: () => void | Promise<void>;
  } = $props();

  let busy = $state(false);

  const spinning = $derived(loading || busy);

  async function handleRetry() {
    if (spinning) return;
    busy = true;
    try {
      await onretry();
    } finally {
      busy = false;
    }
  }
</script>

<div
  class="flex flex-col items-center justify-center gap-4 rounded-2xl border border-ra-danger/35 bg-ra-danger/10 px-6 py-10 text-center"
  role="alert"
>
  <div
    class="flex h-12 w-12 items-center justify-center rounded-full bg-ra-danger/15 text-xl text-ra-danger"
    aria-hidden="true"
  >
    ⚠
  </div>
  <div class="max-w-md space-y-1.5">
    <p class="text-base font-semibold text-ra-text">{title}</p>
    <p class="break-words text-sm leading-relaxed text-ra-muted">{message}</p>
  </div>
  <button
    type="button"
    class="inline-flex min-h-12 min-w-[9rem] items-center justify-center gap-2 rounded-xl bg-ra-accent px-5 text-sm font-semibold text-white transition hover:bg-ra-accent-hover disabled:cursor-not-allowed disabled:opacity-60"
    disabled={spinning}
    aria-busy={spinning}
    onclick={handleRetry}
  >
    {#if spinning}
      <span class="ra-spinner ra-spinner-on-accent" aria-hidden="true"></span>
      <span>Retrying…</span>
    {:else}
      <span>{retryLabel}</span>
    {/if}
  </button>
</div>
