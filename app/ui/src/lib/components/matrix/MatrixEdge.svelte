<script lang="ts">
  import { edgeKind, edgeStyle } from "../../../cellstate";
  import type { Hover } from "../../../cellstate";
  import type { ImpactView } from "../../../domaintree";
  import type { CellEdge } from "../../../api";
  import type { Tile } from "../../../layout";
  import { edgePath } from "../../../matrixgeom";

  // An edge only knows its kind (default / forward / backward / impact). The default
  // ones are dimmed through a CSS variable on the container, so a hover
  // writes to the few edges that change kind, not to all of them.
  let {
    edge,
    from,
    to,
    hover,
    impact,
  }: {
    edge: CellEdge;
    from: Tile;
    to: Tile;
    hover: Hover | null;
    impact: ImpactView | null;
  } = $props();

  const st = $derived(edgeStyle(edgeKind(edge.from, edge.to, hover, impact), edge.weight));
</script>

<path d={edgePath(from, to)} style:stroke={st.stroke} style:opacity={st.opacity} stroke-width={st.width} marker-end="url(#{st.marker})" />

<style>
  path {
    fill: none;
    pointer-events: none;
  }
</style>
