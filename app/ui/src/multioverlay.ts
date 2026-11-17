// Several highlight overlays drawn together on the working-tree matrix (pure; `MatrixCell`,
// `DetailsPanel` and `StatusBar` render it). Each overlay keeps its colour: a cell hit by k
// selected overlays (k <= MAX_BANDS) is split into k equal bands, above that it takes a neutral
// "multiple" style; changed reads as a solid band with a marker, impacted as a faded one.
import type { Impact, Matrix } from "./api";
import { aggregateImpact } from "./domaintree";
import type { ImpactView } from "./domaintree";

/** Most bands in one cell; more overlays hitting it show the "multiple" style. */
export const MAX_BANDS = 3;
/** Fill of a cell hit by more than `MAX_BANDS` overlays. */
export const MULTIPLE_FILL = "var(--overlay-multiple)";

/** What the multi-overlay rendering needs of an overlay. */
export interface OverlayRef {
  id: string;
  label: string;
  /** Theme token, e.g. `--overlay-2`. */
  colour: string;
  impact: Impact;
}

/** One overlay hitting a cell: changed there, or impacted at `distance`. */
export interface OverlayHit {
  id: string;
  label: string;
  colour: string;
  role: "changed" | "impacted";
  /** Call distance from a change; null when changed. */
  distance: number | null;
}

/** The overlays mapped onto view cells: the hits of each cell (in the overlays' order) and
 * their union (changed in any; impacted at the smallest distance), which drives the edges and
 * the dimming of the cells hit by none. */
export interface MultiView {
  hits: Map<string, OverlayHit[]>;
  union: ImpactView;
}

/** Maps every overlay of `overlays` onto the view cells (`cellMap`: matrix cell id -> view
 * cell id). Cells missing from `cellMap` are skipped (see `cellsNotIn`). */
export function overlayViews(overlays: readonly OverlayRef[], cellMap: ReadonlyMap<string, string>): MultiView {
  const hits = new Map<string, OverlayHit[]>();
  const changed = new Set<string>();
  const impacted = new Map<string, number>();
  const push = (cell: string, hit: OverlayHit) => {
    const list = hits.get(cell);
    if (list) list.push(hit);
    else hits.set(cell, [hit]);
  };
  for (const o of overlays) {
    const v = aggregateImpact(o.impact, cellMap);
    const base = { id: o.id, label: o.label, colour: o.colour };
    for (const c of v.changed) {
      push(c, { ...base, role: "changed", distance: null });
      changed.add(c);
    }
    for (const [c, d] of v.impacted) {
      if (v.changed.has(c)) continue;
      push(c, { ...base, role: "impacted", distance: d });
      const was = impacted.get(c);
      if (was === undefined || d < was) impacted.set(c, d);
    }
  }
  return { hits, union: { changed, impacted } };
}

/** Number of distinct cells hit by `impacts` (changed or impacted) that `matrix` does not
 * have: cells of a commit snapshot that the working tree lost. */
export function cellsNotIn(impacts: readonly Impact[], matrix: Matrix): number {
  const have = new Set(matrix.cells.map((c) => c.id));
  const missing = new Set<string>();
  for (const imp of impacts) {
    for (const id of imp.changed_cells) if (!have.has(id)) missing.add(id);
    for (const x of imp.impacted_cells) if (!have.has(x.id)) missing.add(x.id);
  }
  return missing.size;
}

/** Fill opacity of a band: solid when changed, fading with the distance when impacted
 * (kept well under the changed one so that the two read apart). */
export function hitOpacity(hit: OverlayHit): number {
  if (hit.role === "changed") return 1;
  const d = hit.distance === null || !Number.isFinite(hit.distance) ? 5 : Math.min(Math.max(Math.round(hit.distance), 1), 5);
  return [0.55, 0.47, 0.4, 0.34, 0.28][d - 1];
}

export interface Band {
  colour: string;
  opacity: number;
  /** Changed in this overlay: the band carries a marker. */
  changed: boolean;
}

/** How a cell looks in the multi-overlay view: hit by none (dimmed), split into bands, or
 * "multiple"; `changed` when one overlay at least changed it (the cell gets the changed outline). */
export type MultiPaint =
  | { kind: "none" }
  | { kind: "bands"; bands: Band[]; changed: boolean }
  | { kind: "multiple"; count: number; changed: boolean };

export function multiCellPaint(hits: readonly OverlayHit[] | undefined): MultiPaint {
  if (!hits || hits.length === 0) return { kind: "none" };
  const changed = hits.some((h) => h.role === "changed");
  if (hits.length > MAX_BANDS) return { kind: "multiple", count: hits.length, changed };
  return {
    kind: "bands",
    bands: hits.map((h) => ({ colour: `var(${h.colour})`, opacity: hitOpacity(h), changed: h.role === "changed" })),
    changed,
  };
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** `r` split into `k` side-by-side bands of equal width (none when k < 1). */
export function bandRects(r: Rect, k: number): Rect[] {
  if (k < 1) return [];
  const w = r.w / k;
  return Array.from({ length: k }, (_, i) => ({ x: r.x + i * w, y: r.y, w, h: r.h }));
}

/** The changed marker of a band: a strip along its top, a quarter of its height at most. */
export function markerRect(band: Rect): Rect {
  return { x: band.x, y: band.y, w: band.w, h: Math.min(3, band.h / 4) };
}

/** "label — changed" or "label — impacted, distance 2". */
export function hitText(hit: OverlayHit): string {
  return hit.role === "changed" ? `${hit.label} — changed` : `${hit.label} — impacted, distance ${hit.distance}`;
}

/** Tooltip of a cell: its label, then one line per overlay hitting it. */
export function hitsTitle(label: string, hits: readonly OverlayHit[] | undefined): string {
  if (!hits || hits.length === 0) return label;
  return [label, ...hits.map((h) => `• ${hitText(h)}`)].join("\n");
}

/** One row per overlay for the selected cell: how it hits the cell, or "not hit". */
export function cellOverlayRows(
  overlays: readonly OverlayRef[],
  hits: readonly OverlayHit[] | undefined,
): { id: string; label: string; colour: string; hit: boolean; text: string }[] {
  return overlays.map((o) => {
    const h = hits?.find((x) => x.id === o.id);
    const text = !h ? "not hit" : h.role === "changed" ? "changed" : `impacted, distance ${h.distance}`;
    return { id: o.id, label: o.label, colour: o.colour, hit: !!h, text };
  });
}
