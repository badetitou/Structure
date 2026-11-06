<script lang="ts">
  import { tick } from "svelte";
  import { ModeWatcher } from "mode-watcher";
  import Home from "./Home.svelte";
  import ThemeSwitch from "./ThemeSwitch.svelte";
  import { runAnalysis } from "./main";

  let screen = $state<"home" | "analysis">("home");
  let path = $state(""); // lives here so it survives Home remounts
  let error = $state("");
  let host: HTMLElement | undefined = $state();

  function backHome(err = "") {
    error = err;
    screen = "home";
  }
  async function start(p: string) {
    path = p;
    error = "";
    screen = "analysis";
    await tick(); // the host container is mounted after this
    if (host) void runAnalysis(p, host, backHome);
    else backHome("Could not open the analysis view.");
  }
</script>

<ModeWatcher />
<div class="flex items-center justify-between border-b bg-card px-4" style="height: var(--app-header-h, 49px)">
  <span class="flex items-center gap-2 font-semibold"><img src="/logo.svg" alt="" class="size-7" /> Structure</span>
  <ThemeSwitch />
</div>
{#if screen === "home"}
  <Home bind:path {error} onAnalyze={start} />
{:else}
  <div class="legacy" bind:this={host}></div>
{/if}
