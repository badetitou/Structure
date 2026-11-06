export const CHANGED = "#dc2626";
export const DEFAULT_FILL = "#cbd5e1";

/** distance 1 = darkest orange, fading with distance. */
export function impactColor(distance: number): string {
  const t = Math.min(Math.max(distance - 1, 0) / 4, 1);
  const l = 40 + t * 40;
  return `hsl(25 90% ${l}%)`;
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
