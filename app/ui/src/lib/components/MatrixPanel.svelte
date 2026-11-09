<script lang="ts" module>
  import type { MatrixHandle } from "./MatrixView.svelte";

  /** The mounted matrix, for the toolbar's zoom / fit buttons (null while not shown). */
  export const matrixControl = $state<{ current: MatrixHandle | null }>({ current: null });
</script>

<script lang="ts">
  import ImpactBusy from "./ImpactBusy.svelte";
  import MatrixView from "./MatrixView.svelte";
  import { app } from "../../state.svelte";

  // The matrix as a workspace panel, covered by the impact progress while one runs.
  let view = $state<MatrixHandle | null>(null);
  $effect(() => {
    const v = view;
    matrixControl.current = v;
    return () => {
      if (matrixControl.current === v) matrixControl.current = null;
    };
  });
</script>

<div class="relative flex min-h-0 min-w-0 flex-1">
  {#if app.matrix}
    <MatrixView bind:this={view} matrix={app.matrix} />
  {/if}
  <ImpactBusy />
</div>
