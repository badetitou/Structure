<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Toggle } from "$lib/components/ui/toggle";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { app } from "../../state.svelte";

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
  <Toggle
    variant="outline"
    size="sm"
    pressed={app.mode === "dense"}
    onPressedChange={() => app.toggleMode()}
    aria-label="Dense mode"
  >
    {app.mode === "dense" ? "Dense: on" : "Dense: off"}
  </Toggle>
  <Button variant="outline" size="sm" disabled={!app.impact} onclick={() => app.clearImpact()}>Clear</Button>
  <span class="text-destructive">{app.impactError}</span>
  <span class="flex items-center text-sm text-muted-foreground">
    <span class="mr-1 ml-2.5 inline-block size-2.5 bg-(--fwd)"></span>reaches (forward)
    <span class="mr-1 ml-2.5 inline-block size-2.5 bg-(--bwd)"></span>reached by (backward)
  </span>
</div>
