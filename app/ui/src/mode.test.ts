import { afterEach, describe, expect, it, vi } from "vitest";
import { MODE_KEY, TRANSPOSED_KEY, loadMode, loadTransposed, parseMode, saveMode, saveTransposed } from "./mode";

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

describe("matrix orientation persistence", () => {
  it("round-trips through storage, off by default", () => {
    const st = memoryStorage();
    vi.stubGlobal("localStorage", st);
    expect(loadTransposed()).toBe(false);
    saveTransposed(true);
    expect(st.getItem(TRANSPOSED_KEY)).toBe("1");
    expect(loadTransposed()).toBe(true);
    saveTransposed(false);
    expect(loadTransposed()).toBe(false);
  });
  it("does not throw when storage is unavailable", () => {
    const boom = () => {
      throw new Error("blocked");
    };
    vi.stubGlobal("localStorage", { getItem: boom, setItem: boom });
    expect(loadTransposed()).toBe(false);
    expect(() => saveTransposed(true)).not.toThrow();
  });
});
