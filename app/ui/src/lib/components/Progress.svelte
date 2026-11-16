<script lang="ts">
  import { Progress } from "$lib/components/ui/progress";
  import { analysis } from "../../stores/analysis.svelte";
</script>

<div class="mx-auto mt-20 flex w-full max-w-[560px] flex-col gap-3.5 px-4">
  <h2 class="text-2xl font-semibold">Analyzing…</h2>
  <Progress value={analysis.progress.pct} class="h-3.5" aria-label="Analysis progress" />
  <div>{analysis.progress.step} ({analysis.progress.pct}%)</div>
  <div class="min-h-[1em] truncate text-xs text-muted-foreground">{analysis.progress.detail ?? ""}</div>
</div>
