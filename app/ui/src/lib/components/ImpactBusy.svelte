<script lang="ts">
  import LoaderIcon from "@lucide/svelte/icons/loader-circle";
  import { Progress } from "$lib/components/ui/progress";
  import { getViewContext } from "../../workspace/context";

  // Covers the matrix while an impact runs (a commit snapshot may be analysed first).
  const { overlays } = getViewContext();
  const p = $derived(overlays.progress);
</script>

{#if overlays.busy && p}
  <div class="absolute inset-0 z-20 flex items-center justify-center bg-background/70" role="status" aria-live="polite">
    <div class="flex w-[min(420px,90%)] flex-col gap-2 rounded-lg border bg-card p-4 shadow-lg">
      <div class="flex items-center gap-2 text-sm font-semibold">
        <LoaderIcon class="size-4 animate-spin" />
        Computing impact…
      </div>
      <Progress value={p.pct} class="h-2" aria-label="Impact progress" />
      <div class="text-sm">{p.step} ({p.pct}%)</div>
      <div class="min-h-[1em] truncate text-xs text-muted-foreground">{p.detail ?? ""}</div>
    </div>
  </div>
{/if}
