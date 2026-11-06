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
