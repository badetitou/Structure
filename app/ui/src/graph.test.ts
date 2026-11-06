import { describe, expect, it } from "vitest";
import { reach } from "./graph";

const e = (from: string, to: string) => ({ from, to, weight: 1 });
const edges = [e("a", "b"), e("b", "c"), e("d", "c"), e("x", "y")];

describe("reach", () => {
  it("forward", () => {
    expect([...reach("a", edges, "forward")].sort()).toEqual(["b", "c"]);
  });
  it("backward", () => {
    expect([...reach("c", edges, "backward")].sort()).toEqual(["a", "b", "d"]);
  });
  it("disconnected", () => {
    expect(reach("x", edges, "backward").size).toBe(0);
    expect(reach("a", edges, "forward").has("y")).toBe(false);
  });
  it("cycles terminate and include start", () => {
    const cyc = [e("a", "b"), e("b", "a"), e("b", "c")];
    expect([...reach("a", cyc, "forward")].sort()).toEqual(["a", "b", "c"]);
    expect([...reach("c", cyc, "backward")].sort()).toEqual(["a", "b"]);
  });
});
