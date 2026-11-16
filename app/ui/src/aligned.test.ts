import { describe, expect, it } from "vitest";
import { align, canAlign, layerLabel, layerName } from "./aligned";
import type { CellEdge, LayerDef, LayerId } from "./api";
import { aggregate, flatColumns } from "./domaintree";
import type { Matrix } from "./api";

const defs = (...ids: LayerId[]): LayerDef[] => ids.map((id, order) => ({ id, label: id, order }));
const ids = (m: Matrix) => m.layers.map((l) => l.id);
const cell = (layer: LayerId, domain: string, size = 1) => ({ id: `${layer}:${domain}`, layer, domain, size });
const edge = (from: string, to: string, weight = 1): CellEdge => ({ from, to, weight });

// Front "f" (ui, api_client, model) calls back "b" (entry, business, model).
const domains = ["f/orders", "f/users", "f/misc", "b/order", "b/user", "b/audit"];
function build(http: CellEdge[], rpc: CellEdge[] = []) {
  const m: Matrix = {
    project: "x",
    language: "java",
    projects: [
      { name: "f", root: "f", language: "typescript" },
      { name: "b", root: "b", language: "java" },
    ],
    layers: defs("ui", "api_client", "entry", "business", "model"),
    domains,
    cells: [
      cell("ui", "f/orders"),
      cell("api_client", "f/orders"),
      cell("api_client", "f/users"),
      cell("model", "f/users"),
      cell("api_client", "f/misc"),
      cell("entry", "b/order"),
      cell("entry", "b/user"),
      cell("business", "b/order"),
      cell("model", "b/audit"),
    ],
    edges: [edge("ui:f/orders", "api_client:f/orders")],
    httpEdges: http,
    rpcEdges: rpc,
  } as Matrix;
  const cols = flatColumns(domains);
  const vm = aggregate(m, cols);
  return { m, cols, vm, run: () => align(vm, cols, ids(m), ["f", "b"], [...vm.httpEdges, ...vm.rpcEdges]) };
}

describe("align rows", () => {
  it("pairs linked columns on one row, heaviest first", () => {
    const { run } = build([
      edge("api_client:f/orders", "entry:b/order", 5),
      edge("api_client:f/users", "entry:b/user", 2),
      edge("api_client:f/misc", "entry:b/order", 1), // loses to f/orders
    ]);
    const a = run();
    expect(a.primary).toBe("f");
    expect(a.rows.map((r) => r.key)).toEqual(["f/orders~b/order", "f/users~b/user", "f/misc", "b/audit"]);
    expect(a.rows[0]?.members.map((c) => c.key)).toEqual(["f/orders", "b/order"]);
  });
  it("gives each column its own row without remote edges", () => {
    const a = build([]).run();
    expect(a.rows.map((r) => r.key)).toEqual(domains.slice(0, 3).concat(domains.slice(3)));
    expect(a.rows.every((r) => r.members.length === 1)).toBe(true);
  });
  it("is deterministic on equal weights", () => {
    const a = build([edge("api_client:f/orders", "entry:b/order"), edge("api_client:f/orders", "entry:b/user")]).run();
    expect(a.rows[0]?.key).toBe("f/orders~b/order");
  });
  it("makes the caller project primary and puts its rows first", () => {
    const { m, vm } = build([]);
    const back = [edge("entry:b/order", "api_client:f/orders", 3)];
    const a = align(vm, flatColumns(domains), ids(m), ["f", "b"], back.map((e) => ({ ...e, from: e.from, to: e.to })));
    expect(a.primary).toBe("b");
    expect(a.rows.map((r) => r.key)).toEqual(["b/order~f/orders", "b/user", "b/audit", "f/users", "f/misc"]);
    expect(a.layers[0]?.project).toBe("b");
  });
  it("pairs across more than two projects", () => {
    const ds = ["f/a", "b/a", "s/a", "b/x"];
    const m = {
      project: "x",
      language: "java",
      layers: defs("ui", "entry"),
      domains: ds,
      cells: [cell("ui", "f/a"), cell("entry", "b/a"), cell("entry", "s/a"), cell("entry", "b/x")],
      edges: [],
      httpEdges: [edge("ui:f/a", "entry:b/a", 3), edge("entry:b/a", "entry:s/a", 2), edge("ui:f/a", "entry:b/x", 1)],
      rpcEdges: [],
    } as unknown as Matrix;
    const cols = flatColumns(ds);
    const vm = aggregate(m, cols);
    const a = align(vm, cols, ids(m), ["f", "b", "s"], vm.httpEdges);
    expect(a.rows.map((r) => r.key)).toEqual(["f/a~b/a~s/a", "b/x"]);
  });
});

describe("align layers and cells", () => {
  const a = build([edge("api_client:f/orders", "entry:b/order", 5)]).run();
  it("splits layers per project, shared layers once per segment", () => {
    expect(a.layers.map((l) => l.key)).toEqual([
      "f|ui", "f|api_client", "f|model", "b|entry", "b|business", "b|model",
    ]);
    expect(a.segments).toEqual([3]);
  });
  it("re-keys cells, edges and the cell map", () => {
    const ids = a.vm.cells.map((c) => c.id);
    expect(ids).toContain("f|api_client:f/orders~b/order");
    expect(ids).toContain("b|entry:f/orders~b/order");
    expect(new Set(ids).size).toBe(ids.length);
    const c = a.vm.cells.find((x) => x.id === "b|model:b/audit");
    expect(c).toMatchObject({ layer: "b|model", domain: "b/audit", label: "b/audit" });
    expect(a.vm.cellMap.get("api_client:f/orders")).toBe("f|api_client:f/orders~b/order");
    expect(a.vm.edges).toEqual([edge("f|ui:f/orders~b/order", "f|api_client:f/orders~b/order")]);
    expect(a.vm.httpEdges).toEqual([edge("f|api_client:f/orders~b/order", "b|entry:f/orders~b/order", 5)]);
  });
  it("shows the plain layer name", () => {
    expect(layerName("f|api_client")).toBe("api_client");
    expect(layerName("ui")).toBe("ui");
  });
  it("labels a layer from the matrix definitions, aligned or plain", () => {
    const d: LayerDef[] = [{ id: "api_client", label: "API client", order: 2 }];
    expect(layerLabel(d, "f|api_client")).toBe("API client");
    expect(layerLabel(d, "api_client")).toBe("API client");
    expect(layerLabel(d, "ui")).toBe("ui");
    expect(layerLabel(undefined, "f|ui")).toBe("ui");
  });
});

describe("canAlign", () => {
  it("needs two projects and a remote edge", () => {
    const e = [edge("a", "b")];
    expect(canAlign([1, 2], e, [])).toBe(true);
    expect(canAlign([1, 2], [], e)).toBe(true);
    expect(canAlign([1], e, [])).toBe(false);
    expect(canAlign(undefined, e, [])).toBe(false);
    expect(canAlign([1, 2], [], [])).toBe(false);
  });
});
