<script lang="ts">
  import type { Snippet } from "svelte";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { Button } from "$lib/components/ui/button";
  import { COLLAPSED_WIDTH, clampWidth } from "../../panels";
  import type { PanelState } from "../../panels";

  // Resizable, collapsible side panel. The handle sits on the inner edge:
  // drag to resize, chevron or double-click to collapse; a collapsed strip expands on click.
  let {
    side,
    panel,
    other,
    otherHidden = false,
    onsave,
    children,
  }: {
    side: "left" | "right";
    panel: PanelState;
    /** The panel on the other side (the matrix keeps its minimum room next to both). */
    other: PanelState;
    /** The other panel is not shown (e.g. history unavailable): it takes no room. */
    otherHidden?: boolean;
    onsave: () => void;
    children: Snippet;
  } = $props();

  let vw = $state(1200);
  const otherWidth = $derived(otherHidden ? 0 : other.collapsed ? COLLAPSED_WIDTH : other.width);
  const fit = (w: number) => clampWidth(w, w, vw, otherWidth);
  const width = $derived(panel.collapsed ? COLLAPSED_WIDTH : fit(panel.width));
  // The chevron points where the panel goes when it is toggled.
  const pointsRight = $derived(panel.collapsed === (side === "left"));
  const label = $derived(panel.collapsed ? "Expand panel" : "Collapse panel");

  function toggle() {
    panel.collapsed = !panel.collapsed;
    onsave();
  }
  function onPointerDown(e: PointerEvent) {
    if ((e.target as Element).closest("button") || panel.collapsed || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    const x0 = e.clientX;
    const w0 = fit(panel.width);
    document.body.classList.add("select-none", "cursor-col-resize");
    const move = (m: PointerEvent) => {
      const dx = m.clientX - x0;
      panel.width = fit(side === "left" ? w0 + dx : w0 - dx);
    };
    const end = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
      handle.removeEventListener("lostpointercapture", end);
      document.body.classList.remove("select-none", "cursor-col-resize");
      onsave();
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
    handle.addEventListener("lostpointercapture", end);
  }
</script>

<svelte:window bind:innerWidth={vw} />

{#snippet handle()}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div
    class="relative w-[5px] flex-none touch-none bg-secondary select-none hover:bg-input {panel.collapsed
      ? 'cursor-default'
      : 'cursor-col-resize'}"
    onpointerdown={onPointerDown}
    onclick={() => panel.collapsed && toggle()}
    ondblclick={() => toggle()}
  >
    <Button
      variant="outline"
      size="icon-xs"
      class="absolute top-2 -left-[7px] z-10 size-[18px] rounded-full bg-card p-0"
      title={label}
      aria-label={label}
      aria-expanded={!panel.collapsed}
      onclick={(e) => {
        e.stopPropagation();
        toggle();
      }}
      ondblclick={(e) => e.stopPropagation()}
    >
      {#if pointsRight}<ChevronRight class="size-3" />{:else}<ChevronLeft class="size-3" />{/if}
    </Button>
  </div>
{/snippet}

{#if side === "right"}{@render handle()}{/if}
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<aside
  class="box-border flex min-h-0 flex-none flex-col overflow-hidden bg-card text-[13px] {side === 'left'
    ? 'border-r'
    : 'border-l'} {panel.collapsed ? 'cursor-pointer' : ''}"
  style="width: {width}px"
  onclick={() => panel.collapsed && toggle()}
>
  <div class="min-h-0 flex-1" class:hidden={panel.collapsed}>{@render children()}</div>
</aside>
{#if side === "left"}{@render handle()}{/if}
