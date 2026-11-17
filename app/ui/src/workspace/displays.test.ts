import { describe, expect, it } from "vitest";
import type { ViewMatrix } from "../domaintree";
import { PanelDisplays } from "./displays.svelte";
import { snapshot } from "./paneldisplay";
import type { PanelDisplay } from "./paneldisplay";

const globals = (): PanelDisplay => ({
  mode: "normal",
  transposed: false,
  aligned: true,
  showHttp: true,
  showRpc: true,
  lens: "layer-domain",
});

function setup() {
  let defaults = globals();
  const saved: Partial<PanelDisplay>[] = [];
  const d = new PanelDisplays(
    () => defaults,
    (patch) => {
      saved.push(patch);
      defaults = { ...defaults, ...patch };
    },
  );
  const writes = new Map<string, PanelDisplay[]>();
  const persist = (id: string) => (x: PanelDisplay) => void writes.set(id, [...(writes.get(id) ?? []), x]);
  return { d, saved, writes, persist, defaults: () => defaults };
}

describe("panel displays", () => {
  it("adopt the global defaults once for a panel without stored options (migration)", () => {
    const { d, writes, persist } = setup();
    const s = d.attach("matrix", undefined, persist("matrix"));
    expect(s.mode).toBe("normal");
    expect(writes.get("matrix")).toEqual([globals()]);
    // Attached again (mounted again): the same state, no second write.
    expect(d.attach("matrix", undefined, persist("matrix"))).toBe(s);
    expect(writes.get("matrix")).toHaveLength(1);
  });

  it("keep a panel's stored options and do not write them back", () => {
    const { d, writes, persist } = setup();
    const display = { mode: "flame", transposed: true, aligned: false, showHttp: false, showRpc: false, lens: "layer-package" } as const;
    const s = d.attach("matrix-2", { display }, persist("matrix-2"));
    expect(snapshot(s)).toEqual(display);
    expect(writes.has("matrix-2")).toBe(false);
  });

  it("set the active panel's options, write them to its params and to the defaults", () => {
    const { d, writes, persist, saved, defaults } = setup();
    const a = d.attach("matrix", undefined, persist("matrix"));
    const b = d.attach("matrix-2", { display: { mode: "dense" } }, persist("matrix-2"));
    expect(d.activeId).toBe("matrix");
    d.set({ transposed: true });
    expect(a.transposed).toBe(true);
    expect(b.transposed).toBe(false);
    expect(writes.get("matrix")?.at(-1)).toMatchObject({ transposed: true, mode: "normal" });
    expect(saved).toEqual([{ transposed: true }]);
    d.focus("matrix-2");
    expect(d.active).toBe(b);
    d.set({ mode: "flame" });
    expect([a.mode, b.mode]).toEqual(["normal", "flame"]);
    // A given panel, whatever the active one.
    d.set({ showRpc: false }, "matrix");
    expect([a.showRpc, b.showRpc]).toEqual([false, true]);
    expect(defaults()).toMatchObject({ transposed: true, mode: "flame", showRpc: false });
    // A new panel starts from the defaults (the last choices).
    expect(d.attach("matrix-3", undefined, persist("matrix-3")).mode).toBe("flame");
  });

  it("set only the defaults while no panel is open", () => {
    const { d, saved } = setup();
    expect(d.active).toBeNull();
    d.set({ aligned: false });
    expect(saved).toEqual([{ aligned: false }]);
  });

  it("fall back to the last focused panel when the active one closes", () => {
    const { d, persist } = setup();
    d.attach("matrix", undefined, persist("matrix"));
    d.attach("matrix-2", undefined, persist("matrix-2"));
    d.focus("details"); // not a display panel: ignored
    expect(d.activeId).toBe("matrix");
    d.focus("matrix-2");
    d.focus("matrix");
    d.focus("matrix-2");
    d.drop("matrix-2");
    expect(d.activeId).toBe("matrix");
    d.drop("matrix");
    expect(d.activeId).toBeNull();
  });

  it("take a focus that came before the panel was attached", () => {
    const { d, persist } = setup();
    d.attach("matrix", undefined, persist("matrix"));
    d.focus("matrix-2");
    d.attach("matrix-2", undefined, persist("matrix-2"));
    expect(d.activeId).toBe("matrix-2");
  });

  it("name the active panel's view and forget all on clear", () => {
    const { d, persist } = setup();
    d.attach("matrix", undefined, persist("matrix"));
    d.attach("matrix-2", undefined, persist("matrix-2"));
    const va = { cells: [] } as unknown as ViewMatrix;
    const vb = { cells: [] } as unknown as ViewMatrix;
    d.publishView("matrix", va);
    d.publishView("matrix-2", vb);
    expect(d.activeView).toBe(va);
    d.focus("matrix-2");
    expect(d.activeView).toBe(vb);
    d.publishView("matrix-2", null);
    expect(d.activeView).toBeNull();
    d.clear();
    expect(d.activeId).toBeNull();
    expect(d.get("matrix")).toBeUndefined();
  });
});
