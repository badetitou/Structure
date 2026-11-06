import { describe, expect, it } from "vitest";
import { CAP, initialCaps, moreLabel, showMore } from "./symbolcap";

describe("symbol list cap", () => {
  it("starts at 200 per list", () => {
    expect(CAP).toBe(200);
    expect(initialCaps()).toEqual({ changed: 200, impacted: 200 });
  });
  it("grows one list by a page and leaves the other alone", () => {
    expect(showMore(initialCaps(), "impacted")).toEqual({ changed: 200, impacted: 400 });
  });
  it("a new impact starts over from fresh caps", () => {
    const grown = showMore(showMore(initialCaps(), "changed"), "changed");
    expect(grown.changed).toBe(600);
    expect(initialCaps().changed).toBe(200);
  });
  it("labels the button only while some are hidden", () => {
    expect(moreLabel(200, 450)).toBe("Show more (200 / 450)");
    expect(moreLabel(200, 200)).toBeNull();
    expect(moreLabel(200, 3)).toBeNull();
  });
});
