<script lang="ts">
  import { onMount } from "svelte";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { matrixControl } from "./MatrixPanel.svelte";
  import Progress from "./Progress.svelte";
  import ProjectHeader from "./ProjectHeader.svelte";
  import Toolbar from "./Toolbar.svelte";
  import Workspace from "./workspace/Workspace.svelte";
  import { app } from "../../state.svelte";

  // Analyses `path`, shows the progress, then the workspace of panels (matrix, details, history, …).
  let { path, onExit }: { path: string; onExit: (error?: string) => void } = $props();

  const matrix = $derived(matrixControl.current);

  onMount(() => {
    let live = true;
    app.load(path).catch((e) => live && onExit(`Analysis failed: ${String(e)}`));
    return () => {
      live = false;
      app.reset();
    };
  });
</script>

<div class="flex h-[calc(100vh-var(--app-header-h))] flex-col">
  {#if app.phase === "ready" && app.work}
    <Tooltip.Provider>
      <ProjectHeader matrix={app.work} onClose={() => onExit()} />
      <Toolbar onZoomIn={() => matrix?.zoomIn()} onZoomOut={() => matrix?.zoomOut()} onFit={() => matrix?.fitView()} />
      {#if app.warnings.length > 0}
        <div class="flex-none border-b border-(--warn-border) bg-(--warn-bg) px-4 py-1.5 text-[13px] text-(--warn-fg)">
          {#each app.warnings as w}<div>⚠ {w}</div>{/each}
        </div>
      {/if}
      <Workspace />
    </Tooltip.Provider>
  {:else}
    <Progress />
  {/if}
</div>
