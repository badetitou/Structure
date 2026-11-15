import { describe, expect, it, vi } from "vitest";
import { REQUIRED_ORIGINS, hasHostAccess, requestHostAccess } from "./access";
import { pickApi } from "./browser";
import { READY_TYPE, watchFrame } from "./frame";

describe("pickApi", () => {
  it("prefers browser over chrome", () => {
    const b = { id: "b" };
    const c = { id: "c" };
    expect(pickApi({ browser: b, chrome: c })).toBe(b);
  });
  it("falls back to chrome, then undefined", () => {
    const c = { id: "c" };
    expect(pickApi({ chrome: c })).toBe(c);
    expect(pickApi({})).toBeUndefined();
  });
});

describe("host access", () => {
  it("checks the required origins", async () => {
    const contains = vi.fn().mockResolvedValue(false);
    expect(await hasHostAccess({ contains, request: vi.fn() })).toBe(false);
    expect(contains).toHaveBeenCalledWith({ origins: REQUIRED_ORIGINS });
  });
  it("treats a failing check as available", async () => {
    const contains = vi.fn().mockRejectedValue(new Error("nope"));
    expect(await hasHostAccess({ contains, request: vi.fn() })).toBe(true);
  });
  it("returns the user's answer to the request", async () => {
    const request = vi.fn().mockResolvedValueOnce(true).mockResolvedValueOnce(false).mockRejectedValueOnce(new Error("x"));
    const p = { contains: vi.fn(), request };
    expect(await requestHostAccess(p)).toBe(true);
    expect(await requestHostAccess(p)).toBe(false);
    expect(await requestHostAccess(p)).toBe(false);
    expect(request).toHaveBeenCalledWith({ origins: REQUIRED_ORIGINS });
  });
});

describe("watchFrame", () => {
  function env() {
    type L = (e: { origin: string; data: unknown }) => void;
    let listener: L | null = null;
    let fire: (() => void) | null = null;
    return {
      e: {
        addEventListener: (_: "message", l: L) => void (listener = l),
        removeEventListener: () => void (listener = null),
        setTimeout: (f: () => void) => void (fire = f),
        clearTimeout: () => void (fire = null),
      },
      message: (origin: string, data: unknown) => listener?.({ origin, data }),
      timeout: () => fire?.(),
      listening: () => listener !== null,
    };
  }
  const frame = "moz-extension://1234";

  it("reports a frame that never answers", () => {
    const t = env();
    const missing = vi.fn();
    watchFrame(t.e, frame, missing, 100);
    t.timeout();
    expect(missing).toHaveBeenCalledOnce();
    expect(t.listening()).toBe(false);
  });
  it("stays quiet once the frame says it is ready", () => {
    const t = env();
    const missing = vi.fn();
    watchFrame(t.e, frame, missing, 100);
    t.message("https://github.com", { type: READY_TYPE }); // another origin
    expect(t.listening()).toBe(true);
    t.message(frame, { type: "other" });
    expect(t.listening()).toBe(true);
    t.message(frame, { type: READY_TYPE });
    t.timeout();
    expect(missing).not.toHaveBeenCalled();
    expect(t.listening()).toBe(false);
  });
});
