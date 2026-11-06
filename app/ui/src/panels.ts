export const PANELS_KEY = "structure.panels";
export const MIN_WIDTH = 160;
export const MIN_MATRIX_WIDTH = 200;
export const HANDLE_WIDTH = 5;
export const COLLAPSED_WIDTH = 24;

export type PanelState = { width: number; collapsed: boolean };
export type PanelsState = { left: PanelState; right: PanelState };

export const DEFAULT_PANELS: PanelsState = {
  left: { width: 260, collapsed: false },
  right: { width: 340, collapsed: false },
};

/**
 * Clamp a panel width to [MIN_WIDTH, 60% of the viewport], and so that the matrix
 * keeps MIN_MATRIX_WIDTH next to the other panel (`other` = its current width).
 */
export function clampWidth(w: number, fallback: number, viewport: number, other = 0): number {
  if (!Number.isFinite(w)) return fallback;
  const room = viewport - other - 2 * HANDLE_WIDTH - MIN_MATRIX_WIDTH;
  const max = Math.max(MIN_WIDTH, Math.floor(Math.min(0.6 * viewport, room)));
  return Math.min(max, Math.max(MIN_WIDTH, Math.round(w)));
}

function parsePanel(raw: unknown, def: PanelState): PanelState {
  if (typeof raw !== "object" || raw === null) return { ...def };
  const r = raw as Record<string, unknown>;
  return {
    width: typeof r.width === "number" ? clampWidth(r.width, def.width, Infinity) : def.width,
    collapsed: r.collapsed === true,
  };
}

/** Stored JSON → state; anything missing or invalid falls back to defaults. */
export function parsePanels(raw: string | null): PanelsState {
  let v: unknown = null;
  try {
    v = raw === null ? null : JSON.parse(raw);
  } catch {
    v = null;
  }
  const o = typeof v === "object" && v !== null ? (v as Record<string, unknown>) : {};
  return {
    left: parsePanel(o.left, DEFAULT_PANELS.left),
    right: parsePanel(o.right, DEFAULT_PANELS.right),
  };
}

export function loadPanels(): PanelsState {
  try {
    return parsePanels(localStorage.getItem(PANELS_KEY));
  } catch {
    return parsePanels(null);
  }
}

export function savePanels(state: PanelsState) {
  try {
    localStorage.setItem(PANELS_KEY, JSON.stringify(state));
  } catch {
    // storage unavailable: the layout just is not persisted
  }
}
