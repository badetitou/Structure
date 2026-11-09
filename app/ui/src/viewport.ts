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
/** Zooming in has no product limit; it only stops before the view gets narrower
 * than float64 can resolve at its position (keeps the maths finite and exact). */
const MIN_REL_WIDTH = 1e-9;

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

/** True while `vb` is wide enough for float64 to tell its edges apart. */
function resolvable(vb: ViewBox): boolean {
  const scale = Math.max(Math.abs(vb.x), Math.abs(vb.y), Math.abs(vb.x + vb.w), Math.abs(vb.y + vb.h), 1);
  return vb.w > MIN_REL_WIDTH * scale && vb.h > MIN_REL_WIDTH * scale;
}

/** Zoom keeping (px, py) fixed. Zooming out stops at MIN_ZOOM of fit; zooming in
 * is unbounded (up to float precision, see MIN_REL_WIDTH). */
export function zoomAtClamped(vb: ViewBox, factor: number, px: number, py: number, fitVb: ViewBox): ViewBox {
  if (factor > 1) {
    const z = zoomAt(vb, factor, px, py);
    return resolvable(z) ? z : vb;
  }
  const level = zoomLevel(vb, fitVb);
  if (factor < 1 && level <= MIN_ZOOM) return vb;
  // Clamp only in the direction of travel: a start below the bound may move back
  // toward the range without snapping.
  const target = Math.max(MIN_ZOOM, level * factor);
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

/** Wheel zoom rates per pixel of `deltaY`: a pinch sends small deltas, a mouse
 * notch ~100 px (→ ~1.22× per notch). */
const PINCH_RATE = 0.01;
const WHEEL_RATE = 0.002;

/** Zoom factor (>1 zooms in) for a wheel event: scrolling up / pinching out zooms in.
 * `deltaMode` is the WheelEvent's (0 px, 1 line, 2 page); `pinch` is its ctrlKey. */
export function wheelZoomFactor(deltaY: number, deltaMode: number, pinch: boolean): number {
  const unit = deltaMode === 1 ? 16 : deltaMode === 2 ? 400 : 1;
  const f = Math.exp(-deltaY * unit * (pinch ? PINCH_RATE : WHEEL_RATE));
  return Number.isFinite(f) && f > 0 ? f : 1;
}

/** Pane resized: keep the scale (user units per pixel) and the top-left corner. */
export function resize(vb: ViewBox, oldW: number, newW: number, newH: number): ViewBox {
  const k = vb.w / Math.max(oldW, 1);
  return { x: vb.x, y: vb.y, w: Math.max(newW, 1) * k, h: Math.max(newH, 1) * k };
}

