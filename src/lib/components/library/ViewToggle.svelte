<script lang="ts">
  import type { LibraryViewMode } from "$lib/types/models";

  let {
    value = "books",
    onchange,
  }: {
    value?: LibraryViewMode;
    onchange?: (v: LibraryViewMode) => void;
  } = $props();
</script>

<div
  class="inline-flex rounded-lg border border-ra-border bg-ra-surface p-0.5"
  role="group"
  aria-label="View mode"
>
  <button
    type="button"
    class={value === "books"
      ? "min-h-9 rounded-md bg-ra-accent-soft px-3 text-sm font-medium text-ra-text"
      : "min-h-9 rounded-md px-3 text-sm font-medium text-ra-muted hover:text-ra-text"}
    aria-pressed={value === "books"}
    onclick={() => onchange?.("books")}
  >
    Books
  </button>
  <button
    type="button"
    class={value === "authors"
      ? "min-h-9 rounded-md bg-ra-accent-soft px-3 text-sm font-medium text-ra-text"
      : "min-h-9 rounded-md px-3 text-sm font-medium text-ra-muted hover:text-ra-text"}
    aria-pressed={value === "authors"}
    onclick={() => onchange?.("authors")}
  >
    Authors
  </button>
</div>
