// The first load of a session, in its own file: the lens store's singleton has not heard from
// the backend yet (it keeps its list across projects).
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LensInfo } from "../api";

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

import { DEFAULT_LENSES } from "../lens";
import { createViewContext } from "../workspace/context";
import { lenses } from "./lenses.svelte";
import { flush, imp, loaded, mocked, setup, withCells } from "./testing";

beforeEach(() => {
  setup();
  vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {} });
});
afterEach(() => vi.unstubAllGlobals());

describe("a layout saved with another lens, before the backend lists its lenses", () => {
  it("waits for the list instead of drawing the default lens first", async () => {
    let answer: (l: LensInfo[]) => void = () => {};
    mocked.lenses.mockReturnValue(new Promise((r) => (answer = r)));
    const ctx = createViewContext();
    const work = withCells(7, ["service:a", 3]);
    await loaded(work);
    await flush();
    expect(lenses.known).toBe(false);
    const pa = ctx.forPanel("matrix", { display: { lens: "layer-package" } }, () => {});
    expect(pa.lens.id).toBe("layer-package");
    expect(pa.lens.matrix).toBeNull();
    expect(pa.lens.error).toBe("");
    expect(pa.overlays.lensError).toBe("");
    void lenses.impact("layer-package", imp());
    await flush();
    expect(mocked.matrix).not.toHaveBeenCalled();
    expect(mocked.impactCells).not.toHaveBeenCalled();

    const p = withCells(7, ["package/service:com.x", 3]);
    mocked.matrix.mockResolvedValue(p);
    answer([...DEFAULT_LENSES, { id: "layer-package", label: "Layer × package" }]);
    await flush();
    expect(lenses.known).toBe(true);
    expect(pa.lens.id).toBe("layer-package");
    void pa.lens.matrix;
    await flush();
    expect(pa.lens.matrix).toBe(p);
  });
});
