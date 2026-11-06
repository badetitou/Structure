import { describe, expect, it } from "vitest";
import { appendPage, hasMore } from "./cellsymbols";
import type { SymbolList } from "./cellsymbols";

const sym = (id: string) => ({ id, name: id, file: "f" });
const empty: SymbolList = { cell: "ui:a", total: 0, items: [], loading: true };

describe("appendPage", () => {
  it("accumulates consecutive pages", () => {
    let l = appendPage(empty, { total: 3, offset: 0, symbols: [sym("a"), sym("b")] });
    expect(l.loading).toBe(false);
    expect(hasMore(l)).toBe(true);
    l = appendPage(l, { total: 3, offset: 2, symbols: [sym("c")] });
    expect(l.items.map((s) => s.id)).toEqual(["a", "b", "c"]);
    expect(hasMore(l)).toBe(false);
  });
  it("ignores a page that is not the next one", () => {
    const l = appendPage(empty, { total: 3, offset: 2, symbols: [sym("c")] });
    expect(l.items).toEqual([]);
  });
});
