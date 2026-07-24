<script lang="ts">
  import { onMount } from "svelte";
  import { settings } from "$lib/stores/settings";
  import {
    downloads,
    formatBytes,
    isDownloadActive,
    isDownloadComplete,
  } from "$lib/stores/downloads";
  import type { DownloadItem } from "$lib/types/downloads";

  let busyKey = $state<string | null>(null);
  let deleteAllBusy = $state(false);
  let storageError = $state<string | null>(null);

  // Any book with local files or an active/complete download job
  const installed = $derived(
    $downloads.items
      .filter(
        (i) =>
          isDownloadComplete(i) ||
          isDownloadActive(i) ||
          (i.bytesOnDisk ?? 0) > 0 ||
          (i.fileNames?.length ?? 0) > 0 ||
          i.status === "error" ||
          i.status === "cancelled",
      )
      .slice()
      .sort((a, b) => a.title.localeCompare(b.title)),
  );

  const totalBytes = $derived(
    installed.reduce((sum, i) => sum + (i.bytesOnDisk ?? i.bytesDownloaded ?? 0), 0),
  );

  onMount(() => {
    void downloads.refresh();
  });

  function bookSubtitle(item: DownloadItem): string {
    const bits: string[] = [];
    if (item.author) bits.push(item.author);
    if (item.series) {
      bits.push(
        item.seriesIndex
          ? `${item.series} #${item.seriesIndex}`
          : item.series,
      );
    }
    return bits.join(" · ");
  }

  function fileLabel(item: DownloadItem): string {
    const names = item.fileNames?.filter(Boolean) ?? [];
    if (names.length === 0) return "—";
    if (names.length === 1) return names[0];
    return `${names[0]} (+${names.length - 1} more)`;
  }

  async function removeOne(item: DownloadItem) {
    if (
      !confirm(
        `Delete offline copy of “${item.title}”?\nThis removes the audio files from this device.`,
      )
    ) {
      return;
    }
    busyKey = item.ratingKey;
    storageError = null;
    try {
      await downloads.remove(item.ratingKey);
    } catch (e) {
      storageError = e instanceof Error ? e.message : String(e);
    } finally {
      busyKey = null;
    }
  }

  async function removeAll() {
    if (installed.length === 0) return;
    if (
      !confirm(
        `Delete all ${installed.length} offline audiobook${installed.length === 1 ? "" : "s"}?\nThis cannot be undone.`,
      )
    ) {
      return;
    }
    deleteAllBusy = true;
    storageError = null;
    try {
      await downloads.removeAll();
      await downloads.refresh();
    } catch (e) {
      storageError = e instanceof Error ? e.message : String(e);
    } finally {
      deleteAllBusy = false;
    }
  }
</script>

<div class="mx-auto max-w-2xl space-y-6 pb-8">
  <header>
    <h1 class="text-2xl font-semibold tracking-tight">Settings</h1>
    <p class="mt-1 text-sm text-ra-muted">Local preferences and offline storage.</p>
  </header>

  <section class="space-y-4 rounded-2xl border border-ra-border bg-ra-surface p-5">
    <h2 class="text-sm font-semibold uppercase tracking-wide text-ra-muted">Playback</h2>
    <label class="flex items-center justify-between gap-4">
      <span class="text-sm">Skip interval (seconds)</span>
      <input
        type="number"
        min="5"
        max="120"
        step="5"
        class="w-24 rounded-lg border border-ra-border bg-ra-surface-2 px-2 py-2 text-sm"
        value={$settings.skipSeconds}
        onchange={(e) =>
          settings.patch({ skipSeconds: Number((e.target as HTMLInputElement).value) })}
      />
    </label>

    <label class="flex items-center justify-between gap-4">
      <span class="text-sm">Sleep fade (seconds)</span>
      <input
        type="number"
        min="0"
        max="60"
        step="5"
        class="w-24 rounded-lg border border-ra-border bg-ra-surface-2 px-2 py-2 text-sm"
        value={$settings.sleepFadeSeconds}
        onchange={(e) =>
          settings.patch({
            sleepFadeSeconds: Number((e.target as HTMLInputElement).value),
          })}
      />
    </label>
  </section>

  <!-- Offline storage -->
  <section class="space-y-4 rounded-2xl border border-ra-border bg-ra-surface p-5">
    <div class="flex flex-wrap items-start justify-between gap-3">
      <div>
        <h2 class="text-sm font-semibold uppercase tracking-wide text-ra-muted">
          Offline storage
        </h2>
        <p class="mt-1 text-sm text-ra-muted">
          Audiobooks downloaded to this device
          {#if installed.length > 0}
            · <span class="tabular-nums text-ra-text">{formatBytes(totalBytes)}</span>
            total
          {/if}
          · manage the active queue on
          <a href="/downloads" class="text-ra-accent hover:underline">Downloads</a>
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <a
          href="/downloads"
          class="inline-flex min-h-10 items-center rounded-lg border border-ra-border px-3 text-sm text-ra-muted hover:border-ra-accent hover:text-ra-text"
        >
          Queue
        </a>
        <button
          type="button"
          class="min-h-10 rounded-lg border border-ra-border px-3 text-sm text-ra-muted hover:border-ra-accent hover:text-ra-text disabled:opacity-50"
          disabled={$downloads.loading}
          onclick={() => downloads.refresh()}
        >
          {#if $downloads.loading}
            <span class="ra-spinner" aria-hidden="true"></span>
          {:else}
            Refresh
          {/if}
        </button>
        <button
          type="button"
          class="min-h-10 rounded-lg border border-ra-danger/40 px-3 text-sm text-ra-danger hover:bg-ra-danger/10 disabled:opacity-50"
          disabled={deleteAllBusy || installed.length === 0}
          onclick={removeAll}
        >
          {#if deleteAllBusy}
            Deleting…
          {:else}
            Delete all
          {/if}
        </button>
      </div>
    </div>

    {#if storageError || $downloads.error}
      <p class="text-sm text-ra-danger">{storageError ?? $downloads.error}</p>
    {/if}

    {#if $downloads.loading && installed.length === 0}
      <div class="flex items-center gap-2 py-6 text-sm text-ra-muted">
        <span class="ra-spinner" aria-hidden="true"></span>
        Loading installed books…
      </div>
    {:else if installed.length === 0}
      <div
        class="rounded-xl border border-dashed border-ra-border bg-ra-bg/40 px-4 py-8 text-center text-sm text-ra-muted"
      >
        No offline audiobooks yet. Open a book and tap <span class="text-ra-text">Download</span>.
      </div>
    {:else}
      <ul class="divide-y divide-ra-border rounded-xl border border-ra-border overflow-hidden">
        {#each installed as item (item.ratingKey)}
          <li class="flex flex-col gap-2 bg-ra-surface-2/40 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
            <div class="min-w-0 flex-1 space-y-0.5">
              <p class="truncate text-sm font-medium text-ra-text">{item.title}</p>
              {#if bookSubtitle(item)}
                <p class="truncate text-xs text-ra-muted">{bookSubtitle(item)}</p>
              {/if}
              <p class="truncate font-mono text-[11px] text-ra-muted/80" title={fileLabel(item)}>
                {fileLabel(item)}
              </p>
              <p class="text-xs tabular-nums text-ra-muted">
                {formatBytes(item.bytesOnDisk ?? item.bytesDownloaded ?? 0)}
                {#if item.trackCount > 1}
                  · {item.trackCount} files
                {/if}
                {#if item.status !== "complete"}
                  · <span class="text-ra-accent">{item.status}</span>
                {/if}
              </p>
            </div>
            <button
              type="button"
              class="min-h-10 shrink-0 self-start rounded-lg border border-ra-border px-3 text-sm text-ra-muted hover:border-ra-danger hover:text-ra-danger disabled:opacity-50 sm:self-center"
              disabled={busyKey === item.ratingKey || deleteAllBusy}
              onclick={() => removeOne(item)}
            >
              {#if busyKey === item.ratingKey}
                Deleting…
              {:else}
                Delete
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="rounded-2xl border border-ra-border bg-ra-surface p-5 text-sm text-ra-muted">
    <h2 class="mb-2 font-medium text-ra-text">Continue elsewhere</h2>
    <p>
      On each book’s page, turn on <span class="text-ra-text">Continue elsewhere</span> to
      sync your listen position with Plex and Plexamp (and pull their position when
      different). Off by default per title; local bookmarks always work.
    </p>
  </section>

  <section class="rounded-2xl border border-ra-border bg-ra-surface p-5 text-sm text-ra-muted">
    <h2 class="mb-2 font-medium text-ra-text">Roadmap</h2>
    <ul class="list-inside list-disc space-y-1">
      <li>MPRIS media keys</li>
      <li>Flatpak packaging for Steam Deck</li>
    </ul>
  </section>
</div>
