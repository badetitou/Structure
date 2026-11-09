<script lang="ts">
  import { app } from "../../state.svelte";
  import { impactLabel, selectionLabel, snapshotLabel, statsLabel } from "../../statusbar";

  // Context of the analysis, one line at the bottom of the window.
  const m = $derived(app.matrix);
  const impact = $derived(impactLabel(app.impact, app.impactProgress));
  const selection = $derived(selectionLabel(app.selected));
</script>

{#snippet sep()}<span class="text-border" aria-hidden="true">|</span>{/snippet}

<footer
  class="flex h-6 flex-none items-center gap-2 overflow-hidden border-t bg-card px-3 text-xs whitespace-nowrap text-muted-foreground"
  aria-label="Status bar"
>
  <span class="min-w-0 truncate" title={app.project}>{app.project}</span>
  {#if m}
    {@render sep()}
    <span>{m.language}</span>
    {@render sep()}
    <span>{statsLabel(m.stats)}</span>
  {/if}
  {@render sep()}
  <span class:text-foreground={!!app.impact?.matrix}>{snapshotLabel(app.impact)}</span>
  {#if impact}
    {@render sep()}
    <span class="truncate">{impact}</span>
  {/if}
  {#if app.impactError}
    {@render sep()}
    <span class="truncate text-destructive" title={app.impactError}>{app.impactError}</span>
  {/if}
  {#if selection}
    {@render sep()}
    <span class="truncate text-foreground">{selection}</span>
  {/if}
  <span class="ml-auto flex flex-none items-center gap-2.5">
    <span class="flex items-center gap-1"><span class="inline-block size-2 bg-(--fwd)"></span>reaches</span>
    <span class="flex items-center gap-1"><span class="inline-block size-2 bg-(--bwd)"></span>reached by</span>
  </span>
</footer>
