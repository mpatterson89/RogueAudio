<script lang="ts">
  import type { Snippet } from "svelte";
  import PlayerBar from "$lib/components/player/PlayerBar.svelte";
  import SideNav from "$lib/components/layout/SideNav.svelte";
  import { settings } from "$lib/stores/settings";
  import { player } from "$lib/stores/player";

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

  {#if $settings.playerBarVisible}
    <PlayerBar />
  {:else}
    <!-- Collapsed: compact reveal control stays above page content -->
    <div
      class="relative z-[200] flex shrink-0 items-center justify-center border-t border-ra-border bg-ra-surface/95 px-3 py-2 backdrop-blur-md"
    >
      <button
        type="button"
        class="inline-flex min-h-11 items-center gap-2 rounded-full border border-ra-border bg-ra-surface-2 px-4 text-sm font-medium text-ra-text transition hover:border-ra-accent hover:bg-ra-accent-soft"
        onclick={() => settings.patch({ playerBarVisible: true })}
        title="Show player"
        aria-label="Show player"
      >
        <span class="text-base" aria-hidden="true">▲</span>
        <span>Show player</span>
        {#if $player.book}
          <span class="max-w-[12rem] truncate text-xs text-ra-muted">
            · {$player.book.title}
          </span>
          {#if $player.playing}
            <span class="eq" aria-hidden="true"><i></i><i></i><i></i></span>
          {/if}
        {/if}
      </button>
    </div>
  {/if}
</div>

<style>
  .eq {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 12px;
    margin-left: 0.15rem;
  }
  .eq i {
    display: block;
    width: 2.5px;
    border-radius: 1px;
    background: var(--color-ra-accent);
    animation: ra-eq 0.9s ease-in-out infinite;
  }
  .eq i:nth-child(1) {
    height: 40%;
  }
  .eq i:nth-child(2) {
    height: 80%;
    animation-delay: 0.15s;
  }
  .eq i:nth-child(3) {
    height: 55%;
    animation-delay: 0.3s;
  }
  @keyframes ra-eq {
    0%,
    100% {
      transform: scaleY(0.45);
    }
    50% {
      transform: scaleY(1);
    }
  }
</style>
