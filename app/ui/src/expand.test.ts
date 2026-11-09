import { describe, expect, it } from "vitest";
import { expandMatrix } from "./expand";
import type { WireMatrix } from "./api";

const cell = (id: string, layer: "ui" | "model", domain: string) => ({ id, layer, domain, size: 1 });

describe("expandMatrix", () => {
  it("turns grouped index edges into cell-id edges", () => {
    const w = {
      project: "p",
      language: "java",
      layers: ["ui", "model"],
      domains: ["a", "b"],
      cells: [cell("ui:a", "ui", "a"), cell("ui:b", "ui", "b"), cell("model:a", "model", "a")],
      edges: [
        { from: 0, targets: [[1, 5], [2, 2]] },
        { from: 1, targets: [[2, 1], [9, 1]] },
        { from: 7, targets: [[0, 1]] },
      ],
      stats: { files: 0, symbols: 0, edges: 0, elapsed_ms: 0 },
    } as unknown as WireMatrix;
    expect(expandMatrix(w).edges).toEqual([
      { from: "ui:a", to: "ui:b", weight: 5 },
      { from: "ui:a", to: "model:a", weight: 2 },
      { from: "ui:b", to: "model:a", weight: 1 },
    ]);
  });

  it("expands http_edges like edges, and gives an empty list when absent", () => {
    const base = {
      project: "p",
      language: "java",
      layers: ["ui", "model"],
      domains: ["a"],
      cells: [cell("ui:a", "ui", "a"), cell("model:a", "model", "a")],
      edges: [],
      stats: { files: 0, symbols: 0, edges: 0, elapsed_ms: 0 },
    };
    const withHttp = { ...base, http_edges: [{ from: 0, targets: [[1, 3], [5, 1]] }] } as unknown as WireMatrix;
    const m = expandMatrix(withHttp);
    expect(m.httpEdges).toEqual([{ from: "ui:a", to: "model:a", weight: 3 }]);
    expect("http_edges" in m).toBe(false);
    expect(expandMatrix(base as unknown as WireMatrix).httpEdges).toEqual([]);
  });
});
