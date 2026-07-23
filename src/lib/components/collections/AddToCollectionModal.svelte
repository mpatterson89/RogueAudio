<script lang="ts">
  import { userCollections } from "$lib/stores/userCollections";
  import type { AudiobookSummary } from "$lib/types/models";

  let {
    book,
    open = false,
    onclose,
  }: {
    book: AudiobookSummary | null;
    open?: boolean;
    onclose?: () => void;
  } = $props();

  let newName = $state("");
  let selected = $state<Record<string, boolean>>({});
  let busy = $state(false);
  let error = $state<string | null>(null);
  let doneMsg = $state<string | null>(null);

  $effect(() => {
    if (open) {
      newName = "";
      selected = {};
      error = null;
      doneMsg = null;
      // Mark collections that already contain this book
      if (book) {
        const init: Record<string, boolean> = {};
        for (const c of $userCollections.items) {
          if (c.ratingKeys.includes(book.ratingKey)) init[c.id] = true;
        }
        selected = init;
      }
    }
  });

  function close() {
    onclose?.();
  }

  async function createAndAdd() {
    if (!book || !newName.trim()) return;
    busy = true;
    error = null;
    try {
      const col = await userCollections.create(newName.trim());
      await userCollections.addBooks(col.id, [book.ratingKey]);
      doneMsg = `Added to “${col.name}”`;
      newName = "";
      selected = { ...selected, [col.id]: true };
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function saveSelections() {
    if (!book) return;
    busy = true;
    error = null;
    try {
      for (const c of $userCollections.items) {
        const want = !!selected[c.id];
        const has = c.ratingKeys.includes(book.ratingKey);
        if (want && !has) {
          await userCollections.addBooks(c.id, [book.ratingKey]);
        } else if (!want && has) {
          await userCollections.removeBooks(c.id, [book.ratingKey]);
        }
      }
      doneMsg = "Collections updated";
      setTimeout(() => close(), 500);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if open && book}
  <div class="fixed inset-0 z-[400] flex items-end justify-center sm:items-center">
    <button
      type="button"
      class="absolute inset-0 bg-black/60"
      aria-label="Close"
      onclick={close}
    ></button>
    <div
      class="relative z-10 flex max-h-[min(85vh,36rem)] w-full max-w-md flex-col rounded-t-2xl border border-ra-border bg-ra-surface shadow-2xl sm:rounded-2xl"
      role="dialog"
      aria-modal="true"
      aria-labelledby="add-col-title"
    >
      <div class="border-b border-ra-border px-5 py-4">
        <h2 id="add-col-title" class="text-lg font-semibold">Add to collection</h2>
        <p class="mt-1 line-clamp-2 text-sm text-ra-muted">{book.title}</p>
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto px-5 py-3">
        {#if $userCollections.items.length === 0}
          <p class="py-4 text-sm text-ra-muted">
            No collections yet. Create one below.
          </p>
        {:else}
          <ul class="space-y-1">
            {#each $userCollections.items as c (c.id)}
              <li>
                <label
                  class="flex min-h-11 cursor-pointer items-center gap-3 rounded-xl px-2 hover:bg-ra-surface-2"
                >
                  <input
                    type="checkbox"
                    class="h-4 w-4 accent-[var(--color-ra-accent)]"
                    checked={!!selected[c.id]}
                    onchange={(e) => {
                      selected = {
                        ...selected,
                        [c.id]: (e.target as HTMLInputElement).checked,
                      };
                    }}
                  />
                  <span class="min-w-0 flex-1 truncate text-sm">{c.name}</span>
                  <span class="text-xs text-ra-muted tabular-nums"
                    >{c.ratingKeys.length}</span
                  >
                </label>
              </li>
            {/each}
          </ul>
        {/if}

        <div class="mt-4 border-t border-ra-border pt-4">
          <p class="mb-2 text-xs font-medium uppercase tracking-wide text-ra-muted">
            New collection
          </p>
          <div class="flex gap-2">
            <input
              type="text"
              class="min-h-11 min-w-0 flex-1 rounded-xl border border-ra-border bg-ra-surface-2 px-3 text-sm focus:border-ra-accent focus:outline-none"
              placeholder="Name"
              bind:value={newName}
              onkeydown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void createAndAdd();
                }
              }}
            />
            <button
              type="button"
              class="min-h-11 shrink-0 rounded-xl bg-ra-accent px-3 text-sm font-semibold text-white hover:bg-ra-accent-hover disabled:opacity-50"
              disabled={busy || !newName.trim()}
              onclick={createAndAdd}
            >
              Create
            </button>
          </div>
        </div>

        {#if error}
          <p class="mt-3 text-sm text-ra-danger">{error}</p>
        {/if}
        {#if doneMsg}
          <p class="mt-3 text-sm text-ra-success">{doneMsg}</p>
        {/if}
      </div>

      <div class="flex justify-end gap-2 border-t border-ra-border px-5 py-3">
        <button
          type="button"
          class="min-h-11 rounded-xl border border-ra-border px-4 text-sm text-ra-muted hover:text-ra-text"
          onclick={close}
        >
          Cancel
        </button>
        <button
          type="button"
          class="min-h-11 rounded-xl bg-ra-accent px-4 text-sm font-semibold text-white hover:bg-ra-accent-hover disabled:opacity-50"
          disabled={busy || $userCollections.items.length === 0}
          onclick={saveSelections}
        >
          {#if busy}
            Saving…
          {:else}
            Save
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
