// Fixtures shared by the store tests. Each test file mocks "../api" itself (vi.mock is
// hoisted per file); the mock then applies here too.
import { vi } from "vitest";
import * as api from "../api";
import type { Capabilities, Impact, Matrix } from "../api";
import type { ViewCell } from "../domaintree";
import { DEFAULT_LENSES } from "../lens";
import { load, reset } from "./session";

export const mocked = vi.mocked(api);
export const flush = () => new Promise((r) => setTimeout(r, 0));

export const mx = (project: string, warnings?: string[]): Matrix => ({
  project,
  language: "java",
  layers: [],
  domains: [],
  cells: [],
  edges: [],
  httpEdges: [],
  rpcEdges: [],
  stats: { files: 1, symbols: 1, edges: 0, elapsed_ms: 1 },
  generation: 7,
  warnings,
});
export const imp = (over: Partial<Impact> = {}): Impact => ({
  base: "a",
  head: null,
  changed_files: [],
  changed_symbols: [],
  changed_cells: [],
  impacted_cells: [],
  impacted_symbols: [],
  ...over,
});
export const cell = { id: "c1", layer: "service", label: "orders", size: 2, members: ["m1"] } as unknown as ViewCell;
/** Matrix of generation `generation` whose cells are the `[id, size]` pairs. */
export const withCells = (generation: number, ...ids: [string, number][]): Matrix => ({
  ...mx("p"),
  generation,
  cells: ids.map(([id, size]) => ({ id, layer: "service", domain: id, size })),
});
/** An empty page of cell symbols. */
export const page = { total: 0, offset: 0, symbols: [] };
export const twoProjects = [
  { name: "front", root: "/front", language: "typescript" as const },
  { name: "back", root: "/back", language: "java" as const },
];

export const allCaps: Capabilities = { history: true, diff: true, pickFolder: true, reclassify: true, highlightSources: ["git-diff"], lenses: DEFAULT_LENSES };
export const viewerCaps: Capabilities = { history: false, diff: false, pickFolder: false, reclassify: false, highlightSources: ["git-diff"], lenses: DEFAULT_LENSES };

/** Fresh mocks (full capabilities) and empty stores; late answers of a previous test are ignored. */
export function setup() {
  vi.resetAllMocks();
  mocked.capabilities.mockReturnValue(allCaps);
  mocked.lenses.mockResolvedValue(DEFAULT_LENSES);
  reset();
}

/** `/p` analysed as `work`, with an empty history. */
export async function loaded(work = mx("work")) {
  mocked.analyze.mockResolvedValue(work);
  mocked.history.mockResolvedValue({ dirty: false, commits: [] });
  await load("/p");
}
