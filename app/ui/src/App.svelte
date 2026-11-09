<script lang="ts">
  import { ModeWatcher } from "mode-watcher";
  import Home from "./Home.svelte";
  import ThemeSwitch from "./ThemeSwitch.svelte";
  import AnalysisScreen from "$lib/components/AnalysisScreen.svelte";

  let screen = $state<"home" | "analysis">("home");
  let path = $state(""); // lives here so it survives Home remounts
  let error = $state("");
  let starting = false;

  function backHome(err = "") {
    error = err;
    starting = false;
    screen = "home";
  }
  function start(p: string) {
    if (starting) return;
    starting = true;
    path = p;
    error = "";
    screen = "analysis";
  }
</script>

<ModeWatcher />
{#if screen === "home"}
  <div class="flex items-center justify-between border-b bg-card px-4" style="height: var(--app-header-h)">
    <span class="flex items-center gap-2 font-semibold"><img src="/logo.svg" alt="" class="size-7" /> Structure</span>
    <ThemeSwitch />
  </div>
  <Home bind:path onPick={() => (error = "")} {error} onAnalyze={start} />
{:else}
  <!-- The analysis screen has its own compact top bar. -->
  <AnalysisScreen {path} onExit={backHome} />
{/if}
