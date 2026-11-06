<script lang="ts">
  import { DEFAULT_FILL, hoverCellPaint, impactCellPaint } from "../../../cellstate";
  import type { Hover } from "../../../cellstate";
  import type { ViewCell } from "../../../domaintree";
  import type { Tile } from "../../../layout";

  // One cell tile. Its look is derived from the shared hover / impact state, so
  // a hover re-evaluates this cheap derivation but only touches the DOM when the
  // resulting fill or opacity differs; the element itself is never recreated.
  let {
    cell,
    tile,
    detailed,
    hover,
    impact,
    onhover,
    onselect,
  }: {
    cell: ViewCell;
    tile: Tile;
    detailed: boolean;
    hover: Hover | null;
    impact: { changed: ReadonlySet<string>; impacted: ReadonlyMap<string, number> } | null;
    onhover: (id: string | null) => void;
    onselect: (cell: ViewCell) => void;
  } = $props();

  const paint = $derived(
    hover
      ? hoverCellPaint(cell.id, hover)
      : impact
        ? impactCellPaint(cell.id, impact.changed, impact.impacted)
        : { fill: DEFAULT_FILL, opacity: 1 },
  );
  const label = $derived(`${cell.layer}/${cell.label} (${cell.size})`);
  // Dense tiles touch: inset by half the stroke so neighbours do not overlap.
  const inset = $derived(detailed ? 0 : 0.5);
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
    onmouseenter={() => onhover(cell.id)}
    onmouseleave={() => onhover(null)}
    onclick={() => onselect(cell)}
  >
    <title>{label}</title>
  </rect>
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
  text {
    fill: var(--foreground);
    font-size: 11px;
    pointer-events: none;
  }
</style>
