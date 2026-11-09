// The workspace layout: a split tree whose leaves hold panel ids (see registry.ts).
// Pure functions only: every operation returns a new tree.

export type Dir = "row" | "column";
/** A tile: one or more panels, `active` is the one shown. */
export type Leaf = { kind: "leaf"; id: string; panels: string[]; active: string };
/** Children side by side (`row`) or stacked (`column`); `sizes` are fractions summing to 1. */
export type Split = { kind: "split"; id: string; dir: Dir; children: Node[]; sizes: number[] };
export type Node = Leaf | Split;

export const LAYOUT_KEY = "structure.layout";
/** Smallest a tile can be dragged to, in pixels. */
export const MIN_TILE = 120;

export const leaf = (id: string, panels: string[], active = panels[0] ?? ""): Leaf => ({
  kind: "leaf",
  id,
  panels,
  active,
});
export const split = (id: string, dir: Dir, children: Node[], sizes?: number[]): Split => ({
  kind: "split",
  id,
  dir,
  children,
  sizes: sizes ?? children.map(() => 1 / children.length),
});

/** History | matrix | details, as before the workspace existed. */
export const DEFAULT_LAYOUT: Node = split(
  "root",
  "row",
  [leaf("t-history", ["history"]), leaf("t-matrix", ["matrix"]), leaf("t-details", ["details"])],
  [0.2, 0.55, 0.25],
);

/** Sizes scaled to sum to 1; non-positive or non-finite ones get an equal share first. */
export function normalizeSizes(sizes: number[]): number[] {
  if (sizes.length === 0) return [];
  const ok = sizes.map((s) => (Number.isFinite(s) && s > 0 ? s : 0));
  const sum = ok.reduce((a, b) => a + b, 0);
  if (sum <= 0) return sizes.map(() => 1 / sizes.length);
  const fill = sum / ok.filter((s) => s > 0).length;
  const filled = ok.map((s) => (s > 0 ? s : fill));
  const total = filled.reduce((a, b) => a + b, 0);
  return filled.map((s) => s / total);
}

/**
 * Canonical form: empty leaves removed, a valid `active`, single-child splits replaced
 * by their child, a split nested in one of the same direction flattened into it,
 * sizes normalised. Returns null when nothing is left.
 */
export function normalize(node: Node): Node | null {
  if (node.kind === "leaf") {
    const panels = [...new Set(node.panels)];
    if (panels.length === 0) return null;
    return { ...node, panels, active: panels.includes(node.active) ? node.active : panels[0] };
  }
  const kids: Node[] = [];
  const sizes: number[] = [];
  const own = normalizeSizes(node.sizes.length === node.children.length ? node.sizes : node.children.map(() => 1));
  node.children.forEach((c, i) => {
    const n = normalize(c);
    if (!n) return;
    if (n.kind === "split" && n.dir === node.dir) {
      n.children.forEach((g, j) => {
        kids.push(g);
        sizes.push(own[i] * n.sizes[j]);
      });
    } else {
      kids.push(n);
      sizes.push(own[i]);
    }
  });
  if (kids.length === 0) return null;
  if (kids.length === 1) return kids[0];
  return { ...node, children: kids, sizes: normalizeSizes(sizes) };
}

/**
 * The tree as shown: panels for which `available` is false are hidden, tiles left
 * empty take no room. Ids are kept so a resize on the shown tree applies to the stored one.
 */
export function visible(node: Node, available: (panel: string) => boolean): Node | null {
  if (node.kind === "leaf") {
    const panels = node.panels.filter(available);
    return panels.length ? { ...node, panels, active: panels.includes(node.active) ? node.active : panels[0] } : null;
  }
  const kids: Node[] = [];
  const sizes: number[] = [];
  node.children.forEach((c, i) => {
    const v = visible(c, available);
    if (v) {
      kids.push(v);
      sizes.push(node.sizes[i] ?? 0);
    }
  });
  if (kids.length === 0) return null;
  if (kids.length === 1) return kids[0];
  return { ...node, children: kids, sizes: normalizeSizes(sizes) };
}

/**
 * Moves the boundary between two children `a` and `b` of split `splitId`: `a` gets
 * `fraction` of their combined size, `b` the rest; other children are untouched.
 */
export function resizePair(node: Node, splitId: string, a: string, b: string, fraction: number): Node {
  if (node.kind === "leaf") return node;
  if (node.id !== splitId) {
    const children = node.children.map((c) => resizePair(c, splitId, a, b, fraction));
    return children.every((c, i) => c === node.children[i]) ? node : { ...node, children };
  }
  const ia = node.children.findIndex((c) => c.id === a);
  const ib = node.children.findIndex((c) => c.id === b);
  if (ia < 0 || ib < 0 || ia === ib || !Number.isFinite(fraction)) return node;
  const f = Math.min(1, Math.max(0, fraction));
  const pair = node.sizes[ia] + node.sizes[ib];
  const sizes = [...node.sizes];
  sizes[ia] = pair * f;
  sizes[ib] = pair * (1 - f);
  return { ...node, sizes };
}

/** Every panel id placed in the tree, in order. */
export function panelsOf(node: Node): string[] {
  return node.kind === "leaf" ? [...node.panels] : node.children.flatMap(panelsOf);
}

function parseNode(v: unknown, known: (panel: string) => boolean, ids: Set<string>): Node | null {
  if (typeof v !== "object" || v === null) return null;
  const o = v as Record<string, unknown>;
  if (typeof o.id !== "string" || ids.has(o.id)) return null;
  ids.add(o.id);
  if (o.kind === "leaf") {
    if (!Array.isArray(o.panels)) return null;
    const panels = o.panels.filter((p): p is string => typeof p === "string" && known(p));
    return leaf(o.id, panels, typeof o.active === "string" ? o.active : "");
  }
  if (o.kind === "split") {
    if ((o.dir !== "row" && o.dir !== "column") || !Array.isArray(o.children)) return null;
    const children: Node[] = [];
    for (const c of o.children) {
      const n = parseNode(c, known, ids);
      if (!n) return null;
      children.push(n);
    }
    const raw = Array.isArray(o.sizes) ? o.sizes : [];
    const sizes = children.map((_, i) => (typeof raw[i] === "number" ? (raw[i] as number) : 0));
    return split(o.id, o.dir, children, sizes);
  }
  return null;
}

/**
 * Stored JSON → layout. Unknown panels are dropped (a plugin that went away);
 * anything malformed, or a tree left empty, falls back to `fallback`.
 */
export function parseLayout(raw: string | null, known: (panel: string) => boolean, fallback = DEFAULT_LAYOUT): Node {
  let v: unknown = null;
  try {
    v = raw === null ? null : JSON.parse(raw);
  } catch {
    return fallback;
  }
  const n = parseNode(v, known, new Set());
  return (n && normalize(n)) ?? fallback;
}

export function loadLayout(known: (panel: string) => boolean): Node {
  try {
    return parseLayout(localStorage.getItem(LAYOUT_KEY), known);
  } catch {
    return DEFAULT_LAYOUT;
  }
}

export function saveLayout(node: Node) {
  try {
    localStorage.setItem(LAYOUT_KEY, JSON.stringify(node));
  } catch {
    // storage unavailable: the layout just is not persisted
  }
}

/**
 * `a`'s new share of the pair after dragging their boundary by `delta` pixels, given
 * their current pixel sizes; each keeps at least `min` (or half the pair when smaller).
 */
export function dragFraction(aPx: number, bPx: number, delta: number, min = MIN_TILE): number {
  const total = aPx + bPx;
  if (!(total > 0)) return 0.5;
  const lo = Math.min(min, total / 2);
  const a = Math.min(total - lo, Math.max(lo, aPx + delta));
  return a / total;
}
