<script lang="ts" module>
  import { SvelteMap } from "svelte/reactivity";
  import type { MatrixHandle } from "./MatrixView.svelte";

  /** The mounted matrices by panel id ("" outside a panel), for the toolbar's zoom / fit buttons on the active one. */
  export const matrixControl = new SvelteMap<string, MatrixHandle>();
</script>

<script lang="ts">
  import ImpactBusy from "./ImpactBusy.svelte";
  import MatrixView from "./MatrixView.svelte";
  import OverlaySelector from "./OverlaySelector.svelte";
  import { getViewContext } from "../../workspace/context";

  // The matrix as a workspace panel: the overlay selector above it (when there is a choice),
  // the matrix covered by the impact progress while one runs.
  const ctx = getViewContext();
  let view = $state<MatrixHandle | null>(null);
  const key = ctx.panel ?? "";
  $effect(() => {
    const v = view;
    if (!v) return;
    matrixControl.set(key, v);
    return () => {
      if (matrixControl.get(key) === v) matrixControl.delete(key);
    };
  });
</script>

<div class="flex min-h-0 min-w-0 flex-1 flex-col">
  <OverlaySelector />
  <div class="relative flex min-h-0 min-w-0 flex-1">
    {#if ctx.lens.matrix}
      <MatrixView bind:this={view} matrix={ctx.lens.matrix} />
    {/if}
    <ImpactBusy />
  </div>
</div>
