<script lang="ts">
  import { edgeKind, edgeStyle } from "../../../cellstate";
  import type { Hover } from "../../../cellstate";
  import type { ImpactView } from "../../../domaintree";
  import type { CellEdge } from "../../../api";
  import type { Tile } from "../../../layout";
  import type { RemoteKind } from "../../../graph";
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
    kind,
  }: {
    edge: CellEdge;
    from: Tile;
    to: Tile;
    hover: Hover | null;
    impact: ImpactView | null;
    /** Remote link: HTTP dashed, RPC dotted, own colour while not highlighted. */
    kind?: RemoteKind;
  } = $props();

  const st = $derived(edgeStyle(edgeKind(edge.from, edge.to, hover, impact), edge.weight, kind));
</script>

<path class:http={kind === "http"} class:rpc={kind === "rpc"} d={edgePath(from, to)} style:stroke={st.stroke} style:opacity={st.opacity} stroke-width={st.width} marker-end="url(#{st.marker})" />

<style>
  path {
    fill: none;
    pointer-events: none;
  }
  path.http {
    stroke-dasharray: 6 4;
  }
  path.rpc {
    stroke-dasharray: 2 3;
  }
</style>
