// Pure logic for collapsible domain columns: which tree nodes are visible
// columns, and the matrix aggregated onto those columns.
import type { Cell, CellEdge, DomainNode, Impact, Matrix } from "./api";

export const MAX_COLUMNS = 12;
/** Flame view: columns are as wide as their size, so the tree opens much deeper. */
export const FLAME_MAX_COLUMNS = 400;

export interface Column {
  /** Leaf: the domain name. Group: `g:` + index path (`g:0/2`), unique even for same-label siblings. */
  key: string;
  label: string;
  group: boolean;
  /** Domains (leaves) aggregated in this column. */
  domains: string[];
}

/** Header band above the columns of an expanded group. */
export interface Band {
  key: string;
  label: string;
  /** Index of the first column, number of columns, nesting depth (0 = top). */
  start: number;
  span: number;
  depth: number;
}

const isGroup = (n: DomainNode) => n.children.length > 0;

function nodeKey(n: DomainNode, path: string): string {
  return isGroup(n) ? `g:${path}` : (n.domain ?? path);
}
const childPath = (path: string, i: number) => `${path}/${i}`;

function leavesOf(n: DomainNode): string[] {
  if (!isGroup(n)) return n.domain === undefined ? [] : [n.domain];
  return n.children.flatMap(leavesOf);
}

/** Breadth-first expansion: expand groups while the column count stays <= max. */
export function initialExpanded(tree: DomainNode[], max = MAX_COLUMNS): Set<string> {
  const expanded = new Set<string>();
  let count = tree.length;
  let level: { n: DomainNode; path: string }[] = tree.map((n, i) => ({ n, path: String(i) }));
  while (level.length > 0) {
    const next: typeof level = [];
    for (const { n, path } of level) {
      if (!isGroup(n)) continue;
      if (count - 1 + n.children.length > max) continue;
      count += n.children.length - 1;
      expanded.add(`g:${path}`);
      n.children.forEach((c, i) => next.push({ n: c, path: childPath(path, i) }));
    }
    level = next;
  }
  return expanded;
}

/** Group keys (`g:<index path>`) by label path; same-label siblings are told apart by rank. */
function groupKeysByLabel(tree: DomainNode[]): Map<string, string> {
  const out = new Map<string, string>();
  const walk = (nodes: DomainNode[], path: string | null, labels: string) => {
    const seen = new Map<string, number>();
    nodes.forEach((n, i) => {
      if (!isGroup(n)) return;
      const rank = seen.get(n.label) ?? 0;
      seen.set(n.label, rank + 1);
      const p = path === null ? String(i) : childPath(path, i);
      const l = `${labels}/${n.label}#${rank}`;
      out.set(l, `g:${p}`);
      walk(n.children, p, l);
    });
  };
  walk(tree, null, "");
  return out;
}

/** The expanded groups of `from` carried over to `to` (e.g. another commit's tree) by label path:
 * groups that still exist stay open, wherever their index moved. */
export function remapExpanded(from: DomainNode[], to: DomainNode[], expanded: ReadonlySet<string>): Set<string> {
  if (from === to) return new Set(expanded);
  const target = groupKeysByLabel(to);
  const out = new Set<string>();
  for (const [labels, key] of groupKeysByLabel(from)) {
    const k = target.get(labels);
    if (k && expanded.has(key)) out.add(k);
  }
  return out;
}

/** Visible columns, left to right, for the given expanded group keys. */
export function columns(tree: DomainNode[], expanded: ReadonlySet<string>): Column[] {
  const out: Column[] = [];
  const walk = (n: DomainNode, path: string) => {
    const key = nodeKey(n, path);
    if (isGroup(n) && expanded.has(key)) {
      n.children.forEach((c, i) => walk(c, childPath(path, i)));
    } else {
      out.push({ key, label: n.label, group: isGroup(n), domains: leavesOf(n) });
    }
  };
  tree.forEach((n, i) => walk(n, String(i)));
  return out;
}

/** Bands of the expanded groups, with their column range (for collapse headers). */
export function bands(tree: DomainNode[], expanded: ReadonlySet<string>): Band[] {
  const out: Band[] = [];
  let idx = 0;
  const walk = (n: DomainNode, path: string, depth: number) => {
    const key = nodeKey(n, path);
    if (isGroup(n) && expanded.has(key)) {
      const band: Band = { key, label: n.label, start: idx, span: 0, depth };
      out.push(band);
      n.children.forEach((c, i) => walk(c, childPath(path, i), depth + 1));
      band.span = idx - band.start;
    } else idx++;
  };
  tree.forEach((n, i) => walk(n, String(i), 0));
  return out;
}

/** Plain columns when there is no tree: one leaf per domain. */
export function flatColumns(domains: string[]): Column[] {
  return domains.map((d) => ({ key: d, label: d, group: false, domains: [d] }));
}

export interface ViewCell extends Cell {
  /** Column label (equals `domain` for leaf columns). */
  label: string;
  /** Ids of the matrix cells merged into this one. */
  members: string[];
}
export interface ViewMatrix {
  columns: Column[];
  cells: ViewCell[];
  edges: CellEdge[];
  /** HTTP links, re-mapped like `edges`. */
  httpEdges: CellEdge[];
  /** RPC links, re-mapped like `edges`. */
  rpcEdges: CellEdge[];
  /** Original cell id -> view cell id. */
  cellMap: Map<string, string>;
}

/** Aggregate a matrix onto columns: sizes summed, members listed, edges re-mapped. */
export function aggregate(m: Matrix, cols: Column[]): ViewMatrix {
  const colOf = new Map<string, Column>();
  for (const c of cols) for (const d of c.domains) colOf.set(d, c);
  const cells = new Map<string, ViewCell>();
  const cellMap = new Map<string, string>();
  for (const c of m.cells) {
    const col = colOf.get(c.domain);
    if (!col) continue;
    const id = `${c.layer}:${col.key}`;
    cellMap.set(c.id, id);
    let v = cells.get(id);
    if (!v) {
      v = { id, layer: c.layer, domain: col.key, label: col.label, size: 0, members: [] };
      cells.set(id, v);
    }
    v.size += c.size;
    v.members.push(c.id);
  }
  const remap = (list: readonly CellEdge[]) => {
    const edges = new Map<string, CellEdge>();
    for (const e of list) {
      const from = cellMap.get(e.from);
      const to = cellMap.get(e.to);
      if (from === undefined || to === undefined || from === to) continue;
      const k = `${from}\u0000${to}`;
      const prev = edges.get(k);
      if (prev) prev.weight += e.weight;
      else edges.set(k, { from, to, weight: e.weight });
    }
    return [...edges.values()];
  };
  return {
    columns: cols,
    cells: [...cells.values()],
    edges: remap(m.edges),
    httpEdges: remap(m.httpEdges),
    rpcEdges: remap(m.rpcEdges),
    cellMap,
  };
}

/** Map impact cell ids onto view cells: changed if any leaf, min distance. */
export function aggregateImpact(
  impact: Impact | null,
  cellMap: ReadonlyMap<string, string>,
): { changed: Set<string>; impacted: Map<string, number> } {
  const changed = new Set<string>();
  const impacted = new Map<string, number>();
  if (!impact) return { changed, impacted };
  for (const id of impact.changed_cells) {
    const v = cellMap.get(id);
    if (v !== undefined) changed.add(v);
  }
  for (const x of impact.impacted_cells) {
    const v = cellMap.get(x.id);
    if (v === undefined) continue;
    const d = impacted.get(v);
    if (d === undefined || x.distance < d) impacted.set(v, x.distance);
  }
  return { changed, impacted };
}

/** Impact mapped onto view cells (see `aggregateImpact`). */
export interface ImpactView {
  changed: ReadonlySet<string>;
  impacted: ReadonlyMap<string, number>;
}
