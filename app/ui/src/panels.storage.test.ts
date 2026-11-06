import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_PANELS, PANELS_KEY, loadPanels, savePanels } from "./panels";

function memoryStorage() {
  const m = new Map<string, string>();
  return {
    getItem: (k: string) => m.get(k) ?? null,
    setItem: (k: string, v: string) => void m.set(k, v),
  };
}

afterEach(() => vi.unstubAllGlobals());

describe("panels persistence", () => {
  it("round-trips through storage", () => {
    const st = memoryStorage();
    vi.stubGlobal("localStorage", st);
    expect(loadPanels()).toEqual(DEFAULT_PANELS);
    const s = { left: { width: 200, collapsed: true }, right: { width: 400, collapsed: false } };
    savePanels(s);
    expect(st.getItem(PANELS_KEY)).not.toBeNull();
    expect(loadPanels()).toEqual(s);
  });
  it("does not throw when storage is unavailable", () => {
    const boom = () => {
      throw new Error("blocked");
    };
    vi.stubGlobal("localStorage", { getItem: boom, setItem: boom });
    expect(loadPanels()).toEqual(DEFAULT_PANELS);
    expect(() => savePanels(DEFAULT_PANELS)).not.toThrow();
  });
});
