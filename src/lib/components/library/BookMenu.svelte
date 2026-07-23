<script lang="ts">
  import type { AudiobookSummary } from "$lib/types/models";

  let {
    book,
    onaddToCollection,
  }: {
    book: AudiobookSummary;
    onaddToCollection?: (book: AudiobookSummary) => void;
  } = $props();

  let open = $state(false);

  function toggle(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    open = !open;
  }

  function close() {
    open = false;
  }

  function add(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    open = false;
    onaddToCollection?.(book);
  }
</script>

<!-- relative wrapper anchors the menu directly under the ⋮ button -->
<div class="relative">
  <button
    type="button"
    class="book-menu-btn flex h-9 w-9 items-center justify-center rounded-lg border border-ra-border/80 bg-black/60 text-sm text-ra-text shadow-md backdrop-blur-sm transition hover:bg-black/75 focus-visible:opacity-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ra-accent"
    aria-label="Book menu"
    aria-expanded={open}
    aria-haspopup="true"
    onclick={toggle}
  >
    ⋮
  </button>

  {#if open}
    <!-- Backdrop only blocks clicks; menu is positioned next to the button -->
    <button
      type="button"
      class="fixed inset-0 z-[400] cursor-default bg-transparent"
      aria-label="Close menu"
      onclick={(e) => {
        e.stopPropagation();
        close();
      }}
    ></button>
    <div
      class="absolute right-0 top-full z-[410] mt-1 w-48 rounded-xl border border-ra-border bg-ra-surface p-1 shadow-2xl ring-1 ring-white/10"
      role="menu"
      onclick={(e) => e.stopPropagation()}
    >
      <button
        type="button"
        role="menuitem"
        class="flex min-h-10 w-full items-center rounded-lg px-3 text-left text-sm text-ra-text hover:bg-ra-surface-2"
        onclick={add}
      >
        Add to collection…
      </button>
    </div>
  {/if}
</div>
