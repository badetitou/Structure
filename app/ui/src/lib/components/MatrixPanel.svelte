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
  import { Button } from "$lib/components/ui/button";
  import { HeldMatrix, lensLabel } from "../../lens";
  import { getViewContext } from "../../workspace/context";

  // The matrix as a workspace panel: the overlay selector above it (when there is a choice),
  // the matrix covered by the impact progress while one runs. While the next matrix of the
  // panel's lens loads, the last one stays drawn (see `HeldMatrix`), so the open groups and the
  // zoom survive picking a commit snapshot or a rules reload.
  const ctx = getViewContext();
  const hold = new HeldMatrix();
  // The analysed project: a matrix held for another one (or none) is never drawn.
  const project = $derived(ctx.lens.analysed ? ctx.lens.paths.join("\u0000") : null);
  const drawn = $derived(hold.show(ctx.lens.id, ctx.lens.matrix, ctx.lens.error, project));
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
    {#if drawn.matrix}
      <MatrixView bind:this={view} matrix={drawn.matrix} />
      {#if drawn.loading}
        <div
          class="pointer-events-none absolute top-1.5 left-1/2 -translate-x-1/2 rounded border bg-card px-2 py-0.5 text-xs text-muted-foreground"
          role="status"
        >
          Loading {ctx.lens.label}…
        </div>
      {/if}
    {:else if ctx.lens.error}
      <div class="m-auto flex max-w-md flex-col items-center gap-2 px-4 text-sm text-destructive">
        <span>Cannot show {ctx.lens.label}: {ctx.lens.error}</span>
        <Button variant="outline" size="sm" onclick={() => ctx.actions.retryLens()}>Retry</Button>
      </div>
    {:else if drawn.loading}
      <div class="m-auto text-sm text-muted-foreground">Loading {ctx.lens.label}…</div>
    {/if}
    {#if drawn.matrix && ctx.overlays.lensError}
      <!-- Overlays shown that this lens could not map: drawn empty here until computed again. -->
      <div
        class="absolute bottom-2 left-2 z-10 flex max-w-[70%] items-center gap-2 rounded border bg-card px-2 py-1 text-xs text-destructive"
        role="alert"
      >
        <span class="truncate" title={ctx.overlays.lensError}>{ctx.overlays.lensError}</span>
        <Button variant="outline" size="sm" class="h-6 flex-none" onclick={() => ctx.actions.retryLens()}>Retry</Button>
      </div>
    {/if}
    {#if ctx.selection.selected && ctx.selection.lens !== ctx.lens.id}
      <!-- The shared selection was made through another lens: its cell has no tile here. -->
      <div
        class="pointer-events-none absolute top-1.5 right-2 rounded border bg-card px-2 py-0.5 text-xs text-muted-foreground"
        role="status"
      >
        Selected cell not in this view ({lensLabel(ctx.selection.lens, ctx.lens.available)})
      </div>
    {/if}
    <ImpactBusy />
  </div>
</div>
