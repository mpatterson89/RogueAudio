<script lang="ts">
  import { player } from "$lib/stores/player";

  let open = $state(false);

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
      // force dependency on time via reading endsAt
      void $player.sleep.endsAt;
      open = open;
    }, 1000);
    return () => clearInterval(id);
  });

  function setMinutes(m: number) {
    player.setSleepDuration(m);
    open = false;
  }
</script>

<div class="relative">
  <button
    type="button"
    class={$player.sleep.mode !== "off"
      ? "flex min-h-10 min-w-10 items-center gap-1.5 rounded-lg border border-ra-accent bg-ra-surface-2 px-2.5 text-sm text-ra-accent"
      : "flex min-h-10 min-w-10 items-center gap-1.5 rounded-lg border border-ra-border bg-ra-surface-2 px-2.5 text-sm text-ra-text"}
    onclick={() => (open = !open)}
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
    <button
      type="button"
      class="fixed inset-0 z-40 cursor-default bg-transparent"
      aria-label="Close sleep timer menu"
      onclick={() => (open = false)}
    ></button>
    <div
      class="absolute bottom-full right-0 z-50 mb-2 w-48 rounded-xl border border-ra-border bg-ra-surface p-2 shadow-xl"
      role="menu"
    >
      <p class="px-2 pb-1 text-[11px] font-medium uppercase tracking-wide text-ra-muted">
        Sleep timer
      </p>
      {#each [15, 30, 45, 60, 90] as m}
        <button
          type="button"
          role="menuitem"
          class="flex min-h-10 w-full items-center rounded-lg px-2 text-left text-sm hover:bg-ra-surface-2"
          onclick={() => setMinutes(m)}
        >
          {m} minutes
        </button>
      {/each}
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
