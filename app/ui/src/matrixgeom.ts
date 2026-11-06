import type { Tile } from "./layout";

/** Quadratic path between two tile centres, stopping at the (approximate) square border so arrowheads stay visible. */
export function edgePath(a: Tile, b: Tile): string {
  const x1 = a.x + a.w / 2;
  const y1 = a.y + a.h / 2;
  const x2 = b.x + b.w / 2;
  const y2 = b.y + b.h / 2;
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = Math.hypot(dx, dy) || 1;
  const sx = x1 + (dx / len) * (a.w / 2 + 2);
  const sy = y1 + (dy / len) * (a.w / 2 + 2);
  const ex = x2 - (dx / len) * (b.w / 2 + 2);
  const ey = y2 - (dy / len) * (b.w / 2 + 2);
  const bend = 0.15 * len;
  const cx = (sx + ex) / 2 - (dy / len) * bend;
  const cy = (sy + ey) / 2 + (dx / len) * bend;
  return `M${sx},${sy} Q${cx},${cy} ${ex},${ey}`;
}

/** `text` cut with an ellipsis to fit `width` px at about `charW` px per glyph. */
export function fitText(text: string, width: number, charW: number): string {
  const n = Math.floor(width / charW);
  if (text.length <= n) return text;
  return n < 2 ? "" : `${text.slice(0, n - 1)}…`;
}

