<script lang="ts" module>
  /** What the analysis screen can ask of the matrix. */
  export interface MatrixHandle {
    zoomIn(): void;
    zoomOut(): void;
    fitView(): void;
  }
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import type { Matrix } from "../../api";
  import { defaultEdgeOpacity } from "../../cellstate";
  import { FLAME_MAX_COLUMNS, aggregate, aggregateImpact, bands, columns, flatColumns, initialExpanded } from "../../domaintree";
  import { adjacency, reach } from "../../graph";
  import { BAND_DENSE, BAND_NORMAL, STROKE_DENSE, STROKE_NORMAL, layout } from "../../layout";
  import { app } from "../../state.svelte";
  import { fit, isFit, oneToOne, panBy, resize as resizeVb, toUser, zoomAtClamped } from "../../viewport";
  import type { ViewBox } from "../../viewport";
  import ColumnHeaders from "./matrix/ColumnHeaders.svelte";
  import MatrixCell from "./matrix/MatrixCell.svelte";
  import MatrixEdge from "./matrix/MatrixEdge.svelte";

  // The matrix as a component tree. Everything below is derived from the
  // matrix, the mode and the expanded groups; hover, impact and selection only
  // feed per-cell / per-edge derivations, and pan / zoom is one `transform`.
  let { matrix }: { matrix: Matrix } = $props();

  const mode = $derived(app.mode);
  const transposed = $derived(app.transposed);

  // Collapsible columns along the domain tree (module domains), else one per domain.
  const tree = $derived(matrix.domain_tree && matrix.domain_tree.length > 0 ? matrix.domain_tree : undefined);
  // A new matrix (the tree can differ between commits) or mode starts from its default
  // expansion; the flame view opens the tree much deeper so the matrix grows sideways.
  let override = $state.raw<{ matrix: Matrix; flame: boolean; set: Set<string> } | null>(null);
  const flame = $derived(mode === "flame");
  const expanded = $derived(
    override && override.matrix === matrix && override.flame === flame
      ? override.set
      : tree
        ? initialExpanded(tree, flame ? FLAME_MAX_COLUMNS : undefined)
        : new Set<string>(),
  );
  const cols = $derived(tree ? columns(tree, expanded) : flatColumns(matrix.domains));
  const vm = $derived(aggregate(matrix, cols));
  const lay = $derived(
    layout(
      mode,
      matrix.layers,
      cols.map((c) => c.key),
      vm.cells,
      cols.map((c) => (c.group ? `▸ ${c.label}` : c.label)),
      transposed,
    ),
  );
  const bs = $derived(tree ? bands(tree, expanded) : []);
  const bandH = $derived(mode === "dense" ? BAND_DENSE : BAND_NORMAL);
  const top = $derived(bs.length ? (bs.reduce((d, b) => Math.max(d, b.depth), 0) + 1) * bandH : 0);
  // Bands sit above the matrix, or left of it when transposed.
  const content = $derived(transposed ? { w: lay.width + top, h: lay.height } : { w: lay.width, h: lay.height + top });

  const tileById = $derived(new Map(lay.tiles.flatMap((t) => (t.id ? [[t.id, t] as const] : []))));
  const emptyTiles = $derived(lay.tiles.filter((t) => !t.id));
  const cellTiles = $derived(vm.cells.flatMap((cell) => {
    const tile = tileById.get(cell.id);
    return tile ? [{ cell, tile }] : [];
  }));
  // Edges are drawn in normal mode only.
  const edgeTiles = $derived(
    lay.detailed
      ? vm.edges.flatMap((edge) => {
          const from = tileById.get(edge.from);
          const to = tileById.get(edge.to);
          return from && to ? [{ edge, from, to, key: `${edge.from}\u0000${edge.to}` }] : [];
        })
      : [],
  );

  // Hover: adjacency once per view matrix, then one BFS per direction per hovered cell.
  // Tied to the matrix, mode, impact and expansion it was made under: any change drops it with no stale frame.
  let hovered = $state.raw<{ id: string; keys: unknown[] } | null>(null);
  const hoverKeys = () => [matrix, mode, app.impact, expanded];
  const hoveredId = $derived(
    hovered && hovered.keys.every((k, i) => k === hoverKeys()[i]) ? hovered.id : null,
  );
  const setHover = (id: string | null) => (hovered = id ? { id, keys: hoverKeys() } : null);
  const adj = $derived(adjacency(vm.edges));
  const hover = $derived(
    hoveredId ? { id: hoveredId, fwd: reach(hoveredId, adj, "forward"), bwd: reach(hoveredId, adj, "backward") } : null,
  );
  const impact = $derived(app.impact ? aggregateImpact(app.impact, vm.cellMap) : null);
  const selTile = $derived(app.selected ? tileById.get(app.selected.id) : undefined);
  const selWidth = $derived((mode === "dense" ? STROKE_DENSE : STROKE_NORMAL)[1]);

  // A mode switch can change the expansion (flame opens deeper): drop a selection whose cell left the view.
  $effect(() => {
    const ids = new Set(vm.cells.map((c) => c.id));
    untrack(() => {
      if (app.selected && !ids.has(app.selected.id)) app.selectCell(null);
    });
  });

  function setExpanded(key: string, on: boolean) {
    const set = new Set(expanded);
    if (on) set.add(key);
    else set.delete(key);
    override = { matrix, flame, set };
    // Expand / collapse changes the cell ids: drop the selection and hover.
    app.selectCell(null);
  }

  // Zoom / pan. The viewport keeps the pane's aspect (see viewport.ts); the
  // matrix content sits in one group whose `transform` is the only thing that
  // changes while zooming or panning.
  let host: HTMLDivElement | undefined = $state();
  let svg: SVGSVGElement | undefined = $state();
  let pane = $state.raw({ w: 1, h: 1 });
  const fitOf = (p: { w: number; h: number }) => fit(content.w, content.h, p.w, p.h);
  // The user's view, valid only for the matrix, mode and orientation it was set under; a new
  // matrix, mode or orientation falls back to the default view in the same render (expand / collapse keeps it).
  // Dense and flame modes start at 1:1 pixels anchored top-left; Fit shows everything.
  let userVb = $state.raw<{ vb: ViewBox; matrix: Matrix; mode: string; transposed: boolean } | null>(null);
  const vb = $derived(
    userVb && userVb.matrix === matrix && userVb.mode === mode && userVb.transposed === transposed
      ? userVb.vb
      : mode !== "normal"
        ? oneToOne(pane.w, pane.h)
        : fitOf(pane),
  );
  const setVb = (v: ViewBox) => (userVb = { vb: v, matrix, mode, transposed });
  let panning = $state(false);
  const transform = $derived(`scale(${pane.w / vb.w}) translate(${-vb.x} ${-vb.y})`);
  const measure = (h: HTMLElement) => ({ w: Math.max(h.clientWidth, 1), h: Math.max(h.clientHeight, 1) });

  // Measure the pane once mounted (the resize observer below keeps it current).
  $effect(() => {
    if (host) pane = untrack(() => measure(host as HTMLElement));
  });
  // Keep the aspect equal to the pane's when the window or panels resize.
  $effect(() => {
    if (!host) return;
    const h = host;
    const ro = new ResizeObserver(() => {
      if (h.clientWidth === 0 || h.clientHeight === 0) return;
      const old = pane;
      const wasFit = isFit(vb, fitOf(old));
      pane = measure(h);
      setVb(wasFit ? fitOf(pane) : resizeVb(vb, old.w, pane.w, pane.h));
    });
    ro.observe(h);
    return () => ro.disconnect();
  });

  function zoomBy(factor: number, sx: number, sy: number) {
    const p = toUser(vb, sx, sy, pane.w, pane.h);
    setVb(zoomAtClamped(vb, factor, p.x, p.y, fitOf(pane)));
  }
  export const zoomIn = () => zoomBy(1.5, pane.w / 2, pane.h / 2);
  export const zoomOut = () => zoomBy(1 / 1.5, pane.w / 2, pane.h / 2);
  export function fitView() {
    if (host) pane = measure(host);
    setVb(fitOf(pane));
  }

  // Wheel: pinch (ctrlKey) zooms around the cursor, two-finger scroll pans.
  // Registered by hand: Svelte's `onwheel` is passive and could not preventDefault.
  $effect(() => {
    const el = svg;
    if (!el) return;
    const onwheel = (ev: WheelEvent) => {
      ev.preventDefault();
      const unit = ev.deltaMode === 1 ? 16 : 1;
      if (ev.ctrlKey) {
        const r = el.getBoundingClientRect();
        zoomBy(Math.exp(-ev.deltaY * unit * 0.01), ev.clientX - r.left, ev.clientY - r.top);
      } else {
        setVb(panBy(vb, -ev.deltaX * unit, -ev.deltaY * unit, pane.w, pane.h));
      }
    };
    el.addEventListener("wheel", onwheel, { passive: false });
    return () => el.removeEventListener("wheel", onwheel);
  });

  // Drag to pan. Pointer capture only starts past a few px so plain clicks
  // still reach the cells; a real drag swallows the click that follows it.
  const DRAG_PX = 4;
  let drag: { id: number; x: number; y: number; moved: boolean } | null = null;
  let suppressClick = false;
  function onpointerdown(ev: PointerEvent) {
    if (ev.button !== 0) return;
    drag = { id: ev.pointerId, x: ev.clientX, y: ev.clientY, moved: false };
    suppressClick = false;
  }
  function onpointermove(ev: PointerEvent) {
    if (!drag || !svg || ev.pointerId !== drag.id) return;
    if (ev.buttons === 0) {
      // Released outside the svg before capture: the drag is stale.
      drag = null;
      panning = false;
      return;
    }
    if (!drag.moved) {
      if (Math.hypot(ev.clientX - drag.x, ev.clientY - drag.y) < DRAG_PX) return;
      drag.moved = true;
      suppressClick = true;
      svg.setPointerCapture(ev.pointerId);
      panning = true;
    }
    setVb(panBy(vb, ev.clientX - drag.x, ev.clientY - drag.y, pane.w, pane.h));
    drag.x = ev.clientX;
    drag.y = ev.clientY;
  }
  function endDrag(ev: PointerEvent) {
    if (!drag || ev.pointerId !== drag.id) return;
    if (drag.moved && svg?.hasPointerCapture(ev.pointerId)) svg.releasePointerCapture(ev.pointerId);
    drag = null;
    panning = false;
  }
  function onclickcapture(ev: MouseEvent) {
    if (suppressClick) {
      ev.stopPropagation();
      suppressClick = false;
    }
  }
</script>

<div bind:this={host} class="matrix-view flex min-w-0 flex-1 overflow-hidden">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <svg
    bind:this={svg}
    class:panning
    {onpointerdown}
    {onpointermove}
    onpointerup={endDrag}
    onpointercancel={endDrag}
    {onclickcapture}
  >
    <defs>
      {#each [["a-def", "var(--edge)"], ["a-fwd", "var(--fwd)"], ["a-bwd", "var(--bwd)"], ["a-imp", "var(--changed)"]] as [id, color] (id)}
        <marker {id} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
          <path d="M0,0 L10,5 L0,10 z" style:fill={color} />
        </marker>
      {/each}
    </defs>
    <g {transform}>
      <ColumnHeaders
        {lay}
        {cols}
        bands={bs}
        {bandH}
        {top}
        onexpand={(key) => setExpanded(key, true)}
        oncollapse={(key) => setExpanded(key, false)}
      />
      <g
        transform={transposed ? `translate(${top} 0)` : `translate(0 ${top})`}
        class:hovering={!!hover}
        class:impacting={!hover && !!impact}
        style:--edge-opacity={defaultEdgeOpacity(!!hover, !!impact)}
      >
        {#each emptyTiles as t (`${t.layer}\u0000${t.domain}`)}
          {#if lay.detailed}
            <rect class="slot" x={t.x} y={t.y} width={t.w} height={t.h} />
          {:else}
            <rect class="slot-dense" x={t.x + 0.5} y={t.y + 0.5} width={t.w - 1} height={t.h - 1} />
          {/if}
        {/each}
        <!-- Edges sit under the cells -->
        {#each edgeTiles as e (e.key)}
          <MatrixEdge edge={e.edge} from={e.from} to={e.to} {hover} {impact} />
        {/each}
        {#each cellTiles as c (c.cell.id)}
          <MatrixCell
            cell={c.cell}
            tile={c.tile}
            detailed={lay.detailed}
            {hover}
            {impact}
            onhover={setHover}
            onselect={(cell) => app.selectCell(cell)}
          />
        {/each}
        <!-- The selected outline is drawn last so neighbours (dense tiles touch) do not cover it. -->
        {#if selTile}
          <rect
            class="selected"
            x={selTile.x + (lay.detailed ? 0 : 0.5)}
            y={selTile.y + (lay.detailed ? 0 : 0.5)}
            width={selTile.w - (lay.detailed ? 0 : 1)}
            height={selTile.h - (lay.detailed ? 0 : 1)}
            rx={lay.detailed ? 3 : undefined}
            stroke-width={selWidth}
          />
        {/if}
      </g>
    </g>
  </svg>
</div>

<style>
  svg {
    flex: 1;
    min-width: 0;
    height: 100%;
    cursor: grab;
    touch-action: none;
    user-select: none;
  }
  svg.panning,
  svg.panning :global(*) {
    cursor: grabbing !important;
  }
  .slot {
    fill: none;
    stroke: var(--border);
    stroke-dasharray: 3 3;
    opacity: 0.6;
  }
  .slot-dense {
    fill: var(--tile);
  }
  .selected {
    fill: none;
    stroke: var(--cell-stroke);
    pointer-events: none;
  }
</style>
