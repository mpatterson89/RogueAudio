<script lang="ts">
  import "../app.css";
  import AppShell from "$lib/components/layout/AppShell.svelte";
  import { auth } from "$lib/stores/auth";
  import { downloads } from "$lib/stores/downloads";
  import { onMount } from "svelte";

  let { children } = $props();

  onMount(() => {
    void auth.refresh();
    // Restore interrupted download queue (auto-resume if it was active)
    void downloads.init();
  });
</script>

<svelte:head>
  <title>RogueAudio</title>
</svelte:head>

<AppShell>
  {@render children()}
</AppShell>
