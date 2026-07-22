<script lang="ts">
  import type { Snippet } from "svelte";
  import PlayerBar from "$lib/components/player/PlayerBar.svelte";
  import SideNav from "$lib/components/layout/SideNav.svelte";

  let { children }: { children: Snippet } = $props();
</script>

<div class="flex h-dvh flex-col bg-ra-bg text-ra-text">
  <!-- Content stays under the player chrome so overlays/menus are never covered -->
  <div class="relative z-0 flex min-h-0 min-w-0 flex-1">
    <SideNav />
    <main class="min-w-0 flex-1 overflow-y-auto px-4 py-4 sm:px-6 sm:py-5">
      {@render children()}
    </main>
  </div>
  <PlayerBar />
</div>
