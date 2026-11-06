// Typed wrappers over the Tauri commands/events described in docs/api.md.
// Outside Tauri (plain `npm run dev`), falls back to the sample JSON files.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

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
  size: number;
  symbols: CellSymbol[];
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
export interface Matrix {
  project: string;
  language: Language;
  layers: Layer[];
  domains: string[];
  cells: Cell[];
  edges: CellEdge[];
  stats: Stats;
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
}
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
export interface Progress {
  step: string;
  pct: number;
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

export async function analyze(
  path: string,
  onProgress: (p: Progress) => void,
): Promise<Matrix> {
  if (!inTauri) {
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
    return fetchSample<Matrix>("matrix");
  }
  const unlisten = await listen<Progress>("analysis-progress", (e) =>
    onProgress(e.payload),
  );
  try {
    return await invoke<Matrix>("analyze", { path });
  } finally {
    unlisten();
  }
}

export async function impact(
  path: string,
  base: string | null,
  head: string | null,
): Promise<Impact> {
  if (!inTauri) return fetchSample<Impact>("impact");
  return invoke<Impact>("impact", { path, base, head });
}

export async function history(path: string, limit?: number): Promise<History> {
  if (!inTauri) return fetchSample<History>("history");
  return invoke<History>("history", { path, limit: limit ?? null });
}
