import type { Component } from "svelte";
import { describe, expect, it } from "vitest";
import { PanelRegistry } from "./registry";

const C = (() => {}) as unknown as Component;

describe("PanelRegistry", () => {
  it("lists contributions in registration order", () => {
    const r = new PanelRegistry();
    r.register({ id: "b", title: "B", component: C });
    r.register({ id: "a", title: "A", component: C });
    expect(r.all().map((c) => c.id)).toEqual(["b", "a"]);
    expect(r.get("a")?.title).toBe("A");
    expect(r.has("c")).toBe(false);
  });
  it("refuses a second panel with the same id", () => {
    const r = new PanelRegistry();
    r.register({ id: "a", title: "A", component: C });
    expect(() => r.register({ id: "a", title: "Other", component: C })).toThrow(/already registered/);
  });
  it("is available when registered and its predicate holds", () => {
    const r = new PanelRegistry();
    let on = false;
    r.register({ id: "a", title: "A", component: C });
    r.register({ id: "b", title: "B", component: C, available: () => on });
    const { isAvailable } = r;
    expect(isAvailable("a")).toBe(true);
    expect(isAvailable("b")).toBe(false);
    on = true;
    expect(isAvailable("b")).toBe(true);
    expect(isAvailable("missing")).toBe(false);
  });
});
