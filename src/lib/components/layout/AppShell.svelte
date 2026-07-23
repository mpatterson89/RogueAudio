<script lang="ts">
  import type { Snippet } from "svelte";
  import { navigating } from "$app/stores";
  import PlayerBar from "$lib/components/player/PlayerBar.svelte";
  import SideNav from "$lib/components/layout/SideNav.svelte";
  import NavProgress from "$lib/components/ui/NavProgress.svelte";
  import { settings } from "$lib/stores/settings";
  import { navBusy } from "$lib/stores/navBusy";

  let { children }: { children: Snippet } = $props();

  const busy = $derived(!!$navigating || $navBusy.active);
  const busyLabel = $derived(
    $navBusy.label ?? ($navigating ? "Loading…" : null),
  );
</script>

<div class="flex h-dvh flex-col bg-ra-bg text-ra-text">
  <NavProgress visible={busy} label={busyLabel} />

  <!-- Content stays under the player chrome so overlays/menus are never covered -->
  <div class="relative z-0 flex min-h-0 min-w-0 flex-1">
    <SideNav />
    <main
      class="min-w-0 flex-1 overflow-y-auto px-4 py-4 sm:px-6 sm:py-5"
      aria-busy={busy || undefined}
    >
      {@render children()}
    </main>
  </div>

  <!-- Fully unmount when hidden so no bottom chrome uses vertical space -->
  {#if $settings.playerBarVisible}
    <PlayerBar />
  {/if}
</div>
