/** Pure viewBox maths for zooming and panning the matrix SVG.
 *
 * The svg uses preserveAspectRatio "xMidYMid meet" and its viewBox always has
 * the pane's aspect ratio, so screen → user is a plain scale of `vb.w / paneW`. */
export interface ViewBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

export const MIN_ZOOM = 0.1;
export const MAX_ZOOM = 10;

/** Smallest viewBox of the pane's aspect that contains the content, centred. */
export function fit(contentW: number, contentH: number, paneW: number, paneH: number): ViewBox {
  const cw = Math.max(contentW, 1);
  const ch = Math.max(contentH, 1);
  const pw = Math.max(paneW, 1);
  const ph = Math.max(paneH, 1);
  const k = Math.max(cw / pw, ch / ph);
  const w = pw * k;
  const h = ph * k;
  return { x: (cw - w) / 2, y: (ch - h) / 2, w, h };
}

/** 1 user unit = 1 screen pixel, content top-left at `margin` from the pane corner. */
export function oneToOne(paneW: number, paneH: number, margin = 16): ViewBox {
  return { x: -margin, y: -margin, w: Math.max(paneW, 1), h: Math.max(paneH, 1) };
}

/** Screen point (px relative to the pane) → user coordinates. */
export function toUser(vb: ViewBox, sx: number, sy: number, paneW: number, paneH: number) {
  return { x: vb.x + (sx / Math.max(paneW, 1)) * vb.w, y: vb.y + (sy / Math.max(paneH, 1)) * vb.h };
}

/** Zoom by `factor` (>1 zooms in) keeping user point (px, py) at the same screen spot. */
export function zoomAt(vb: ViewBox, factor: number, px: number, py: number): ViewBox {
  if (!(factor > 0) || !Number.isFinite(factor)) return vb;
  return { x: px - (px - vb.x) / factor, y: py - (py - vb.y) / factor, w: vb.w / factor, h: vb.h / factor };
}

/** Zoom level relative to the fit view (1 = fit). */
export function zoomLevel(vb: ViewBox, fitVb: ViewBox): number {
  return fitVb.w / vb.w;
}

/** Zoom keeping (px, py) fixed, with the resulting level clamped to [MIN_ZOOM, MAX_ZOOM] of fit. */
export function zoomAtClamped(vb: ViewBox, factor: number, px: number, py: number, fitVb: ViewBox): ViewBox {
  const level = zoomLevel(vb, fitVb);
  if ((factor > 1 && level >= MAX_ZOOM) || (factor < 1 && level <= MIN_ZOOM)) return vb;
  // Clamp only in the direction of travel: an out-of-range start may move back
  // toward the range without snapping to the opposite bound.
  const target = factor > 1 ? Math.min(MAX_ZOOM, level * factor) : Math.max(MIN_ZOOM, level * factor);
  return zoomAt(vb, target / level, px, py);
}

/** True when `vb` is (within a small tolerance) the fit view. */
export function isFit(vb: ViewBox, fitVb: ViewBox, eps = 1e-6): boolean {
  const tol = eps * Math.max(fitVb.w, 1);
  return (
    Math.abs(vb.x - fitVb.x) <= tol &&
    Math.abs(vb.y - fitVb.y) <= tol &&
    Math.abs(vb.w - fitVb.w) <= tol &&
    Math.abs(vb.h - fitVb.h) <= tol
  );
}

/** Move the view by a screen delta (content follows the pointer). */
export function panBy(vb: ViewBox, dx: number, dy: number, paneW: number, paneH: number): ViewBox {
  return { ...vb, x: vb.x - (dx * vb.w) / Math.max(paneW, 1), y: vb.y - (dy * vb.h) / Math.max(paneH, 1) };
}

/** Pane resized: keep the scale (user units per pixel) and the top-left corner. */
export function resize(vb: ViewBox, oldW: number, newW: number, newH: number): ViewBox {
  const k = vb.w / Math.max(oldW, 1);
  return { x: vb.x, y: vb.y, w: Math.max(newW, 1) * k, h: Math.max(newH, 1) * k };
}

export const viewBoxAttr = (vb: ViewBox) => `${vb.x} ${vb.y} ${vb.w} ${vb.h}`;
