<script lang="ts">
  import { settings } from "$lib/stores/settings";
</script>

<div class="mx-auto max-w-lg space-y-6">
  <header>
    <h1 class="text-2xl font-semibold tracking-tight">Settings</h1>
    <p class="mt-1 text-sm text-ra-muted">Local preferences. More options coming soon.</p>
  </header>

  <section class="space-y-4 rounded-2xl border border-ra-border bg-ra-surface p-5">
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

  <section class="rounded-2xl border border-ra-border bg-ra-surface p-5 text-sm text-ra-muted">
    <h2 class="mb-2 font-medium text-ra-text">Roadmap</h2>
    <ul class="list-inside list-disc space-y-1">
      <li>Real Plex PIN auth & library fetch</li>
      <li>Progress sync to Plex timeline</li>
      <li>Chapter support & end-of-chapter sleep</li>
      <li>Offline downloads (book view → Download)</li>
      <li>MPRIS media keys</li>
      <li>Flatpak packaging for Steam Deck</li>
    </ul>
  </section>
</div>
