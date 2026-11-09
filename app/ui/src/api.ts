// Typed wrappers over the Tauri commands/events described in docs/api.md.
// Outside Tauri (plain `npm run dev`), falls back to the sample JSON files.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { expandMatrix } from "./expand";

export type Language = "java" | "typescript";
export type Layer =
  | "ui"
  | "state"
  | "api_client"
  | "entry"
  | "business"
  | "persistence"
  | "model"
  | "util"
  | "unknown";

export interface CellSymbol {
  id: string;
  name: string;
  file: string;
}
export interface Cell {
  id: string;
  layer: Layer;
  domain: string;
  /** Number of top-level symbols; list them with `cellSymbols`. */
  size: number;
}
export interface CellSymbols {
  total: number;
  offset: number;
  symbols: CellSymbol[];
}
/** Wire form of the edges: grouped by source cell, cells as indices into `cells`. */
export interface EdgeGroup {
  from: number;
  targets: [number, number][];
}
export interface CellEdge {
  from: string;
  to: string;
  weight: number;
}
export interface Stats {
  files: number;
  symbols: number;
  edges: number;
  elapsed_ms: number;
}
/** Domain hierarchy (#16): leaves carry `domain`, groups carry children. */
export interface DomainNode {
  label: string;
  domain?: string;
  size: number;
  children: DomainNode[];
}
/** Matrix as sent by the backend (docs/api.md). */
export type WireMatrix = Omit<Matrix, "edges" | "httpEdges"> & {
  edges: EdgeGroup[];
  /** Same wire form as `edges`; present only for several folders, when non-empty. */
  http_edges?: EdgeGroup[];
};
export interface ProjectInfo {
  name: string;
  root: string;
  language: Language;
}
export interface Matrix {
  project: string;
  language: Language;
  /** Present only when several folders were analysed together. */
  projects?: ProjectInfo[];
  layers: Layer[];
  domains: string[];
  /** Present only when the domains form a hierarchy. */
  domain_tree?: DomainNode[];
  cells: Cell[];
  edges: CellEdge[];
  /** Front-to-back HTTP links (api client cell -> handler cell); empty unless several folders were analysed. */
  httpEdges: CellEdge[];
  stats: Stats;
  /** Id of the cached analysis (pass to `cellSymbols`); set by the app. */
  generation?: number;
  /** Present only when non-empty (indexing failed, stale index...). */
  warnings?: string[];
}
export interface ImpactedCell {
  id: string;
  distance: number;
}
export interface ImpactedSymbol {
  id: string;
  distance: number;
}
export interface Impact {
  base: string;
  head: string | null;
  changed_files: string[];
  changed_symbols: string[];
  changed_cells: string[];
  impacted_cells: ImpactedCell[];
  impacted_symbols: ImpactedSymbol[];
  warnings?: string[];
  /** Present when `head` was analysed in its snapshot: the matrix at that commit. */
  matrix?: Matrix;
}
type WireImpact = Omit<Impact, "matrix"> & { matrix?: WireMatrix };
export interface Commit {
  id: string;
  short: string;
  parents: string[];
  /** Diff base: first parent, or the empty tree for a root commit. */
  base: string;
  subject: string;
  author: string;
  /** ISO 8601 */
  date: string;
  refs: string[];
}
export interface History {
  dirty: boolean;
  commits: Commit[];
}
export type FileStatus = "added" | "deleted" | "modified" | "renamed";
export type LineKind = "context" | "add" | "del";
export interface DiffLine {
  kind: LineKind;
  /** 1-based; absent for an added line. */
  old?: number;
  /** 1-based; absent for a removed line. */
  new?: number;
  text: string;
}
export interface Hunk {
  header: string;
  lines: DiffLine[];
}
export interface FileDiff {
  path: string;
  /** Only for a rename. */
  old_path?: string;
  status: FileStatus;
  binary: boolean;
  additions: number;
  deletions: number;
  hunks: Hunk[];
  /** Some lines were left out (counts stay exact). */
  truncated: boolean;
}
export interface Diff {
  base: string;
  head: string | null;
  files: FileDiff[];
  /** Files or lines were left out to bound the payload. */
  truncated: boolean;
}
export interface Progress {
  step: string;
  pct: number;
  detail?: string;
}

export const inTauri = "__TAURI_INTERNALS__" in window;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function fetchSample<T>(name: string): Promise<T> {
  const res = await fetch(`/samples/${name}.json`);
  if (!res.ok) throw new Error(`sample ${name}: HTTP ${res.status}`);
  return (await res.json()) as T;
}

export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return "/browser/sample-project";
  const sel = await open({ directory: true, multiple: false });
  return typeof sel === "string" ? sel : null;
}

/** Analyses one folder, or several together (`paths.length > 1`). */
export async function analyze(
  paths: string[],
  onProgress: (p: Progress) => void,
): Promise<Matrix> {
  if (!inTauri) {
    onProgress({
      step: "indexing",
      pct: 8,
      detail: "[INFO] Building sample 1.0 [2/12] · 4s",
    });
    await sleep(250);
    for (const [step, pct] of [
      ["indexing", 10],
      ["parsing", 40],
      ["linking", 65],
      ["classifying", 85],
      ["building matrix", 100],
    ] as [string, number][]) {
      onProgress({ step, pct });
      await sleep(250);
    }
    const name = new URLSearchParams(location.search).get("sample");
    return expandMatrix(await fetchSample<WireMatrix>(name && /^[\w-]+$/.test(name) ? name : "matrix"));
  }
  const unlisten = await listen<Progress>("analysis-progress", (e) =>
    onProgress(e.payload),
  );
  try {
    return expandMatrix(await invoke<WireMatrix>("analyze", { paths }));
  } finally {
    unlisten();
  }
}

/** Impact of `base..head`; a commit snapshot may be analysed first, its
 * progress goes to `onProgress`. */
export async function impact(
  path: string,
  base: string | null,
  head: string | null,
  onProgress: (p: Progress) => void = () => {},
): Promise<Impact> {
  let w: WireImpact;
  if (inTauri) {
    const unlisten = await listen<Progress>("analysis-progress", (e) =>
      onProgress(e.payload),
    );
    try {
      w = await invoke<WireImpact>("impact", { path, base, head });
    } finally {
      unlisten();
    }
  } else {
    for (const [step, pct] of [
      ["checking out", 2],
      ["parsing", 40],
      ["computing impact", 80],
    ] as [string, number][]) {
      onProgress({ step, pct });
      await sleep(300);
    }
    w = await fetchSample<WireImpact>("impact");
  }
  const { matrix, ...rest } = w;
  return matrix ? { ...rest, matrix: expandMatrix(matrix) } : rest;
}

/** One page of the symbols of `cells` (sorted by name); `head` = commit whose
 * matrix is shown, null for the working tree. */
export async function cellSymbols(
  paths: string[],
  head: string | null,
  generation: number,
  cells: string[],
  offset: number,
  limit: number,
): Promise<CellSymbols> {
  if (!inTauri) {
    const all = await fetchSample<Record<string, CellSymbol[]>>("cell-symbols");
    const list = cells.flatMap((c) => all[c] ?? []);
    return { total: list.length, offset, symbols: list.slice(offset, offset + limit) };
  }
  return invoke<CellSymbols>("cell_symbols", { paths, head, generation, cells, offset, limit });
}

export async function history(path: string, limit?: number): Promise<History> {
  if (!inTauri) return fetchSample<History>("history");
  return invoke<History>("history", { path, limit: limit ?? null });
}

/** Textual diff of `base..head`, same meaning as `impact` (null base = HEAD vs working tree). */
export async function diff(path: string, base: string | null, head: string | null): Promise<Diff> {
  if (!inTauri) return fetchSample<Diff>("diff");
  return invoke<Diff>("diff", { path, base, head });
}
