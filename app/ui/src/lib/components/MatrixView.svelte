<script lang="ts">
  import { untrack } from "svelte";
  import { createMatrixView } from "../../matrixview";
  import type { MatrixController } from "../../matrixview";
  import { app } from "../../state.svelte";

  // Hosts the imperative matrix SVG (matrixview.ts): state flows in through the effects below.
  let { controller = $bindable(null) }: { controller?: MatrixController | null } = $props();
  let host: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (!host) return;
    const h = host;
    const c = untrack(() => {
      const matrix = app.matrix;
      if (!matrix) return null;
      return createMatrixView(
        h,
        { matrix, mode: app.mode, impact: app.impact, selected: app.selected?.id ?? null },
        (cell) => app.selectCell(cell),
      );
    });
    controller = c;
    return () => {
      c?.destroy();
      controller = null;
    };
  });
  $effect(() => controller?.setImpact(app.impact));
  $effect(() => {
    const m = app.matrix;
    if (m) controller?.setMatrix(m);
  });
  $effect(() => controller?.setMode(app.mode));
  $effect(() => controller?.setSelected(app.selected?.id ?? null));
</script>

<div bind:this={host} class="matrix-view flex min-w-0 flex-1 overflow-hidden"></div>

<style>
  /* The svg is built imperatively, hence :global. */
  .matrix-view :global(svg) {
    flex: 1;
    min-width: 0;
    height: 100%;
    cursor: grab;
    touch-action: none;
    user-select: none;
  }
  .matrix-view :global(svg.panning),
  .matrix-view :global(svg.panning *) {
    cursor: grabbing !important;
  }
  .matrix-view :global(svg text) {
    fill: var(--foreground);
    font-size: 11px;
  }
</style>
