<script lang="ts">
  let {
    value = $bindable(""),
    placeholder = "Search library…",
    onsearch,
  }: {
    value?: string;
    placeholder?: string;
    onsearch?: (q: string) => void;
  } = $props();

  let debounce: ReturnType<typeof setTimeout> | null = null;

  function handleInput(e: Event) {
    const v = (e.target as HTMLInputElement).value;
    value = v;
    if (debounce) clearTimeout(debounce);
    debounce = setTimeout(() => onsearch?.(v), 250);
  }
</script>

<label class="relative block w-full max-w-md">
  <span class="sr-only">Search</span>
  <span class="pointer-events-none absolute inset-y-0 left-3 flex items-center text-ra-muted"
    >⌕</span
  >
  <input
    type="search"
    class="min-h-11 w-full rounded-xl border border-ra-border bg-ra-surface py-2 pl-9 pr-3 text-sm text-ra-text placeholder:text-ra-muted/70 focus:border-ra-accent focus:outline-none"
    {placeholder}
    {value}
    oninput={handleInput}
  />
</label>
