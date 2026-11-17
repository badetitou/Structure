import { describe, expect, it } from "vitest";

// Views read and act through the view context (context.ts), never the stores. A source scan over
// the components and the .ts modules they import: a store import reached from a view is the
// decoupling this guards against.
// Vite keys files of this directory as `./x`; they are rekeyed `../workspace/x` so every key is
// relative to `src/`'s parent the same way, as `resolve` builds them.
const sources: Record<string, string> = Object.fromEntries(
  Object.entries(
    import.meta.glob<string>(["../**/*.svelte", "../**/*.ts", "!../**/*.test.ts", "!../**/*.d.ts"], {
      query: "?raw",
      import: "default",
      eager: true,
    }),
  ).map(([k, v]) => [k.startsWith("./") ? `../workspace/${k.slice(2)}` : k, v]),
);

const BUILTIN = "../workspace/builtin.ts";
/** The view context: the one module views go through to reach the stores. The scan stops there. */
const CONTEXT = "../workspace/context.ts";

/** Components allowed to import the stores: the analysis screen owns the session (load, reset,
 * rules watch, and provides the view context); the progress screen shows the analysis running,
 * before any view exists. */
const STORE_USERS = new Set(["../lib/components/AnalysisScreen.svelte", "../lib/components/Progress.svelte"]);
/** Modules allowed to import the stores, besides the stores themselves: the view context, and
 * the built-in registrations (panel availability and default layout read the shown matrix and
 * git capability). */
const STORE_MODULES = new Set([CONTEXT, BUILTIN]);

const importsOf = (src: string): string[] =>
  [...src.matchAll(/^\s*(?:import|export)\s[^;]*?from\s+"([^"]+)"/gm)].map((m) => m[1]);
const importsStores = (src: string) => importsOf(src).some((i) => /(^|\/)stores\//.test(i));

/** Resolves a relative or `$lib/` import of `from` to a key of `sources` (`.svelte`, `.ts`,
 * `.svelte.ts`, `index.ts`, or a `.js` specifier naming a `.ts` file). */
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
  const candidates = [key, `${key}.ts`, `${key}/index.ts`, key.replace(/\.js$/, ".ts")];
  return candidates.find((c) => c in sources) ?? null;
}

describe("views", () => {
  it("registered panels and the modules they reach do not import the stores", () => {
    const builtin = sources[BUILTIN];
    expect(builtin).toBeDefined();
    const roots = importsOf(builtin ?? "")
      .filter((i) => i.startsWith("$lib/components/"))
      .map((i) => resolve(BUILTIN, i));
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
      if (f === CONTEXT) continue;
      const src = sources[f] ?? "";
      expect(importsStores(src), `${f} imports a store`).toBe(false);
      for (const i of importsOf(src)) {
        const r = resolve(f, i);
        if (r) todo.push(r);
      }
    }
    // The matrix and its overlay selector are reached through MatrixPanel, and .ts helpers too.
    expect(seen).toContain("../lib/components/MatrixView.svelte");
    expect(seen).toContain("../lib/components/OverlaySelector.svelte");
    expect(seen).toContain("../lib/components/matrix/MatrixCell.svelte");
    expect(seen).toContain(CONTEXT);
    expect(seen).toContain("../domaintree.ts");
  });

  it("only the analysis screen, the progress screen and the view context import the stores", () => {
    const users = Object.entries(sources)
      .filter(([f, src]) => !f.startsWith("../stores/") && importsStores(src))
      .map(([f]) => f);
    expect(users.length).toBeGreaterThan(0);
    for (const f of users) expect(STORE_USERS.has(f) || STORE_MODULES.has(f), `${f} imports a store`).toBe(true);
  });
});
