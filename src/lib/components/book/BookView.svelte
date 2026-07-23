<script lang="ts">
  import { goto } from "$app/navigation";
  import { progressApi } from "$lib/api/progress";
  import { player, formatTime } from "$lib/stores/player";
  import { navBusy } from "$lib/stores/navBusy";
  import {
    downloads,
    downloadsByKey,
    isDownloadActive,
    isDownloadComplete,
    formatBytes,
  } from "$lib/stores/downloads";
  import {
    getBookDetail,
    peekBookDetail,
  } from "$lib/stores/bookDetail";
  import { resolveChapterWindow } from "$lib/chapterProgress";
  import RetryPanel from "$lib/components/ui/RetryPanel.svelte";
  import SleepTimer from "$lib/components/player/SleepTimer.svelte";
  import type { AudiobookSummary, BookChapter, BookDetail, ProgressSnapshot } from "$lib/types/models";

  let {
    serverId,
    ratingKey,
  }: {
    serverId: string;
    ratingKey: string;
  } = $props();

  let detail = $state<BookDetail | null>(null);
  let progress = $state<ProgressSnapshot | null>(null);
  let loading = $state(true);
  let refreshing = $state(false);
  let error = $state<string | null>(null);
  let summaryExpanded = $state(false);
  let loadGen = 0;
  let downloadBusy = $state(false);
  let downloadError = $state<string | null>(null);

  const downloadItem = $derived($downloadsByKey.get(ratingKey) ?? null);
  const downloadPending = $derived(!!$downloads.pending[ratingKey]);
  const downloading = $derived(
    downloadPending || isDownloadActive(downloadItem),
  );
  const downloaded = $derived(isDownloadComplete(downloadItem));
  const downloadPct = $derived(
    Math.round(Math.min(100, Math.max(0, (downloadItem?.progress ?? 0) * 100))),
  );

  const isCurrent = $derived(
    $player.book?.ratingKey === ratingKey && $player.serverId === serverId,
  );

  const livePositionMs = $derived(
    isCurrent ? Math.floor($player.positionSec * 1000) : (progress?.positionMs ?? 0),
  );

  const durationMs = $derived(
    detail?.durationMs ??
      progress?.durationMs ??
      (isCurrent ? Math.floor($player.durationSec * 1000) : 0) ??
      0,
  );

  const progressPct = $derived(
    durationMs > 0 ? Math.min(100, (livePositionMs / durationMs) * 100) : 0,
  );

  const chapterWindow = $derived(
    resolveChapterWindow(detail?.chapters ?? [], livePositionMs, durationMs || null),
  );

  const activeChapterIndex = $derived(chapterWindow?.index ?? -1);

  $effect(() => {
    // Reload when route params change
    void serverId;
    void ratingKey;
    void load();
  });

  async function load(opts: { force?: boolean } = {}) {
    const force = opts.force ?? false;
    const gen = ++loadGen;
    error = null;
    downloadError = null;

    // Paint cached detail immediately (avoids blank flash + zero network)
    if (!force) {
      const cached = peekBookDetail(serverId, ratingKey);
      if (cached) {
        detail = cached;
        loading = false;
      } else {
        loading = true;
      }
    } else {
      refreshing = true;
    }

    try {
      const [d, p] = await Promise.all([
        getBookDetail(serverId, ratingKey, { force }),
        progressApi.get(ratingKey).catch(() => null),
      ]);
      if (gen !== loadGen) return;
      detail = d;
      progress = p;
      if (force) summaryExpanded = false;
      void downloads.refresh();
    } catch (e) {
      if (gen !== loadGen) return;
      // Keep showing cached detail if we had it
      if (!detail) {
        error = e instanceof Error ? e.message : String(e);
      } else {
        downloadError = e instanceof Error ? e.message : String(e);
      }
    } finally {
      if (gen === loadGen) {
        loading = false;
        refreshing = false;
      }
    }
  }

  async function refreshDetail() {
    await load({ force: true });
  }

  async function startDownload() {
    downloadBusy = true;
    downloadError = null;
    try {
      await downloads.enqueue(serverId, ratingKey);
    } catch (e) {
      downloadError = e instanceof Error ? e.message : String(e);
    } finally {
      downloadBusy = false;
    }
  }

  async function cancelDownload() {
    downloadBusy = true;
    downloadError = null;
    try {
      await downloads.cancel(ratingKey);
    } catch (e) {
      downloadError = e instanceof Error ? e.message : String(e);
    } finally {
      downloadBusy = false;
    }
  }

  async function removeDownload() {
    if (!confirm("Remove the offline copy of this audiobook?")) return;
    downloadBusy = true;
    downloadError = null;
    try {
      await downloads.remove(ratingKey);
    } catch (e) {
      downloadError = e instanceof Error ? e.message : String(e);
    } finally {
      downloadBusy = false;
    }
  }

  function asSummary(d: BookDetail): AudiobookSummary {
    return {
      ratingKey: d.ratingKey,
      title: d.title,
      author: d.author,
      thumb: d.thumb,
      year: d.year,
      durationMs: d.durationMs,
      libraryKey: d.libraryKey,
    };
  }

  async function playOrResume() {
    if (!detail) return;
    const book = asSummary(detail);
    if (isCurrent && $player.ready && !$player.loading) {
      await player.toggle();
      // Refresh bookmark card after pause/resume
      if (!$player.playing) {
        progress = await progressApi.get(ratingKey).catch(() => progress);
      }
      return;
    }
    // Resume from saved bookmark when opening play
    await player.loadBook(serverId, book, { autoplay: true });
  }

  async function seekChapter(ch: BookChapter) {
    if (!detail) return;
    const book = asSummary(detail);
    // Jump to chapter start and play — ignores old bookmark so selection wins
    await player.playAt(serverId, book, ch.startMs / 1000, true);
    // Keep local UI progress in sync after the jump
    progress = {
      ratingKey: detail.ratingKey,
      positionMs: ch.startMs,
      durationMs: detail.durationMs ?? null,
      updatedAt: new Date().toISOString(),
      source: "local",
    };
  }

  function chapterDurationLabel(ch: BookChapter, next?: BookChapter): string | null {
    const end = ch.endMs ?? next?.startMs ?? durationMs;
    if (!end || end <= ch.startMs) return null;
    return formatTime((end - ch.startMs) / 1000);
  }
</script>

{#if loading && !detail}
  <div class="flex min-h-[50vh] flex-col items-center justify-center gap-3">
    <span class="ra-spinner ra-spinner-lg" aria-hidden="true"></span>
    <p class="text-sm text-ra-muted">Opening book…</p>
  </div>
{:else if error && !detail}
  <div class="mx-auto max-w-xl space-y-4 py-10">
    <RetryPanel
      title="Couldn't load this book"
      message={error}
      loading={loading}
      onretry={load}
    />
    <div class="text-center">
      <button
        type="button"
        class="btn-ghost"
        onclick={async () => {
          navBusy.start("Back to library…");
          try {
            await goto("/");
          } finally {
            navBusy.stop();
          }
        }}>Back to library</button
      >
    </div>
  </div>
{:else if detail}
  <div class="book-view relative -mx-4 -mt-4 min-h-full sm:-mx-6 sm:-mt-5">
    <!-- Ambient art backdrop -->
    <div class="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
      {#if detail.art || detail.thumb}
        <img
          src={detail.art || detail.thumb}
          alt=""
          class="h-full w-full scale-110 object-cover opacity-40 blur-3xl saturate-150"
        />
      {/if}
      <div
        class="absolute inset-0 bg-gradient-to-b from-ra-bg/40 via-ra-bg/85 to-ra-bg"
      ></div>
      <div
        class="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--color-ra-accent-soft),_transparent_55%)]"
      ></div>
    </div>

    <!-- Keep page content below the global player chrome (z-[200]+) -->
    <div class="relative z-0 mx-auto max-w-5xl px-4 pb-10 pt-4 sm:px-6 sm:pt-6">
      <!-- Top bar -->
      <div class="mb-6 flex items-center justify-between gap-3">
        <button
          type="button"
          class="btn-ghost"
          onclick={async () => {
            navBusy.start("Back to library…");
            try {
              await goto("/");
            } finally {
              navBusy.stop();
            }
          }}
        >
          ← Library
        </button>
        <div class="flex items-center gap-2">
          <button
            type="button"
            class="btn-ghost"
            onclick={refreshDetail}
            disabled={refreshing || loading}
            title="Refresh book details from Plex"
            aria-label="Refresh book details from Plex"
          >
            {#if refreshing}
              <span class="ra-spinner" aria-hidden="true"></span>
            {:else}
              <span aria-hidden="true">↻</span>
            {/if}
            Refresh
          </button>
          {#if isCurrent}
            <span
              class="rounded-full border border-ra-accent/40 bg-ra-accent-soft px-3 py-1 text-xs font-medium text-ra-accent"
            >
              Now playing
            </span>
          {/if}
        </div>
      </div>

      <!-- Hero -->
      <section
        class="glass grid gap-6 rounded-3xl p-5 sm:grid-cols-[minmax(0,240px)_1fr] sm:gap-8 sm:p-8"
      >
        <div class="mx-auto w-full max-w-[240px]">
          <div
            class="cover-shadow aspect-square overflow-hidden rounded-2xl bg-ra-surface-2 ring-1 ring-white/10"
          >
            {#if detail.thumb}
              <img
                src={detail.thumb}
                alt=""
                class="h-full w-full object-cover"
              />
            {:else}
              <div class="flex h-full items-center justify-center text-5xl opacity-40">📖</div>
            {/if}
          </div>
        </div>

        <div class="flex min-w-0 flex-col justify-center gap-4">
          <div class="space-y-2">
            <p class="text-xs font-semibold uppercase tracking-[0.2em] text-ra-accent/90">
              Audiobook
            </p>
            <h1 class="text-balance text-3xl font-semibold tracking-tight sm:text-4xl">
              {detail.title}
            </h1>
            <p class="text-lg text-ra-muted">
              {detail.author ?? "Unknown author"}
              {#if detail.year}
                <span class="text-ra-muted/60"> · {detail.year}</span>
              {/if}
            </p>
          </div>

          <!-- Meta chips -->
          <div class="flex flex-wrap gap-2 text-xs text-ra-muted">
            {#if durationMs}
              <span class="chip">{formatTime(durationMs / 1000)}</span>
            {/if}
            {#if detail.chapters.length}
              <span class="chip">{detail.chapters.length} chapters</span>
            {:else if detail.trackCount > 1}
              <span class="chip">{detail.trackCount} parts</span>
            {/if}
            {#if detail.studio}
              <span class="chip">{detail.studio}</span>
            {/if}
          </div>

          <!-- Bookmark / book progress -->
          <div class="rounded-2xl border border-white/10 bg-black/25 p-4 backdrop-blur-md">
            <div class="mb-2 flex items-center justify-between gap-2 text-xs">
              <span class="font-medium text-ra-text">Book progress</span>
              <span class="tabular-nums text-ra-muted">
                {#if livePositionMs > 0}
                  {formatTime(livePositionMs / 1000)}
                  {#if durationMs}
                    <span class="text-ra-muted/50"> / {formatTime(durationMs / 1000)}</span>
                  {/if}
                {:else}
                  Not started
                {/if}
              </span>
            </div>
            <div class="h-2 overflow-hidden rounded-full bg-white/10">
              <div
                class="h-full rounded-full bg-gradient-to-r from-ra-accent to-ra-accent-hover transition-[width] duration-300"
                style="width: {progressPct}%"
              ></div>
            </div>
            {#if progress?.updatedAt && !isCurrent}
              <p class="mt-2 text-[11px] text-ra-muted/70">
                Saved bookmark · {new Date(progress.updatedAt).toLocaleString()}
              </p>
            {:else if isCurrent}
              <p class="mt-2 text-[11px] text-ra-muted/70">Live progress while listening</p>
            {/if}

            <!-- Chapter progress (when markers exist) -->
            {#if chapterWindow}
              <div class="mt-4 border-t border-white/10 pt-3">
                <div class="mb-2 flex items-center justify-between gap-2 text-xs">
                  <span class="min-w-0 truncate font-medium text-ra-text">
                    <span class="text-ra-muted">Chapter</span>
                    <span class="text-ra-muted/50"> · </span>
                    <span class="text-ra-accent/90">{chapterWindow.title}</span>
                  </span>
                  <span class="shrink-0 tabular-nums text-ra-muted">
                    {formatTime(chapterWindow.positionSec)}
                    <span class="text-ra-muted/50">
                      / {formatTime(chapterWindow.durationSec)}</span
                    >
                  </span>
                </div>
                <div class="h-1.5 overflow-hidden rounded-full bg-white/10">
                  <div
                    class="h-full rounded-full bg-gradient-to-r from-violet-400/90 to-ra-accent transition-[width] duration-300"
                    style="width: {chapterWindow.progressPct}%"
                  ></div>
                </div>
                <p class="mt-1.5 text-[11px] text-ra-muted/70">
                  Ch {chapterWindow.index + 1} of {detail.chapters.length}
                </p>
              </div>
            {/if}
          </div>

          <div class="flex flex-wrap items-center gap-3 pt-1">
            <button type="button" class="btn-primary" onclick={playOrResume}>
              {#if isCurrent && $player.loading}
                <span class="ra-spinner ra-spinner-on-accent" aria-hidden="true"></span>
                Loading…
              {:else if isCurrent && $player.playing}
                Pause
              {:else if livePositionMs > 15_000}
                Resume
              {:else}
                Play
              {/if}
            </button>
            {#if isCurrent && $player.ready}
              <button type="button" class="btn-secondary" onclick={() => player.skip(-30)}>
                −30s
              </button>
              <button type="button" class="btn-secondary" onclick={() => player.skip(30)}>
                +30s
              </button>
            {/if}

            <!-- Offline download -->
            {#if downloaded}
              <button
                type="button"
                class="btn-secondary"
                disabled={downloadBusy}
                onclick={removeDownload}
                title="Remove offline copy"
              >
                ✓ Downloaded
              </button>
            {:else if downloading}
              <button
                type="button"
                class="btn-secondary min-w-[9.5rem]"
                disabled={downloadBusy}
                onclick={cancelDownload}
                title="Cancel download"
              >
                <span class="ra-spinner" aria-hidden="true"></span>
                {downloadPct}% · Cancel
              </button>
            {:else}
              <button
                type="button"
                class="btn-secondary"
                disabled={downloadBusy}
                onclick={startDownload}
                title="Download for offline listening"
              >
                {#if downloadBusy}
                  <span class="ra-spinner" aria-hidden="true"></span>
                {:else}
                  <span aria-hidden="true">⬇</span>
                {/if}
                Download
              </button>
            {/if}

            <!-- Same SleepTimer component + player.sleep store as the bottom bar -->
            <div class="flex items-center gap-2">
              <span class="hidden text-xs text-ra-muted sm:inline">Sleep</span>
              <SleepTimer />
            </div>
          </div>

          {#if downloading && downloadItem}
            <div class="space-y-1.5">
              <div class="h-1.5 overflow-hidden rounded-full bg-white/10">
                <div
                  class="h-full rounded-full bg-gradient-to-r from-ra-accent to-violet-400 transition-[width] duration-300"
                  style="width: {downloadPct}%"
                ></div>
              </div>
              <p class="text-[11px] text-ra-muted/80">
                {#if (downloadItem.trackCount ?? 0) > 1}
                  Whole book · part {Math.min(downloadItem.tracksDone + 1, downloadItem.trackCount)}
                  of {downloadItem.trackCount}
                {:else}
                  Whole book
                {/if}
                {#if downloadItem.bytesDownloaded > 0}
                  · {formatBytes(downloadItem.bytesDownloaded)}{#if downloadItem.bytesTotal}
                    {" "}/ ~{formatBytes(downloadItem.bytesTotal)}{/if}
                {/if}
                · {downloadPct}%
              </p>
            </div>
          {:else if downloaded && downloadItem}
            <p class="text-[11px] text-ra-muted/80">
              Offline · whole book
              {#if downloadItem.trackCount > 1}
                · {downloadItem.trackCount} parts
              {/if}
              {#if downloadItem.bytesDownloaded > 0}
                · {formatBytes(downloadItem.bytesDownloaded)}
              {/if}
              {#if downloadItem.downloadedAt}
                · {new Date(downloadItem.downloadedAt).toLocaleString()}
              {/if}
            </p>
          {:else if downloadItem?.status === "error" || downloadError}
            <p class="text-[11px] text-ra-danger">
              {downloadError ?? downloadItem?.error ?? "Download failed"}
              <button
                type="button"
                class="ml-2 font-medium text-ra-accent underline-offset-2 hover:underline"
                onclick={startDownload}
              >
                Retry
              </button>
            </p>
          {/if}
        </div>
      </section>

      <!-- Summary -->
      {#if detail.summary}
        <section class="glass mt-6 rounded-3xl p-5 sm:p-6">
          <h2 class="mb-3 text-sm font-semibold uppercase tracking-wider text-ra-muted">
            Summary
          </h2>
          <p
            class={summaryExpanded
              ? "text-sm leading-relaxed text-ra-text/90 sm:text-[15px]"
              : "line-clamp-5 text-sm leading-relaxed text-ra-text/90 sm:text-[15px]"}
          >
            {detail.summary}
          </p>
          {#if detail.summary.length > 280}
            <button
              type="button"
              class="mt-2 text-sm font-medium text-ra-accent hover:text-ra-accent-hover"
              onclick={() => (summaryExpanded = !summaryExpanded)}
            >
              {summaryExpanded ? "Show less" : "Read more"}
            </button>
          {/if}
        </section>
      {/if}

      <!-- Chapters -->
      <section class="glass mt-6 rounded-3xl p-5 sm:p-6">
        <div class="mb-4 flex items-end justify-between gap-3">
          <div>
            <h2 class="text-sm font-semibold uppercase tracking-wider text-ra-muted">
              Chapters
            </h2>
            <p class="mt-1 text-xs text-ra-muted/70">
              {#if detail.chapters.length}
                Tap a chapter to jump · offsets use your book timeline
              {:else}
                No chapter markers for this title
              {/if}
            </p>
          </div>
          {#if detail.chapters.length}
            <span class="text-xs tabular-nums text-ra-muted">{detail.chapters.length}</span>
          {/if}
        </div>

        {#if detail.chapters.length === 0}
          <div
            class="rounded-2xl border border-dashed border-white/10 bg-black/20 px-4 py-8 text-center text-sm text-ra-muted"
          >
            When Plex has embedded chapter markers (or multi-file parts), they’ll show up here.
          </div>
        {:else}
          <ul class="max-h-[min(52vh,28rem)] space-y-1 overflow-y-auto pr-1">
            {#each detail.chapters as ch, i (ch.index + ch.startMs)}
              {@const active = i === activeChapterIndex}
              {@const next = detail.chapters[i + 1]}
              {@const dur = chapterDurationLabel(ch, next)}
              <li>
                <button
                  type="button"
                  class={active
                    ? "group flex w-full items-center gap-3 rounded-xl bg-ra-accent-soft px-3 py-2.5 text-left ring-1 ring-ra-accent/40 transition"
                    : "group flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition hover:bg-white/5"}
                  onclick={() => seekChapter(ch)}
                >
                  <span
                    class={active
                      ? "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-ra-accent text-xs font-semibold tabular-nums text-white"
                      : "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-white/5 text-xs font-semibold tabular-nums text-ra-muted"}
                  >
                    {i + 1}
                  </span>
                  <span class="min-w-0 flex-1">
                    <span
                      class={active
                        ? "block truncate text-sm font-medium text-ra-text"
                        : "block truncate text-sm font-medium text-ra-text/90"}
                    >
                      {ch.title}
                    </span>
                    <span class="text-[11px] text-ra-muted/70">
                      {formatTime(ch.startMs / 1000)}
                      {#if dur}
                        <span class="text-ra-muted/40"> · {dur}</span>
                      {/if}
                    </span>
                  </span>
                  {#if active && isCurrent && $player.playing}
                    <span class="eq" aria-hidden="true">
                      <i></i><i></i><i></i>
                    </span>
                  {:else}
                    <span
                      class="text-xs text-ra-muted opacity-0 transition group-hover:opacity-100"
                      >Play</span
                    >
                  {/if}
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      {#if error}
        <p class="mt-4 text-center text-sm text-ra-danger">{error}</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .glass {
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: linear-gradient(
      145deg,
      rgba(20, 22, 28, 0.72),
      rgba(12, 13, 16, 0.55)
    );
    box-shadow:
      0 20px 50px rgba(0, 0, 0, 0.35),
      inset 0 1px 0 rgba(255, 255, 255, 0.04);
    backdrop-filter: blur(18px);
  }

  .cover-shadow {
    box-shadow:
      0 25px 50px -12px rgba(0, 0, 0, 0.65),
      0 0 0 1px rgba(255, 255, 255, 0.06);
  }

  .chip {
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(0, 0, 0, 0.25);
    padding: 0.35rem 0.7rem;
  }

  .btn-primary {
    display: inline-flex;
    min-height: 48px;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    border-radius: 999px;
    background: var(--color-ra-accent);
    padding: 0 1.4rem;
    font-size: 0.9rem;
    font-weight: 600;
    color: white;
    transition: background 0.15s ease;
  }
  .btn-primary:hover {
    background: var(--color-ra-accent-hover);
  }

  .btn-secondary {
    display: inline-flex;
    min-height: 48px;
    align-items: center;
    justify-content: center;
    gap: 0.45rem;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(0, 0, 0, 0.25);
    padding: 0 1rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--color-ra-text);
  }
  .btn-secondary:hover {
    border-color: var(--color-ra-accent);
  }
  .btn-secondary:disabled {
    opacity: 0.65;
  }

  .btn-ghost {
    min-height: 40px;
    border-radius: 999px;
    border: 1px solid transparent;
    background: rgba(0, 0, 0, 0.2);
    padding: 0 0.9rem;
    font-size: 0.85rem;
    color: var(--color-ra-muted);
  }
  .btn-ghost:hover {
    border-color: rgba(255, 255, 255, 0.1);
    color: var(--color-ra-text);
  }

  .eq {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 14px;
  }
  .eq i {
    display: block;
    width: 3px;
    border-radius: 1px;
    background: var(--color-ra-accent);
    animation: ra-eq 0.9s ease-in-out infinite;
  }
  .eq i:nth-child(1) {
    height: 40%;
    animation-delay: 0s;
  }
  .eq i:nth-child(2) {
    height: 80%;
    animation-delay: 0.15s;
  }
  .eq i:nth-child(3) {
    height: 55%;
    animation-delay: 0.3s;
  }

  @keyframes ra-eq {
    0%,
    100% {
      transform: scaleY(0.45);
    }
    50% {
      transform: scaleY(1);
    }
  }
</style>
