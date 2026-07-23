<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { auth, isAuthenticated } from "$lib/stores/auth";
  import { library } from "$lib/stores/library";
  import { settings } from "$lib/stores/settings";
  import { userCollections } from "$lib/stores/userCollections";
  import { navBusy } from "$lib/stores/navBusy";
  import { plexApi } from "$lib/api/plex";
  import { collectionHref } from "$lib/nav";
  import SortSelect from "$lib/components/ui/SortSelect.svelte";
  import {
    COLLECTION_SORT_OPTIONS,
    sortPlexCollections,
    sortUserCollections,
    type CollectionSort,
  } from "$lib/sort";
  import type { PlexCollection, UserCollection } from "$lib/types/models";

  let plexCols = $state<PlexCollection[]>([]);
  let plexLoading = $state(false);
  let plexError = $state<string | null>(null);
  let newName = $state("");
  let creating = $state(false);

  const sortedUser = $derived(
    sortUserCollections($userCollections.items, $settings.collectionSort),
  );
  const sortedPlex = $derived(
    sortPlexCollections(plexCols, $settings.collectionSort),
  );

  onMount(async () => {
    await auth.refresh();
    if ($isAuthenticated) {
      await library.ensureLoaded();
    }
  });

  $effect(() => {
    const sid = $library.serverId;
    const lk = $library.libraryKey;
    if (sid && lk) {
      void userCollections.ensure(sid, lk);
      void loadPlex(sid, lk);
    }
  });

  async function loadPlex(serverId: string, libraryKey: string) {
    plexLoading = true;
    plexError = null;
    try {
      plexCols = await plexApi.listCollections(serverId, libraryKey);
    } catch (e) {
      plexCols = [];
      plexError = e instanceof Error ? e.message : String(e);
    } finally {
      plexLoading = false;
    }
  }

  async function openUser(c: UserCollection) {
    navBusy.start("Opening collection…");
    try {
      await goto(collectionHref(c.id, "user"));
    } finally {
      navBusy.stop();
    }
  }

  async function openPlex(c: PlexCollection) {
    navBusy.start("Opening collection…");
    try {
      await goto(collectionHref(c.ratingKey, "plex"));
    } finally {
      navBusy.stop();
    }
  }

  async function createCollection() {
    if (!newName.trim()) return;
    creating = true;
    try {
      const col = await userCollections.create(newName.trim());
      newName = "";
      await openUser(col);
    } catch {
      /* store error */
    } finally {
      creating = false;
    }
  }

  function mosaicThumbs(ratingKeys: string[]): string[] {
    const books = $library.allBooks;
    const thumbs: string[] = [];
    for (const k of ratingKeys) {
      const b = books.find((x) => x.ratingKey === k);
      if (b?.thumb) thumbs.push(b.thumb);
      if (thumbs.length >= 4) break;
    }
    return thumbs;
  }
</script>

{#if !$isAuthenticated}
  <div class="flex min-h-[50vh] flex-col items-center justify-center gap-3 text-center">
    <p class="text-sm text-ra-muted">Sign in to manage collections.</p>
    <button
      type="button"
      class="min-h-11 rounded-xl bg-ra-accent px-5 text-sm font-semibold text-white"
      onclick={() => goto("/auth")}
    >
      Connect Plex
    </button>
  </div>
{:else}
  <div class="space-y-8 pb-4">
    <header class="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
      <div class="space-y-1">
        <h1 class="text-2xl font-semibold tracking-tight">Collections</h1>
        <p class="text-sm text-ra-muted">
          Your lists for this library
          {#if $library.libraries.find((l) => l.key === $library.libraryKey)}
            · {$library.libraries.find((l) => l.key === $library.libraryKey)?.title}
          {/if}
        </p>
      </div>
      <SortSelect
        value={$settings.collectionSort}
        options={COLLECTION_SORT_OPTIONS}
        label="Sort"
        onchange={(v) => settings.patch({ collectionSort: v as CollectionSort })}
      />
    </header>

    <!-- Create -->
    <section class="rounded-2xl border border-ra-border bg-ra-surface p-4">
      <h2 class="mb-3 text-xs font-semibold uppercase tracking-wide text-ra-muted">
        Create collection
      </h2>
      <div class="flex flex-wrap gap-2">
        <input
          type="text"
          class="min-h-11 min-w-[12rem] flex-1 rounded-xl border border-ra-border bg-ra-surface-2 px-3 text-sm focus:border-ra-accent focus:outline-none"
          placeholder="e.g. Military SF"
          bind:value={newName}
          onkeydown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void createCollection();
            }
          }}
        />
        <button
          type="button"
          class="min-h-11 rounded-xl bg-ra-accent px-4 text-sm font-semibold text-white hover:bg-ra-accent-hover disabled:opacity-50"
          disabled={creating || !newName.trim()}
          onclick={createCollection}
        >
          Create
        </button>
      </div>
    </section>

    <!-- Yours -->
    <section class="space-y-3">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-ra-muted">
        Yours
      </h2>
      {#if $userCollections.loading && $userCollections.items.length === 0}
        <p class="text-sm text-ra-muted">Loading…</p>
      {:else if sortedUser.length === 0}
        <p class="text-sm text-ra-muted">
          No personal collections yet. Create one or use ⋮ on a book → Add to collection.
        </p>
      {:else}
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {#each sortedUser as c (c.id)}
            {@const thumbs = mosaicThumbs(c.ratingKeys)}
            <button
              type="button"
              class="flex gap-3 rounded-2xl border border-ra-border bg-ra-surface p-3 text-left transition hover:border-ra-accent/50"
              onclick={() => openUser(c)}
            >
              <div
                class="h-16 w-16 shrink-0 overflow-hidden rounded-xl bg-ra-surface-2"
              >
                {#if thumbs.length >= 4}
                  <div class="grid h-full w-full grid-cols-2 grid-rows-2">
                    {#each thumbs as src}
                      <img {src} alt="" class="h-full w-full object-cover" />
                    {/each}
                  </div>
                {:else if thumbs[0]}
                  <img src={thumbs[0]} alt="" class="h-full w-full object-cover" />
                {:else}
                  <div class="flex h-full items-center justify-center text-xl opacity-40">
                    🗂️
                  </div>
                {/if}
              </div>
              <div class="min-w-0 flex-1">
                <p class="truncate font-medium">{c.name}</p>
                <p class="text-xs text-ra-muted">
                  {c.ratingKeys.length} title{c.ratingKeys.length === 1 ? "" : "s"}
                  · Yours
                </p>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </section>

    <!-- Plex -->
    <section class="space-y-3">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-ra-muted">
        From Plex
      </h2>
      {#if plexLoading}
        <div class="flex items-center gap-2 text-sm text-ra-muted">
          <span class="ra-spinner" aria-hidden="true"></span>
          Loading Plex collections…
        </div>
      {:else if plexError}
        <p class="text-sm text-ra-muted">
          Couldn’t load Plex collections ({plexError}). Your personal collections still work.
        </p>
      {:else if sortedPlex.length === 0}
        <p class="text-sm text-ra-muted">
          No Plex collections found for this library section.
        </p>
      {:else}
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {#each sortedPlex as c (c.ratingKey)}
            <button
              type="button"
              class="flex gap-3 rounded-2xl border border-ra-border bg-ra-surface p-3 text-left transition hover:border-ra-accent/50"
              onclick={() => openPlex(c)}
            >
              <div
                class="h-16 w-16 shrink-0 overflow-hidden rounded-xl bg-ra-surface-2"
              >
                {#if c.thumb}
                  <img src={c.thumb} alt="" class="h-full w-full object-cover" />
                {:else}
                  <div class="flex h-full items-center justify-center text-xl opacity-40">
                    📚
                  </div>
                {/if}
              </div>
              <div class="min-w-0 flex-1">
                <p class="truncate font-medium">{c.title}</p>
                <p class="text-xs text-ra-muted">
                  {#if c.childCount != null}
                    {c.childCount} items
                  {:else}
                    Collection
                  {/if}
                  · Plex
                </p>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </section>
  </div>
{/if}
