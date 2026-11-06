import { setSvgAttr } from "./svgattr";
import type { LaneRow, Segment } from "./lanes";

export const ROW_H = 40;
export const LANE_W = 14;
const MID = ROW_H / 2;
const SVG = "http://www.w3.org/2000/svg";

const x = (lane: number) => lane * LANE_W + LANE_W / 2;

function el(tag: string, attrs: Record<string, string | number>): SVGElement {
  const e = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) setSvgAttr(e, k, v);
  return e;
}

/** Line between (x1,y1) and (x2,y2), as an S-curve when the lanes differ. */
function link(x1: number, y1: number, x2: number, y2: number, color: string): SVGElement {
  const my = (y1 + y2) / 2;
  const d =
    x1 === x2 ? `M${x1},${y1} L${x2},${y2}` : `M${x1},${y1} C${x1},${my} ${x2},${my} ${x2},${y2}`;
  return el("path", { d, stroke: color, fill: "none", "stroke-width": 2 });
}

/** One row of the commit graph: fixed `ROW_H` height so consecutive rows connect. */
export function rowSvg(
  row: LaneRow | null,
  width: number,
  opts: { hollow?: boolean; lineAbove?: string; lineBelow?: string } = {},
): SVGElement {
  const svg = el("svg", { width: Math.max(1, width) * LANE_W, height: ROW_H, class: "graph" });
  const lane = row?.lane ?? 0;
  const draw = (segs: Segment[], f: (s: Segment) => SVGElement) => {
    for (const sg of segs) svg.append(f(sg));
  };
  if (row) {
    draw(row.through, (s) => link(x(s.from), 0, x(s.to), ROW_H, s.color));
    draw(row.top, (s) => link(x(s.from), 0, x(s.to), MID, s.color));
    draw(row.bottom, (s) => link(x(s.from), MID, x(s.to), ROW_H, s.color));
  }
  if (opts.lineAbove) svg.append(link(x(lane), 0, x(lane), MID, opts.lineAbove));
  if (opts.lineBelow) svg.append(link(x(lane), MID, x(lane), ROW_H, opts.lineBelow));
  const color = row?.color ?? opts.lineBelow ?? "var(--edge)";
  svg.append(
    el("circle", {
      cx: x(lane),
      cy: MID,
      r: 4,
      stroke: color,
      "stroke-width": 2,
      fill: opts.hollow ? "var(--card)" : color,
    }),
  );
  return svg;
}
