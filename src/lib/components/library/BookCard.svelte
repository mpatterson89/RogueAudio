<script lang="ts">
  import type { AudiobookSummary } from "$lib/types/models";
  import { formatTime } from "$lib/stores/player";
  import BookMenu from "./BookMenu.svelte";

  let {
    book,
    selected = false,
    showMenu = true,
    onclick,
    onaddToCollection,
  }: {
    book: AudiobookSummary;
    selected?: boolean;
    showMenu?: boolean;
    onclick?: () => void;
    onaddToCollection?: (book: AudiobookSummary) => void;
  } = $props();

  const durationLabel = $derived(
    book.durationMs ? formatTime(book.durationMs / 1000) : null,
  );

  const authorLabel = $derived(
    book.author ??
      (book.authors && book.authors.length ? book.authors.join(" & ") : null) ??
      "Unknown author",
  );

  // overflow-visible so the ⋮ dropdown isn't clipped; image area clips cover art.
  // Only animate border/background (not z-index) so scrubbing the grid stays snappy.
  const cardClass = $derived(
    selected
      ? "book-card group relative z-0 flex w-full flex-col overflow-visible rounded-2xl border border-ra-accent bg-ra-surface text-left ring-1 ring-ra-accent/40 transition-[border-color,background-color] duration-75 ease-out hover:border-ra-accent/50 hover:bg-ra-surface-2 focus-within:border-ra-accent"
      : "book-card group relative z-0 flex w-full flex-col overflow-visible rounded-2xl border border-ra-border bg-ra-surface text-left transition-[border-color,background-color] duration-75 ease-out hover:border-ra-accent/50 hover:bg-ra-surface-2 focus-within:border-ra-accent",
  );
</script>

<div class={cardClass}>
  <button type="button" class="flex w-full flex-col text-left" {onclick}>
    <div
      class="relative aspect-square w-full overflow-hidden rounded-t-2xl bg-gradient-to-br from-ra-surface-2 to-ra-bg flex items-center justify-center"
    >
      {#if book.thumb}
        <img src={book.thumb} alt="" class="h-full w-full object-cover" />
      {:else}
        <span class="text-4xl opacity-40" aria-hidden="true">📖</span>
      {/if}
      {#if book.year}
        <span
          class="absolute bottom-2 right-2 rounded-md bg-black/60 px-1.5 py-0.5 text-[10px] text-ra-text"
        >
          {book.year}
        </span>
      {/if}
    </div>
    <div class="flex flex-1 flex-col gap-1 p-3">
      <h3 class="line-clamp-2 text-sm font-semibold leading-snug">{book.title}</h3>
      <p class="line-clamp-1 text-xs text-ra-muted">{authorLabel}</p>
      {#if durationLabel}
        <p class="mt-auto pt-1 text-[11px] text-ra-muted/80">{durationLabel}</p>
      {/if}
    </div>
  </button>

  {#if showMenu}
    <!-- Only visible when this card is hovered / focused / menu open -->
    <div
      class="book-card-menu absolute right-2 top-2 z-20 opacity-0 transition-opacity duration-75 ease-out group-hover:opacity-100 group-focus-within:opacity-100 has-[.book-menu-btn[aria-expanded=true]]:opacity-100"
    >
      <BookMenu {book} {onaddToCollection} />
    </div>
  {/if}
</div>

