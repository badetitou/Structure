// Typed wrappers over the Tauri commands/events described in docs/api.md.
// Outside Tauri (plain `npm run dev`), falls back to the sample JSON files.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { expandMatrix } from "./expand";

// Wire types are generated from crates/ir (ts-rs) into ./generated: do not
// redeclare them here, regenerate them (see CLAUDE.md, Commands).
import type { Cell } from "./generated/Cell";
import type { CellSymbol } from "./generated/CellSymbol";
import type { CellSymbols } from "./generated/CellSymbols";
import type { Commit } from "./generated/Commit";
import type { Diff } from "./generated/Diff";
import type { DiffLine } from "./generated/DiffLine";
import type { DomainNode } from "./generated/DomainNode";
import type { EdgeGroup } from "./generated/EdgeGroup";
import type { FileDiff } from "./generated/FileDiff";
import type { FileStatus } from "./generated/FileStatus";
import type { History } from "./generated/History";
import type { Hunk } from "./generated/Hunk";
import type { ImpactedCell } from "./generated/ImpactedCell";
import type { ImpactedSymbol } from "./generated/ImpactedSymbol";
import type { Language } from "./generated/Language";
import type { LayerDef } from "./generated/LayerDef";
import type { LayerId } from "./generated/LayerId";
import type { LineKind } from "./generated/LineKind";
import type { Progress } from "./generated/Progress";
import type { Stats } from "./generated/Stats";
import type { Impact as WireImpact } from "./generated/Impact";
import type { Matrix as WireMatrix } from "./generated/Matrix";
import type { Project as ProjectInfo } from "./generated/Project";
export type {
  Cell,
  CellSymbol,
  CellSymbols,
  Commit,
  Diff,
  DiffLine,
  DomainNode,
  EdgeGroup,
  FileDiff,
  FileStatus,
  History,
  Hunk,
  ImpactedCell,
  ImpactedSymbol,
  Language,
  LayerDef,
  LayerId,
  LineKind,
  Progress,
  Stats,
  ProjectInfo,
};
/** Matrix as sent by the backend (docs/api.md): edges grouped by source cell, cells as indices. */
export type { WireMatrix };
/** Impact as sent by the backend: its snapshot matrix is a `WireMatrix`. */
export type { WireImpact };

/** One edge between two cells, by cell id (`expandMatrix` builds them from `EdgeGroup`s). */
export interface CellEdge {
  from: string;
  to: string;
  weight: number;
}
/** Matrix as the UI uses it: the wire matrix with its edge groups expanded to `CellEdge`s. */
export type Matrix = Omit<WireMatrix, "edges" | "http_edges" | "rpc_edges"> & {
  edges: CellEdge[];
  /** Front-to-back HTTP links (api client cell -> handler cell); empty unless several folders were analysed. */
  httpEdges: CellEdge[];
  /** GWT RPC links (client caller cell -> server implementation cell); empty when absent. */
  rpcEdges: CellEdge[];
};
/** Impact as the UI uses it: the snapshot matrix, when present, is expanded. */
export type Impact = Omit<WireImpact, "matrix"> & { matrix?: Matrix };

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function fetchSample<T>(name: string): Promise<T> {
  const res = await fetch(`/samples/${name}.json`);
  if (!res.ok) throw new Error(`sample ${name}: HTTP ${res.status}`);
  return (await res.json()) as T;
}

async function default_pickFolder(): Promise<string | null> {
  if (!inTauri) return "/browser/sample-project";
  const sel = await open({ directory: true, multiple: false });
  return typeof sel === "string" ? sel : null;
}

/** Analyses one folder, or several together (`paths.length > 1`). */
async function default_analyze(
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
async function default_impact(
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
async function default_cellSymbols(
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

async function default_history(path: string, limit?: number): Promise<History> {
  if (!inTauri) return fetchSample<History>("history");
  return invoke<History>("history", { path, limit: limit ?? null });
}

/** Textual diff of `base..head`, same meaning as `impact` (null base = HEAD vs working tree). */
async function default_diff(path: string, base: string | null, head: string | null): Promise<Diff> {
  if (!inTauri) return fetchSample<Diff>("diff");
  return invoke<Diff>("diff", { path, base, head });
}

/** Where the data comes from: Tauri or samples (default), or a host page (viewer). */
export interface Backend {
  pickFolder(): Promise<string | null>;
  analyze(paths: string[], onProgress: (p: Progress) => void): Promise<Matrix>;
  impact(
    path: string,
    base: string | null,
    head: string | null,
    onProgress?: (p: Progress) => void,
  ): Promise<Impact>;
  cellSymbols(
    paths: string[],
    head: string | null,
    generation: number,
    cells: string[],
    offset: number,
    limit: number,
  ): Promise<CellSymbols>;
  history(path: string, limit?: number): Promise<History>;
  diff(path: string, base: string | null, head: string | null): Promise<Diff>;
}

export const defaultBackend: Backend = {
  pickFolder: default_pickFolder,
  analyze: default_analyze,
  impact: default_impact,
  cellSymbols: default_cellSymbols,
  history: default_history,
  diff: default_diff,
};

let current: Backend = defaultBackend;

/** Replaces the backend the functions below delegate to (the viewer page does it once at startup). */
export function setBackend(b: Backend): void {
  current = b;
}

export const pickFolder: Backend["pickFolder"] = (...a) => current.pickFolder(...a);
export const analyze: Backend["analyze"] = (...a) => current.analyze(...a);
export const impact: Backend["impact"] = (...a) => current.impact(...a);
export const cellSymbols: Backend["cellSymbols"] = (...a) => current.cellSymbols(...a);
export const history: Backend["history"] = (...a) => current.history(...a);
export const diff: Backend["diff"] = (...a) => current.diff(...a);
