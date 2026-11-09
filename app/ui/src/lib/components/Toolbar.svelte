<script lang="ts">
  import XIcon from "@lucide/svelte/icons/x";
  import ZoomIn from "@lucide/svelte/icons/zoom-in";
  import ZoomOut from "@lucide/svelte/icons/zoom-out";
  import Scan from "@lucide/svelte/icons/scan";
  import ArrowLeftRight from "@lucide/svelte/icons/arrow-left-right";
  import type { Component } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import { Toggle } from "$lib/components/ui/toggle";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import ThemeSwitch from "../../ThemeSwitch.svelte";
  import { parseMode } from "../../mode";
  import { app } from "../../state.svelte";
  import { baseName } from "../../statusbar";
  import ImpactRange from "./ImpactRange.svelte";
  import ViewsMenu from "./workspace/ViewsMenu.svelte";

  // The analysis screen's single top bar: project, matrix controls, views, theme, close.
  // Stats and other context live in the status bar.
  let {
    onZoomIn,
    onZoomOut,
    onFit,
    onClose,
    ready,
  }: { onZoomIn: () => void; onZoomOut: () => void; onFit: () => void; onClose: () => void; ready: boolean } = $props();
</script>

{#snippet iconBtn(Icon: Component, tip: string, onclick: () => void)}
  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <Button {...props} variant="ghost" size="icon-sm" aria-label={tip} {onclick}><Icon /></Button>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content>{tip}</Tooltip.Content>
  </Tooltip.Root>
{/snippet}

{#snippet divider()}<span class="mx-1 h-5 w-px flex-none bg-border" aria-hidden="true"></span>{/snippet}

<header class="flex h-10 flex-none items-center gap-1.5 overflow-x-auto border-b bg-card px-2 whitespace-nowrap">
  <img src="/logo.svg" alt="Structure" class="size-6 flex-none" />
  <h1 class="m-0 max-w-56 truncate text-sm font-semibold" title={app.project}>{app.work?.project || baseName(app.project)}</h1>
  {#if ready}
  {@render divider()}
  {@render iconBtn(ZoomOut, "Zoom out", onZoomOut)}
  {@render iconBtn(ZoomIn, "Zoom in", onZoomIn)}
  {@render iconBtn(Scan, "Fit: show the whole matrix", onFit)}
  {@render divider()}
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
  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props })}
        <Toggle
          {...props}
          variant="outline"
          size="sm"
          aria-label="Swap axes"
          pressed={app.transposed}
          onPressedChange={(on) => app.setTransposed(on)}><ArrowLeftRight /></Toggle
        >
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content
      >Swap axes — {app.transposed ? "domains as rows, layers as columns" : "layers as rows, domains as columns"}</Tooltip.Content
    >
  </Tooltip.Root>
  {@render divider()}
  <ImpactRange />
  <Button variant="outline" size="sm" disabled={!app.impact || app.impactBusy} onclick={() => app.clearImpact()}
    >Clear impact</Button
  >
  {/if}
  <span class="ml-auto flex flex-none items-center gap-1.5">
    {#if ready}<ViewsMenu />{/if}
    <ThemeSwitch />
    {@render iconBtn(XIcon, "Close project", onClose)}
  </span>
</header>
