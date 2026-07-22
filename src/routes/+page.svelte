<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { auth, isAuthenticated } from "$lib/stores/auth";
  import { library } from "$lib/stores/library";
  import { player } from "$lib/stores/player";
  import SearchBar from "$lib/components/library/SearchBar.svelte";
  import BookGrid from "$lib/components/library/BookGrid.svelte";

  let search = $state("");

  onMount(async () => {
    await auth.refresh();
    if ($isAuthenticated) {
      await library.loadServers();
    }
  });

  $effect(() => {
    // When auth flips true after stub login, load library
    if ($isAuthenticated && $library.servers.length === 0 && !$library.loading) {
      void library.loadServers();
    }
  });

  async function selectBook(book: (typeof $library.books)[0]) {
    const serverId = $library.serverId;
    if (!serverId) return;
    await player.loadBook(serverId, book);
    await player.toggle();
  }
</script>

{#if !$isAuthenticated}
  <div
    class="flex min-h-[60vh] flex-col items-center justify-center gap-4 text-center"
  >
    <div
      class="flex h-16 w-16 items-center justify-center rounded-2xl bg-ra-accent-soft text-3xl"
    >
      🎧
    </div>
    <h1 class="text-2xl font-semibold">Welcome to RogueAudio</h1>
    <p class="max-w-md text-sm text-ra-muted">
      An open-source audiobook client for Linux and Steam Deck, built around your
      self-hosted Plex library.
    </p>
    <button
      type="button"
      class="min-h-12 rounded-xl bg-ra-accent px-6 text-sm font-semibold text-white hover:bg-ra-accent-hover"
      onclick={() => goto("/auth")}
    >
      Connect Plex
    </button>
  </div>
{:else}
  <div class="space-y-5 pb-4">
    <header class="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
      <div>
        <h1 class="text-2xl font-semibold tracking-tight">Library</h1>
        <p class="text-sm text-ra-muted">
          {$library.servers.find((s) => s.id === $library.serverId)?.name ?? "Plex"}
          {#if $library.libraries.length}
            ·
            <select
              class="ml-1 rounded-md border border-ra-border bg-ra-surface px-2 py-1 text-sm text-ra-text"
              value={$library.libraryKey ?? ""}
              onchange={(e) =>
                library.selectLibrary((e.target as HTMLSelectElement).value)}
            >
              {#each $library.libraries as lib}
                <option value={lib.key}>{lib.title}</option>
              {/each}
            </select>
          {/if}
        </p>
      </div>
      <SearchBar
        bind:value={search}
        onsearch={(q) => library.search(q)}
      />
    </header>

    {#if $library.error}
      <p class="text-sm text-ra-danger">{$library.error}</p>
    {/if}

    {#if $library.loading && $library.books.length === 0}
      <p class="text-sm text-ra-muted">Loading library…</p>
    {:else}
      <BookGrid
        books={$library.books}
        selectedKey={$player.book?.ratingKey}
        onselect={selectBook}
      />
    {/if}
  </div>
{/if}
