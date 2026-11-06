export type Mode = "normal" | "dense";

export interface SlotCell {
  id: string;
  layer: string;
  domain: string;
  size: number;
}

export interface Tile {
  /** Cell id, or null for an empty slot. */
  id: string | null;
  layer: string;
  domain: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Label {
  text: string;
  x: number;
  y: number;
}

export interface Layout {
  width: number;
  height: number;
  tiles: Tile[];
  domainLabels: Label[];
  layerLabels: Label[];
  /** Rotate domain labels by -90 degrees around their anchor. */
  rotateDomains: boolean;
  /** Labels under cells and edges are only drawn in normal mode. */
  detailed: boolean;
}

export const PITCH = 140;
export const MARGIN_L = 110;
export const MARGIN_T = 40;
export const DENSE_STEP = 14;
export const DENSE_MARGIN_L = 90;
export const DENSE_MARGIN_T = 90;

export function layout(
  mode: Mode,
  layers: readonly string[],
  domains: readonly string[],
  cells: readonly SlotCell[],
): Layout {
  const byId = new Map(cells.map((c) => [c.id, c]));
  const maxSize = Math.max(1, ...cells.map((c) => c.size));
  const dense = mode === "dense";
  const ml = dense ? DENSE_MARGIN_L : MARGIN_L;
  const mt = dense ? DENSE_MARGIN_T : MARGIN_T;
  const pitch = dense ? DENSE_STEP : PITCH;
  const tiles: Tile[] = [];
  layers.forEach((layer, r) => {
    domains.forEach((domain, c) => {
      const cell = byId.get(`${layer}:${domain}`);
      const x0 = ml + c * pitch;
      const y0 = mt + r * pitch;
      let side: number;
      if (dense) side = pitch;
      else if (cell) side = 20 + (PITCH - 50) * Math.sqrt(cell.size / maxSize);
      else side = PITCH - 40;
      tiles.push({
        id: cell ? cell.id : null,
        layer,
        domain,
        x: x0 + (pitch - side) / 2,
        y: y0 + (pitch - side) / 2,
        w: side,
        h: side,
      });
    });
  });
  const domainLabels = domains.map((text, i) => ({
    text,
    x: ml + i * pitch + pitch / 2,
    y: dense ? mt - 4 : 24,
  }));
  const layerLabels = layers.map((text, i) => ({
    text,
    x: 8,
    y: mt + i * pitch + pitch / 2 + (dense ? 3 : 0),
  }));
  return {
    width: ml + domains.length * pitch + 20,
    height: mt + layers.length * pitch + 20,
    tiles,
    domainLabels,
    layerLabels,
    rotateDomains: dense,
    detailed: !dense,
  };
}

const KEY = "structure.matrix.mode";

export function loadMode(): Mode {
  try {
    return localStorage.getItem(KEY) === "dense" ? "dense" : "normal";
  } catch {
    return "normal";
  }
}

export function saveMode(mode: Mode) {
  try {
    localStorage.setItem(KEY, mode);
  } catch {
    // storage unavailable: the choice just is not persisted
  }
}
