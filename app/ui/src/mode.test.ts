import { afterEach, describe, expect, it, vi } from "vitest";
import { MODE_KEY, loadMode, parseMode, saveMode } from "./mode";

function memoryStorage() {
  const m = new Map<string, string>();
  return {
    getItem: (k: string) => m.get(k) ?? null,
    setItem: (k: string, v: string) => void m.set(k, v),
  };
}

afterEach(() => vi.unstubAllGlobals());

describe("matrix mode persistence", () => {
  it("parses dense and flame, anything else is normal", () => {
    expect(parseMode("dense")).toBe("dense");
    expect(parseMode("flame")).toBe("flame");
    expect(parseMode("normal")).toBe("normal");
    expect(parseMode("weird")).toBe("normal");
    expect(parseMode(null)).toBe("normal");
  });
  it("round-trips through storage", () => {
    const st = memoryStorage();
    vi.stubGlobal("localStorage", st);
    expect(loadMode()).toBe("normal");
    saveMode("dense");
    expect(st.getItem(MODE_KEY)).toBe("dense");
    expect(loadMode()).toBe("dense");
  });
  it("does not throw when storage is unavailable", () => {
    const boom = () => {
      throw new Error("blocked");
    };
    vi.stubGlobal("localStorage", { getItem: boom, setItem: boom });
    expect(loadMode()).toBe("normal");
    expect(() => saveMode("dense")).not.toThrow();
  });
});
