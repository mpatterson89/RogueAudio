<script lang="ts">
  import type { AudiobookSummary } from "$lib/types/models";
  import BookCard from "./BookCard.svelte";

  let {
    books,
    selectedKey = null,
    onselect,
  }: {
    books: AudiobookSummary[];
    selectedKey?: string | null;
    onselect?: (book: AudiobookSummary) => void;
  } = $props();
</script>

{#if books.length === 0}
  <div
    class="flex min-h-48 flex-col items-center justify-center rounded-2xl border border-dashed border-ra-border bg-ra-surface/50 p-8 text-center"
  >
    <p class="text-ra-muted">No audiobooks found</p>
  </div>
{:else}
  <div
    class="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
  >
    {#each books as book (book.ratingKey)}
      <BookCard
        {book}
        selected={selectedKey === book.ratingKey}
        onclick={() => onselect?.(book)}
      />
    {/each}
  </div>
{/if}
