import { describe, expect, it } from "vitest";
import { setSvgAttr } from "./svgattr";

function fake() {
  const attrs = new Map<string, string>();
  const style = new Map<string, string>();
  const el = {
    setAttribute: (k: string, v: string) => void attrs.set(k, v),
    style: {
      setProperty: (k: string, v: string) => void style.set(k, v),
      removeProperty: (k: string) => void style.delete(k),
    },
  };
  return { el: el as unknown as Element, attrs, style };
}

describe("setSvgAttr", () => {
  it("routes var() values to style", () => {
    const f = fake();
    setSvgAttr(f.el, "fill", "var(--cell)");
    expect(f.style.get("fill")).toBe("var(--cell)");
    expect(f.attrs.has("fill")).toBe(false);
  });
  it("routes plain values to the attribute", () => {
    const f = fake();
    setSvgAttr(f.el, "opacity", 0.5);
    expect(f.attrs.get("opacity")).toBe("0.5");
  });
  it("a plain value clears a stale inline var()", () => {
    const f = fake();
    setSvgAttr(f.el, "fill", "var(--cell)");
    setSvgAttr(f.el, "fill", "none");
    expect(f.style.has("fill")).toBe(false);
    expect(f.attrs.get("fill")).toBe("none");
  });
});
