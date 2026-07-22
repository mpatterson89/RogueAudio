<script lang="ts">
  import "../app.css";
  import AppShell from "$lib/components/layout/AppShell.svelte";
  import { auth } from "$lib/stores/auth";
  import { onMount } from "svelte";

  let { children } = $props();

  onMount(() => {
    void auth.refresh();
  });
</script>

<svelte:head>
  <title>RogueAudio</title>
</svelte:head>

<AppShell>
  {@render children()}
</AppShell>
