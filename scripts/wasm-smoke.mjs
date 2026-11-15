// Smoke test of the built wasm module (run scripts/build-wasm.sh first).
// Usage: node scripts/wasm-smoke.mjs
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const pkg = fileURLToPath(new URL("../crates/wasm/pkg/", import.meta.url));
const glue = await import(pkg + "structure_wasm.js");
const bytes = readFileSync(pkg + "structure_wasm_bg.wasm");

// No unresolved imports beyond wasm-bindgen's own module.
const mod = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(mod);
const foreign = imports.filter((i) => i.module !== "./structure_wasm_bg.js");
assert.deepEqual(foreign, [], "unexpected imports: " + JSON.stringify(foreign));

glue.initSync({ module: mod });

const files = [
  {
    path: "src/main/java/p/Svc.java",
    content:
      "package p;\n@Service\npublic class Svc {\n    public int run() {\n        return 1;\n    }\n}\n",
  },
  {
    path: "src/main/java/p/Ctl.java",
    content:
      "package p;\n@RestController\npublic class Ctl {\n    private Svc svc;\n    public int get() {\n        return svc.run();\n    }\n}\n",
  },
];
const p = "src/main/java/p/Svc.java";
const diff = `diff --git a/${p} b/${p}\n--- a/${p}\n+++ b/${p}\n@@ -1,3 +1,3 @@\n a\n-b\n+c\n d\n`;

const a = new glue.PrAnalysis(JSON.stringify(files), diff, "demo", "base", "head");
const impact = JSON.parse(a.impact());
assert.ok(impact.matrix && impact.matrix.cells.length > 0, "empty matrix");
assert.equal(impact.changed_cells.length, 1, "expected one changed cell");
assert.ok(impact.impacted_cells.length >= 1, "expected an impacted cell");
const syms = JSON.parse(a.cell_symbols(JSON.stringify(impact.changed_cells), 0, 10));
assert.equal(syms.total, 1);

let threw = false;
try {
  new glue.PrAnalysis("[]", "", "demo", "b", "h");
} catch (e) {
  threw = /no supported sources/.test(String(e.message ?? e));
}
assert.ok(threw, "expected an error for no sources");

console.log("wasm smoke OK:", impact.matrix.cells.length, "cells, changed", impact.changed_cells);
