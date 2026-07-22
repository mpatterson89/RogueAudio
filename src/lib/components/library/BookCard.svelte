<script lang="ts">
  import type { AudiobookSummary } from "$lib/types/models";
  import { formatTime } from "$lib/stores/player";

  let {
    book,
    selected = false,
    onclick,
  }: {
    book: AudiobookSummary;
    selected?: boolean;
    onclick?: () => void;
  } = $props();

  const durationLabel = $derived(
    book.durationMs ? formatTime(book.durationMs / 1000) : null,
  );

  const cardClass = $derived(
    selected
      ? "group flex w-full flex-col overflow-hidden rounded-2xl border border-ra-accent bg-ra-surface text-left ring-1 ring-ra-accent/40 transition hover:border-ra-accent/50 hover:bg-ra-surface-2 focus-visible:border-ra-accent"
      : "group flex w-full flex-col overflow-hidden rounded-2xl border border-ra-border bg-ra-surface text-left transition hover:border-ra-accent/50 hover:bg-ra-surface-2 focus-visible:border-ra-accent",
  );
</script>

<button
  type="button"
  class={cardClass}
  {onclick}
>
  <div
    class="relative aspect-square w-full bg-gradient-to-br from-ra-surface-2 to-ra-bg flex items-center justify-center"
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
    <p class="line-clamp-1 text-xs text-ra-muted">{book.author ?? "Unknown author"}</p>
    {#if durationLabel}
      <p class="mt-auto pt-1 text-[11px] text-ra-muted/80">{durationLabel}</p>
    {/if}
  </div>
</button>
