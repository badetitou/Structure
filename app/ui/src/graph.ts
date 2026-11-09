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

/** Kind of a remote (cross-process) link. */
export type RemoteKind = "http" | "rpc";

/** Edges that take part in hover reach: code edges, plus HTTP / RPC links while they are shown. */
export function visibleEdges(
  edges: readonly CellEdge[],
  http: readonly CellEdge[],
  showHttp: boolean,
  rpc: readonly CellEdge[] = [],
  showRpc = false,
): readonly CellEdge[] {
  const extra = [...(showHttp ? http : []), ...(showRpc ? rpc : [])];
  return extra.length > 0 ? [...edges, ...extra] : edges;
}

export interface RemoteLink {
  /** The cell at the other end. */
  other: string;
  count: number;
}

/** Remote (HTTP or RPC) links of a (view) cell given its member cell ids: outgoing (callers) and incoming (handlers), counts summed per other cell. */
export function remoteLinks(
  links: readonly CellEdge[],
  members: readonly string[],
): { out: RemoteLink[]; in: RemoteLink[] } {
  const mine = new Set(members);
  const out = new Map<string, number>();
  const inc = new Map<string, number>();
  const bump = (m: Map<string, number>, k: string, n: number) => m.set(k, (m.get(k) ?? 0) + n);
  for (const e of links) {
    if (mine.has(e.from) && !mine.has(e.to)) bump(out, e.to, e.weight);
    else if (mine.has(e.to) && !mine.has(e.from)) bump(inc, e.from, e.weight);
  }
  const list = (m: Map<string, number>) =>
    [...m].map(([other, count]) => ({ other, count })).sort((a, b) => b.count - a.count || a.other.localeCompare(b.other));
  return { out: list(out), in: list(inc) };
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
