<script lang="ts" module>
  import type { MatrixHandle } from "./MatrixView.svelte";

  /** The mounted matrix, for the toolbar's zoom / fit buttons (null while not shown). */
  export const matrixControl = $state<{ current: MatrixHandle | null }>({ current: null });
</script>

<script lang="ts">
  import ImpactBusy from "./ImpactBusy.svelte";
  import MatrixView from "./MatrixView.svelte";
  import OverlaySelector from "./OverlaySelector.svelte";
  import { shown } from "../../stores/shown";

  // The matrix as a workspace panel: the overlay selector above it (when there is a choice),
  // the matrix covered by the impact progress while one runs.
  let view = $state<MatrixHandle | null>(null);
  $effect(() => {
    const v = view;
    matrixControl.current = v;
    return () => {
      if (matrixControl.current === v) matrixControl.current = null;
    };
  });
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col">
  <OverlaySelector />
  <div class="relative flex min-h-0 min-w-0 flex-1">
    {#if shown.matrix}
      <MatrixView bind:this={view} matrix={shown.matrix} />
    {/if}
    <ImpactBusy />
  </div>
</div>
