import { afterEach, describe, expect, it, vi } from "vitest";
import { DOCK_KEY, loadDock, parseDock, pruneUnknown, setTabHiddenIn, syncVisibility } from "./dock";
import type { DockLike } from "./dock";

function group(ids: string[], visible = true, active?: string) {
  const g = {
    activePanel: active ? { id: active } : undefined,
    activated: [] as string[],
    panels: ids.map((id) => ({ id, api: { setActive: () => void g.activated.push(id) } })),
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

describe("syncVisibility with a mixed group", () => {
  it("hides the unavailable tab, shows the others, then restores it", () => {
    const g = group(["details", "diff"], true, "diff");
    const tabs = new Map<string, boolean>();
    const set = (id: string, hidden: boolean) => void tabs.set(id, hidden);
    let diffAvailable = false;
    const avail = (p: string) => p !== "diff" || diffAvailable;
    syncVisibility({ groups: [g], panels: [] }, avail, set);
    expect(tabs.get("diff")).toBe(true);
    expect(tabs.get("details")).toBe(false);
    expect(g.api.isVisible).toBe(true);
    expect(g.activated).toEqual(["details"]);
    diffAvailable = true;
    syncVisibility({ groups: [g], panels: [] }, avail, set);
    expect(tabs.get("diff")).toBe(false);
    expect(g.panels.map((p) => p.id)).toEqual(["details", "diff"]);
  });
  it("keeps the active panel when it is available", () => {
    const g = group(["details", "diff"], true, "details");
    syncVisibility({ groups: [g], panels: [] }, (p) => p !== "diff");
    expect(g.activated).toEqual([]);
  });
});

describe("setTabHiddenIn", () => {
  afterEach(() => vi.unstubAllGlobals());
  it("toggles the tab element of a panel", () => {
    const tab = { style: { display: "" } };
    const host = { querySelector: () => tab } as unknown as HTMLElement;
    vi.stubGlobal("CSS", { escape: (s: string) => s });
    setTabHiddenIn(host, "diff", true);
    expect(tab.style.display).toBe("none");
    setTabHiddenIn(host, "diff", false);
    expect(tab.style.display).toBe("");
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
