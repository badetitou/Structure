import { describe, expect, it } from "vitest";
import { activeOf, displayParams, enlist, forget, nextInstanceId, parseDisplay, touch } from "./paneldisplay";
import type { PanelDisplay } from "./paneldisplay";

const defaults: PanelDisplay = { mode: "dense", transposed: true, aligned: false, showHttp: false, showRpc: true };

describe("panel display params", () => {
  it("adopt the global defaults when the params hold no display", () => {
    for (const params of [undefined, null, {}, { other: 1 }, { display: null }, { display: "dense" }, 3]) {
      const r = parseDisplay(params, defaults);
      expect(r).toEqual({ display: defaults, stored: false });
      expect(r.display).not.toBe(defaults);
    }
  });

  it("read a stored display, field by field", () => {
    const display = { mode: "flame", transposed: false, aligned: true, showHttp: true, showRpc: false };
    expect(parseDisplay({ display }, defaults)).toEqual({ display, stored: true });
    // Missing or malformed fields take the defaults'; an unknown mode is the normal view.
    expect(parseDisplay({ display: { mode: "weird", transposed: "yes", showRpc: false } }, defaults)).toEqual({
      display: { mode: "normal", transposed: true, aligned: false, showHttp: false, showRpc: false },
      stored: true,
    });
    expect(parseDisplay({ display: {} }, defaults).display).toEqual(defaults);
  });

  it("store only the display fields", () => {
    const d = { ...defaults, extra: 1 } as PanelDisplay;
    expect(displayParams(d)).toEqual({ display: defaults });
    expect(parseDisplay(displayParams(d), { ...defaults, mode: "normal" })).toEqual({ display: defaults, stored: true });
  });
});

describe("active panel", () => {
  it("is the focused one, else the last focused still open", () => {
    let recent: string[] = [];
    expect(activeOf(recent)).toBeNull();
    recent = enlist(recent, "matrix");
    recent = enlist(recent, "matrix-2");
    expect(activeOf(recent)).toBe("matrix");
    recent = enlist(recent, "matrix");
    expect(recent).toEqual(["matrix", "matrix-2"]);
    recent = touch(recent, "matrix-2");
    expect(activeOf(recent)).toBe("matrix-2");
    recent = touch(recent, "matrix-3");
    expect(recent).toEqual(["matrix-3", "matrix-2", "matrix"]);
    recent = forget(recent, "matrix-3");
    expect(activeOf(recent)).toBe("matrix-2");
    recent = forget(recent, "matrix-2");
    recent = forget(recent, "matrix");
    expect(activeOf(recent)).toBeNull();
  });
});

describe("instance ids", () => {
  it("number new instances from 2, filling gaps", () => {
    expect(nextInstanceId("matrix", new Set())).toEqual({ id: "matrix-2", n: 2 });
    expect(nextInstanceId("matrix", new Set(["matrix", "matrix-2"]))).toEqual({ id: "matrix-3", n: 3 });
    expect(nextInstanceId("matrix", new Map([["matrix-3", "matrix"]]))).toEqual({ id: "matrix-2", n: 2 });
  });
});
