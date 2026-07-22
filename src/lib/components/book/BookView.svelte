<script lang="ts">
  import { goto } from "$app/navigation";
  import { plexApi } from "$lib/api/plex";
  import { progressApi } from "$lib/api/progress";
  import { player, formatTime } from "$lib/stores/player";
  import RetryPanel from "$lib/components/ui/RetryPanel.svelte";
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
  let error = $state<string | null>(null);
  let summaryExpanded = $state(false);
  let loadGen = 0;

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

  const activeChapterIndex = $derived.by(() => {
    const chs = detail?.chapters ?? [];
    if (!chs.length) return -1;
    let idx = 0;
    for (let i = 0; i < chs.length; i++) {
      if (chs[i].startMs <= livePositionMs) idx = i;
      else break;
    }
    return idx;
  });

  $effect(() => {
    // Reload when route params change
    void serverId;
    void ratingKey;
    void load();
  });

  async function load() {
    const gen = ++loadGen;
    loading = true;
    error = null;
    try {
      const [d, p] = await Promise.all([
        plexApi.getBookDetail(serverId, ratingKey),
        progressApi.get(ratingKey).catch(() => null),
      ]);
      if (gen !== loadGen) return;
      detail = d;
      progress = p;
      summaryExpanded = false;
    } catch (e) {
      if (gen !== loadGen) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (gen === loadGen) loading = false;
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
      <button type="button" class="btn-ghost" onclick={() => goto("/")}>Back to library</button>
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

    <div class="relative z-10 mx-auto max-w-5xl px-4 pb-10 pt-4 sm:px-6 sm:pt-6">
      <!-- Top bar -->
      <div class="mb-6 flex items-center justify-between gap-3">
        <button type="button" class="btn-ghost" onclick={() => goto("/")}>
          ← Library
        </button>
        {#if isCurrent}
          <span
            class="rounded-full border border-ra-accent/40 bg-ra-accent-soft px-3 py-1 text-xs font-medium text-ra-accent"
          >
            Now playing
          </span>
        {/if}
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

          <!-- Bookmark / progress -->
          <div class="rounded-2xl border border-white/10 bg-black/25 p-4 backdrop-blur-md">
            <div class="mb-2 flex items-center justify-between gap-2 text-xs">
              <span class="font-medium text-ra-text">Your place</span>
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
          </div>

          <div class="flex flex-wrap gap-3 pt-1">
            <button type="button" class="btn-primary" onclick={playOrResume}>
              {#if isCurrent && $player.loading}
                <span class="spinner-sm" aria-hidden="true"></span>
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
          </div>
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
    min-height: 48px;
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

  .spinner-lg {
    width: 2rem;
    height: 2rem;
    border: 3px solid rgba(124, 106, 247, 0.25);
    border-top-color: var(--color-ra-accent);
    border-radius: 999px;
    animation: ra-spin 0.75s linear infinite;
  }

  .spinner-sm {
    width: 1rem;
    height: 1rem;
    border: 2px solid rgba(255, 255, 255, 0.35);
    border-top-color: #fff;
    border-radius: 999px;
    animation: ra-spin 0.7s linear infinite;
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

  @keyframes ra-spin {
    to {
      transform: rotate(360deg);
    }
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
