<script lang="ts">
  import { player } from "$lib/stores/player";

  let open = $state(false);
  let customMinutes = $state("20");
  let customError = $state<string | null>(null);

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

  // tick label
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

  function applyCustom(e?: Event) {
    e?.preventDefault();
    const raw = customMinutes.trim();
    const n = Number(raw);
    if (!raw || !Number.isFinite(n) || !Number.isInteger(n)) {
      customError = "Enter a whole number";
      return;
    }
    if (n < MIN_MINUTES || n > MAX_MINUTES) {
      customError = `${MIN_MINUTES}–${MAX_MINUTES} min`;
      return;
    }
    setMinutes(n);
  }

  function openMenu() {
    open = !open;
    if (open) {
      customError = null;
      // Prefill with current sleep minutes when a duration timer is active
      if ($player.sleep.mode === "duration" && $player.sleep.minutes > 0) {
        customMinutes = String($player.sleep.minutes);
      }
    }
  }
</script>

<div class="relative">
  <button
    type="button"
    class={$player.sleep.mode !== "off"
      ? "flex min-h-10 min-w-10 items-center gap-1.5 rounded-lg border border-ra-accent bg-ra-surface-2 px-2.5 text-sm text-ra-accent"
      : "flex min-h-10 min-w-10 items-center gap-1.5 rounded-lg border border-ra-border bg-ra-surface-2 px-2.5 text-sm text-ra-text"}
    onclick={openMenu}
    aria-expanded={open}
    aria-haspopup="true"
    title="Sleep timer"
  >
    <span aria-hidden="true">☾</span>
    {#if remainingLabel}
      <span class="tabular-nums text-xs">{remainingLabel}</span>
    {:else if $player.sleep.mode === "end_of_chapter"}
      <span class="text-xs">Ch</span>
    {/if}
  </button>

  {#if open}
    <!-- Backdrop + menu use high z-index so they sit above book view layers -->
    <button
      type="button"
      class="fixed inset-0 z-[300] cursor-default bg-transparent"
      aria-label="Close sleep timer menu"
      onclick={() => (open = false)}
    ></button>
    <div
      class="absolute bottom-full right-0 z-[310] mb-2 w-56 rounded-xl border border-ra-border bg-ra-surface p-2 shadow-2xl ring-1 ring-white/10"
      role="menu"
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

      <form
        class="flex flex-col gap-1.5 px-1 py-1.5"
        onsubmit={applyCustom}
      >
        <label class="px-1 text-[11px] font-medium uppercase tracking-wide text-ra-muted" for="sleep-custom-min">
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
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => e.stopPropagation()}
            aria-invalid={customError ? true : undefined}
            aria-describedby={customError ? "sleep-custom-error" : undefined}
          />
          <span class="shrink-0 text-xs text-ra-muted">min</span>
          <button
            type="submit"
            class="min-h-10 shrink-0 rounded-lg bg-ra-accent px-3 text-sm font-semibold text-white hover:bg-ra-accent-hover"
          >
            Set
          </button>
        </div>
        {#if customError}
          <p id="sleep-custom-error" class="px-1 text-[11px] text-ra-danger">{customError}</p>
        {/if}
      </form>

      <div class="my-1 border-t border-ra-border"></div>

      <button
        type="button"
        role="menuitem"
        class="flex min-h-10 w-full items-center rounded-lg px-2 text-left text-sm hover:bg-ra-surface-2"
        onclick={() => {
          player.setSleepEndOfChapter();
          open = false;
        }}
      >
        End of chapter
      </button>
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
