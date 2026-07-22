<script lang="ts">
  import { page } from "$app/stores";
  import { auth } from "$lib/stores/auth";
  import { settings } from "$lib/stores/settings";
  import { player } from "$lib/stores/player";

  const links = [
    { href: "/", label: "Library", icon: "📚" },
    { href: "/auth", label: "Plex", icon: "🔗" },
    { href: "/settings", label: "Settings", icon: "⚙️" },
  ];
</script>

<aside
  class="flex w-[72px] shrink-0 flex-col border-r border-ra-border bg-ra-surface sm:w-52"
  aria-label="Main navigation"
>
  <div class="flex h-14 items-center gap-2 border-b border-ra-border px-3 sm:px-4">
    <span
      class="flex h-9 w-9 items-center justify-center rounded-xl bg-ra-accent-soft text-lg"
      aria-hidden="true"
    >
      🎧
    </span>
    <div class="hidden min-w-0 sm:block">
      <p class="truncate text-sm font-semibold tracking-tight">RogueAudio</p>
      <p class="truncate text-xs text-ra-muted">Plex audiobooks</p>
    </div>
  </div>

  <nav class="flex flex-1 flex-col gap-1 p-2">
    {#each links as link}
      {@const active = $page.url.pathname === link.href}
      <a
        href={link.href}
        class={active
          ? "flex min-h-11 items-center gap-3 rounded-xl bg-ra-accent-soft px-3 text-sm font-medium text-ra-text transition-colors"
          : "flex min-h-11 items-center gap-3 rounded-xl px-3 text-sm font-medium text-ra-muted transition-colors hover:bg-ra-surface-2 hover:text-ra-text"}
        aria-current={active ? "page" : undefined}
      >
        <span class="text-base" aria-hidden="true">{link.icon}</span>
        <span class="hidden sm:inline">{link.label}</span>
      </a>
    {/each}
  </nav>

  <div class="space-y-2 border-t border-ra-border p-2">
    <button
      type="button"
      class="flex min-h-11 w-full items-center gap-3 rounded-xl px-3 text-sm font-medium text-ra-muted transition-colors hover:bg-ra-surface-2 hover:text-ra-text"
      onclick={() => settings.togglePlayerBar()}
      title={$settings.playerBarVisible ? "Hide player" : "Show player"}
      aria-pressed={$settings.playerBarVisible}
      aria-label={$settings.playerBarVisible ? "Hide player" : "Show player"}
    >
      <span class="text-base" aria-hidden="true"
        >{$settings.playerBarVisible ? "▼" : "▲"}</span
      >
      <span class="hidden sm:inline">
        {$settings.playerBarVisible ? "Hide player" : "Show player"}
      </span>
      {#if !$settings.playerBarVisible && $player.playing}
        <span class="hidden text-[10px] text-ra-accent sm:inline">playing</span>
      {/if}
    </button>

    <div class="px-1 text-xs text-ra-muted">
      {#if $auth.status.authenticated}
        <p
          class="hidden truncate sm:block"
          title={$auth.status.username?.trim() || "Signed in"}
        >
          {$auth.status.username?.trim() || "Signed in"}
        </p>
        <p class="sm:hidden text-center" title="Signed in">●</p>
      {:else}
        <p class="hidden sm:block">Not signed in</p>
        <p class="sm:hidden text-center opacity-50">○</p>
      {/if}
    </div>
  </div>
</aside>
