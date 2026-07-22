<script lang="ts">
  import { page } from "$app/stores";
  import BookView from "$lib/components/book/BookView.svelte";

  const serverId = $derived(decodeURIComponent($page.params.serverId ?? ""));
  const ratingKey = $derived(decodeURIComponent($page.params.ratingKey ?? ""));
</script>

{#if serverId && ratingKey}
  <BookView {serverId} {ratingKey} />
{:else}
  <p class="text-sm text-ra-muted">Missing book reference.</p>
{/if}
