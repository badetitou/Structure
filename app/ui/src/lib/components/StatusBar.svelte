<script lang="ts">
  import { analysis } from "../../stores/analysis.svelte";
  import { impact } from "../../stores/impact.svelte";
  import { selection } from "../../stores/selection.svelte";
  import { shown } from "../../stores/shown";
  import { impactLabel, selectionLabel, snapshotLabel, statsLabel } from "../../statusbar";

  // Context of the analysis, one line at the bottom of the window.
  const m = $derived(shown.matrix);
  const impactText = $derived(impactLabel(impact.current, impact.progress));
  const selectionText = $derived(selectionLabel(selection.selected, shown.matrix?.layers));
</script>

{#snippet sep()}<span class="text-border" aria-hidden="true">|</span>{/snippet}

<footer
  class="flex h-6 flex-none items-center gap-2 overflow-hidden border-t bg-card px-3 text-xs whitespace-nowrap text-muted-foreground"
  aria-label="Status bar"
>
  <span class="min-w-16 truncate" title={analysis.paths.join("\n")}>{analysis.paths.join(" + ")}</span>
  {#if m}
    {@render sep()}
    <span class="flex-none">{m.projects?.length ? `${m.projects.length} projects` : m.language}</span>
    {@render sep()}
    <span class="flex-none">{statsLabel(m.stats)}</span>
  {/if}
  {@render sep()}
  <span class="flex-none" class:text-foreground={!!impact.current?.matrix}>{snapshotLabel(impact.current)}</span>
  {#if impactText}
    {@render sep()}
    <span class="truncate">{impactText}</span>
  {/if}
  {#if impact.error}
    {@render sep()}
    <span class="max-w-[40%] flex-none truncate text-destructive" title={impact.error}>{impact.error}</span>
  {/if}
  {#if selectionText}
    {@render sep()}
    <span class="truncate text-foreground">{selectionText}</span>
  {/if}
  <span class="ml-auto flex flex-none items-center gap-2.5">
    {#if impact.current}
      <span class="flex items-center gap-1"
        ><span class="inline-block size-2 border border-(--changed-outline) bg-(--changed)"></span>changed</span
      >
      <span class="flex items-center gap-1" title="Impacted cells, by call distance from a change (1 to 5+)">
        <span class="flex">
          <span class="inline-block size-2 bg-(--impact-1)"></span>
          <span class="inline-block size-2 bg-(--impact-2)"></span>
          <span class="inline-block size-2 bg-(--impact-3)"></span>
          <span class="inline-block size-2 bg-(--impact-4)"></span>
          <span class="inline-block size-2 bg-(--impact-5)"></span>
        </span>impacted (distance 1 → 5+)
      </span>
      {@render sep()}
    {/if}
    <span class="flex items-center gap-1"><span class="inline-block size-2 bg-(--fwd)"></span>reaches</span>
    <span class="flex items-center gap-1"><span class="inline-block size-2 bg-(--bwd)"></span>reached by</span>
  </span>
</footer>
