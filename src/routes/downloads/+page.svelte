<script lang="ts">
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { bookHref } from "$lib/nav";
  import {
    downloads,
    queueItems,
    formatBytes,
    statusLabel,
    isDownloadComplete,
  } from "$lib/stores/downloads";
  import type { DownloadItem } from "$lib/types/downloads";

  let actionBusy = $state(false);
  let rowBusy = $state<string | null>(null);
  let pageError = $state<string | null>(null);

  const queue = $derived($downloads.queue);
  const activeItems = $derived($queueItems);
  const completed = $derived(
    $downloads.items
      .filter((i) => isDownloadComplete(i))
      .slice()
      .sort((a, b) => a.title.localeCompare(b.title)),
  );

  const estTotal = $derived(queue.estimatedBytes || sumEstimate(activeItems));
  const estDone = $derived(queue.bytesDownloaded || sumDownloaded(activeItems));
  const estRemaining = $derived(
    queue.bytesRemaining || Math.max(0, estTotal - estDone),
  );
  const overallPct = $derived(
    estTotal > 0 ? Math.min(100, Math.round((estDone / estTotal) * 100)) : 0,
  );

  onMount(() => {
    void downloads.init();
  });

  function sumEstimate(items: DownloadItem[]): number {
    return items.reduce((sum, i) => {
      const total = i.bytesTotal ?? i.bytesDownloaded ?? 0;
      return sum + Math.max(total, i.bytesDownloaded ?? 0);
    }, 0);
  }

  function sumDownloaded(items: DownloadItem[]): number {
    return items.reduce((sum, i) => sum + (i.bytesDownloaded ?? 0), 0);
  }

  function coverSrc(item: DownloadItem): string | null {
    if (!item.coverPath) return null;
    try {
      return convertFileSrc(item.coverPath);
    } catch {
      return null;
    }
  }

  function subtitle(item: DownloadItem): string {
    const bits: string[] = [];
    if (item.author) bits.push(item.author);
    if (item.series) {
      bits.push(
        item.seriesIndex ? `${item.series} #${item.seriesIndex}` : item.series,
      );
    }
    return bits.join(" · ");
  }

  function itemPct(item: DownloadItem): number {
    return Math.round(Math.min(100, Math.max(0, (item.progress ?? 0) * 100)));
  }

  function sizeLine(item: DownloadItem): string {
    const done = item.bytesDownloaded ?? 0;
    const total = item.bytesTotal;
    if (total && total > 0) {
      return `${formatBytes(done)} / ~${formatBytes(total)}`;
    }
    if (done > 0) return formatBytes(done);
    return "Size pending…";
  }

  async function togglePause() {
    actionBusy = true;
    pageError = null;
    try {
      if (queue.paused) {
        await downloads.resumeQueue();
      } else {
        await downloads.pauseQueue();
      }
    } catch (e) {
      pageError = e instanceof Error ? e.message : String(e);
    } finally {
      actionBusy = false;
    }
  }

  async function cancelOne(item: DownloadItem) {
    rowBusy = item.ratingKey;
    pageError = null;
    try {
      await downloads.cancel(item.ratingKey);
    } catch (e) {
      pageError = e instanceof Error ? e.message : String(e);
    } finally {
      rowBusy = null;
    }
  }

  async function retryOne(item: DownloadItem) {
    rowBusy = item.ratingKey;
    pageError = null;
    try {
      // Re-enqueue keeps partials and puts the book back on the worker
      await downloads.enqueue(item.serverId, item.ratingKey);
      if (queue.paused) {
        await downloads.resumeQueue();
      }
    } catch (e) {
      pageError = e instanceof Error ? e.message : String(e);
    } finally {
      rowBusy = null;
    }
  }

  async function removeOne(item: DownloadItem) {
    if (
      !confirm(
        `Remove “${item.title}” from this device?\nAudio files will be deleted.`,
      )
    ) {
      return;
    }
    rowBusy = item.ratingKey;
    pageError = null;
    try {
      await downloads.remove(item.ratingKey);
    } catch (e) {
      pageError = e instanceof Error ? e.message : String(e);
    } finally {
      rowBusy = null;
    }
  }
</script>

<div class="mx-auto max-w-3xl space-y-6 pb-10">
  <header class="flex flex-wrap items-start justify-between gap-3">
    <div>
      <h1 class="text-2xl font-semibold tracking-tight">Downloads</h1>
      <p class="mt-1 text-sm text-ra-muted">
        Offline queue — pauses keep partials so large books can resume.
      </p>
    </div>
    <div class="flex flex-wrap items-center gap-2">
      <button
        type="button"
        class="min-h-10 rounded-lg border border-ra-border px-3 text-sm text-ra-muted hover:border-ra-accent hover:text-ra-text disabled:opacity-50"
        disabled={$downloads.loading || actionBusy}
        onclick={() => downloads.refresh()}
      >
        Refresh
      </button>
      <button
        type="button"
        class="min-h-10 rounded-lg bg-ra-accent px-4 text-sm font-semibold text-white hover:bg-ra-accent-hover disabled:opacity-50"
        disabled={actionBusy || (activeItems.length === 0 && !queue.paused)}
        onclick={togglePause}
      >
        {#if actionBusy}
          …
        {:else if queue.paused}
          Resume queue
        {:else}
          Pause queue
        {/if}
      </button>
    </div>
  </header>

  <!-- Queue summary -->
  <section
    class="space-y-3 rounded-2xl border border-ra-border bg-ra-surface p-5"
    aria-label="Queue summary"
  >
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="flex items-center gap-2">
        <h2 class="text-sm font-semibold uppercase tracking-wide text-ra-muted">
          Queue
        </h2>
        {#if queue.paused}
          <span
            class="rounded-full bg-ra-accent-soft px-2 py-0.5 text-xs font-medium text-ra-accent"
          >
            Paused
          </span>
        {:else if activeItems.some((i) => i.status === "downloading")}
          <span
            class="rounded-full bg-ra-success/15 px-2 py-0.5 text-xs font-medium text-ra-success"
          >
            Active
          </span>
        {:else if activeItems.length > 0}
          <span
            class="rounded-full bg-ra-surface-2 px-2 py-0.5 text-xs font-medium text-ra-muted"
          >
            Waiting
          </span>
        {/if}
      </div>
      <p class="text-xs tabular-nums text-ra-muted">
        {activeItems.length}
        book{activeItems.length === 1 ? "" : "s"}
      </p>
    </div>

    {#if activeItems.length > 0}
      <div class="space-y-1.5">
        <div class="flex flex-wrap justify-between gap-2 text-sm">
          <span class="text-ra-text">
            Estimated total
            <span class="tabular-nums font-medium">~{formatBytes(estTotal)}</span>
          </span>
          <span class="tabular-nums text-ra-muted">
            {formatBytes(estDone)} done · ~{formatBytes(estRemaining)} left
          </span>
        </div>
        <div
          class="h-2 overflow-hidden rounded-full bg-ra-surface-2"
          role="progressbar"
          aria-valuenow={overallPct}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <div
            class="h-full rounded-full bg-ra-accent transition-[width] duration-300"
            style="width: {overallPct}%"
          ></div>
        </div>
      </div>
    {:else}
      <p class="text-sm text-ra-muted">
        Nothing in the queue. Open a book and tap Download to add it.
      </p>
    {/if}
  </section>

  {#if pageError || $downloads.error}
    <p class="text-sm text-ra-danger">{pageError ?? $downloads.error}</p>
  {/if}

  <!-- Active / queued list -->
  <section class="space-y-3">
    <h2 class="text-sm font-semibold uppercase tracking-wide text-ra-muted">
      In progress
    </h2>

    {#if $downloads.loading && activeItems.length === 0}
      <div class="flex items-center gap-2 py-8 text-sm text-ra-muted">
        <span class="ra-spinner" aria-hidden="true"></span>
        Loading queue…
      </div>
    {:else if activeItems.length === 0}
      <div
        class="rounded-2xl border border-dashed border-ra-border bg-ra-surface/40 px-4 py-10 text-center text-sm text-ra-muted"
      >
        Queue is empty.
      </div>
    {:else}
      <ul class="space-y-2">
        {#each activeItems as item, idx (item.ratingKey)}
          {@const cover = coverSrc(item)}
          {@const pct = itemPct(item)}
          <li
            class="rounded-2xl border border-ra-border bg-ra-surface p-3 sm:p-4"
          >
            <div class="flex gap-3">
              <a
                href={bookHref(item.serverId, item.ratingKey)}
                class="h-16 w-16 shrink-0 overflow-hidden rounded-xl bg-ra-surface-2"
                aria-label="Open {item.title}"
              >
                {#if cover}
                  <img
                    src={cover}
                    alt=""
                    class="h-full w-full object-cover"
                  />
                {:else}
                  <div
                    class="flex h-full w-full items-center justify-center text-lg opacity-40"
                  >
                    🎧
                  </div>
                {/if}
              </a>

              <div class="min-w-0 flex-1 space-y-2">
                <div class="flex flex-wrap items-start justify-between gap-2">
                  <div class="min-w-0">
                    <a
                      href={bookHref(item.serverId, item.ratingKey)}
                      class="block truncate text-sm font-medium text-ra-text hover:text-ra-accent"
                    >
                      {item.title}
                    </a>
                    {#if subtitle(item)}
                      <p class="truncate text-xs text-ra-muted">{subtitle(item)}</p>
                    {/if}
                  </div>
                  <div class="flex shrink-0 items-center gap-2 text-xs">
                    <span class="tabular-nums text-ra-muted">#{idx + 1}</span>
                    <span
                      class="rounded-md bg-ra-surface-2 px-2 py-0.5 font-medium text-ra-text"
                    >
                      {statusLabel(item.status)}
                    </span>
                  </div>
                </div>

                <div class="space-y-1">
                  <div
                    class="h-1.5 overflow-hidden rounded-full bg-ra-surface-2"
                    role="progressbar"
                    aria-valuenow={pct}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-label="{item.title} progress"
                  >
                    <div
                      class="h-full rounded-full bg-ra-accent transition-[width] duration-200"
                      style="width: {pct}%"
                    ></div>
                  </div>
                  <div
                    class="flex flex-wrap justify-between gap-2 text-[11px] tabular-nums text-ra-muted"
                  >
                    <span>{pct}% · {sizeLine(item)}</span>
                    {#if item.trackCount > 1}
                      <span>
                        {item.tracksDone}/{item.trackCount} files
                      </span>
                    {/if}
                  </div>
                </div>

                {#if item.status === "error" && item.error}
                  <p class="text-xs text-ra-danger">{item.error}</p>
                {/if}

                <div class="flex flex-wrap gap-2">
                  {#if item.status === "error" || item.status === "paused"}
                    <button
                      type="button"
                      class="min-h-9 rounded-lg border border-ra-border px-3 text-xs font-medium text-ra-text hover:border-ra-accent disabled:opacity-50"
                      disabled={rowBusy === item.ratingKey}
                      onclick={() => retryOne(item)}
                    >
                      Retry
                    </button>
                  {/if}
                  <button
                    type="button"
                    class="min-h-9 rounded-lg border border-ra-border px-3 text-xs text-ra-muted hover:border-ra-danger hover:text-ra-danger disabled:opacity-50"
                    disabled={rowBusy === item.ratingKey}
                    onclick={() => cancelOne(item)}
                  >
                    {rowBusy === item.ratingKey ? "…" : "Cancel"}
                  </button>
                  <button
                    type="button"
                    class="min-h-9 rounded-lg border border-ra-border px-3 text-xs text-ra-muted hover:border-ra-danger hover:text-ra-danger disabled:opacity-50"
                    disabled={rowBusy === item.ratingKey}
                    onclick={() => removeOne(item)}
                  >
                    Remove files
                  </button>
                </div>
              </div>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <!-- Completed offline library shortcut -->
  <section class="space-y-3">
    <div class="flex items-center justify-between gap-2">
      <h2 class="text-sm font-semibold uppercase tracking-wide text-ra-muted">
        Installed
      </h2>
      <a href="/" class="text-xs text-ra-accent hover:underline">
        Open library
      </a>
    </div>
    {#if completed.length === 0}
      <p class="text-sm text-ra-muted">No completed offline books yet.</p>
    {:else}
      <ul
        class="divide-y divide-ra-border overflow-hidden rounded-2xl border border-ra-border"
      >
        {#each completed as item (item.ratingKey)}
          <li
            class="flex items-center justify-between gap-3 bg-ra-surface/60 px-4 py-3"
          >
            <a
              href={bookHref(item.serverId, item.ratingKey)}
              class="min-w-0 flex-1 truncate text-sm text-ra-text hover:text-ra-accent"
            >
              {item.title}
            </a>
            <span class="shrink-0 text-xs tabular-nums text-ra-muted">
              {formatBytes(item.bytesOnDisk ?? item.bytesDownloaded ?? 0)}
            </span>
            <button
              type="button"
              class="min-h-9 shrink-0 rounded-lg border border-ra-border px-2 text-xs text-ra-muted hover:border-ra-danger hover:text-ra-danger disabled:opacity-50"
              disabled={rowBusy === item.ratingKey}
              onclick={() => removeOne(item)}
            >
              Delete
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>
