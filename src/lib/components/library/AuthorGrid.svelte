<script lang="ts">
  import type { AuthorSummary } from "$lib/types/models";
  import AuthorCard from "./AuthorCard.svelte";

  let {
    authors,
    onselect,
  }: {
    authors: AuthorSummary[];
    onselect?: (author: AuthorSummary) => void;
  } = $props();
</script>

{#if authors.length === 0}
  <div
    class="flex min-h-48 flex-col items-center justify-center rounded-2xl border border-dashed border-ra-border bg-ra-surface/50 p-8 text-center"
  >
    <p class="text-ra-muted">No authors found</p>
  </div>
{:else}
  <div
    class="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
  >
    {#each authors as author (author.key)}
      <AuthorCard {author} onclick={() => onselect?.(author)} />
    {/each}
  </div>
{/if}
