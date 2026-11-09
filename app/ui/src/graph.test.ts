import { describe, expect, it } from "vitest";
import { adjacency, remoteLinks, reach, visibleEdges } from "./graph";

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
  it("visits every node once on a long chain plus a wide fan-out", () => {
    const n = 5000;
    const es = [
      ...Array.from({ length: n }, (_, i) => e(`c${i}`, `c${i + 1}`)),
      ...Array.from({ length: n }, (_, i) => e("c0", `f${i}`)),
      // every fan-out leaf also points at the chain end: extra edges, no extra nodes
      ...Array.from({ length: n }, (_, i) => e(`f${i}`, `c${n}`)),
    ];
    const g = adjacency(es);
    expect(reach("c0", g, "forward").size).toBe(2 * n); // c1..cn and f0..f(n-1)
    expect(reach(`c${n}`, g, "backward").size).toBe(2 * n); // c0..c(n-1) and f*
  });
});

describe("http links", () => {
  const http = [{ from: "front", to: "back", weight: 3 }, { from: "front", to: "back2", weight: 1 }];
  it("hover reach crosses http edges only while shown", () => {
    const shown = adjacency(visibleEdges(edges, http, true));
    expect([...reach("front", shown, "forward")].sort()).toEqual(["back", "back2"]);
    expect([...reach("back", shown, "backward")]).toEqual(["front"]);
    const hidden = adjacency(visibleEdges(edges, http, false));
    expect(reach("front", hidden, "forward").size).toBe(0);
  });
  it("hover reach crosses rpc edges only while rpc is shown", () => {
    const rpc = [{ from: "front", to: "srv", weight: 2 }];
    const both = adjacency(visibleEdges(edges, http, true, rpc, true));
    expect([...reach("front", both, "forward")].sort()).toEqual(["back", "back2", "srv"]);
    const noRpc = adjacency(visibleEdges(edges, http, true, rpc, false));
    expect([...reach("front", noRpc, "forward")].sort()).toEqual(["back", "back2"]);
    const onlyRpc = adjacency(visibleEdges(edges, http, false, rpc, true));
    expect([...reach("front", onlyRpc, "forward")]).toEqual(["srv"]);
    expect(remoteLinks(rpc, ["srv"])).toEqual({ out: [], in: [{ other: "front", count: 2 }] });
  });
  it("lists a cell's outgoing and incoming links with counts", () => {
    expect(remoteLinks(http, ["front"])).toEqual({
      out: [{ other: "back", count: 3 }, { other: "back2", count: 1 }],
      in: [],
    });
    expect(remoteLinks(http, ["back"])).toEqual({ out: [], in: [{ other: "front", count: 3 }] });
  });
});
