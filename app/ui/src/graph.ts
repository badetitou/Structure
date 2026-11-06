import type { CellEdge } from "./api";

/** Forward and backward neighbour lists, built once per matrix. */
export interface Adjacency {
  forward: Map<string, string[]>;
  backward: Map<string, string[]>;
}

export function adjacency(edges: readonly CellEdge[]): Adjacency {
  const forward = new Map<string, string[]>();
  const backward = new Map<string, string[]>();
  const add = (m: Map<string, string[]>, a: string, b: string) => {
    const l = m.get(a);
    if (l) l.push(b);
    else m.set(a, [b]);
  };
  for (const e of edges) {
    add(forward, e.from, e.to);
    add(backward, e.to, e.from);
  }
  return { forward, backward };
}

/** Cells reachable from `start` following edges forward (or backward), in O(V + E). Includes `start` only if it is reachable from itself via a cycle. */
export function reach(start: string, adj: Adjacency, dir: "forward" | "backward"): Set<string> {
  const next = adj[dir];
  const seen = new Set<string>();
  // The queue is only appended to and read through a head index: no shift().
  const queue = [start];
  for (let head = 0; head < queue.length; head++) {
    for (const n of next.get(queue[head] as string) ?? []) {
      if (!seen.has(n)) {
        seen.add(n);
        queue.push(n);
      }
    }
  }
  return seen;
}
