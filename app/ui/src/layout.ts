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

/** Slot of one column (or row) along its axis: start and length. */
export interface ColumnSlot {
  x: number;
  w: number;
}

export interface Layout {
  width: number;
  height: number;
  tiles: Tile[];
  /**
   * One slot per domain, along the domain axis: left to right, or top to bottom
   * when transposed (`x` / `w` are then a top and a height). Bands span them.
   */
  columns: ColumnSlot[];
  /** One slot per layer, along the layer axis (top to bottom, or left to right when transposed). */
  rows: ColumnSlot[];
  domainLabels: Label[];
  layerLabels: Label[];
  /** Rotate domain labels by -90 degrees around their anchor. */
  rotateDomains: boolean;
  /** Rotate layer labels by -90 degrees around their anchor (dense, transposed). */
  rotateLayers: boolean;
  /** Domains are rows and layers columns; domain labels and bands sit on the left. */
  transposed: boolean;
  /** Labels under cells and edges are only drawn in normal mode. */
  detailed: boolean;
  /** Centre of each extra gap between layer segments, along the layer axis (see `layout`'s `gapsBefore`). */
  separators: number[];
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
/** Upper bound of one character's width in a normal header label (bold, font-size 11). */
export const LABEL_CHAR_W = 8;
const longest = (names: readonly string[]) => Math.max(0, ...names.map((n) => n.length));
const denseMargin = (names: readonly string[]) => 12 + DENSE_CHAR_W * longest(names);
/** Left room for horizontal domain labels when transposed. */
const labelMargin = (names: readonly string[]) => 12 + LABEL_CHAR_W * longest(names);

/** Flame mode: row height, minimum column width, target width before minimums. */
export const FLAME_ROW = 56;
export const FLAME_MIN_W = 18;
export const FLAME_TARGET_W = 1400;
/** Flame mode, transposed: width of one layer column (bars grow along it). */
export const FLAME_COL = 120;
/** Flame mode: gap between neighbouring cells, height of an empty slot's baseline. */
export const FLAME_GAP = 2;
export const FLAME_BASELINE = 2;
/** Extra room before a layer index that starts a new segment (aligned view), per mode. */
export const GAP_NORMAL = 24;
export const GAP_DENSE = 8;

/** Flame column widths: proportional to each column's summed size, never below `FLAME_MIN_W`. */
export function flameWidths(sizes: readonly number[]): number[] {
  const total = sizes.reduce((a, b) => a + b, 0);
  const unit = total > 0 ? FLAME_TARGET_W / total : 0;
  return sizes.map((s) => Math.max(FLAME_MIN_W, s * unit));
}

/**
 * Geometry before orientation: `x` runs along the domains, `y` along the layers.
 * `uEnd` / `vEnd` are the extents along each axis, padding included.
 */
interface Core {
  tiles: Tile[];
  columns: ColumnSlot[];
  rows: ColumnSlot[];
  uEnd: number;
  vEnd: number;
}

function flameCore(
  layers: readonly string[],
  domains: readonly string[],
  cells: readonly SlotCell[],
  u0: number,
  v0: number,
  row: number,
  fromStart: boolean,
  offs: readonly number[],
): Core {
  const byId = new Map(cells.map((c) => [c.id, c]));
  const maxSize = Math.max(1, ...cells.map((c) => c.size));
  const colSize = new Map<string, number>();
  for (const c of cells) colSize.set(c.domain, (colSize.get(c.domain) ?? 0) + c.size);
  const widths = flameWidths(domains.map((d) => colSize.get(d) ?? 0));
  const columns: ColumnSlot[] = [];
  let x = u0;
  for (const w of widths) {
    columns.push({ x, w });
    x += w;
  }
  const tiles: Tile[] = [];
  layers.forEach((layer, r) => {
    const o = offs[r] ?? 0;
    const start = v0 + o + r * row + FLAME_GAP;
    const bottom = v0 + o + (r + 1) * row - FLAME_GAP;
    domains.forEach((domain, c) => {
      const cell = byId.get(`${layer}:${domain}`);
      const slot = columns[c];
      if (!slot) return;
      // Bars rise from the row's baseline (or grow from its start): full length is the largest cell.
      const h = cell ? Math.max(4, (row - 2 * FLAME_GAP) * Math.sqrt(cell.size / maxSize)) : FLAME_BASELINE;
      tiles.push({
        id: cell ? cell.id : null,
        layer,
        domain,
        x: slot.x + FLAME_GAP / 2,
        y: fromStart ? start : bottom - h,
        w: Math.max(1, slot.w - FLAME_GAP),
        h,
      });
    });
  });
  return {
    tiles,
    columns,
    rows: layers.map((_, i) => ({ x: v0 + (offs[i] ?? 0) + i * row, w: row })),
    uEnd: x + 20,
    vEnd: v0 + (offs[layers.length - 1] ?? 0) + layers.length * row + 20,
  };
}

function gridCore(
  dense: boolean,
  layers: readonly string[],
  domains: readonly string[],
  cells: readonly SlotCell[],
  u0: number,
  v0: number,
  offs: readonly number[],
): Core {
  const byId = new Map(cells.map((c) => [c.id, c]));
  const maxSize = Math.max(1, ...cells.map((c) => c.size));
  const pitch = dense ? DENSE_STEP : PITCH;
  const tiles: Tile[] = [];
  layers.forEach((layer, r) => {
    domains.forEach((domain, c) => {
      const cell = byId.get(`${layer}:${domain}`);
      const x0 = u0 + c * pitch;
      const y0 = v0 + (offs[r] ?? 0) + r * pitch;
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
  const pad = dense ? 4 : 20;
  return {
    tiles,
    columns: domains.map((_, i) => ({ x: u0 + i * pitch, w: pitch })),
    rows: layers.map((_, i) => ({ x: v0 + (offs[i] ?? 0) + i * pitch, w: pitch })),
    uEnd: u0 + domains.length * pitch + pad,
    vEnd: v0 + (offs[layers.length - 1] ?? 0) + layers.length * pitch + pad,
  };
}

const centre = (s: ColumnSlot | undefined) => (s ? s.x + s.w / 2 : 0);

export function layout(
  mode: Mode,
  layers: readonly string[],
  domains: readonly string[],
  cells: readonly SlotCell[],
  /** Header text per domain (defaults to the domain); sizes the dense margin. */
  domainTexts: readonly string[] = domains,
  /** Domains as rows and layers as columns. */
  transposed = false,
  /** Header text per layer (defaults to the layer); sizes the dense margin. */
  layerTexts: readonly string[] = layers,
  /** Layer indices that start a new segment: each gets an extra gap before it. */
  gapsBefore: readonly number[] = [],
): Layout {
  const dense = mode === "dense";
  const flame = mode === "flame";
  // Room before the domain axis holds the layer labels, room before the layer axis the domain labels.
  const layerRoom = dense ? denseMargin(layerTexts) : transposed ? MARGIN_T : MARGIN_L;
  const domainRoom = transposed
    ? dense
      ? denseMargin(domainTexts)
      : labelMargin(domainTexts)
    : dense
      ? denseMargin(domainTexts)
      : MARGIN_T;
  const gap = dense ? GAP_DENSE : GAP_NORMAL;
  const offs = layers.map((_, i) => gap * gapsBefore.filter((g) => g > 0 && g <= i).length);
  const pitch = flame ? (transposed ? FLAME_COL : FLAME_ROW) : dense ? DENSE_STEP : PITCH;
  const separators = gapsBefore
    .filter((g) => g > 0 && g < layers.length)
    .map((g) => domainRoom + (offs[g] ?? 0) + g * pitch - gap / 2);
  const core = flame
    ? flameCore(layers, domains, cells, layerRoom, domainRoom, transposed ? FLAME_COL : FLAME_ROW, transposed, offs)
    : gridCore(dense, layers, domains, cells, layerRoom, domainRoom, offs);
  const common = {
    columns: core.columns,
    rows: core.rows,
    detailed: !dense && !flame,
    flame,
    transposed,
    separators,
  };
  if (!transposed) {
    return {
      ...common,
      width: core.uEnd,
      height: core.vEnd,
      tiles: core.tiles,
      domainLabels: domains.map((d, i) => ({
        text: domainTexts[i] ?? d,
        x: centre(core.columns[i]),
        y: dense ? domainRoom - 4 : 24,
      })),
      layerLabels: layerTexts.map((text, i) => ({
        text,
        x: 8,
        y: centre(core.rows[i]) + (dense ? 3 : flame ? 4 : 0),
      })),
      rotateDomains: dense,
      rotateLayers: false,
    };
  }
  return {
    ...common,
    width: core.vEnd,
    height: core.uEnd,
    tiles: core.tiles.map((t) => ({ ...t, x: t.y, y: t.x, w: t.h, h: t.w })),
    domainLabels: domains.map((d, i) => ({
      text: domainTexts[i] ?? d,
      x: 8,
      y: centre(core.columns[i]) + (dense ? 3 : 4),
    })),
    layerLabels: layerTexts.map((text, i) => ({
      text,
      x: centre(core.rows[i]),
      y: dense ? layerRoom - 4 : 24,
    })),
    rotateDomains: false,
    rotateLayers: dense,
  };
}
