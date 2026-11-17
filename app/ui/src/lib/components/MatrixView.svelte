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
  import type { CellEdge, DomainNode, Matrix } from "../../api";
  import { align, canAlign, layerLabel, projectOf } from "../../aligned";
  import { defaultEdgeOpacity } from "../../cellstate";
  import { FLAME_MAX_COLUMNS, MAX_COLUMNS, aggregate, aggregateImpact, bands, columns, flatColumns, initialExpanded, remapExpanded } from "../../domaintree";
  import { adjacency, reach, visibleEdges, type RemoteKind } from "../../graph";
  import { overlayViews } from "../../multioverlay";
  import type { OverlayHit } from "../../multioverlay";
  import { BAND_DENSE, BAND_NORMAL, STROKE_DENSE, STROKE_NORMAL, layout } from "../../layout";
  import { getViewContext } from "../../workspace/context";
  import { fit, isFit, oneToOne, panBy, resize as resizeVb, toUser, wheelZoomFactor, zoomAtClamped } from "../../viewport";
  import type { ViewBox } from "../../viewport";
  import ColumnHeaders from "./matrix/ColumnHeaders.svelte";
  import MatrixCell from "./matrix/MatrixCell.svelte";
  import MatrixEdge from "./matrix/MatrixEdge.svelte";

  // The matrix as a component tree. Everything below is derived from the
  // matrix, the mode and the expanded groups; hover, impact and selection only
  // feed per-cell / per-edge derivations, and pan / zoom is one `transform`.
  let { matrix }: { matrix: Matrix } = $props();
  const { lens, overlays: impact, selection, actions } = getViewContext();
  const options = lens.display;

  const mode = $derived(options.mode);
  const transposed = $derived(options.transposed);

  // Collapsible columns along the domain tree (module domains), else one per domain.
  const tree = $derived(matrix.domain_tree && matrix.domain_tree.length > 0 ? matrix.domain_tree : undefined);
  // A new analysis or mode starts from its default expansion; the flame view opens the tree
  // much deeper so the matrix grows sideways. A commit snapshot of the same analysis keeps
  // the open groups (remapped by label, as its tree can differ) and the zoom.
  const base = $derived(lens.work ?? matrix);
  let override = $state.raw<{ base: Matrix; tree: DomainNode[]; flame: boolean; set: Set<string> } | null>(null);
  const flame = $derived(mode === "flame");
  // Aligned view: several projects with remote links, front and back domains share rows.
  const alignedOn = $derived(options.aligned && canAlign(matrix.projects, matrix.httpEdges, matrix.rpcEdges));
  const budget = $derived(flame ? FLAME_MAX_COLUMNS : MAX_COLUMNS);
  // Aligned: each project (top-level node) gets its own column budget, so the backend opens too.
  const defaultExpanded = (t: DomainNode[]) => {
    if (!alignedOn) return initialExpanded(t, budget);
    const set = new Set<string>();
    t.forEach((n, i) => {
      for (const k of initialExpanded([n], budget)) set.add(`g:${i}${k.slice(3)}`);
    });
    return set;
  };
  const baseTree = $derived(base.domain_tree && base.domain_tree.length > 0 ? base.domain_tree : undefined);
  const expanded = $derived.by(() => {
    if (!tree) return new Set<string>();
    const src =
      override && override.base === base && override.flame === flame
        ? override
        : baseTree
          ? { tree: baseTree, set: defaultExpanded(baseTree) }
          : { tree, set: defaultExpanded(tree) };
    return remapExpanded(src.tree, tree, src.set);
  });
  const cols = $derived(tree ? columns(tree, expanded) : flatColumns(matrix.domains));
  const vm0 = $derived(aggregate(matrix, cols));
  const layerIds = $derived(matrix.layers.map((l) => l.id));
  const projectNames = $derived((matrix.projects ?? []).map((p) => p.name));
  const al = $derived(
    alignedOn ? align(vm0, cols, layerIds, projectNames, [...vm0.httpEdges, ...vm0.rpcEdges]) : null,
  );
  const vm = $derived(al ? al.vm : vm0);
  const heads = $derived(al ? al.rows : cols.map((c) => ({ key: c.key, members: [c] })));
  const lay = $derived(
    layout(
      mode,
      al ? al.layers.map((l) => l.key) : layerIds,
      heads.map((h) => h.key),
      vm.cells,
      heads.map((h) => h.members.map((c) => (c.group ? `▸ ${c.label}` : c.label)).join(" ⇄ ")),
      transposed,
      al ? al.layers.map((l) => layerLabel(matrix.layers, l.layer)) : matrix.layers.map((l) => l.label),
      al?.segments,
      options.angledLabels,
    ),
  );
  // Aligned: only the primary project's bands, shifted to its first row.
  const bs = $derived.by(() => {
    if (!tree) return [];
    const all = bands(tree, expanded);
    if (!al) return all;
    const pi = projectNames.indexOf(al.primary);
    const first = cols.findIndex((c) => projectOf(c, projectNames) === pi);
    return all
      .filter((b) => b.key === `g:${pi}` || b.key.startsWith(`g:${pi}/`))
      .map((b) => ({ ...b, start: b.start - first }));
  });
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
  const showHttp = $derived(options.showHttp);
  const showRpc = $derived(options.showRpc);
  const tiled = (list: readonly CellEdge[], kind?: RemoteKind) =>
    list.flatMap((edge) => {
      const from = tileById.get(edge.from);
      const to = tileById.get(edge.to);
      return from && to ? [{ edge, from, to, kind, key: `${kind?.[0] ?? "c"}${edge.from}\u0000${edge.to}` }] : [];
    });
  const edgeTiles = $derived(
    lay.detailed ? [
          ...tiled(vm.edges),
          ...(showHttp ? tiled(vm.httpEdges, "http") : []),
          ...(showRpc ? tiled(vm.rpcEdges, "rpc") : []),
        ] : [],
  );

  // Hover: adjacency once per view matrix, then one BFS per direction per hovered cell.
  // Tied to the matrix, mode, impact and expansion it was made under: any change drops it with no stale frame.
  let hovered = $state.raw<{ id: string; keys: unknown[] } | null>(null);
  const hoverKeys = () => [matrix, mode, impact.current, impact.selectedIds, expanded, alignedOn];
  const hoveredId = $derived(
    hovered && hovered.keys.every((k, i) => k === hoverKeys()[i]) ? hovered.id : null,
  );
  const setHover = (id: string | null) => (hovered = id ? { id, keys: hoverKeys() } : null);
  const adj = $derived(adjacency(visibleEdges(vm.edges, vm.httpEdges, showHttp, vm.rpcEdges, showRpc)));
  const hover = $derived(
    hoveredId ? { id: hoveredId, fwd: reach(hoveredId, adj, "forward"), bwd: reach(hoveredId, adj, "backward") } : null,
  );
  // Several overlays drawn together: each cell's hits (bands), their union drives edges and dimming.
  const multiView = $derived(impact.multi ? overlayViews(impact.selectedOverlays, vm.cellMap) : null);
  const NO_HITS: OverlayHit[] = [];
  const shownImpact = $derived(
    multiView ? multiView.union : impact.current ? aggregateImpact(impact.current, vm.cellMap) : null,
  );
  const selTile = $derived(selection.selected ? tileById.get(selection.selected.id) : undefined);
  const selWidth = $derived((mode === "dense" ? STROKE_DENSE : STROKE_NORMAL)[1]);

  // Publish the aggregated view so the details panel names visible cells.
  $effect(() => {
    actions.publishView(vm);
    return () => {
      actions.publishView(null);
    };
  });

  // A mode switch can change the expansion (flame opens deeper): drop a selection whose cell left the view;
  // after a rules reload, take the view's cell of the same id (its members may have changed).
  $effect(() => {
    const cells = vm.cells;
    untrack(() => actions.syncSelection(cells));
  });

  function setExpanded(key: string, on: boolean) {
    const set = new Set(expanded);
    if (on) set.add(key);
    else set.delete(key);
    if (tree) override = { base, tree, flame, set };
    // Expand / collapse changes the cell ids: drop the selection and hover.
    actions.selectCell(null);
  }

  // Zoom / pan. The viewport keeps the pane's aspect (see viewport.ts); the
  // matrix content sits in one group whose `transform` is the only thing that
  // changes while zooming or panning.
  let host: HTMLDivElement | undefined = $state();
  let svg: SVGSVGElement | undefined = $state();
  let pane = $state.raw({ w: 1, h: 1 });
  const fitOf = (p: { w: number; h: number }) => fit(content.w, content.h, p.w, p.h);
  // The user's view, valid only for the analysis, mode and orientation it was set under; a new
  // analysis, mode or orientation falls back to the default view in the same render (expand / collapse
  // and switching to a commit snapshot keep it).
  // Dense and flame modes start at 1:1 pixels anchored top-left; Fit shows everything.
  let userVb = $state.raw<{ vb: ViewBox; base: Matrix; mode: string; transposed: boolean } | null>(null);
  const vb = $derived(
    userVb && userVb.base === base && userVb.mode === mode && userVb.transposed === transposed
      ? userVb.vb
      : mode !== "normal"
        ? oneToOne(pane.w, pane.h)
        : fitOf(pane),
  );
  const setVb = (v: ViewBox) => (userVb = { vb: v, base, mode, transposed });
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

  // Wheel: scroll and pinch zoom around the cursor; shift+wheel pans (drag pans too).
  // Registered by hand: Svelte's `onwheel` is passive and could not preventDefault.
  $effect(() => {
    const el = svg;
    if (!el) return;
    const onwheel = (ev: WheelEvent) => {
      ev.preventDefault();
      if (ev.shiftKey) {
        const unit = ev.deltaMode === 1 ? 16 : 1;
        setVb(panBy(vb, -ev.deltaX * unit, -ev.deltaY * unit, pane.w, pane.h));
      } else {
        const r = el.getBoundingClientRect();
        zoomBy(wheelZoomFactor(ev.deltaY, ev.deltaMode, ev.ctrlKey), ev.clientX - r.left, ev.clientY - r.top);
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
      {#each [["a-def", "var(--edge)"], ["a-fwd", "var(--fwd)"], ["a-bwd", "var(--bwd)"], ["a-imp", "var(--edge-impact)"], ["a-http", "var(--edge-http)"], ["a-rpc", "var(--edge-rpc)"]] as [id, color] (id)}
        <marker {id} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
          <path d="M0,0 L10,5 L0,10 z" style:fill={color} />
        </marker>
      {/each}
    </defs>
    <g {transform}>
      <ColumnHeaders
        {lay}
        {heads}
        bands={bs}
        {bandH}
        {top}
        onexpand={(key) => setExpanded(key, true)}
        oncollapse={(key) => setExpanded(key, false)}
      />
      <g
        transform={transposed ? `translate(${top} 0)` : `translate(0 ${top})`}
        class:hovering={!!hover}
        class:impacting={!hover && !!shownImpact}
        style:--edge-opacity={defaultEdgeOpacity(!!hover, !!shownImpact)}
      >
        {#each emptyTiles as t (`${t.layer}\u0000${t.domain}`)}
          {#if lay.detailed}
            <rect class="slot" x={t.x} y={t.y} width={t.w} height={t.h} />
          {:else}
            <rect class="slot-dense" x={t.x + 0.5} y={t.y + 0.5} width={t.w - 1} height={t.h - 1} />
          {/if}
        {/each}
        <!-- Double line between project segments, across the whole domain axis -->
        {#each lay.separators as s (s)}
          {#each [-1.5, 1.5] as d (d)}
            {#if transposed}
              <line class="segment" x1={s + d} y1={0} x2={s + d} y2={lay.height} />
            {:else}
              <line class="segment" x1={0} y1={s + d} x2={lay.width} y2={s + d} />
            {/if}
          {/each}
        {/each}
        <!-- Edges sit under the cells -->
        {#each edgeTiles as e (e.key)}
          <MatrixEdge edge={e.edge} from={e.from} to={e.to} {hover} impact={shownImpact} kind={e.kind} />
        {/each}
        {#each cellTiles as c (c.cell.id)}
          <MatrixCell
            cell={c.cell}
            layers={matrix.layers}
            tile={c.tile}
            detailed={lay.detailed}
            {hover}
            impact={shownImpact}
            hits={multiView ? (multiView.hits.get(c.cell.id) ?? NO_HITS) : null}
            onhover={setHover}
            onselect={(cell) => actions.selectCell(cell)}
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
  .segment {
    stroke: var(--segment-line);
    stroke-width: 1;
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
