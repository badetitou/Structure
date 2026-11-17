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
  import { canAlign } from "../../aligned";
  import ThemeSwitch from "../../ThemeSwitch.svelte";
  import { parseMode } from "../../mode";
  import { analysis } from "../../stores/analysis.svelte";
  import { impact } from "../../stores/impact.svelte";
  import { options } from "../../stores/options.svelte";
  import { clearImpact, keepOverlay } from "../../stores/session";
  import { shown } from "../../stores/shown";
  import { folderName } from "../../recent";
  import ImpactRange from "./ImpactRange.svelte";
  import OptionsButton from "./OptionsButton.svelte";
  import ViewsMenu from "./workspace/ViewsMenu.svelte";

  // The analysis screen's single top bar: project, matrix controls, views, theme, close.
  // Stats and other context live in the status bar.
  let {
    onZoomIn,
    onZoomOut,
    onFit,
    onClose,
    ready,
    embedded = false,
  }: {
    onZoomIn: () => void;
    onZoomOut: () => void;
    onFit: () => void;
    onClose: () => void;
    ready: boolean;
    embedded?: boolean;
  } = $props();

  // Several projects: their names joined; else the project name or folder.
  const title = $derived(
    analysis.work?.projects?.length ? analysis.work.projects.map((p) => p.name).join(" + ") : analysis.work?.project || folderName(analysis.project),
  );
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
  <h1 class="m-0 max-w-56 truncate text-sm font-semibold" title={analysis.paths.join("\n")}>{title}</h1>
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
    value={options.mode}
    onValueChange={(v) => v && options.setMode(parseMode(v))}
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
          pressed={options.transposed}
          onPressedChange={(on) => options.setTransposed(on)}><ArrowLeftRight /></Toggle
        >
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content
      >Swap axes — {options.transposed ? "domains as rows, layers as columns" : "layers as rows, domains as columns"}</Tooltip.Content
    >
  </Tooltip.Root>
  {#if canAlign(shown.matrix?.projects, shown.matrix?.httpEdges ?? [], shown.matrix?.rpcEdges ?? [])}
    <Toggle
      variant="outline"
      size="sm"
      class="px-2"
      aria-label="Align projects: front and back domains on shared rows"
      pressed={options.aligned}
      onPressedChange={(v) => options.setAligned(v)}>Align projects</Toggle
    >
  {/if}
  {#if (shown.matrix?.httpEdges.length ?? 0) > 0}
    <Toggle
      variant="outline"
      size="sm"
      class="px-2"
      aria-label="Show front-to-back HTTP links"
      pressed={options.showHttp}
      onPressedChange={(v) => options.setShowHttp(v)}>HTTP links</Toggle
    >
  {/if}
  {#if (shown.matrix?.rpcEdges.length ?? 0) > 0}
    <Toggle
      variant="outline"
      size="sm"
      class="px-2"
      aria-label="Show GWT RPC links"
      pressed={options.showRpc}
      onPressedChange={(v) => options.setShowRpc(v)}>RPC links</Toggle
    >
  {/if}
  {#if !analysis.multi && !embedded}
  {@render divider()}
  <ImpactRange />
  <Button variant="outline" size="sm" disabled={!impact.selected || impact.busy} onclick={() => clearImpact()}
    >Clear impact</Button
  >
  {#if impact.selectedOverlays.some((o) => !o.pinned)}
    <Button
      variant="outline"
      size="sm"
      disabled={impact.busy}
      title="Keep this impact as an overlay: the next impact is added next to it"
      onclick={() => keepOverlay()}>Keep</Button
    >
  {/if}
  {/if}
  {/if}
  <span class="ml-auto flex flex-none items-center gap-1.5">
    {#if ready}<ViewsMenu />{/if}
    <ThemeSwitch />
    <OptionsButton />
    {#if !embedded}{@render iconBtn(XIcon, "Close project", onClose)}{/if}
  </span>
</header>
