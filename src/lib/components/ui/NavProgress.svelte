<script lang="ts">
  /**
   * Global navigation feedback: top indeterminate bar + delayed soft overlay.
   * Driven by parent via `visible` (SvelteKit navigating and/or navBusy).
   */
  import { settings } from "$lib/stores/settings";

  let {
    visible = false,
    label = null,
  }: {
    visible?: boolean;
    label?: string | null;
  } = $props();

  /** Sticky visible for a short min time so fast routes still flash feedback */
  let shown = $state(false);
  let showOverlay = $state(false);
  let shownSince = 0;
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let overlayTimer: ReturnType<typeof setTimeout> | null = null;

  const MIN_MS = 160;
  const OVERLAY_AFTER_MS = 280;

  $effect(() => {
    if (visible) {
      if (hideTimer) {
        clearTimeout(hideTimer);
        hideTimer = null;
      }
      shown = true;
      shownSince = Date.now();
      if (!overlayTimer) {
        overlayTimer = setTimeout(() => {
          overlayTimer = null;
          if (visible) showOverlay = true;
        }, OVERLAY_AFTER_MS);
      }
      return;
    }

    // Hide after minimum display time
    const elapsed = Date.now() - shownSince;
    const wait = Math.max(0, MIN_MS - elapsed);
    if (overlayTimer) {
      clearTimeout(overlayTimer);
      overlayTimer = null;
    }
    showOverlay = false;
    hideTimer = setTimeout(() => {
      hideTimer = null;
      shown = false;
    }, wait);

    return () => {
      if (hideTimer) clearTimeout(hideTimer);
      if (overlayTimer) clearTimeout(overlayTimer);
    };
  });

  const reduceMotion = $derived($settings.reduceMotion);
</script>

{#if shown}
  <!-- Top indeterminate progress bar -->
  <div
    class="pointer-events-none fixed inset-x-0 top-0 z-[400]"
    role="progressbar"
    aria-valuetext={label ?? "Loading"}
    aria-busy="true"
    aria-label={label ?? "Loading page"}
  >
    <div
      class={reduceMotion ? "h-0.5 w-full bg-ra-accent/80" : "nav-progress-track h-0.5 w-full overflow-hidden bg-ra-accent/20"}
    >
      {#if !reduceMotion}
        <div class="nav-progress-bar h-full w-1/3 rounded-full bg-ra-accent"></div>
      {/if}
    </div>
  </div>

  <!-- Soft overlay only if nav is slow — keeps fast clicks from feeling heavy -->
  {#if showOverlay}
    <div
      class="pointer-events-none fixed inset-0 z-[390] flex items-start justify-center bg-ra-bg/20 pt-[min(30vh,12rem)] backdrop-blur-[1px]"
      aria-hidden="true"
    >
      <div
        class="flex items-center gap-2 rounded-full border border-ra-border bg-ra-surface/95 px-4 py-2.5 text-sm text-ra-muted shadow-xl ring-1 ring-white/5"
      >
        <span class="ra-spinner" aria-hidden="true"></span>
        <span>{label ?? "Loading…"}</span>
      </div>
    </div>
  {/if}

  <span class="sr-only" aria-live="polite">{label ?? "Loading"}</span>
{/if}

