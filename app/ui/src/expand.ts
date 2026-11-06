import type { CellEdge, Matrix, WireMatrix } from "./api";

/** Expand grouped, index-based edges to `{from, to, weight}` with cell ids. */
export function expandMatrix(w: WireMatrix): Matrix {
  const edges: CellEdge[] = [];
  for (const g of w.edges) {
    const from = w.cells[g.from]?.id;
    if (from === undefined) continue;
    for (const [t, weight] of g.targets) {
      const to = w.cells[t]?.id;
      if (to !== undefined) edges.push({ from, to, weight });
    }
  }
  return { ...w, edges };
}
