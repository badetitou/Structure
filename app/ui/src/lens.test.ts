import { describe, expect, it } from "vitest";
import type { Impact } from "./api";
import { DEFAULT_LENS, DEFAULT_LENSES, lensLabel, lensSelectorVisible, parseLens, resolveLens, withLensCells } from "./lens";

const both = [...DEFAULT_LENSES, { id: "layer-package", label: "Layer × package" }];

describe("lenses", () => {
  it("default to layer × domain", () => {
    expect(DEFAULT_LENS).toBe("layer-domain");
    expect(DEFAULT_LENSES.map((l) => l.id)).toEqual([DEFAULT_LENS]);
  });

  it("parse a stored id: any non-empty string, else the default lens", () => {
    expect(parseLens("layer-package")).toBe("layer-package");
    for (const v of [undefined, null, "", 3, {}]) expect(parseLens(v)).toBe(DEFAULT_LENS);
  });

  it("resolve to the default lens when the backend does not offer the one asked", () => {
    expect(resolveLens("layer-package", both)).toBe("layer-package");
    expect(resolveLens("layer-package", DEFAULT_LENSES)).toBe(DEFAULT_LENS);
    expect(resolveLens("gone", both)).toBe(DEFAULT_LENS);
  });

  it("show the selector only when there is a choice", () => {
    expect(lensSelectorVisible([])).toBe(false);
    expect(lensSelectorVisible(DEFAULT_LENSES)).toBe(false);
    expect(lensSelectorVisible(both)).toBe(true);
  });

  it("label a lens, the id when unknown", () => {
    expect(lensLabel("layer-package", both)).toBe("Layer × package");
    expect(lensLabel("gone", both)).toBe("gone");
  });

  it("swap an impact's cells for another lens' (none while unknown)", () => {
    const imp: Impact = {
      base: "a",
      head: null,
      changed_files: ["f"],
      changed_symbols: ["S"],
      changed_cells: ["b:x"],
      impacted_cells: [{ id: "b:y", distance: 1 }],
      impacted_symbols: [{ id: "T", distance: 1 }],
    };
    const cells = { changed_cells: ["package/b:p"], impacted_cells: [{ id: "package/b:q", distance: 1 }] };
    expect(withLensCells(imp, cells)).toEqual({ ...imp, ...cells });
    expect(withLensCells(imp, null)).toEqual({ ...imp, changed_cells: [], impacted_cells: [] });
    expect(imp.changed_cells).toEqual(["b:x"]);
  });
});
