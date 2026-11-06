import { describe, expect, it } from "vitest";
import type { Impact, Matrix } from "./api";
import { RequestGuard, shownMatrix } from "./matrixsel";

const mk = (project: string) => ({ project }) as unknown as Matrix;
const imp = (matrix?: Matrix) => ({ base: "a", head: "b", matrix }) as unknown as Impact;

describe("RequestGuard", () => {
  it("only the latest request is current", () => {
    const g = new RequestGuard();
    const a = g.begin();
    const b = g.begin();
    expect(g.isCurrent(a)).toBe(false);
    expect(g.isCurrent(b)).toBe(true);
  });
  it("invalidate makes pending requests stale", () => {
    const g = new RequestGuard();
    const a = g.begin();
    g.invalidate();
    expect(g.isCurrent(a)).toBe(false);
  });
});

describe("shownMatrix", () => {
  const work = mk("work");
  it("uses the working-tree matrix without impact or snapshot", () => {
    expect(shownMatrix(work, null)).toBe(work);
    expect(shownMatrix(work, imp())).toBe(work);
  });
  it("uses the snapshot matrix when present", () => {
    const snap = mk("snap");
    expect(shownMatrix(work, imp(snap))).toBe(snap);
  });
});
