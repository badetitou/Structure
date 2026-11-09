export type Mode = "normal" | "dense" | "flame";

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

/** Horizontal slot of one column: left edge and width. */
export interface ColumnSlot {
  x: number;
  w: number;
}

export interface Layout {
  width: number;
  height: number;
  tiles: Tile[];
  /** One slot per domain column, left to right (bands span them). */
  columns: ColumnSlot[];
  domainLabels: Label[];
  layerLabels: Label[];
  /** Rotate domain labels by -90 degrees around their anchor. */
  rotateDomains: boolean;
  /** Labels under cells and edges are only drawn in normal mode. */
  detailed: boolean;
  /** Flame mode: column widths follow sizes, domain labels are cut to their column. */
  flame: boolean;
}

export const PITCH = 140;
export const MARGIN_L = 110;
export const MARGIN_T = 40;
export const DENSE_STEP = 14;
/** Cell outline width [unselected, selected] per mode. */
export const STROKE_NORMAL = [1, 3] as const;
export const STROKE_DENSE = [0.5, 2] as const;
/** Height of one row of group bands above the column headers, per mode. */
export const BAND_NORMAL = 22;
export const BAND_DENSE = 14;
/** Upper bound of one character's width in a dense header label (bold, font-size 10). */
export const DENSE_CHAR_W = 7;
const denseMargin = (names: readonly string[]) =>
  12 + DENSE_CHAR_W * Math.max(0, ...names.map((n) => n.length));

/** Flame mode: row height, minimum column width, target width before minimums. */
export const FLAME_ROW = 56;
export const FLAME_MIN_W = 18;
export const FLAME_TARGET_W = 1400;
/** Flame mode: gap between neighbouring cells, height of an empty slot's baseline. */
export const FLAME_GAP = 2;
export const FLAME_BASELINE = 2;

/** Flame column widths: proportional to each column's summed size, never below `FLAME_MIN_W`. */
export function flameWidths(sizes: readonly number[]): number[] {
  const total = sizes.reduce((a, b) => a + b, 0);
  const unit = total > 0 ? FLAME_TARGET_W / total : 0;
  return sizes.map((s) => Math.max(FLAME_MIN_W, s * unit));
}

function flameLayout(
  layers: readonly string[],
  domains: readonly string[],
  cells: readonly SlotCell[],
  domainTexts: readonly string[],
): Layout {
  const byId = new Map(cells.map((c) => [c.id, c]));
  const maxSize = Math.max(1, ...cells.map((c) => c.size));
  const colSize = new Map<string, number>();
  for (const c of cells) colSize.set(c.domain, (colSize.get(c.domain) ?? 0) + c.size);
  const widths = flameWidths(domains.map((d) => colSize.get(d) ?? 0));
  const columns: ColumnSlot[] = [];
  let x = MARGIN_L;
  for (const w of widths) {
    columns.push({ x, w });
    x += w;
  }
  const tiles: Tile[] = [];
  layers.forEach((layer, r) => {
    const bottom = MARGIN_T + (r + 1) * FLAME_ROW - FLAME_GAP;
    domains.forEach((domain, c) => {
      const cell = byId.get(`${layer}:${domain}`);
      const slot = columns[c];
      if (!slot) return;
      // Bars rise from the row's baseline: full height is the largest cell.
      const h = cell ? Math.max(4, (FLAME_ROW - 2 * FLAME_GAP) * Math.sqrt(cell.size / maxSize)) : FLAME_BASELINE;
      tiles.push({
        id: cell ? cell.id : null,
        layer,
        domain,
        x: slot.x + FLAME_GAP / 2,
        y: bottom - h,
        w: Math.max(1, slot.w - FLAME_GAP),
        h,
      });
    });
  });
  return {
    width: x + 20,
    height: MARGIN_T + layers.length * FLAME_ROW + 20,
    tiles,
    columns,
    domainLabels: domains.map((d, i) => ({
      text: domainTexts[i] ?? d,
      x: (columns[i]?.x ?? 0) + (columns[i]?.w ?? 0) / 2,
      y: 24,
    })),
    layerLabels: layers.map((text, i) => ({ text, x: 8, y: MARGIN_T + i * FLAME_ROW + FLAME_ROW / 2 + 4 })),
    rotateDomains: false,
    detailed: false,
    flame: true,
  };
}

export function layout(
  mode: Mode,
  layers: readonly string[],
  domains: readonly string[],
  cells: readonly SlotCell[],
  /** Header text per domain (defaults to the domain); sizes the dense margin. */
  domainTexts: readonly string[] = domains,
): Layout {
  if (mode === "flame") return flameLayout(layers, domains, cells, domainTexts);
  const byId = new Map(cells.map((c) => [c.id, c]));
  const maxSize = Math.max(1, ...cells.map((c) => c.size));
  const dense = mode === "dense";
  const ml = dense ? denseMargin(layers) : MARGIN_L;
  const mt = dense ? denseMargin(domainTexts) : MARGIN_T;
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
  const domainLabels = domains.map((d, i) => ({
    text: domainTexts[i] ?? d,
    x: ml + i * pitch + pitch / 2,
    y: dense ? mt - 4 : 24,
  }));
  const layerLabels = layers.map((text, i) => ({
    text,
    x: 8,
    y: mt + i * pitch + pitch / 2 + (dense ? 3 : 0),
  }));
  return {
    width: ml + domains.length * pitch + (dense ? 4 : 20),
    height: mt + layers.length * pitch + (dense ? 4 : 20),
    tiles,
    columns: domains.map((_, i) => ({ x: ml + i * pitch, w: pitch })),
    domainLabels,
    layerLabels,
    rotateDomains: dense,
    detailed: !dense,
    flame: false,
  };
}
