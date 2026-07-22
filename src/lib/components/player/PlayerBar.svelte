<script lang="ts">
  import { goto } from "$app/navigation";
  import { player, formatTime, PLAYBACK_RATES } from "$lib/stores/player";
  import { bookHref } from "$lib/nav";
  import SleepTimer from "./SleepTimer.svelte";

  function onSeek(e: Event) {
    const v = Number((e.target as HTMLInputElement).value);
    void player.seek(v);
  }

  const trackLabel = $derived.by(() => {
    const n = $player.tracks.length;
    if (n <= 1) return null;
    return `Track ${$player.trackIndex + 1}/${n}`;
  });

  /** Transport only when a stream is ready and not mid-load */
  const canControl = $derived($player.ready && !$player.loading);
  const showPause = $derived($player.playing && !$player.loading);

  function openBookView() {
    if (!$player.book || !$player.serverId) return;
    void goto(bookHref($player.serverId, $player.book.ratingKey));
  }
</script>

<footer
  class="shrink-0 border-t border-ra-border bg-ra-surface/95 backdrop-blur-md"
  aria-label="Player"
>
  <div class="px-3 pt-2 sm:px-4">
    <input
      type="range"
      class="w-full"
      min="0"
      max={Math.max($player.durationSec, 1)}
      step="1"
      value={$player.positionSec}
      disabled={!canControl}
      oninput={onSeek}
      aria-label="Seek"
    />
    <div class="mt-0.5 flex justify-between text-[11px] tabular-nums text-ra-muted">
      <span>{formatTime($player.positionSec)}</span>
      <span>{formatTime($player.durationSec)}</span>
    </div>
  </div>

  <div
    class="flex flex-wrap items-center gap-2 px-3 pb-3 pt-1 sm:flex-nowrap sm:gap-4 sm:px-4 sm:pb-3"
  >
    <button
      type="button"
      class="flex min-w-0 flex-1 items-center gap-3 rounded-xl text-left transition
        {$player.book
        ? 'hover:bg-white/5 focus-visible:bg-white/5 cursor-pointer'
        : 'cursor-default'}"
      disabled={!$player.book}
      onclick={openBookView}
      title={$player.book ? "Open book view" : undefined}
      aria-label={$player.book ? `Open details for ${$player.book.title}` : "Nothing playing"}
    >
      <div
        class="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-ra-surface-2 text-xl ring-1 ring-white/5"
      >
        {#if $player.book?.thumb}
          <img src={$player.book.thumb} alt="" class="h-full w-full object-cover" />
        {:else}
          <span aria-hidden="true">{$player.book ? "🎧" : "—"}</span>
        {/if}
      </div>
      <div class="min-w-0">
        {#if $player.book}
          <p class="truncate text-sm font-semibold">{$player.book.title}</p>
          <p class="truncate text-xs text-ra-muted">
            {$player.book.author ?? "Unknown"}
            {#if trackLabel}
              <span class="text-ra-muted/70"> · {trackLabel}</span>
            {/if}
            {#if $player.loading}
              <span class="ml-1 text-ra-accent">loading…</span>
            {:else}
              <span class="ml-1 text-ra-muted/50">· details</span>
            {/if}
          </p>
        {:else}
          <p class="text-sm text-ra-muted">Nothing playing</p>
          <p class="text-xs text-ra-muted/70">Pick a book from your library</p>
        {/if}
      </div>
    </button>

    <div class="flex items-center justify-center gap-1 sm:gap-2">
      <button
        type="button"
        class="btn-icon"
        disabled={!canControl}
        onclick={() => player.skip(-30)}
        aria-label="Back 30 seconds"
        title="-30s"
      >
        −30
      </button>

      <button
        type="button"
        class="flex h-12 w-12 items-center justify-center rounded-full bg-ra-accent text-base font-bold text-white transition hover:bg-ra-accent-hover disabled:cursor-not-allowed disabled:opacity-50"
        disabled={!canControl}
        onclick={() => player.toggle()}
        aria-label={$player.loading ? "Loading" : showPause ? "Pause" : "Play"}
        aria-busy={$player.loading}
        title={$player.loading ? "Loading…" : showPause ? "Pause" : "Play"}
      >
        {#if $player.loading}
          <span class="spinner" aria-hidden="true"></span>
          <span class="sr-only">Loading</span>
        {:else if showPause}
          <span aria-hidden="true" class="pause-icon">❚❚</span>
        {:else}
          <span aria-hidden="true" class="play-icon">▶</span>
        {/if}
      </button>

      <button
        type="button"
        class="btn-icon"
        disabled={!canControl}
        onclick={() => player.skip(30)}
        aria-label="Forward 30 seconds"
        title="+30s"
      >
        +30
      </button>
    </div>

    <div class="flex flex-1 items-center justify-end gap-2">
      <label class="flex items-center gap-1.5 text-xs text-ra-muted">
        <span class="hidden sm:inline">Speed</span>
        <select
          class="min-h-10 rounded-lg border border-ra-border bg-ra-surface-2 px-2 text-sm text-ra-text disabled:opacity-50"
          value={$player.rate}
          disabled={!canControl}
          onchange={(e) => player.setRate(Number((e.target as HTMLSelectElement).value))}
          aria-label="Playback speed"
        >
          {#each PLAYBACK_RATES as rate}
            <option value={rate}>{rate}×</option>
          {/each}
        </select>
      </label>
      <SleepTimer />
    </div>
  </div>

  {#if $player.error}
    <p class="px-4 pb-2 text-xs text-ra-danger">{$player.error}</p>
  {/if}
</footer>

<style>
  .btn-icon {
    min-height: 44px;
    min-width: 44px;
    border-radius: 0.75rem;
    border: 1px solid var(--color-ra-border);
    background: var(--color-ra-surface-2);
    padding: 0 0.5rem;
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--color-ra-text);
  }
  .btn-icon:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .btn-icon:hover:not(:disabled) {
    border-color: var(--color-ra-accent);
  }

  .play-icon {
    display: inline-block;
    margin-left: 2px; /* optical center triangle */
  }

  .pause-icon {
    letter-spacing: -0.05em;
    font-size: 0.85em;
  }

  .spinner {
    width: 1.35rem;
    height: 1.35rem;
    border: 2.5px solid rgba(255, 255, 255, 0.35);
    border-top-color: #fff;
    border-radius: 999px;
    animation: ra-spin 0.7s linear infinite;
  }

  @keyframes ra-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
