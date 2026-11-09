import { describe, expect, it } from "vitest";
import { DENSE_CHAR_W, DENSE_STEP, FLAME_GAP, FLAME_MIN_W, FLAME_ROW, FLAME_TARGET_W, MARGIN_L, flameWidths, layout } from "./layout";

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
