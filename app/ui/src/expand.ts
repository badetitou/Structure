import type { CellEdge, EdgeGroup, Matrix, WireMatrix } from "./api";

function expandGroups(w: WireMatrix, groups: readonly EdgeGroup[]): CellEdge[] {
  const edges: CellEdge[] = [];
  for (const g of groups) {
    const from = w.cells[g.from]?.id;
    if (from === undefined) continue;
    for (const [t, weight] of g.targets) {
      const to = w.cells[t]?.id;
      if (to !== undefined) edges.push({ from, to, weight });
    }
  }
  return edges;
}

/** Expand grouped, index-based edges to `{from, to, weight}` with cell ids (code edges and HTTP links). */
export function expandMatrix(w: WireMatrix): Matrix {
  const { http_edges, ...rest } = w;
  return { ...rest, edges: expandGroups(w, w.edges), httpEdges: expandGroups(w, http_edges ?? []) };
}
