<script lang="ts" module>
  import type { MatrixHandle } from "./MatrixView.svelte";

  /** The mounted matrix, for the toolbar's zoom / fit buttons (null while not shown). */
  export const matrixControl = $state<{ current: MatrixHandle | null }>({ current: null });
</script>

<script lang="ts">
  import MatrixView from "./MatrixView.svelte";
  import { app } from "../../state.svelte";

  // The matrix as a workspace panel.
  let view = $state<MatrixHandle | null>(null);
  $effect(() => {
    const v = view;
    matrixControl.current = v;
    return () => {
      if (matrixControl.current === v) matrixControl.current = null;
    };
  });
</script>

{#if app.matrix}
  <MatrixView bind:this={view} matrix={app.matrix} />
{/if}
