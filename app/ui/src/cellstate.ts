export const CHANGED = "var(--changed)";
export const DEFAULT_FILL = "var(--cell)";

/** distance 1 = strongest step of the `--impact-*` token ramp, fading to step 5. */
export function impactColor(distance: number): string {
  if (!Number.isFinite(distance)) return "var(--impact-5)";
  const step = Math.min(Math.max(Math.round(distance), 1), 5);
  return `var(--impact-${step})`;
}

export interface CellPaint {
  fill: string;
  opacity: number;
}

import type { ImpactView } from "./domaintree";

export const FWD = "var(--fwd)";
export const BWD = "var(--bwd)";

/** What the hovered cell reaches (`fwd`) and is reached by (`bwd`). */
export interface Hover {
  id: string;
  fwd: ReadonlySet<string>;
  bwd: ReadonlySet<string>;
}

/** A cell while `h` is hovered: the hovered one, its reach, or dimmed. */
export function hoverCellPaint(id: string, h: Hover): CellPaint {
  if (id === h.id) return { fill: "var(--hover)", opacity: 1 };
  const f = h.fwd.has(id);
  const b = h.bwd.has(id);
  if (f && b) return { fill: "var(--both)", opacity: 1 };
  if (f) return { fill: FWD, opacity: 1 };
  if (b) return { fill: BWD, opacity: 1 };
  return { fill: DEFAULT_FILL, opacity: 0.25 };
}

/** How an edge stands out; `def` edges are only dimmed (by their container) while hovering / showing impact. */
export type EdgeKind = "def" | "fwd" | "bwd" | "imp";

/** Edge from -> to: hover colouring beats impact colouring beats the default. */
export function edgeKind(
  from: string,
  to: string,
  hover: Hover | null,
  impact: ImpactView | null,
): EdgeKind {
  if (hover) {
    const inF = (id: string) => id === hover.id || hover.fwd.has(id);
    const inB = (id: string) => id === hover.id || hover.bwd.has(id);
    if (inF(from) && hover.fwd.has(to)) return "fwd";
    if (inB(to) && hover.bwd.has(from)) return "bwd";
    return "def";
  }
  if (impact) {
    const on = (id: string) => impact.changed.has(id) || impact.impacted.has(id);
    if (on(from) && on(to)) return "imp";
  }
  return "def";
}

/** Opacity of default edges: dimmed while hovering or showing impact (set once on their container). */
export function defaultEdgeOpacity(hovering: boolean, impacting: boolean): number {
  return hovering ? 0.1 : impacting ? 0.12 : 0.5;
}

export interface EdgeStyle {
  stroke: string;
  /** Id of the arrowhead marker. */
  marker: string;
  /** CSS opacity; default edges take their container's `--edge-opacity`. */
  opacity: string;
  width: number;
}

export function edgeStyle(kind: EdgeKind, weight: number, http = false): EdgeStyle {
  const width = Math.max(1, 1 + Math.log(weight));
  switch (kind) {
    case "fwd":
      return { stroke: FWD, marker: "a-fwd", opacity: "0.95", width };
    case "bwd":
      return { stroke: BWD, marker: "a-bwd", opacity: "0.95", width };
    case "imp":
      return { stroke: CHANGED, marker: "a-imp", opacity: "0.9", width };
    default:
      if (http) return { stroke: "var(--edge-http)", marker: "a-http", opacity: "var(--edge-opacity)", width };
      return { stroke: "var(--edge)", marker: "a-def", opacity: "var(--edge-opacity)", width };
  }
}

/** Impact-mode state of a cell: changed beats impacted beats dimmed. */
export function impactCellPaint(
  id: string,
  changed: ReadonlySet<string>,
  impacted: ReadonlyMap<string, number>,
): CellPaint {
  if (changed.has(id)) return { fill: CHANGED, opacity: 1 };
  const d = impacted.get(id);
  if (d !== undefined) return { fill: impactColor(d), opacity: 1 };
  return { fill: DEFAULT_FILL, opacity: 0.25 };
}
