// Backend of the embeddable viewer: serves one impact (with its matrix) handed
// in by the host page. No Tauri, no project, no git history, no diff.
import type { Backend, CellSymbols, Impact, WireImpact } from "../api";
import { expandMatrix } from "../expand";

export interface ViewerData {
  /** Wire impact; its `matrix` is required (it is what gets displayed). */
  impact: WireImpact;
  cellSymbols(cells: string[], offset: number, limit: number): Promise<CellSymbols>;
}

export function viewerBackend(data: ViewerData): Backend {
  const { matrix: wire, ...rest } = data.impact;
  if (!wire) throw new Error("viewer: the impact has no matrix");
  // Expanded once: analyze and impact share the object, so the app treats it as the working tree.
  const matrix = expandMatrix(wire);
  const impact: Impact = { ...rest, matrix };
  const unavailable = (what: string) => () => Promise.reject(new Error(`${what} is not available in the viewer`));
  return {
    pickFolder: unavailable("Picking a folder"),
    analyze: async () => matrix,
    reclassify: unavailable("Reloading rules"),
    rulesStamp: async () => null,
    impact: async () => impact,
    cellSymbols: (_paths, _head, _generation, cells, offset, limit) => data.cellSymbols(cells, offset, limit),
    // Rejecting leaves `app.history` null, which makes the history and diff panels unavailable.
    history: unavailable("History"),
    diff: unavailable("Diff"),
  };
}
