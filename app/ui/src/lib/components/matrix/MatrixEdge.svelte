<script lang="ts">
  import { edgeKind } from "../../../cellstate";
  import type { Hover } from "../../../cellstate";
  import type { CellEdge } from "../../../api";
  import type { Tile } from "../../../layout";
  import { edgePath } from "../../../matrixgeom";

  // An edge only knows its kind (default / forward / backward / impact). Dimming
  // the default ones while hovering is a class on the container, so a hover
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
    impact: { changed: ReadonlySet<string>; impacted: ReadonlyMap<string, number> } | null;
  } = $props();

  const kind = $derived(edgeKind(edge.from, edge.to, hover, impact));
</script>

<path class={kind} d={edgePath(from, to)} stroke-width={Math.max(1, 1 + Math.log(edge.weight))} />

<style>
  path {
    fill: none;
    pointer-events: none;
  }
  .def {
    stroke: var(--edge);
    opacity: 0.5;
    marker-end: url(#a-def);
  }
  :global(.hovering) .def {
    opacity: 0.1;
  }
  :global(.impacting) .def {
    opacity: 0.12;
  }
  .fwd {
    stroke: var(--fwd);
    opacity: 0.95;
    marker-end: url(#a-fwd);
  }
  .bwd {
    stroke: var(--bwd);
    opacity: 0.95;
    marker-end: url(#a-bwd);
  }
  .imp {
    stroke: var(--changed);
    opacity: 0.9;
    marker-end: url(#a-imp);
  }
</style>
