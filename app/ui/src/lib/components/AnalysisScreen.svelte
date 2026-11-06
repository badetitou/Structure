<script lang="ts">
  import { onMount } from "svelte";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import DetailsPanel from "./DetailsPanel.svelte";
  import HistorySidebar from "./HistorySidebar.svelte";
  import MatrixView from "./MatrixView.svelte";
  import Progress from "./Progress.svelte";
  import ProjectHeader from "./ProjectHeader.svelte";
  import SidePanel from "./SidePanel.svelte";
  import Toolbar from "./Toolbar.svelte";
  import { app } from "../../state.svelte";

  // Analyses `path`, shows the progress, then the matrix with its panels.
  let { path, onExit }: { path: string; onExit: (error?: string) => void } = $props();

  let matrix = $state<ReturnType<typeof MatrixView> | null>(null);

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
      <div class="flex min-h-0 flex-1">
        {#if app.history}
          <SidePanel side="left" panel={app.panels.left} other={app.panels.right} onsave={() => app.savePanels()}>
            <HistorySidebar />
          </SidePanel>
        {/if}
        {#if app.matrix}
          <MatrixView bind:this={matrix} matrix={app.matrix} />
        {/if}
        <SidePanel
          side="right"
          panel={app.panels.right}
          other={app.panels.left}
          otherHidden={!app.history}
          onsave={() => app.savePanels()}
        >
          <DetailsPanel />
        </SidePanel>
      </div>
    </Tooltip.Provider>
  {:else}
    <Progress />
  {/if}
</div>
