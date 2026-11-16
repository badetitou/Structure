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
  // No folder picker, rules reload, history or diff: those methods are absent.
  return {
    capabilities: { history: false, diff: false, pickFolder: false, reclassify: false, highlightSources: ["git-diff"] },
    analyze: async () => matrix,
    impact: async () => impact,
    cellSymbols: (_paths, _head, _generation, cells, offset, limit) => data.cellSymbols(cells, offset, limit),
  };
}
