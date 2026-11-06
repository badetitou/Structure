export const PANELS_KEY = "structure.panels";
export const MIN_WIDTH = 160;
export const MAX_WIDTH = 700;
export const COLLAPSED_WIDTH = 24;

export type PanelState = { width: number; collapsed: boolean };
export type PanelsState = { left: PanelState; right: PanelState };

export const DEFAULT_PANELS: PanelsState = {
  left: { width: 260, collapsed: false },
  right: { width: 340, collapsed: false },
};

export function clampWidth(w: number, fallback: number): number {
  if (!Number.isFinite(w)) return fallback;
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.round(w)));
}

function parsePanel(raw: unknown, def: PanelState): PanelState {
  if (typeof raw !== "object" || raw === null) return { ...def };
  const r = raw as Record<string, unknown>;
  return {
    width: typeof r.width === "number" ? clampWidth(r.width, def.width) : def.width,
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
