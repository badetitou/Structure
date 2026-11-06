import { describe, expect, it } from "vitest";
import type { Impact, Matrix } from "./api";
import { shownMatrix } from "./matrixsel";

const mk = (project: string) => ({ project }) as unknown as Matrix;
const imp = (matrix?: Matrix) => ({ base: "a", head: "b", matrix }) as unknown as Impact;

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
