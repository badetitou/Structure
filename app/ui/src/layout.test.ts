import { describe, expect, it } from "vitest";
import {
  DENSE_CHAR_W,
  DENSE_STEP,
  FLAME_COL,
  FLAME_GAP,
  FLAME_MIN_W,
  FLAME_ROW,
  FLAME_TARGET_W,
  LABEL_CHAR_W,
  MARGIN_L,
  MARGIN_T,
  flameWidths,
  layout,
} from "./layout";

const layers = ["api", "service", "data"];
const domains = ["user", "order", "billing", "audit"];
const cells = [
  { id: "api:user", layer: "api", domain: "user", size: 10 },
  { id: "service:order", layer: "service", domain: "order", size: 40 },
];

describe("layout dense", () => {
  const l = layout("dense", layers, domains, cells);
  it("has one uniform tile per slot, empty ones with null id", () => {
    expect(l.tiles).toHaveLength(12);
    expect(l.tiles.filter((t) => t.id === null)).toHaveLength(10);
    for (const t of l.tiles) expect([t.w, t.h]).toEqual([DENSE_STEP, DENSE_STEP]);
  });
  it("packs tiles edge to edge without overlap", () => {
    const xs = [...new Set(l.tiles.map((t) => t.x))].sort((a, b) => a - b);
    for (let i = 1; i < xs.length; i++) expect(xs[i] - xs[i - 1]).toBe(DENSE_STEP);
    const ys = [...new Set(l.tiles.map((t) => t.y))].sort((a, b) => a - b);
    for (let i = 1; i < ys.length; i++) expect(ys[i] - ys[i - 1]).toBe(DENSE_STEP);
  });
  it("viewBox covers the grid", () => {
    for (const t of l.tiles) {
      expect(t.x + t.w).toBeLessThanOrEqual(l.width);
      expect(t.y + t.h).toBeLessThanOrEqual(l.height);
    }
    expect(l.rotateDomains).toBe(true);
    expect(l.detailed).toBe(false);
  });
});

describe("layout normal", () => {
  const l = layout("normal", layers, domains, cells);
  it("reproduces the 140px pitch geometry", () => {
    expect(l.width).toBe(110 + 4 * 140 + 20);
    expect(l.height).toBe(40 + 3 * 140 + 20);
    const empty = l.tiles.find((t) => t.layer === "api" && t.domain === "order");
    expect(empty).toMatchObject({ id: null, x: 110 + 140 + 20, y: 40 + 20, w: 100, h: 100 });
  });
  it("sizes the biggest cell at 20 + 90 and centres it", () => {
    const t = l.tiles.find((x) => x.id === "service:order");
    expect(t?.w).toBeCloseTo(110);
    expect((t?.x ?? 0) + (t?.w ?? 0) / 2).toBeCloseTo(110 + 140 + 70);
    expect(l.detailed).toBe(true);
    expect(l.rotateDomains).toBe(false);
  });
});

describe("layout dense margins", () => {
  it("leave room for the longest layer and domain names", () => {
    const long = "averyveryverylongdomainname";
    const l = layout("dense", ["persistence"], ["a", long], []);
    const top = Math.min(...l.tiles.map((t) => t.y));
    const left = Math.min(...l.tiles.map((t) => t.x));
    expect(top).toBeGreaterThanOrEqual(DENSE_CHAR_W * long.length);
    expect(left).toBeGreaterThanOrEqual(DENSE_CHAR_W * "persistence".length);
  });
});

describe("layout flame", () => {
  const sized = [
    { id: "api:user", layer: "api", domain: "user", size: 10 },
    { id: "service:user", layer: "service", domain: "user", size: 30 },
    { id: "service:order", layer: "service", domain: "order", size: 160 },
  ];
  const l = layout("flame", layers, domains, sized);
  it("gives each column a width proportional to its summed size, with a floor", () => {
    const [user, order, billing, audit] = l.columns;
    expect(order.w / user.w).toBeCloseTo(4);
    expect(user.w + order.w).toBeCloseTo(FLAME_TARGET_W);
    expect(billing.w).toBe(FLAME_MIN_W);
    expect(audit.w).toBe(FLAME_MIN_W);
  });
  it("packs columns edge to edge from the left margin", () => {
    expect(l.columns[0].x).toBe(MARGIN_L);
    for (let i = 1; i < l.columns.length; i++)
      expect(l.columns[i].x).toBeCloseTo(l.columns[i - 1].x + l.columns[i - 1].w);
    const last = l.columns[l.columns.length - 1];
    expect(l.width).toBeGreaterThanOrEqual(last.x + last.w);
  });
  it("fills the column width and grows bars from the row baseline", () => {
    const big = l.tiles.find((t) => t.id === "service:order");
    const small = l.tiles.find((t) => t.id === "api:user");
    expect(big?.w).toBeCloseTo(l.columns[1].w - FLAME_GAP);
    expect(big?.h).toBeCloseTo(FLAME_ROW - 2 * FLAME_GAP);
    expect(small && big && small.h < big.h).toBe(true);
    const bottom = (t: { y: number; h: number }) => t.y + t.h;
    const serviceTiles = l.tiles.filter((t) => t.layer === "service");
    for (const t of serviceTiles) expect(bottom(t)).toBeCloseTo(40 + 2 * FLAME_ROW - FLAME_GAP);
    for (const t of l.tiles) expect(bottom(t)).toBeLessThanOrEqual(l.height);
  });
  it("labels each column at its centre, horizontally", () => {
    expect(l.domainLabels[1].x).toBeCloseTo(l.columns[1].x + l.columns[1].w / 2);
    expect(l.rotateDomains).toBe(false);
    expect(l.flame).toBe(true);
  });
  it("falls back to the minimum width when nothing has a size", () => {
    expect(flameWidths([0, 0])).toEqual([FLAME_MIN_W, FLAME_MIN_W]);
  });
});

describe("layout columns", () => {
  it("are one pitch wide in normal and dense modes", () => {
    const n = layout("normal", layers, domains, cells);
    expect(n.columns.map((c) => c.w)).toEqual([140, 140, 140, 140]);
    expect(n.columns[1].x).toBe(110 + 140);
    const d = layout("dense", layers, domains, cells);
    expect(d.columns.every((c) => c.w === DENSE_STEP)).toBe(true);
  });
});

describe("layout transposed", () => {
  const key = (t: { layer: string; domain: string }) => `${t.layer}:${t.domain}`;
  it("puts domains on rows and layers on columns, for every mode", () => {
    for (const mode of ["normal", "dense", "flame"] as const) {
      const l = layout(mode, layers, domains, cells, domains, true);
      expect(l.transposed).toBe(true);
      const at = new Map(l.tiles.map((t) => [key(t), t]));
      // Same domain → same row band; same layer → same column band.
      const cx = (t?: { x: number; w: number }) => (t ? t.x + t.w / 2 : NaN);
      const cy = (t?: { y: number; h: number }) => (t ? t.y + t.h / 2 : NaN);
      if (mode !== "flame") {
        expect(cy(at.get("api:order"))).toBeCloseTo(cy(at.get("data:order")));
        expect(cx(at.get("api:user"))).toBeCloseTo(cx(at.get("api:audit")));
      }
      expect(cy(at.get("api:user"))).toBeLessThan(cy(at.get("api:order")));
      expect(cx(at.get("api:user"))).toBeLessThan(cx(at.get("service:user")));
      for (const t of l.tiles) {
        expect(t.x + t.w).toBeLessThanOrEqual(l.width);
        expect(t.y + t.h).toBeLessThanOrEqual(l.height);
      }
    }
  });
  it("swaps the normal geometry and leaves room for horizontal domain labels", () => {
    const long = "averyverylongdomain";
    const ds = ["user", long];
    const l = layout("normal", layers, ds, cells, ds, true);
    const left = 12 + LABEL_CHAR_W * long.length;
    expect(l.width).toBe(left + 3 * 140 + 20);
    expect(l.height).toBe(MARGIN_T + 2 * 140 + 20);
    const big = l.tiles.find((t) => t.layer === "service" && t.domain === long);
    expect(big).toMatchObject({ id: null, x: left + 140 + 20, y: MARGIN_T + 140 + 20, w: 100, h: 100 });
    expect(l.domainLabels[1]).toMatchObject({ x: 8 });
    expect(l.domainLabels[1].y).toBeCloseTo(MARGIN_T + 140 + 70 + 4);
    expect(l.layerLabels[2]).toMatchObject({ x: left + 2 * 140 + 70, y: 24 });
    expect([l.rotateDomains, l.rotateLayers]).toEqual([false, false]);
  });
  it("rotates the layer labels in dense mode", () => {
    const l = layout("dense", layers, domains, cells, domains, true);
    expect([l.rotateDomains, l.rotateLayers]).toEqual([false, true]);
    const top = Math.min(...l.tiles.map((t) => t.y));
    expect(top).toBeGreaterThanOrEqual(DENSE_CHAR_W * "service".length);
  });
  it("grows flame bars from the left of each layer column, rows sized by domain", () => {
    const sized = [
      { id: "api:user", layer: "api", domain: "user", size: 10 },
      { id: "service:order", layer: "service", domain: "order", size: 40 },
    ];
    const l = layout("flame", layers, domains, sized, domains, true);
    const user = l.tiles.find((t) => t.id === "api:user");
    const order = l.tiles.find((t) => t.id === "service:order");
    expect(l.columns[1].w / l.columns[0].w).toBeCloseTo(4);
    expect(order?.h).toBeCloseTo(l.columns[1].w - FLAME_GAP);
    expect(order?.w).toBeCloseTo(FLAME_COL - 2 * FLAME_GAP);
    expect(order?.x).toBeCloseTo(l.rows[1].x + FLAME_GAP);
    expect(user?.x).toBeCloseTo(l.rows[0].x + FLAME_GAP);
    expect(l.rows.map((r) => r.w)).toEqual([FLAME_COL, FLAME_COL, FLAME_COL]);
  });
  it("leaves the default orientation untouched", () => {
    const l = layout("normal", layers, domains, cells);
    expect([l.transposed, l.rotateLayers]).toEqual([false, false]);
    expect(l.rows[0]).toEqual({ x: MARGIN_T, w: 140 });
    expect(l.layerLabels[0]).toMatchObject({ x: 8, y: MARGIN_T + 70 });
    expect(MARGIN_L).toBe(110);
  });
});
