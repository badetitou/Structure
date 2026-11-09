<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import { parseMode } from "../../mode";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { app } from "../../state.svelte";
  import ViewsMenu from "./workspace/ViewsMenu.svelte";

  let { onZoomIn, onZoomOut, onFit }: { onZoomIn: () => void; onZoomOut: () => void; onFit: () => void } = $props();

  let base = $state("");
  let head = $state("");
</script>

{#snippet zoomBtn(text: string, tip: string, onclick: () => void)}
  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <Button {...props} variant="outline" size="sm" aria-label={tip} {onclick}>{text}</Button>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content>{tip}</Tooltip.Content>
  </Tooltip.Root>
{/snippet}

<div class="flex flex-none items-center gap-2 overflow-x-auto border-b bg-card px-4 py-2 whitespace-nowrap">
  <details class="group flex items-center gap-2 open:flex-wrap">
    <summary class="cursor-pointer text-sm">Advanced</summary>
    <Input class="h-7 w-48" placeholder="base (empty = HEAD)" bind:value={base} />
    <Input class="h-7 w-48" placeholder="head (empty = working tree)" bind:value={head} />
    <Button
      variant="outline"
      size="sm"
      disabled={app.impactBusy}
      onclick={() => app.runTypedImpact(base.trim() || null, head.trim() || null)}>Show impact</Button
    >
  </details>
  {@render zoomBtn("−", "Zoom out", onZoomOut)}
  {@render zoomBtn("+", "Zoom in", onZoomIn)}
  {@render zoomBtn("Fit", "Show the whole matrix", onFit)}
  <ToggleGroup.Root
    type="single"
    variant="outline"
    size="sm"
    aria-label="Matrix view"
    value={app.mode}
    onValueChange={(v) => v && app.setMode(parseMode(v))}
  >
    <ToggleGroup.Item value="normal" aria-label="Normal view" class="px-2">Normal</ToggleGroup.Item>
    <ToggleGroup.Item value="dense" aria-label="Dense view" class="px-2">Dense</ToggleGroup.Item>
    <ToggleGroup.Item value="flame" aria-label="Flame view: deep domains, widths by size" class="px-2">Flame</ToggleGroup.Item>
  </ToggleGroup.Root>
  <Button variant="outline" size="sm" disabled={!app.impact || app.impactBusy} onclick={() => app.clearImpact()}>Clear</Button>
  <span class="text-destructive">{app.impactError}</span>
  <span class="flex items-center text-sm text-muted-foreground">
    <span class="mr-1 ml-2.5 inline-block size-2.5 bg-(--fwd)"></span>reaches (forward)
    <span class="mr-1 ml-2.5 inline-block size-2.5 bg-(--bwd)"></span>reached by (backward)
  </span>
  <span class="ml-auto"><ViewsMenu /></span>
</div>
