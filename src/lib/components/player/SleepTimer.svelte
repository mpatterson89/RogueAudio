<script lang="ts">
  import { player } from "$lib/stores/player";

  let open = $state(false);
  /** Bound to number input — may be number | string depending on browser/Svelte */
  let customMinutes = $state<string | number>(20);
  let customError = $state<string | null>(null);
  let applyingCustom = $state(false);
  let applyingChapter = $state(false);
  let applyingNextChapter = $state(false);

  const PRESETS = [15, 30, 45, 60, 90] as const;
  const MIN_MINUTES = 1;
  const MAX_MINUTES = 24 * 60; // 24 hours

  const remainingLabel = $derived.by(() => {
    const endsAt = $player.sleep.endsAt;
    if ($player.sleep.mode !== "duration" || !endsAt) return null;
    const sec = Math.max(0, Math.ceil((endsAt - Date.now()) / 1000));
    const m = Math.floor(sec / 60);
    const s = sec % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  });

  // tick remaining countdown label
  $effect(() => {
    if ($player.sleep.mode !== "duration") return;
    const id = setInterval(() => {
      void $player.sleep.endsAt;
      open = open;
    }, 1000);
    return () => clearInterval(id);
  });

  function setMinutes(m: number) {
    player.setSleepDuration(m);
    customError = null;
    open = false;
  }

  function parseCustomMinutes(): number | null {
    const raw =
      typeof customMinutes === "number"
        ? customMinutes
        : Number(String(customMinutes ?? "").trim());
    if (!Number.isFinite(raw)) return null;
    // Accept 20 or 20.0 from number inputs
    const n = Math.round(raw);
    if (Math.abs(raw - n) > 1e-9) return null;
    if (n < MIN_MINUTES || n > MAX_MINUTES) return null;
    return n;
  }

  function applyCustom(e?: Event) {
    e?.preventDefault();
    e?.stopPropagation();
    const n = parseCustomMinutes();
    if (n == null) {
      const raw =
        typeof customMinutes === "number"
          ? customMinutes
          : Number(String(customMinutes ?? "").trim());
      if (!Number.isFinite(raw)) {
        customError = "Enter a whole number";
      } else if (raw < MIN_MINUTES || raw > MAX_MINUTES) {
        customError = `${MIN_MINUTES}–${MAX_MINUTES} min`;
      } else {
        customError = "Enter a whole number";
      }
      return;
    }
    applyingCustom = true;
    try {
      setMinutes(n);
    } finally {
      applyingCustom = false;
    }
  }

  async function applyEndOfChapter() {
    applyingChapter = true;
    try {
      await player.setSleepEndOfChapter();
      open = false;
    } finally {
      applyingChapter = false;
    }
  }

  async function applyEndOfNextChapter() {
    applyingNextChapter = true;
    try {
      await player.setSleepEndOfNextChapter();
      open = false;
    } finally {
      applyingNextChapter = false;
    }
  }

  const chapterSleepActive = $derived(
    $player.sleep.mode === "end_of_chapter" ||
      $player.sleep.mode === "end_of_next_chapter",
  );

  function openMenu() {
    open = !open;
    if (open) {
      customError = null;
      if ($player.sleep.mode === "duration" && $player.sleep.minutes > 0) {
        customMinutes = $player.sleep.minutes;
      }
    }
  }
</script>

<div class="relative">
  <button
    type="button"
    class={$player.sleep.mode !== "off"
      ? "inline-flex min-h-10 min-w-10 items-center justify-center gap-1.5 rounded-lg border border-ra-accent bg-ra-surface-2 px-2.5 text-sm leading-none text-ra-accent"
      : "inline-flex min-h-10 min-w-10 items-center justify-center gap-1.5 rounded-lg border border-ra-border bg-ra-surface-2 px-2.5 text-sm leading-none text-ra-text"}
    onclick={openMenu}
    aria-expanded={open}
    aria-haspopup="true"
    title="Sleep timer"
  >
    <span class="inline-flex h-4 w-4 items-center justify-center text-base leading-none" aria-hidden="true"
      >☾</span
    >
    {#if remainingLabel}
      <span class="tabular-nums text-xs leading-none">{remainingLabel}</span>
    {:else if $player.sleep.mode === "end_of_chapter"}
      <span class="text-xs leading-none">Ch</span>
    {:else if $player.sleep.mode === "end_of_next_chapter"}
      <span class="text-xs leading-none">NCh</span>
    {/if}
  </button>

  {#if open}
    <button
      type="button"
      class="fixed inset-0 z-[300] cursor-default bg-transparent"
      aria-label="Close sleep timer menu"
      onclick={() => (open = false)}
    ></button>
    <div
      class="absolute bottom-full right-0 z-[310] mb-2 w-56 rounded-xl border border-ra-border bg-ra-surface p-2 shadow-2xl ring-1 ring-white/10"
      role="menu"
      onclick={(e) => e.stopPropagation()}
    >
      <p class="px-2 pb-1 text-[11px] font-medium uppercase tracking-wide text-ra-muted">
        Sleep timer
      </p>
      {#each PRESETS as m}
        <button
          type="button"
          role="menuitem"
          class="flex min-h-10 w-full items-center rounded-lg px-2 text-left text-sm hover:bg-ra-surface-2"
          onclick={() => setMinutes(m)}
        >
          {m} minutes
        </button>
      {/each}

      <div class="my-1 border-t border-ra-border"></div>

      <!-- type=button Set avoids form/menu quirks; parse handles number|string bind -->
      <div class="flex flex-col gap-1.5 px-1 py-1.5">
        <label
          class="px-1 text-[11px] font-medium uppercase tracking-wide text-ra-muted"
          for="sleep-custom-min"
        >
          Custom
        </label>
        <div class="flex items-center gap-1.5">
          <input
            id="sleep-custom-min"
            type="number"
            inputmode="numeric"
            min={MIN_MINUTES}
            max={MAX_MINUTES}
            step="1"
            class="min-h-10 w-full min-w-0 rounded-lg border border-ra-border bg-ra-surface-2 px-2 text-sm text-ra-text tabular-nums focus:border-ra-accent focus:outline-none"
            bind:value={customMinutes}
            onkeydown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") {
                e.preventDefault();
                applyCustom();
              }
            }}
            aria-invalid={customError ? true : undefined}
            aria-describedby={customError ? "sleep-custom-error" : undefined}
          />
          <span class="shrink-0 text-xs text-ra-muted">min</span>
          <button
            type="button"
            class="min-h-10 shrink-0 rounded-lg bg-ra-accent px-3 text-sm font-semibold text-white hover:bg-ra-accent-hover disabled:opacity-60"
            disabled={applyingCustom}
            onclick={applyCustom}
          >
            Set
          </button>
        </div>
        {#if customError}
          <p id="sleep-custom-error" class="px-1 text-[11px] text-ra-danger">{customError}</p>
        {/if}
      </div>

      <div class="my-1 border-t border-ra-border"></div>

      <button
        type="button"
        role="menuitem"
        class="flex min-h-10 w-full items-center gap-2 rounded-lg px-2 text-left text-sm hover:bg-ra-surface-2 disabled:opacity-60"
        disabled={applyingChapter || applyingNextChapter || !$player.book}
        onclick={applyEndOfChapter}
      >
        {#if applyingChapter}
          <span class="ra-spinner" aria-hidden="true"></span>
          Finding chapter…
        {:else}
          End of chapter
        {/if}
      </button>
      <button
        type="button"
        role="menuitem"
        class="flex min-h-10 w-full items-center gap-2 rounded-lg px-2 text-left text-sm hover:bg-ra-surface-2 disabled:opacity-60"
        disabled={applyingChapter || applyingNextChapter || !$player.book}
        onclick={applyEndOfNextChapter}
      >
        {#if applyingNextChapter}
          <span class="ra-spinner" aria-hidden="true"></span>
          Finding chapter…
        {:else}
          End of next chapter
        {/if}
      </button>
      {#if chapterSleepActive && $player.sleep.chapterTitle}
        <p class="px-2 pb-1 text-[11px] text-ra-muted">
          {#if $player.sleep.mode === "end_of_next_chapter"}
            Stops after next: {$player.sleep.chapterTitle}
          {:else}
            Stops after: {$player.sleep.chapterTitle}
          {/if}
        </p>
      {/if}
      <button
        type="button"
        role="menuitem"
        class="flex min-h-10 w-full items-center rounded-lg px-2 text-left text-sm text-ra-muted hover:bg-ra-surface-2"
        onclick={() => {
          player.clearSleep();
          open = false;
        }}
      >
        Off
      </button>
    </div>
  {/if}
</div>
