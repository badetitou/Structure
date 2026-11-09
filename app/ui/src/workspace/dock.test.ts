import { afterEach, describe, expect, it, vi } from "vitest";
import { DOCK_KEY, loadDock, parseDock, pruneUnknown, syncVisibility } from "./dock";
import type { DockLike } from "./dock";

function group(ids: string[], visible = true) {
  const g = {
    panels: ids.map((id) => ({ id })),
    api: {
      isVisible: visible,
      calls: [] as boolean[],
      setVisible(v: boolean) {
        g.api.isVisible = v;
        g.api.calls.push(v);
      },
    },
  };
  return g;
}

describe("syncVisibility", () => {
  it("hides groups whose panels are all unavailable and shows the others", () => {
    const history = group(["history"]);
    const mixed = group(["history", "details"]);
    const hidden = group(["matrix"], false);
    syncVisibility({ groups: [history, mixed, hidden], panels: [] }, (p) => p !== "history");
    expect(history.api.isVisible).toBe(false);
    expect(mixed.api.isVisible).toBe(true);
    expect(hidden.api.isVisible).toBe(true);
  });
  it("only touches groups that change", () => {
    const g = group(["matrix"]);
    syncVisibility({ groups: [g], panels: [] }, () => true);
    expect(g.api.calls).toEqual([]);
  });
  it("leaves an empty group visible", () => {
    const g = group([], true);
    syncVisibility({ groups: [g], panels: [] }, () => false);
    expect(g.api.calls).toEqual([]);
  });
});

describe("pruneUnknown", () => {
  it("closes panels no plugin provides", () => {
    const closed: string[] = [];
    const panel = (id: string) => ({ id, api: { close: () => void closed.push(id) } });
    const dock: DockLike = { groups: [], panels: [panel("matrix"), panel("gone"), panel("details")] };
    pruneUnknown(dock, (p) => p !== "gone");
    expect(closed).toEqual(["gone"]);
  });
});

describe("parseDock", () => {
  it("rejects missing, invalid or foreign JSON", () => {
    expect(parseDock(null)).toBeNull();
    expect(parseDock("{nope")).toBeNull();
    expect(parseDock("42")).toBeNull();
    expect(parseDock('{"kind":"split","id":"root"}')).toBeNull();
  });
  it("accepts a dockview layout", () => {
    const v = { grid: { root: {}, width: 1, height: 1, orientation: "HORIZONTAL" }, panels: {} };
    expect(parseDock(JSON.stringify(v))).toEqual(v);
  });
});

describe("loadDock", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("reads the stored layout", () => {
    const v = { grid: {}, panels: {} };
    vi.stubGlobal("localStorage", { getItem: (k: string) => (k === DOCK_KEY ? JSON.stringify(v) : null) });
    expect(loadDock()).toEqual(v);
  });
  it("is null when storage throws", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("denied");
      },
    });
    expect(loadDock()).toBeNull();
  });
});
