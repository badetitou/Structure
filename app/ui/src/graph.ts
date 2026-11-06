import type { CellEdge } from "./api";

/** Cells reachable from `start` following edges forward (or backward). Excludes `start` unless in a cycle. */
export function reach(
  start: string,
  edges: CellEdge[],
  dir: "forward" | "backward",
): Set<string> {
  const next = new Map<string, string[]>();
  for (const e of edges) {
    const [a, b] = dir === "forward" ? [e.from, e.to] : [e.to, e.from];
    const l = next.get(a);
    if (l) l.push(b);
    else next.set(a, [b]);
  }
  const seen = new Set<string>();
  const queue = [start];
  while (queue.length > 0) {
    const cur = queue.shift() as string;
    for (const n of next.get(cur) ?? []) {
      if (!seen.has(n)) {
        seen.add(n);
        queue.push(n);
      }
    }
  }
  seen.delete(start);
  return seen;
}
