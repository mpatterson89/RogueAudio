<script lang="ts">
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { library } from "$lib/stores/library";
  import { player } from "$lib/stores/player";
  import { settings } from "$lib/stores/settings";
  import { navBusy } from "$lib/stores/navBusy";
  import { userCollections } from "$lib/stores/userCollections";
  import BookGrid from "$lib/components/library/BookGrid.svelte";
  import SortSelect from "$lib/components/ui/SortSelect.svelte";
  import AddToCollectionModal from "$lib/components/collections/AddToCollectionModal.svelte";
  import { bookHref } from "$lib/nav";
  import { authorKey, booksForAuthor, groupByAuthor } from "$lib/authors";
  import { BOOK_SORT_OPTIONS, sortBooks, type BookSort } from "$lib/sort";
  import type { AudiobookSummary } from "$lib/types/models";

  const key = $derived(
    decodeURIComponent(($page.params as { key?: string }).key ?? ""),
  );
  const books = $derived(
    sortBooks(booksForAuthor($library.allBooks, key), $settings.bookSort),
  );
  const authorMeta = $derived(
    groupByAuthor($library.allBooks).find((a) => a.key === authorKey(key)),
  );

  let openingKey = $state<string | null>(null);
  let modalBook = $state<AudiobookSummary | null>(null);
  let modalOpen = $state(false);

  onMount(async () => {
    await library.ensureLoaded();
    if ($library.serverId && $library.libraryKey) {
      await userCollections.ensure($library.serverId, $library.libraryKey);
    }
  });

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

  function openAddToCollection(book: AudiobookSummary) {
    modalBook = book;
    modalOpen = true;
  }
</script>

<div class="space-y-5 pb-4">
  <header class="space-y-3">
    <button
      type="button"
      class="text-sm text-ra-muted hover:text-ra-text"
      onclick={async () => {
        navBusy.start("Back to library…");
        try {
          await goto("/");
        } finally {
          navBusy.stop();
        }
      }}
    >
      ← Library
    </button>
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <p class="text-xs font-semibold uppercase tracking-wider text-ra-accent/90">
          Author
        </p>
        <h1 class="text-2xl font-semibold tracking-tight">
          {authorMeta?.name ?? (key === "__unknown__" ? "Unknown author" : key)}
        </h1>
        <p class="mt-1 text-sm text-ra-muted">
          {books.length} title{books.length === 1 ? "" : "s"}
          <span class="text-ra-muted/60"> · includes collaborations</span>
        </p>
      </div>
      <SortSelect
        value={$settings.bookSort}
        options={BOOK_SORT_OPTIONS}
        label="Sort"
        onchange={(v) => settings.patch({ bookSort: v as BookSort })}
      />
    </div>
  </header>

  {#if $library.loading && $library.allBooks.length === 0}
    <div class="flex min-h-48 flex-col items-center justify-center gap-3 py-12">
      <span class="ra-spinner ra-spinner-lg" aria-hidden="true"></span>
      <p class="text-sm text-ra-muted">Loading library…</p>
    </div>
  {:else}
    <BookGrid
      {books}
      selectedKey={openingKey ?? $player.book?.ratingKey}
      onselect={selectBook}
      onaddToCollection={openAddToCollection}
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
