<script lang="ts">
  import { onMount } from "svelte";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { matrixControl } from "./MatrixPanel.svelte";
  import Progress from "./Progress.svelte";
  import StatusBar from "./StatusBar.svelte";
  import Toolbar from "./Toolbar.svelte";
  import Workspace from "./workspace/Workspace.svelte";
  import { app } from "../../state.svelte";

  // Analyses `paths`, shows the progress, then the workspace of panels (matrix, details, history, …)
  // between one compact top bar and a status bar.
  let { paths, onExit }: { paths: string[]; onExit: (error?: string) => void } = $props();

  const matrix = $derived(matrixControl.current);

  onMount(() => {
    let live = true;
    app.load(paths).catch((e) => live && onExit(`Analysis failed: ${String(e)}`));
    return () => {
      live = false;
      app.reset();
    };
  });
</script>

<Tooltip.Provider>
  <div class="flex h-screen flex-col">
    <Toolbar
      ready={app.phase === "ready" && !!app.work}
      onZoomIn={() => matrix?.zoomIn()}
      onZoomOut={() => matrix?.zoomOut()}
      onFit={() => matrix?.fitView()}
      onClose={() => onExit()}
    />
    {#if app.phase === "ready" && app.work}
      {#if app.warnings.length > 0}
        <div class="flex-none border-b border-(--warn-border) bg-(--warn-bg) px-4 py-1 text-[13px] text-(--warn-fg)">
          {#each app.warnings as w}<div>⚠ {w}</div>{/each}
        </div>
      {/if}
      <Workspace />
    {:else}
      <div class="min-h-0 flex-1"><Progress /></div>
    {/if}
    <StatusBar />
  </div>
</Tooltip.Provider>
