// Aligned view of a multi-project matrix: one layer segment per project and one
// domain row per group of columns linked by remote (http / rpc) edges, so a front
// domain and the back domain it calls sit on the same row.
import type { CellEdge, LayerDef, LayerId } from "./api";
import type { Column, ViewCell, ViewMatrix } from "./domaintree";

/** One row of the aligned domain axis: at most one column per project. */
export interface AlignedRow {
  key: string;
  /** Member columns, primary project first. */
  members: Column[];
}

/** One layer of the aligned layer axis: a layer of one project. */
export interface AlignedLayer {
  /** `${project}|${layer}` */
  key: string;
  project: string;
  layer: LayerId;
}

export interface Aligned {
  vm: ViewMatrix;
  rows: AlignedRow[];
  layers: AlignedLayer[];
  /** Layer indices where a new segment starts (the first segment's start is not listed). */
  segments: number[];
  /** Name of the caller project (most outgoing remote weight); its segment comes first. */
  primary: string;
}

const SEP = "|";

/** Layer name of a view cell's layer, aligned (`project|layer`) or plain. */
export function layerName(key: string): string {
  const i = key.lastIndexOf(SEP);
  return i < 0 ? key : key.slice(i + 1);
}

/** Label of a view cell's layer (aligned or plain) from the matrix definitions; the id when undefined. */
export function layerLabel(defs: readonly LayerDef[] | undefined, key: string): string {
  const id = layerName(key);
  return defs?.find((d) => d.id === id)?.label ?? id;
}

/** Project owning a column: the prefix before the first `/` of its first domain. */
export function projectOf(col: Column, projects: readonly string[]): number {
  const d = col.domains[0] ?? col.key;
  const i = projects.indexOf(d.split("/")[0] ?? "");
  return i < 0 ? 0 : i;
}

export function align(
  vm: ViewMatrix,
  cols: readonly Column[],
  layers: readonly LayerId[],
  projects: readonly string[],
  remote: readonly CellEdge[],
): Aligned {
  const colByKey = new Map(cols.map((c) => [c.key, c]));
  const projOf = new Map(cols.map((c) => [c.key, projectOf(c, projects)]));
  const colOfCell = new Map(vm.cells.map((c) => [c.id, c.domain]));

  // Weights between columns of different projects, and outgoing weight per project.
  const pairs = new Map<string, { a: string; b: string; w: number }>();
  const outgoing = projects.map(() => 0);
  for (const e of remote) {
    const a = colOfCell.get(e.from);
    const b = colOfCell.get(e.to);
    if (a === undefined || b === undefined) continue;
    const pa = projOf.get(a);
    const pb = projOf.get(b);
    if (pa === undefined || pb === undefined || pa === pb) continue;
    outgoing[pa] = (outgoing[pa] ?? 0) + e.weight;
    const k = `${a}\u0000${b}`;
    const p = pairs.get(k);
    if (p) p.w += e.weight;
    else pairs.set(k, { a, b, w: e.weight });
  }
  let primary = 0;
  outgoing.forEach((w, i) => {
    if (w > (outgoing[primary] ?? 0)) primary = i;
  });
  const order = projects.map((_, i) => i).sort((x, y) => (x === primary ? -1 : y === primary ? 1 : x - y));

  // Pairing: heaviest link first; join when the project slot is free.
  const groupOf = new Map<string, Map<number, string>>();
  const sorted = [...pairs.values()].sort(
    (x, y) => y.w - x.w || x.a.localeCompare(y.a) || x.b.localeCompare(y.b),
  );
  for (const { a, b } of sorted) {
    const ga = groupOf.get(a);
    const gb = groupOf.get(b);
    const pa = projOf.get(a) ?? 0;
    const pb = projOf.get(b) ?? 0;
    if (!ga && !gb) {
      const g = new Map([
        [pa, a],
        [pb, b],
      ]);
      groupOf.set(a, g);
      groupOf.set(b, g);
    } else if (ga && !gb && !ga.has(pb)) {
      ga.set(pb, b);
      groupOf.set(b, ga);
    } else if (gb && !ga && !gb.has(pa)) {
      gb.set(pa, a);
      groupOf.set(a, gb);
    }
  }

  // Rows: primary project's column order first, then the rest.
  const rows: AlignedRow[] = [];
  const rowOf = new Map<string, AlignedRow>();
  const addRow = (col: Column) => {
    if (rowOf.has(col.key)) return;
    const g = groupOf.get(col.key);
    const members = (g ? order.flatMap((p) => (g.has(p) ? [g.get(p) as string] : [])) : [col.key]).flatMap((k) => {
      const c = colByKey.get(k);
      return c ? [c] : [];
    });
    const row: AlignedRow = { key: members.map((m) => m.key).join("~"), members };
    rows.push(row);
    for (const m of members) rowOf.set(m.key, row);
  };
  for (const c of cols) if (projOf.get(c.key) === primary) addRow(c);
  for (const c of cols) addRow(c);

  // Layers: per project (primary first), the layers it uses, in global order.
  const used = new Set(vm.cells.map((c) => `${projOf.get(c.domain) ?? 0}${SEP}${c.layer}`));
  const alayers: AlignedLayer[] = [];
  const segments: number[] = [];
  for (const p of order) {
    const mine = layers.filter((l) => used.has(`${p}${SEP}${l}`));
    if (mine.length === 0) continue;
    if (alayers.length > 0) segments.push(alayers.length);
    const project = projects[p] ?? "";
    for (const layer of mine) alayers.push({ key: `${project}${SEP}${layer}`, project, layer });
  }

  // Re-key cells and edges; the mapping is one to one, so edges are only renamed.
  const idMap = new Map<string, string>();
  const cells: ViewCell[] = [];
  for (const c of vm.cells) {
    const p = projOf.get(c.domain) ?? 0;
    const row = rowOf.get(c.domain);
    if (!row) continue;
    const layer: LayerId = `${projects[p] ?? ""}${SEP}${c.layer}`;
    const id = `${layer}:${row.key}`;
    idMap.set(c.id, id);
    cells.push({ ...c, id, layer, domain: row.key });
  }
  const rename = (list: readonly CellEdge[]) =>
    list.flatMap((e) => {
      const from = idMap.get(e.from);
      const to = idMap.get(e.to);
      return from !== undefined && to !== undefined ? [{ ...e, from, to }] : [];
    });
  const cellMap = new Map<string, string>();
  for (const [orig, id] of vm.cellMap) {
    const next = idMap.get(id);
    if (next !== undefined) cellMap.set(orig, next);
  }
  return {
    vm: {
      columns: vm.columns,
      cells,
      edges: rename(vm.edges),
      httpEdges: rename(vm.httpEdges),
      rpcEdges: rename(vm.rpcEdges),
      cellMap,
    },
    rows,
    layers: alayers,
    segments,
    primary: projects[primary] ?? "",
  };
}

/** Whether the aligned view applies: several projects and at least one remote edge. */
export function canAlign(projects: readonly unknown[] | undefined, http: readonly unknown[], rpc: readonly unknown[]): boolean {
  return (projects?.length ?? 0) >= 2 && http.length + rpc.length > 0;
}
