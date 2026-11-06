import { describe, expect, it } from "vitest";
import { layoutLanes, PALETTE } from "./lanes";

const c = (id: string, ...parents: string[]) => ({ id, parents });

describe("layoutLanes", () => {
  it("keeps a linear history on lane 0", () => {
    const { rows, width } = layoutLanes([c("a", "b"), c("b", "c"), c("c")]);
    expect(width).toBe(1);
    expect(rows.map((r) => r.lane)).toEqual([0, 0, 0]);
    expect(rows[0].top).toEqual([]);
    expect(rows[1].top).toHaveLength(1);
    expect(rows[2].bottom).toEqual([]);
    expect(new Set(rows.map((r) => r.color)).size).toBe(1);
  });

  it("forks out at a merge and converges at the fork point", () => {
    const { rows, width } = layoutLanes([c("m", "a", "s"), c("s", "r"), c("a", "r"), c("r")]);
    expect(width).toBe(2);
    expect(rows[0].bottom.map((s) => [s.from, s.to])).toEqual([
      [0, 0],
      [0, 1],
    ]);
    expect(rows[0].bottom[1].color).not.toBe(rows[0].color);
    expect(rows[1].lane).toBe(1);
    expect(rows[1].through.map((s) => s.from)).toEqual([0]);
    expect(rows[2].lane).toBe(0);
    expect(rows[2].through.map((s) => s.from)).toEqual([1]);
    expect(rows[3].lane).toBe(0);
    expect(rows[3].top.map((s) => [s.from, s.to])).toEqual([
      [0, 0],
      [1, 0],
    ]);
  });

  it("uses separate lanes for two parallel branches", () => {
    const { rows, width } = layoutLanes([c("x", "r"), c("y", "r"), c("r")]);
    expect(width).toBe(2);
    expect(rows.map((r) => r.lane)).toEqual([0, 1, 0]);
    expect(rows[0].color).not.toBe(rows[1].color);
    expect(rows[2].top.map((s) => s.from)).toEqual([0, 1]);
  });

  it("handles a merge with three parents", () => {
    const { rows, width } = layoutLanes([c("m", "a", "b", "d"), c("a"), c("b"), c("d")]);
    expect(width).toBe(3);
    expect(rows[0].bottom.map((s) => s.to)).toEqual([0, 1, 2]);
    expect(rows.map((r) => r.lane)).toEqual([0, 0, 1, 2]);
    expect(new Set(rows[0].bottom.map((s) => s.color)).size).toBe(3);
  });

  it("stops the lane when the parent is outside the list", () => {
    const { rows, width } = layoutLanes([c("a", "gone"), c("b", "gone2")]);
    expect(rows[0].bottom).toEqual([{ from: 0, to: 0, color: rows[0].color }]);
    expect(width).toBe(1);
    expect(rows[1].lane).toBe(0);
    expect(rows[1].top).toEqual([]);
  });

  it("reuses a lane freed by a merge", () => {
    const { rows, width } = layoutLanes([
      c("m", "a", "s"),
      c("s", "a"),
      c("a", "r"),
      c("x", "r"),
      c("r"),
    ]);
    // lane 1 (side) converged into a on lane 0 and is free again: x takes it
    expect(rows[2].lane).toBe(0);
    expect(rows[3].lane).toBe(1);
    expect(width).toBe(2);
    expect(rows[4].top.map((s) => s.from)).toEqual([0, 1]);
  });

  it("cycles the palette", () => {
    const commits = Array.from({ length: PALETTE.length + 1 }, (_, i) => c(`p${i}`, "root"));
    const { rows } = layoutLanes([...commits, c("root")]);
    expect(rows[PALETTE.length].color).toBe(rows[0].color);
  });
});
