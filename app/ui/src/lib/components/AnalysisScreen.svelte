<script lang="ts">
  import { onMount } from "svelte";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { matrixControl } from "./MatrixPanel.svelte";
  import Progress from "./Progress.svelte";
  import StatusBar from "./StatusBar.svelte";
  import Toolbar from "./Toolbar.svelte";
  import Workspace from "./workspace/Workspace.svelte";
  import { analysis } from "../../stores/analysis.svelte";
  import { load, reset, runImpact, watchRules } from "../../stores/session";
  import { shown } from "../../stores/shown";

  // Analyses `paths`, shows the progress, then the workspace of panels (matrix, details, history, …)
  // between one compact top bar and a status bar.
  // `embedded` (viewer page): the impact is shown right after the analysis, and the git / project
  // controls (range, clear impact, close) are left out.
  let {
    paths,
    onExit,
    embedded = false,
  }: { paths: string[]; onExit: (error?: string) => void; embedded?: boolean } = $props();

  const matrix = $derived(matrixControl.current);

  onMount(() => {
    let live = true;
    let unwatch = () => {};
    load(paths)
      .then(() => {
        if (!live) return;
        if (embedded) return runImpact(null, null);
        // Reload the matrix when the project's rules files change.
        unwatch = watchRules();
      })
      .catch((e) => live && onExit(`Analysis failed: ${String(e)}`));
    return () => {
      live = false;
      unwatch();
      reset();
    };
  });
</script>

<Tooltip.Provider>
  <div class="flex h-screen flex-col">
    <Toolbar
      ready={analysis.phase === "ready" && !!analysis.work}
      onZoomIn={() => matrix?.zoomIn()}
      onZoomOut={() => matrix?.zoomOut()}
      onFit={() => matrix?.fitView()}
      onClose={() => onExit()}
      {embedded}
    />
    {#if analysis.phase === "ready" && analysis.work}
      {#if analysis.rulesError}
        <div class="flex-none border-b px-4 py-1 text-[13px] text-destructive" role="alert">{analysis.rulesError}</div>
      {/if}
      {#if shown.warnings.length > 0}
        <div class="flex-none border-b border-(--warn-border) bg-(--warn-bg) px-4 py-1 text-[13px] text-(--warn-fg)">
          {#each shown.warnings as w}<div>⚠ {w}</div>{/each}
        </div>
      {/if}
      <Workspace />
    {:else}
      <div class="min-h-0 flex-1"><Progress /></div>
    {/if}
    <StatusBar />
  </div>
</Tooltip.Provider>
