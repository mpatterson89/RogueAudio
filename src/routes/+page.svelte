<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { auth, isAuthenticated } from "$lib/stores/auth";
  import { library } from "$lib/stores/library";
  import { player } from "$lib/stores/player";
  import { settings } from "$lib/stores/settings";
  import { navBusy } from "$lib/stores/navBusy";
  import { userCollections } from "$lib/stores/userCollections";
  import { downloads } from "$lib/stores/downloads";
  import SearchBar from "$lib/components/library/SearchBar.svelte";
  import BookGrid from "$lib/components/library/BookGrid.svelte";
  import AuthorGrid from "$lib/components/library/AuthorGrid.svelte";
  import ViewToggle from "$lib/components/library/ViewToggle.svelte";
  import SortSelect from "$lib/components/ui/SortSelect.svelte";
  import AddToCollectionModal from "$lib/components/collections/AddToCollectionModal.svelte";
  import RetryPanel from "$lib/components/ui/RetryPanel.svelte";
  import { bookHref } from "$lib/nav";
  import { authorHref, groupByAuthor } from "$lib/authors";
  import {
    filterBooksByQuery,
    installedBooks,
    serverIdForBook,
  } from "$lib/installed";
  import {
    AUTHOR_SORT_OPTIONS,
    BOOK_SORT_OPTIONS,
    sortAuthors,
    sortBooks,
    type AuthorSort,
    type BookSort,
  } from "$lib/sort";
  import type { AudiobookSummary, AuthorSummary, LibraryViewMode } from "$lib/types/models";

  let search = $state("");
  let openingKey = $state<string | null>(null);
  let modalBook = $state<AudiobookSummary | null>(null);
  let modalOpen = $state(false);

  const viewMode = $derived($settings.libraryViewMode);
  const installedOnly = $derived($settings.libraryInstalledOnly);
  /** Show full library UI when signed in, or offline installed mode when signed out. */
  const showLibrary = $derived($isAuthenticated || installedOnly);

  /** Base list before search/sort: full library or downloads only. */
  const baseBooks = $derived.by(() => {
    if (installedOnly) {
      return installedBooks($library.allBooks, $downloads.items);
    }
    return $library.allBooks;
  });

  /** Active list after Installed filter + search. */
  const displayBooks = $derived(
    installedOnly
      ? filterBooksByQuery(baseBooks, search)
      : $library.query
        ? $library.books
        : baseBooks,
  );

  const sortedBooks = $derived(sortBooks(displayBooks, $settings.bookSort));
  const authors = $derived(
    sortAuthors(groupByAuthor(displayBooks), $settings.authorSort),
  );

  function clearFilter() {
    search = "";
    library.search("");
  }

  function setInstalledOnly(on: boolean) {
    settings.patch({ libraryInstalledOnly: on });
    clearFilter();
    if (on) void downloads.refresh();
  }

  onMount(async () => {
    clearFilter();
    await auth.refresh();
    await downloads.refresh();
    if ($isAuthenticated) {
      await library.ensureLoaded();
    }
  });

  $effect(() => {
    if (
      $isAuthenticated &&
      $library.servers.length === 0 &&
      $library.allBooks.length === 0 &&
      !$library.loading &&
      !$library.error
    ) {
      void library.ensureLoaded();
    }
  });

  $effect(() => {
    if (!$isAuthenticated) return;
    const sid = $library.serverId;
    const lk = $library.libraryKey;
    if (sid && lk) void userCollections.ensure(sid, lk);
  });

  async function selectBook(book: AudiobookSummary) {
    const serverId = serverIdForBook(book, $downloads.items, $library.serverId);
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

  function openAddToCollection(book: AudiobookSummary) {
    if (!$isAuthenticated) return;
    modalBook = book;
    modalOpen = true;
  }

  function setView(mode: LibraryViewMode) {
    clearFilter();
    settings.patch({ libraryViewMode: mode });
  }

  function onSearch(q: string) {
    search = q;
    if (installedOnly) {
      // Client-only filter via derived; keep store query clear
      library.search("");
    } else {
      library.search(q);
    }
  }

  async function retryLibrary() {
    await library.retry();
  }

  async function refreshLibrary() {
    clearFilter();
    if (installedOnly) {
      await downloads.refresh();
    } else {
      await library.refresh();
    }
  }
</script>

{#if !showLibrary}
  <!-- Signed out and Installed off -->
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
    <div class="flex flex-wrap items-center justify-center gap-3">
      <button
        type="button"
        class="min-h-12 rounded-xl bg-ra-accent px-6 text-sm font-semibold text-white hover:bg-ra-accent-hover"
        onclick={() => goto("/auth")}
      >
        Connect Plex
      </button>
      <button
        type="button"
        class="min-h-12 rounded-xl border border-ra-border px-6 text-sm font-medium text-ra-muted hover:border-ra-accent hover:text-ra-text"
        onclick={() => setInstalledOnly(true)}
      >
        Browse installed
      </button>
    </div>
  </div>
{:else}
  <div class="space-y-5 pb-4">
    <header class="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
      <!-- Left: title + controls (reorganized); search stays on the right -->
      <div class="min-w-0 flex-1 space-y-3">
        <!-- Row 1: title + refresh -->
        <div class="flex flex-wrap items-center gap-2">
          <h1 class="text-2xl font-semibold tracking-tight">Library</h1>
          {#if $isAuthenticated && !installedOnly}
            <button
              type="button"
              class="inline-flex min-h-9 min-w-9 items-center justify-center rounded-lg border border-ra-border bg-ra-surface px-2.5 text-sm text-ra-muted transition hover:border-ra-accent hover:text-ra-text disabled:opacity-50"
              onclick={refreshLibrary}
              disabled={$library.loading}
              title="Refresh library from Plex"
              aria-label="Refresh library from Plex"
            >
              {#if $library.loading && $library.allBooks.length > 0}
                <span class="ra-spinner" aria-hidden="true"></span>
              {:else}
                <span aria-hidden="true">↻</span>
              {/if}
            </button>
          {:else if installedOnly}
            <button
              type="button"
              class="inline-flex min-h-9 min-w-9 items-center justify-center rounded-lg border border-ra-border bg-ra-surface px-2.5 text-sm text-ra-muted transition hover:border-ra-accent hover:text-ra-text"
              onclick={refreshLibrary}
              title="Refresh installed list"
              aria-label="Refresh installed list"
            >
              <span aria-hidden="true">↻</span>
            </button>
          {/if}
        </div>

        <!-- Row 2: Installed | Books/Authors | Sort -->
        <div class="flex flex-wrap items-center gap-2">
          <button
            type="button"
            class={installedOnly
              ? "inline-flex min-h-9 items-center rounded-lg border border-ra-accent bg-ra-accent-soft px-3 text-sm font-medium text-ra-text"
              : "inline-flex min-h-9 items-center rounded-lg border border-ra-border bg-ra-surface px-3 text-sm font-medium text-ra-muted hover:text-ra-text"}
            aria-pressed={installedOnly}
            onclick={() => setInstalledOnly(!installedOnly)}
            title="Show only offline downloads"
          >
            Installed
          </button>
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
        </div>

        <!-- Row 3: context / counts -->
        <div class="flex flex-wrap items-center gap-x-2 gap-y-2 text-sm text-ra-muted">
          {#if $isAuthenticated && !installedOnly}
            {#if $library.servers.length > 1}
              <label class="inline-flex items-center gap-1.5">
                <span class="text-xs uppercase tracking-wide text-ra-muted/80">Server</span>
                <select
                  class="min-h-9 rounded-md border border-ra-border bg-ra-surface px-2 py-1 text-sm text-ra-text"
                  value={$library.serverId ?? ""}
                  onchange={(e) => {
                    clearFilter();
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
                    clearFilter();
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
          {:else if installedOnly}
            <span class="text-xs">Offline downloads</span>
            {#if !$isAuthenticated}
              <span aria-hidden="true">·</span>
              <button
                type="button"
                class="text-xs text-ra-accent hover:underline"
                onclick={() => goto("/auth")}
              >
                Sign in
              </button>
            {/if}
          {/if}

          {#if displayBooks.length > 0 || (installedOnly && !$downloads.loading)}
            <span aria-hidden="true">·</span>
            <span class="text-xs">
              {#if viewMode === "authors"}
                {authors.length} authors
              {:else}
                {displayBooks.length} title{displayBooks.length === 1 ? "" : "s"}
              {/if}
            </span>
          {/if}
        </div>
      </div>

      <!-- Search: placement unchanged -->
      <SearchBar
        bind:value={search}
        placeholder={viewMode === "authors"
          ? "Filter by author or title…"
          : "Search titles or authors…"}
        onsearch={onSearch}
      />
    </header>

    {#if $isAuthenticated && !installedOnly && $library.error}
      <RetryPanel
        title="Couldn't load your library"
        message={$library.error}
        loading={$library.loading}
        onretry={retryLibrary}
      />
    {:else if $isAuthenticated && !installedOnly && $library.loading && $library.allBooks.length === 0}
      <div class="flex min-h-48 flex-col items-center justify-center gap-3 py-12">
        <span class="ra-spinner ra-spinner-lg" aria-hidden="true"></span>
        <p class="text-sm text-ra-muted">Loading library from your Plex server…</p>
      </div>
    {:else if $isAuthenticated && !installedOnly && !$library.loading && $library.libraries.length === 0}
      <div
        class="flex min-h-48 flex-col items-center justify-center rounded-2xl border border-dashed border-ra-border bg-ra-surface/50 p-8 text-center"
      >
        <p class="text-sm text-ra-muted">
          No music / audiobook libraries found on this server.
        </p>
        <button
          type="button"
          class="mt-4 inline-flex min-h-11 items-center gap-2 rounded-xl border border-ra-border px-4 text-sm text-ra-muted hover:border-ra-accent hover:text-ra-text"
          onclick={refreshLibrary}
        >
          Refresh
        </button>
      </div>
    {:else if installedOnly && $downloads.loading && displayBooks.length === 0}
      <div class="flex min-h-48 flex-col items-center justify-center gap-3 py-12">
        <span class="ra-spinner ra-spinner-lg" aria-hidden="true"></span>
        <p class="text-sm text-ra-muted">Loading installed books…</p>
      </div>
    {:else if displayBooks.length === 0 && search.trim()}
      <div
        class="flex min-h-48 flex-col items-center justify-center rounded-2xl border border-dashed border-ra-border bg-ra-surface/50 p-8 text-center"
      >
        <p class="text-sm text-ra-muted">No titles match “{search}”.</p>
        <button
          type="button"
          class="mt-4 inline-flex min-h-11 items-center gap-2 rounded-xl border border-ra-border px-4 text-sm text-ra-muted hover:border-ra-accent hover:text-ra-text"
          onclick={clearFilter}
        >
          Clear search
        </button>
      </div>
    {:else if displayBooks.length === 0 && installedOnly}
      <div
        class="flex min-h-48 flex-col items-center justify-center rounded-2xl border border-dashed border-ra-border bg-ra-surface/50 p-8 text-center"
      >
        <p class="text-sm text-ra-muted">No offline audiobooks yet.</p>
        <p class="mt-2 max-w-md text-xs text-ra-muted/80">
          Sign in, open a book, and tap Download. Then use Installed here anytime — even offline.
        </p>
        {#if !$isAuthenticated}
          <button
            type="button"
            class="mt-4 min-h-11 rounded-xl bg-ra-accent px-5 text-sm font-semibold text-white"
            onclick={() => goto("/auth")}
          >
            Connect Plex
          </button>
        {:else}
          <button
            type="button"
            class="mt-4 min-h-11 rounded-xl border border-ra-border px-4 text-sm text-ra-muted hover:text-ra-text"
            onclick={() => setInstalledOnly(false)}
          >
            Show full library
          </button>
        {/if}
      </div>
    {:else if viewMode === "authors"}
      <AuthorGrid authors={authors} onselect={selectAuthor} />
    {:else}
      <BookGrid
        books={sortedBooks}
        selectedKey={openingKey ?? $player.book?.ratingKey}
        showMenu={$isAuthenticated}
        onselect={selectBook}
        onaddToCollection={$isAuthenticated ? openAddToCollection : undefined}
      />
    {/if}
  </div>

  {#if $isAuthenticated}
    <AddToCollectionModal
      book={modalBook}
      open={modalOpen}
      onclose={() => {
        modalOpen = false;
        modalBook = null;
      }}
    />
  {/if}
{/if}
