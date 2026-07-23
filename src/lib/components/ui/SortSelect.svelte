<script lang="ts">
  let {
    value,
    options,
    label = "Sort",
    onchange,
  }: {
    value: string;
    options: { value: string; label: string }[];
    label?: string;
    onchange?: (v: string) => void;
  } = $props();
</script>

<label class="inline-flex items-center gap-1.5 text-sm text-ra-muted">
  <span class="text-xs uppercase tracking-wide text-ra-muted/80">{label}</span>
  <select
    class="min-h-9 max-w-[11rem] rounded-md border border-ra-border bg-ra-surface px-2 py-1 text-sm text-ra-text sm:max-w-none"
    {value}
    onchange={(e) => onchange?.((e.target as HTMLSelectElement).value)}
    aria-label={label}
  >
    {#each options as opt}
      <option value={opt.value}>{opt.label}</option>
    {/each}
  </select>
</label>
