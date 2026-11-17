import { describe, expect, it } from "vitest";

// Views read and act through the view context (context.ts), never the stores. A source scan:
// a store import in a view is the decoupling this guards against.
const sources = import.meta.glob<string>("../lib/components/**/*.svelte", {
  query: "?raw",
  import: "default",
  eager: true,
});
const builtin = import.meta.glob<string>("./builtin.ts", { query: "?raw", import: "default", eager: true })["./builtin.ts"];

/** Components allowed to import the stores: the analysis screen owns the session (load, reset,
 * rules watch, and provides the view context); the progress screen shows the analysis running,
 * before any view exists. */
const STORE_USERS = new Set(["../lib/components/AnalysisScreen.svelte", "../lib/components/Progress.svelte"]);

const importsOf = (src: string): string[] => [...src.matchAll(/^\s*import\s[^;]*?from\s+"([^"]+)"/gm)].map((m) => m[1]);
const importsStores = (src: string) => importsOf(src).some((i) => /(^|\/)stores\//.test(i));

/** Resolves a relative import of `from` to a key of `sources`. */
function resolve(from: string, spec: string): string | null {
  let parts: string[];
  if (spec.startsWith("$lib/")) parts = ["..", "lib", ...spec.slice(5).split("/")];
  else if (spec.startsWith(".")) parts = [...from.split("/").slice(0, -1), ...spec.split("/")];
  else return null;
  const out: string[] = [];
  for (const p of parts) {
    if (p === "." || p === "") continue;
    if (p === ".." && out.length > 0 && out[out.length - 1] !== "..") out.pop();
    else out.push(p);
  }
  const key = out.join("/");
  return key in sources ? key : null;
}

describe("views", () => {
  it("registered panels and the components they render do not import the stores", () => {
    expect(builtin).toBeDefined();
    const roots = importsOf(builtin ?? "")
      .filter((i) => i.startsWith("$lib/components/"))
      .map((i) => resolve("./builtin.ts", i));
    expect(roots).toContain("../lib/components/MatrixPanel.svelte");
    expect(roots).toContain("../lib/components/DetailsPanel.svelte");
    expect(roots).toContain("../lib/components/HistorySidebar.svelte");
    expect(roots).toContain("../lib/components/DiffPanel.svelte");
    const seen = new Set<string>();
    const todo = roots.filter((r): r is string => r !== null);
    while (todo.length > 0) {
      const f = todo.pop() as string;
      if (seen.has(f)) continue;
      seen.add(f);
      const src = sources[f] ?? "";
      expect(importsStores(src), `${f} imports a store`).toBe(false);
      for (const i of importsOf(src)) {
        const r = i.endsWith(".svelte") ? resolve(f, i) : null;
        if (r) todo.push(r);
      }
    }
    // The matrix and its overlay selector are reached through MatrixPanel.
    expect(seen).toContain("../lib/components/MatrixView.svelte");
    expect(seen).toContain("../lib/components/OverlaySelector.svelte");
    expect(seen).toContain("../lib/components/matrix/MatrixCell.svelte");
  });

  it("only the analysis screen and the progress screen import the stores", () => {
    const users = Object.entries(sources)
      .filter(([, src]) => importsStores(src))
      .map(([f]) => f);
    expect(users.length).toBeGreaterThan(0);
    for (const f of users) expect(STORE_USERS.has(f), `${f} imports a store`).toBe(true);
  });
});
