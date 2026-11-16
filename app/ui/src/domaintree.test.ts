import { describe, expect, it } from "vitest";
import type { Cell, DomainNode, Impact, Matrix } from "./api";
import { aggregate, aggregateImpact, bands, columns, flatColumns, initialExpanded, remapExpanded } from "./domaintree";

const leaf = (d: string, size = 1): DomainNode => ({ label: d, domain: d, size, children: [] });
const group = (label: string, children: DomainNode[]): DomainNode => ({
  label,
  size: children.reduce((a, c) => a + c.size, 0),
  children,
});

const tree = [group("A", [leaf("a1"), leaf("a2"), group("AG", [leaf("a3"), leaf("a4")])]), leaf("b")];
const keys = (t: DomainNode[], e: Set<string>) => columns(t, e).map((c) => c.key);

describe("initialExpanded / columns", () => {
  it("expands breadth-first within the budget", () => {
    // A expands (a1 a2 AG b = 4 columns) but AG would make 5.
    expect(keys(tree, initialExpanded(tree, 4))).toEqual(["a1", "a2", "g:0/2", "b"]);
    expect(keys(tree, initialExpanded(tree, 5))).toEqual(["a1", "a2", "a3", "a4", "b"]);
    expect(keys(tree, initialExpanded(tree, 2))).toEqual(["g:0", "b"]);
  });
  it("never exceeds 12 columns by default", () => {
    const wide = [group("W", Array.from({ length: 20 }, (_, i) => leaf(`w${i}`))), leaf("x")];
    expect(columns(wide, initialExpanded(wide)).length).toBe(2);
  });
  it("group columns list their leaves", () => {
    const cols = columns(tree, new Set());
    expect(cols[0]).toMatchObject({ key: "g:0", group: true, domains: ["a1", "a2", "a3", "a4"] });
    expect(cols[1]).toMatchObject({ key: "b", group: false, domains: ["b"] });
  });
  it("bands cover expanded groups with depth", () => {
    expect(bands(tree, new Set(["g:0", "g:0/2"]))).toEqual([
      { key: "g:0", label: "A", start: 0, span: 4, depth: 0 },
      { key: "g:0/2", label: "AG", start: 2, span: 2, depth: 1 },
    ]);
  });
});

const cell = (layer: Cell["layer"], domain: string, size: number): Cell => ({
  id: `${layer}:${domain}`,
  layer,
  domain,
  size,
});
const m: Matrix = {
  project: "p",
  language: "java",
  layers: [
    { id: "ui", label: "ui", order: 0 },
    { id: "model", label: "model", order: 6 },
  ],
  domains: ["a1", "a2", "a3", "a4", "b"],
  cells: [
    cell("ui", "a1", 2),
    cell("ui", "a2", 3),
    cell("model", "a3", 1),
    cell("ui", "a3", 1),
    cell("ui", "a4", 2),
    cell("ui", "b", 1),
  ],
  httpEdges: [],
  rpcEdges: [],
  edges: [
    { from: "ui:a1", to: "ui:a2", weight: 5 },
    { from: "ui:a1", to: "model:a3", weight: 2 },
    { from: "ui:a2", to: "model:a3", weight: 3 },
    { from: "ui:a2", to: "ui:b", weight: 1 },
    { from: "ui:a3", to: "ui:a4", weight: 7 },
  ],
  stats: { files: 0, symbols: 0, edges: 0, elapsed_ms: 0 },
};

describe("aggregate", () => {
  it("sums sizes, lists members, remaps edges without self-loops", () => {
    const v = aggregate(m, columns(tree, new Set(["g:0"])));
    expect(v.cells.find((c) => c.id === "model:g:0/2")?.size).toBe(1);
    const ui = v.cells.find((c) => c.id === "ui:g:0/2");
    expect(ui?.size).toBe(3);
    expect(ui?.members).toEqual(["ui:a3", "ui:a4"]);
    expect(ui?.label).toBe("AG");
    // ui:a3 -> ui:a4 became a self-loop and is dropped.
    expect(v.edges).toEqual([
      { from: "ui:a1", to: "ui:a2", weight: 5 },
      { from: "ui:a1", to: "model:g:0/2", weight: 2 },
      { from: "ui:a2", to: "model:g:0/2", weight: 3 },
      { from: "ui:a2", to: "ui:b", weight: 1 },
    ]);
  });
  it("remaps rpc edges like edges", () => {
    const withRpc: Matrix = {
      ...m,
      rpcEdges: [
        { from: "ui:a1", to: "ui:a3", weight: 2 },
        { from: "ui:a1", to: "ui:a4", weight: 3 },
        { from: "ui:a3", to: "ui:a4", weight: 9 },
      ],
    };
    const v = aggregate(withRpc, columns(tree, new Set(["g:0"])));
    expect(v.rpcEdges).toEqual([{ from: "ui:a1", to: "ui:g:0/2", weight: 5 }]);
  });
  it("remaps http edges like edges: merged targets summed, internal ones dropped", () => {
    const withHttp: Matrix = {
      ...m,
      httpEdges: [
        { from: "ui:a1", to: "ui:a3", weight: 2 },
        { from: "ui:a1", to: "ui:a4", weight: 3 },
        { from: "ui:a3", to: "ui:a4", weight: 4 },
      ],
    };
    const v = aggregate(withHttp, columns(tree, new Set(["g:0"])));
    expect(v.httpEdges).toEqual([{ from: "ui:a1", to: "ui:g:0/2", weight: 5 }]);
    expect(v.edges).toEqual(aggregate(m, columns(tree, new Set(["g:0"]))).edges);
  });
  it("sums parallel edges when fully collapsed", () => {
    const v = aggregate(m, columns(tree, new Set()));
    expect(v.edges).toEqual([
      { from: "ui:g:0", to: "model:g:0", weight: 5 },
      { from: "ui:g:0", to: "ui:b", weight: 1 },
    ]);
  });
  it("is the identity shape without a tree", () => {
    const v = aggregate(m, flatColumns(m.domains));
    expect(v.cells.map((c) => c.id)).toEqual(m.cells.map((c) => c.id));
    expect(v.edges).toEqual(m.edges);
  });
});

describe("aggregateImpact", () => {
  it("takes min distance and changed-if-any", () => {
    const v = aggregate(m, columns(tree, new Set()));
    const impact: Impact = {
      base: "a",
      head: null,
      changed_files: [],
      changed_symbols: [],
      changed_cells: ["model:a3"],
      impacted_cells: [
        { id: "ui:a1", distance: 3 },
        { id: "ui:a2", distance: 1 },
        { id: "ui:b", distance: 2 },
      ],
      impacted_symbols: [],
    };
    const r = aggregateImpact(impact, v.cellMap);
    expect([...r.changed]).toEqual(["model:g:0"]);
    expect(r.impacted.get("ui:g:0")).toBe(1);
    expect(r.impacted.get("ui:b")).toBe(2);
  });
});

describe("review cases", () => {
  it("keeps same-label sibling groups distinct", () => {
    const twins = [group("x", [leaf("p"), leaf("q")]), group("x", [leaf("r"), leaf("s")])];
    const cols = columns(twins, new Set(["g:1"]));
    expect(cols.map((c) => c.key)).toEqual(["g:0", "r", "s"]);
  });
  it("expands up to exactly the default budget and skips groups that are too wide", () => {
    const fit = [group("F", Array.from({ length: 11 }, (_, i) => leaf(`f${i}`))), leaf("y")];
    expect(columns(fit, initialExpanded(fit)).length).toBe(12);
    const t = [group("W", Array.from({ length: 20 }, (_, i) => leaf(`w${i}`))), group("S", [leaf("s1"), leaf("s2")])];
    expect(keys(t, initialExpanded(t))).toEqual(["g:0", "s1", "s2"]);
  });
  it("partially expanded impact: changed leaf and impacted sibling in one group", () => {
    const v = aggregate(m, columns(tree, new Set(["g:0"])));
    const impact: Impact = {
      base: "a",
      head: null,
      changed_files: [],
      changed_symbols: [],
      changed_cells: ["ui:a3"],
      impacted_cells: [
        { id: "ui:a4", distance: 1 },
        { id: "ui:a1", distance: 2 },
      ],
      impacted_symbols: [],
    };
    const r = aggregateImpact(impact, v.cellMap);
    expect([...r.changed]).toEqual(["ui:g:0/2"]);
    expect(r.impacted.get("ui:g:0/2")).toBe(1);
    expect(r.impacted.get("ui:a1")).toBe(2);
    expect(aggregateImpact(null, v.cellMap).changed.size).toBe(0);
  });
});

describe("remapExpanded", () => {
  it("keeps open groups open when their index moves in another tree", () => {
    const other = [leaf("z"), group("A", [leaf("a0"), leaf("a1"), leaf("a2"), group("AG", [leaf("a3")])]), group("N", [leaf("n")])];
    expect(remapExpanded(tree, other, new Set(["g:0", "g:0/2"]))).toEqual(new Set(["g:1", "g:1/3"]));
  });
  it("drops groups missing from the target and keeps closed ones closed", () => {
    const other = [group("A", [leaf("a1")]), leaf("b")];
    expect(remapExpanded(tree, other, new Set(["g:0/2"]))).toEqual(new Set());
    expect(remapExpanded(tree, other, new Set(["g:0"]))).toEqual(new Set(["g:0"]));
  });
  it("tells same-label siblings apart by rank", () => {
    const from = [group("X", [leaf("p")]), group("X", [leaf("q")])];
    const to = [leaf("new"), group("X", [leaf("p")]), group("X", [leaf("q")])];
    expect(remapExpanded(from, to, new Set(["g:1"]))).toEqual(new Set(["g:2"]));
  });
});
