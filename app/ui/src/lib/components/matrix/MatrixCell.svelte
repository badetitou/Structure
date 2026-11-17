<script lang="ts">
  import { layerLabel } from "../../../aligned";
  import type { LayerDef } from "../../../api";
  import { DEFAULT_FILL, hoverCellPaint, impactCellPaint } from "../../../cellstate";
  import type { Hover } from "../../../cellstate";
  import type { ImpactView, ViewCell } from "../../../domaintree";
  import type { Tile } from "../../../layout";
  import { MULTIPLE_FILL, bandRects, hitsTitle, markerRect, multiCellPaint } from "../../../multioverlay";
  import type { OverlayHit } from "../../../multioverlay";

  // One cell tile. Its look is derived from the shared hover / impact state, so
  // a hover re-evaluates this cheap derivation but only touches the DOM when the
  // resulting fill or opacity differs; the element itself is never recreated.
  let {
    cell,
    layers,
    tile,
    detailed,
    hover,
    impact,
    hits = null,
    onhover,
    onselect,
  }: {
    cell: ViewCell;
    /** Layer definitions of the matrix, for the label. */
    layers: readonly LayerDef[];
    tile: Tile;
    detailed: boolean;
    hover: Hover | null;
    impact: ImpactView | null;
    /** Several overlays drawn together: the ones hitting this cell (empty: none); null otherwise. */
    hits?: readonly OverlayHit[] | null;
    onhover: (id: string | null) => void;
    onselect: (cell: ViewCell) => void;
  } = $props();

  const multi = $derived(!hover && hits ? multiCellPaint(hits) : null);
  const paint = $derived(
    hover
      ? hoverCellPaint(cell.id, hover)
      : multi
        ? { fill: multi.kind === "multiple" ? MULTIPLE_FILL : DEFAULT_FILL, opacity: multi.kind === "none" ? 0.25 : 1 }
        : impact
          ? impactCellPaint(cell.id, impact.changed, impact.impacted)
          : { fill: DEFAULT_FILL, opacity: 1 },
  );
  // Changed cells also get an outline, so they read apart from impacted ones by more than hue.
  const changed = $derived(!hover && (multi ? multi.kind !== "none" && multi.changed : !!impact?.changed.has(cell.id)));
  const label = $derived(`${layerLabel(layers, cell.layer)}/${cell.label} (${cell.size})`);
  // Dense tiles touch: inset by half the stroke so neighbours do not overlap.
  const inset = $derived(detailed ? 0 : 0.5);
  // Bands sit inside the stroke so that the outline (changed, selected) stays visible.
  const bands = $derived.by(() => {
    if (multi?.kind !== "bands") return [];
    const pad = inset + 1;
    const rects = bandRects(
      { x: tile.x + pad, y: tile.y + pad, w: Math.max(tile.w - 2 * pad, 0), h: Math.max(tile.h - 2 * pad, 0) },
      multi.bands.length,
    );
    return rects.map((r, i) => ({ r, ...multi.bands[i], marker: markerRect(r) }));
  });
</script>

<g>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <rect
    class="cell"
    class:dense={!detailed}
    x={tile.x + inset}
    y={tile.y + inset}
    width={tile.w - 2 * inset}
    height={tile.h - 2 * inset}
    rx={detailed ? 3 : undefined}
    style:fill={paint.fill}
    class:lit={paint.opacity === 1}
    class:changed
    onmouseenter={() => onhover(cell.id)}
    onmouseleave={() => onhover(null)}
    onclick={() => onselect(cell)}
  >
    <title>{hitsTitle(label, hits ?? undefined)}</title>
  </rect>
  {#each bands as b, i (i)}
    <rect class="band" x={b.r.x} y={b.r.y} width={b.r.w} height={b.r.h} style:fill={b.colour} style:fill-opacity={b.opacity} />
    {#if b.changed}
      <rect class="band marker" x={b.marker.x} y={b.marker.y} width={b.marker.w} height={b.marker.h} />
    {/if}
  {/each}
  {#if detailed}
    <text x={tile.x + tile.w / 2} y={tile.y + tile.h + 13} text-anchor="middle">{label}</text>
  {/if}
</g>

<style>
  .cell {
    cursor: pointer;
    stroke: var(--cell-stroke);
    stroke-width: 1;
  }
  /* Cells that are not lit are dimmed by the container while hovering / showing impact. */
  :global(.hovering) .cell:not(.lit),
  :global(.impacting) .cell:not(.lit) {
    opacity: 0.25;
  }
  .dense {
    stroke-width: 0.5;
  }
  .changed {
    stroke: var(--changed-outline);
    stroke-width: 2;
  }
  .changed.dense {
    stroke-width: 1.5;
  }
  /* Bands of the overlays hitting the cell; the cell under them takes the pointer. */
  .band {
    pointer-events: none;
  }
  .marker {
    fill: var(--changed-outline);
  }
  text {
    fill: var(--foreground);
    font-size: 11px;
    pointer-events: none;
  }
</style>
