<script lang="ts">
  import Self from "./TileNode.svelte";
  import Tile from "./Tile.svelte";
  import { dragFraction } from "../../../workspace/tiling";
  import type { Node } from "../../../workspace/tiling";

  // A node of the split tree: a tile, or its children separated by draggable splitters.
  let {
    node,
    onresize,
    oncommit,
  }: {
    node: Node;
    /** Boundary between children `a` and `b` of split `splitId` moved: `a` now has `fraction` of the pair. */
    onresize: (splitId: string, a: string, b: string, fraction: number) => void;
    /** A drag ended: persist. */
    oncommit: () => void;
  } = $props();

  const row = $derived(node.kind === "split" && node.dir === "row");
  let cells: HTMLElement[] = $state([]);

  // Ends an in-progress drag (listeners, body classes); also run on unmount.
  let endDrag: (() => void) | null = null;
  $effect(() => () => endDrag?.());

  /** Drag on the splitter before child `i`: resizes children i-1 and i. */
  function onPointerDown(e: PointerEvent, i: number) {
    if (node.kind !== "split" || e.button !== 0) return;
    const ea = cells[i - 1];
    const eb = cells[i];
    if (!ea || !eb) return;
    e.preventDefault();
    e.stopPropagation();
    const splitId = node.id;
    const a = node.children[i - 1].id;
    const b = node.children[i].id;
    const horizontal = row;
    const size = (el: HTMLElement) => (horizontal ? el.getBoundingClientRect().width : el.getBoundingClientRect().height);
    const aPx = size(ea);
    const bPx = size(eb);
    const p0 = horizontal ? e.clientX : e.clientY;
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    const cursor = horizontal ? "cursor-col-resize" : "cursor-row-resize";
    document.body.classList.add("select-none", cursor);
    let pending: number | null = null;
    let frame = 0;
    const flush = () => {
      frame = 0;
      if (pending !== null) onresize(splitId, a, b, pending);
    };
    const move = (m: PointerEvent) => {
      pending = dragFraction(aPx, bPx, (horizontal ? m.clientX : m.clientY) - p0);
      if (!frame) frame = requestAnimationFrame(flush);
    };
    const stop = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
      handle.removeEventListener("lostpointercapture", end);
      if (frame) cancelAnimationFrame(frame);
      document.body.classList.remove("select-none", cursor);
      endDrag = null;
    };
    const end = () => {
      stop();
      if (pending !== null) {
        onresize(splitId, a, b, pending);
        oncommit();
      }
    };
    endDrag = stop;
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
    handle.addEventListener("lostpointercapture", end);
  }
</script>

{#if node.kind === "leaf"}
  <Tile leaf={node} />
{:else}
  <div class="flex min-h-0 min-w-0 flex-1 {row ? 'flex-row' : 'flex-col'}">
    {#each node.children as child, i (child.id)}
      {#if i > 0}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="flex-none touch-none bg-secondary select-none hover:bg-input {row
            ? 'w-[5px] cursor-col-resize'
            : 'h-[5px] cursor-row-resize'}"
          data-splitter
          onpointerdown={(e) => onPointerDown(e, i)}
        ></div>
      {/if}
      <div bind:this={cells[i]} class="flex min-h-0 min-w-0 overflow-hidden" style="flex: {node.sizes[i]} 1 0px">
        <Self node={child} {onresize} {oncommit} />
      </div>
    {/each}
  </div>
{/if}
