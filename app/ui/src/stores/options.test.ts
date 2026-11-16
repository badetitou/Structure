import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ViewOptions } from "./options.svelte";

describe("view options", () => {
  let store: Map<string, string>;
  beforeEach(() => {
    store = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, v),
    });
  });
  afterEach(() => vi.unstubAllGlobals());

  it("toggles dense mode and persists it, whatever the stored value was", () => {
    const s = new ViewOptions();
    expect(s.mode).toBe("normal");
    s.setMode("dense");
    expect(s.mode).toBe("dense");
    expect(store.get("structure.matrix.mode")).toBe("dense");
    s.setMode("flame");
    expect(s.mode).toBe("flame");
    expect(store.get("structure.matrix.mode")).toBe("flame");
    s.setMode("normal");
    expect(store.get("structure.matrix.mode")).toBe("normal");
  });

  it("persists every toggle and reads it back in a new store", () => {
    const s = new ViewOptions();
    s.setShowHttp(!s.showHttp);
    s.setShowRpc(!s.showRpc);
    s.setTransposed(!s.transposed);
    s.setAligned(!s.aligned);
    s.setAngledLabels(!s.angledLabels);
    s.setMode("flame");
    const t = new ViewOptions();
    expect([t.showHttp, t.showRpc, t.transposed, t.aligned, t.angledLabels, t.mode]).toEqual([
      s.showHttp,
      s.showRpc,
      s.transposed,
      s.aligned,
      s.angledLabels,
      "flame",
    ]);
  });
});
