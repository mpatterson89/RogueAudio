<script lang="ts">
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { library } from "$lib/stores/library";
  import { player } from "$lib/stores/player";
  import { settings } from "$lib/stores/settings";
  import { navBusy } from "$lib/stores/navBusy";
  import { userCollections } from "$lib/stores/userCollections";
  import { plexApi } from "$lib/api/plex";
  import BookGrid from "$lib/components/library/BookGrid.svelte";
  import AuthorGrid from "$lib/components/library/AuthorGrid.svelte";
  import ViewToggle from "$lib/components/library/ViewToggle.svelte";
  import SortSelect from "$lib/components/ui/SortSelect.svelte";
  import AddToCollectionModal from "$lib/components/collections/AddToCollectionModal.svelte";
  import { bookHref } from "$lib/nav";
  import { authorHref, groupByAuthor } from "$lib/authors";
  import {
    AUTHOR_SORT_OPTIONS,
    BOOK_SORT_OPTIONS,
    sortAuthors,
    sortBooks,
    type AuthorSort,
    type BookSort,
  } from "$lib/sort";
  import type {
    AudiobookSummary,
    AuthorSummary,
    LibraryViewMode,
  } from "$lib/types/models";

  const rawId = $derived(
    decodeURIComponent(($page.params as { id?: string }).id ?? ""),
  );
  const isPlex = $derived(rawId.startsWith("plex:"));
  const collectionId = $derived(isPlex ? rawId.slice(5) : rawId);

  let books = $state<AudiobookSummary[]>([]);
  let title = $state("Collection");
  let loading = $state(true);
  let error = $state<string | null>(null);
  let openingKey = $state<string | null>(null);
  let modalBook = $state<AudiobookSummary | null>(null);
  let modalOpen = $state(false);

  const viewMode = $derived($settings.libraryViewMode);
  const sortedBooks = $derived(sortBooks(books, $settings.bookSort));
  const authors = $derived(
    sortAuthors(groupByAuthor(books), $settings.authorSort),
  );

  onMount(async () => {
    await library.ensureLoaded();
    await load();
  });

  $effect(() => {
    void rawId;
    void $library.serverId;
    void $library.libraryKey;
    void load();
  });

  async function load() {
    const serverId = $library.serverId;
    const libraryKey = $library.libraryKey;
    if (!serverId || !libraryKey || !collectionId) return;

    loading = true;
    error = null;
    try {
      if (isPlex) {
        title = "Plex collection";
        books = await plexApi.collectionBooks(serverId, collectionId);
        // Best-effort title from list
        try {
          const cols = await plexApi.listCollections(serverId, libraryKey);
          const hit = cols.find((c) => c.ratingKey === collectionId);
          if (hit) title = hit.title;
        } catch {
          /* keep default */
        }
      } else {
        await userCollections.ensure(serverId, libraryKey);
        const col = $userCollections.items.find((c) => c.id === collectionId);
        if (!col) {
          // try fetch
          const { userCollectionsApi } = await import("$lib/api/userCollections");
          const c = await userCollectionsApi.get(serverId, libraryKey, collectionId);
          if (!c) throw new Error("Collection not found");
          title = c.name;
          books = c.ratingKeys
            .map((k) => $library.allBooks.find((b) => b.ratingKey === k))
            .filter((b): b is AudiobookSummary => !!b);
        } else {
          title = col.name;
          books = col.ratingKeys
            .map((k) => $library.allBooks.find((b) => b.ratingKey === k))
            .filter((b): b is AudiobookSummary => !!b);
        }
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      books = [];
    } finally {
      loading = false;
    }
  }

  async function selectBook(book: AudiobookSummary) {
    const serverId = $library.serverId;
    if (!serverId) return;
    openingKey = book.ratingKey;
    navBusy.start("Opening book…");
    try {
      await goto(bookHref(serverId, book.ratingKey));
      void player.loadBook(serverId, book, { autoplay: false });
    } finally {
      navBusy.stop();
      openingKey = null;
    }
  }

  async function selectAuthor(author: AuthorSummary) {
    navBusy.start("Opening author…");
    try {
      await goto(authorHref(author.key));
    } finally {
      navBusy.stop();
    }
  }

  function setView(mode: LibraryViewMode) {
    settings.patch({ libraryViewMode: mode });
  }

  async function deleteUserCollection() {
    if (isPlex) return;
    if (!confirm(`Delete collection “${title}”?`)) return;
    await userCollections.remove(collectionId);
    navBusy.start("Back…");
    try {
      await goto("/collections");
    } finally {
      navBusy.stop();
    }
  }
</script>

<div class="space-y-5 pb-4">
  <header class="space-y-3">
    <button
      type="button"
      class="text-sm text-ra-muted hover:text-ra-text"
      onclick={async () => {
        navBusy.start("Back…");
        try {
          await goto("/collections");
        } finally {
          navBusy.stop();
        }
      }}
    >
      ← Collections
    </button>
    <div class="flex flex-wrap items-start justify-between gap-3">
      <div>
        <p class="text-xs font-semibold uppercase tracking-wider text-ra-accent/90">
          {isPlex ? "Plex collection" : "Your collection"}
        </p>
        <h1 class="text-2xl font-semibold tracking-tight">{title}</h1>
        <p class="mt-1 text-sm text-ra-muted">
          {books.length} title{books.length === 1 ? "" : "s"}
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <ViewToggle value={viewMode} onchange={setView} />
        {#if viewMode === "authors"}
          <SortSelect
            value={$settings.authorSort}
            options={AUTHOR_SORT_OPTIONS}
            label="Sort"
            onchange={(v) => settings.patch({ authorSort: v as AuthorSort })}
          />
        {:else}
          <SortSelect
            value={$settings.bookSort}
            options={BOOK_SORT_OPTIONS}
            label="Sort"
            onchange={(v) => settings.patch({ bookSort: v as BookSort })}
          />
        {/if}
        {#if !isPlex}
          <button
            type="button"
            class="min-h-9 rounded-lg border border-ra-danger/40 px-3 text-sm text-ra-danger hover:bg-ra-danger/10"
            onclick={deleteUserCollection}
          >
            Delete
          </button>
        {/if}
      </div>
    </div>
  </header>

  {#if loading}
    <div class="flex min-h-48 flex-col items-center justify-center gap-3 py-12">
      <span class="ra-spinner ra-spinner-lg" aria-hidden="true"></span>
      <p class="text-sm text-ra-muted">Loading collection…</p>
    </div>
  {:else if error}
    <p class="text-sm text-ra-danger">{error}</p>
  {:else if viewMode === "authors"}
    <AuthorGrid authors={authors} onselect={selectAuthor} />
  {:else}
    <BookGrid
      books={sortedBooks}
      selectedKey={openingKey ?? $player.book?.ratingKey}
      onselect={selectBook}
      onaddToCollection={(b) => {
        modalBook = b;
        modalOpen = true;
      }}
    />
  {/if}
</div>

<AddToCollectionModal
  book={modalBook}
  open={modalOpen}
  onclose={() => {
    modalOpen = false;
    modalBook = null;
  }}
/>
