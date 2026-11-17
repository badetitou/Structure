import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ViewCell } from "../domaintree";

vi.mock("../api", () => ({
  capabilities: vi.fn(),
  analyze: vi.fn(),
  reclassify: vi.fn(),
  rulesStamp: vi.fn(),
  impact: vi.fn(),
  cellSymbols: vi.fn(),
  lenses: vi.fn(),
  matrix: vi.fn(),
  impactCells: vi.fn(),
  history: vi.fn(),
  diff: vi.fn(),
}));

import { selection } from "./selection.svelte";
import { runImpact } from "./session";
import { cell, flush, imp, loaded, mocked, mx, page, setup } from "./testing";

beforeEach(setup);

describe("selection", () => {
  it("pages the symbols of the selected cell and reports errors", async () => {
    await loaded();
    const sym = (name: string) => ({ id: name, name, file: "f" });
    mocked.cellSymbols.mockResolvedValueOnce({ total: 2, offset: 0, symbols: [sym("a")] });
    selection.select(cell);
    expect(selection.symbols?.loading).toBe(true);
    await flush();
    expect(mocked.cellSymbols).toHaveBeenCalledWith(["/p"], null, 7, ["m1"], 0, expect.any(Number));
    expect(selection.symbols?.items).toHaveLength(1);
    mocked.cellSymbols.mockResolvedValueOnce({ total: 2, offset: 1, symbols: [sym("b")] });
    await selection.loadSymbols(true);
    expect(selection.symbols?.items).toHaveLength(2);
    mocked.cellSymbols.mockRejectedValueOnce(new Error("nope"));
    await selection.loadSymbols(false);
    expect(selection.symbols?.error).toContain("nope");
    selection.select(null);
    expect(selection.symbols).toBeNull();
  });

  it("lists snapshot symbols against the snapshot head", async () => {
    await loaded();
    mocked.impact.mockResolvedValue(imp({ head: "h1", matrix: mx("snap") }));
    await runImpact("a", "h1");
    mocked.cellSymbols.mockResolvedValue({ total: 0, offset: 0, symbols: [] });
    selection.select(cell);
    await flush();
    expect(mocked.cellSymbols).toHaveBeenLastCalledWith(["/p"], "h1", 7, ["m1"], 0, expect.any(Number));
  });

  it("follows the view's cell of the same id, or drops a selection that left the view", async () => {
    await loaded();
    mocked.cellSymbols.mockResolvedValue(page);
    selection.select(cell);
    selection.sync([cell]);
    expect(selection.selected).toBe(cell);
    const wider = { ...cell, size: 3, members: ["m1", "m2"] } as ViewCell;
    selection.sync([wider]);
    expect(selection.selected).toBe(wider);
    selection.sync([]);
    expect(selection.selected).toBeNull();
  });
});
