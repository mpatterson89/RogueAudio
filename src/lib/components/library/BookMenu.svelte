<script lang="ts">
  import type { AudiobookSummary } from "$lib/types/models";
  import { library } from "$lib/stores/library";
  import {
    downloads,
    downloadsByKey,
    isDownloadComplete,
    isInDownloadQueue,
  } from "$lib/stores/downloads";
  import { serverIdForBook } from "$lib/installed";

  let {
    book,
    onaddToCollection,
  }: {
    book: AudiobookSummary;
    onaddToCollection?: (book: AudiobookSummary) => void;
  } = $props();

  let open = $state(false);
  let busy = $state(false);
  let actionError = $state<string | null>(null);

  const downloadItem = $derived($downloadsByKey.get(book.ratingKey) ?? null);
  const pending = $derived(!!$downloads.pending[book.ratingKey]);
  const complete = $derived(isDownloadComplete(downloadItem));
  const inQueue = $derived(pending || isInDownloadQueue(downloadItem));
  const serverId = $derived(
    serverIdForBook(book, $downloads.items, $library.serverId),
  );
  const canDownload = $derived(!!serverId && !complete && !inQueue);
  const canCancel = $derived(inQueue);
  const canRemove = $derived(complete);

  const hasActions = $derived(
    !!onaddToCollection || canDownload || canCancel || canRemove,
  );

  function toggle(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    open = !open;
    actionError = null;
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

  async function startDownload(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    if (!serverId || busy) return;
    busy = true;
    actionError = null;
    try {
      await downloads.enqueue(serverId, book.ratingKey);
      open = false;
    } catch (err) {
      actionError = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function cancelDownload(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    if (busy) return;
    busy = true;
    actionError = null;
    try {
      await downloads.cancel(book.ratingKey);
      open = false;
    } catch (err) {
      actionError = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function removeDownload(e: Event) {
    e.preventDefault();
    e.stopPropagation();
    if (busy) return;
    if (
      !confirm(
        `Remove offline copy of “${book.title}”?\nThis deletes the audio files from this device.`,
      )
    ) {
      return;
    }
    busy = true;
    actionError = null;
    try {
      await downloads.remove(book.ratingKey);
      open = false;
    } catch (err) {
      actionError = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  function downloadLabel(): string {
    if (busy && canDownload) return "Starting…";
    if (pending) return "Queuing…";
    if (downloadItem?.status === "downloading") {
      const pct = Math.round(
        Math.min(100, Math.max(0, (downloadItem.progress ?? 0) * 100)),
      );
      return `Downloading… ${pct}%`;
    }
    if (downloadItem?.status === "paused") return "Paused in queue";
    if (downloadItem?.status === "queued") return "Queued";
    if (downloadItem?.status === "error") return "Download error";
    return "Download";
  }
</script>

{#if hasActions}
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
        class="absolute right-0 top-full z-[410] mt-1 w-52 rounded-xl border border-ra-border bg-ra-surface p-1 shadow-2xl ring-1 ring-white/10"
        role="menu"
        tabindex="-1"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => {
          if (e.key === "Escape") close();
        }}
      >
        {#if onaddToCollection}
          <button
            type="button"
            role="menuitem"
            class="flex min-h-10 w-full items-center rounded-lg px-3 text-left text-sm text-ra-text hover:bg-ra-surface-2"
            onclick={add}
          >
            Add to collection…
          </button>
        {/if}

        {#if canDownload}
          <button
            type="button"
            role="menuitem"
            class="flex min-h-10 w-full items-center rounded-lg px-3 text-left text-sm text-ra-text hover:bg-ra-surface-2 disabled:opacity-50"
            disabled={busy}
            onclick={startDownload}
          >
            Download
          </button>
        {:else if canCancel}
          <div
            class="px-3 py-1.5 text-[11px] text-ra-muted"
            role="presentation"
          >
            {downloadLabel()}
          </div>
          <button
            type="button"
            role="menuitem"
            class="flex min-h-10 w-full items-center rounded-lg px-3 text-left text-sm text-ra-danger hover:bg-ra-danger/10 disabled:opacity-50"
            disabled={busy}
            onclick={cancelDownload}
          >
            {busy ? "Cancelling…" : "Cancel download"}
          </button>
        {:else if canRemove}
          <div
            class="px-3 py-1.5 text-[11px] text-ra-muted"
            role="presentation"
          >
            Downloaded offline
          </div>
          <button
            type="button"
            role="menuitem"
            class="flex min-h-10 w-full items-center rounded-lg px-3 text-left text-sm text-ra-danger hover:bg-ra-danger/10 disabled:opacity-50"
            disabled={busy}
            onclick={removeDownload}
          >
            {busy ? "Removing…" : "Remove download"}
          </button>
        {/if}

        {#if actionError}
          <p class="px-3 py-1.5 text-[11px] text-ra-danger">{actionError}</p>
        {/if}
      </div>
    {/if}
  </div>
{/if}
