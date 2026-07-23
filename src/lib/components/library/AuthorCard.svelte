<script lang="ts">
  import type { AuthorSummary } from "$lib/types/models";

  let {
    author,
    onclick,
  }: {
    author: AuthorSummary;
    onclick?: () => void;
  } = $props();
</script>

<button
  type="button"
  class="group flex w-full flex-col overflow-hidden rounded-2xl border border-ra-border bg-ra-surface text-left transition hover:border-ra-accent/50 hover:bg-ra-surface-2 focus-visible:border-ra-accent"
  {onclick}
>
  <div
    class="relative aspect-square w-full overflow-hidden bg-gradient-to-br from-ra-surface-2 to-ra-bg"
  >
    {#if author.thumbs.length >= 4}
      <div class="grid h-full w-full grid-cols-2 grid-rows-2">
        {#each author.thumbs.slice(0, 4) as src}
          <img {src} alt="" class="h-full w-full object-cover" />
        {/each}
      </div>
    {:else if author.thumbs.length > 0}
      <img
        src={author.thumbs[0]}
        alt=""
        class="h-full w-full object-cover"
      />
    {:else}
      <div class="flex h-full items-center justify-center text-4xl opacity-40" aria-hidden="true">
        ✍️
      </div>
    {/if}
  </div>
  <div class="flex flex-1 flex-col gap-1 p-3">
    <h3 class="line-clamp-2 text-sm font-semibold leading-snug">{author.name}</h3>
    <p class="text-xs text-ra-muted">
      {author.bookCount} title{author.bookCount === 1 ? "" : "s"}
    </p>
  </div>
</button>
