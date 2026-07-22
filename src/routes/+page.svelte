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
    if ($isAuthenticated && $library.servers.length === 0 && !$library.loading) {
      void library.loadServers();
    }
  });

  async function selectBook(book: (typeof $library.books)[0]) {
    const serverId = $library.serverId;
    if (!serverId) return;
    // Resolve streams and start playback.
    await player.loadBook(serverId, book, true);
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
      <div class="space-y-1">
        <h1 class="text-2xl font-semibold tracking-tight">Library</h1>
        <div class="flex flex-wrap items-center gap-x-2 gap-y-2 text-sm text-ra-muted">
          {#if $library.servers.length > 1}
            <label class="inline-flex items-center gap-1.5">
              <span class="text-xs uppercase tracking-wide text-ra-muted/80">Server</span>
              <select
                class="min-h-9 rounded-md border border-ra-border bg-ra-surface px-2 py-1 text-sm text-ra-text"
                value={$library.serverId ?? ""}
                onchange={(e) => {
                  search = "";
                  library.selectServer((e.target as HTMLSelectElement).value);
                }}
              >
                {#each $library.servers as s}
                  <option value={s.id}>{s.name}</option>
                {/each}
              </select>
            </label>
          {:else}
            <span
              >{$library.servers.find((s) => s.id === $library.serverId)?.name ??
                "Plex"}</span
            >
          {/if}

          {#if $library.libraries.length > 1}
            <span aria-hidden="true">·</span>
            <label class="inline-flex items-center gap-1.5">
              <span class="text-xs uppercase tracking-wide text-ra-muted/80">Library</span>
              <select
                class="min-h-9 rounded-md border border-ra-border bg-ra-surface px-2 py-1 text-sm text-ra-text"
                value={$library.libraryKey ?? ""}
                onchange={(e) => {
                  search = "";
                  library.selectLibrary((e.target as HTMLSelectElement).value);
                }}
              >
                {#each $library.libraries as lib}
                  <option value={lib.key}>{lib.title}</option>
                {/each}
              </select>
            </label>
          {:else if $library.libraries.length === 1}
            <span aria-hidden="true">·</span>
            <span>{$library.libraries[0].title}</span>
          {/if}

          {#if !$library.loading && $library.books.length > 0}
            <span aria-hidden="true">·</span>
            <span class="text-xs">{$library.books.length} titles</span>
          {/if}
        </div>
      </div>
      <SearchBar
        bind:value={search}
        placeholder="Search titles or authors…"
        onsearch={(q) => library.search(q)}
      />
    </header>

    {#if $library.error}
      <div class="rounded-xl border border-ra-danger/40 bg-ra-danger/10 px-4 py-3 text-sm text-ra-danger">
        {$library.error}
      </div>
    {/if}

    {#if $library.loading && $library.books.length === 0}
      <p class="text-sm text-ra-muted">Loading library from your Plex server…</p>
    {:else if !$library.loading && $library.libraries.length === 0 && !$library.error}
      <div
        class="flex min-h-48 flex-col items-center justify-center rounded-2xl border border-dashed border-ra-border bg-ra-surface/50 p-8 text-center"
      >
        <p class="text-sm text-ra-muted">
          No music / audiobook libraries found on this server.
        </p>
        <p class="mt-2 max-w-md text-xs text-ra-muted/80">
          In Plex, audiobooks need to live in a Music-type library (often named
          “Audiobooks”).
        </p>
      </div>
    {:else}
      <BookGrid
        books={$library.books}
        selectedKey={$player.book?.ratingKey}
        onselect={selectBook}
      />
    {/if}
  </div>
{/if}
