// Runs the extension pipeline against the real GitHub API with the built wasm.
// Usage: node scripts/try-pr.mjs <owner>/<repo>#<number>   (GITHUB_TOKEN optional)
// Needs crates/wasm/pkg (scripts/build-wasm.sh) and a Node that strips TypeScript types
// (>= 22.18): it imports src/extension/{github,pipeline,tar}.ts directly, so nothing is duplicated.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parsePrShorthand } from "../src/extension/github.ts";
import { fetchPr } from "../src/extension/pipeline.ts";

const ref = parsePrShorthand(process.argv[2] ?? "");
if (!ref) {
  console.error("usage: node scripts/try-pr.mjs <owner>/<repo>#<number>");
  process.exit(2);
}

const pkg = fileURLToPath(new URL("../../../crates/wasm/pkg/", import.meta.url));
const glue = await import(pkg + "structure_wasm.js").catch(() => {
  console.error("crates/wasm/pkg is missing: run scripts/build-wasm.sh first");
  process.exit(1);
});
glue.initSync({ module: new WebAssembly.Module(readFileSync(pkg + "structure_wasm_bg.wasm")) });

const data = await fetchPr(ref, { token: process.env.GITHUB_TOKEN || undefined });
const analysis = new glue.PrAnalysis(
  JSON.stringify(data.files),
  data.diff,
  `${ref.owner}/${ref.repo}`,
  data.base,
  data.head,
);
const impact = JSON.parse(analysis.impact());
const exts = new Set(data.files.map((f) => f.path.slice(f.path.lastIndexOf("."))));
console.log(
  `${ref.owner}/${ref.repo}#${ref.number}: language=${[...exts].join(",")} files=${data.files.length}` +
    ` cells=${impact.matrix.cells.length} changed=${JSON.stringify(impact.changed_cells)}` +
    ` impacted=${JSON.stringify(impact.impacted_cells)}`,
);
