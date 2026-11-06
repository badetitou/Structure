import { describe, expect, it } from "vitest";
import { adjacency, reach } from "./graph";

const e = (from: string, to: string) => ({ from, to, weight: 1 });
const edges = [e("a", "b"), e("b", "c"), e("d", "c"), e("x", "y")];
const adj = adjacency(edges);

describe("reach", () => {
  it("forward", () => {
    expect([...reach("a", adj, "forward")].sort()).toEqual(["b", "c"]);
  });
  it("backward", () => {
    expect([...reach("c", adj, "backward")].sort()).toEqual(["a", "b", "d"]);
  });
  it("disconnected", () => {
    expect(reach("x", adj, "backward").size).toBe(0);
    expect(reach("a", adj, "forward").has("y")).toBe(false);
  });
  it("cycles terminate and include start", () => {
    const cyc = adjacency([e("a", "b"), e("b", "a"), e("b", "c")]);
    expect([...reach("a", cyc, "forward")].sort()).toEqual(["a", "b", "c"]);
    expect([...reach("c", cyc, "backward")].sort()).toEqual(["a", "b"]);
  });
  it("is built once and reused for any number of queries", () => {
    expect(adj.forward.get("a")).toEqual(["b"]);
    expect(adj.backward.get("c")).toEqual(["b", "d"]);
    expect(reach("a", adj, "forward")).toEqual(reach("a", adj, "forward"));
  });
  it("walks a long chain in linear time", () => {
    const n = 200_000;
    const chain = adjacency(Array.from({ length: n }, (_, i) => e(`n${i}`, `n${i + 1}`)));
    const t = performance.now();
    expect(reach("n0", chain, "forward").size).toBe(n);
    expect(performance.now() - t).toBeLessThan(1000);
  });
});
