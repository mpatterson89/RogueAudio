<script lang="ts">
  import type { Snippet } from "svelte";
  import PlayerBar from "$lib/components/player/PlayerBar.svelte";
  import SideNav from "$lib/components/layout/SideNav.svelte";

  let { children }: { children: Snippet } = $props();
</script>

<div class="flex h-dvh flex-col bg-ra-bg text-ra-text">
  <div class="flex min-h-0 flex-1">
    <SideNav />
    <main class="min-w-0 flex-1 overflow-y-auto px-4 py-4 sm:px-6 sm:py-5">
      {@render children()}
    </main>
  </div>
  <PlayerBar />
</div>
