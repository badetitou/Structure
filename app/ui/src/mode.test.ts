import { afterEach, describe, expect, it, vi } from "vitest";
import {
  HTTP_KEY,
  MODE_KEY,
  RPC_KEY,
  TRANSPOSED_KEY,
  loadMode,
  loadShowHttp,
  loadShowRpc,
  loadTransposed,
  parseMode,
  parseShowHttp,
  parseShowRpc,
  saveMode,
  saveShowHttp,
  saveShowRpc,
  saveTransposed,
} from "./mode";

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

describe("http links toggle persistence", () => {
  it("is on unless stored as false", () => {
    expect(parseShowHttp(null)).toBe(true);
    expect(parseShowHttp("true")).toBe(true);
    expect(parseShowHttp("false")).toBe(false);
  });
  it("round-trips through storage and survives blocked storage", () => {
    const st = memoryStorage();
    vi.stubGlobal("localStorage", st);
    expect(loadShowHttp()).toBe(true);
    saveShowHttp(false);
    expect(st.getItem(HTTP_KEY)).toBe("false");
    expect(loadShowHttp()).toBe(false);
    const boom = () => {
      throw new Error("blocked");
    };
    vi.stubGlobal("localStorage", { getItem: boom, setItem: boom });
    expect(loadShowHttp()).toBe(true);
    expect(() => saveShowHttp(false)).not.toThrow();
  });
});

describe("rpc links toggle persistence", () => {
  it("is on unless stored as false, and round-trips", () => {
    expect(parseShowRpc(null)).toBe(true);
    expect(parseShowRpc("false")).toBe(false);
    const st = memoryStorage();
    vi.stubGlobal("localStorage", st);
    expect(loadShowRpc()).toBe(true);
    saveShowRpc(false);
    expect(st.getItem(RPC_KEY)).toBe("false");
    expect(loadShowRpc()).toBe(false);
    const boom = () => {
      throw new Error("blocked");
    };
    vi.stubGlobal("localStorage", { getItem: boom, setItem: boom });
    expect(loadShowRpc()).toBe(true);
    expect(() => saveShowRpc(false)).not.toThrow();
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
