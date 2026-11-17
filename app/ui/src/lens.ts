// Lenses (pure): the ways the backend quotients the graph into matrix cells (see
// `structure_classify::lens`). Each matrix panel picks one (its `lens` display option); the
// rendering stays the panel's. The reactive cache of lens matrices and impact cells is
// `stores/lenses.svelte.ts`.
import type { Impact, LensCells, LensInfo, Matrix } from "./api";

/** Id of the default lens (layers × inferred domains): the matrix `analyze` returns. */
export const DEFAULT_LENS = "layer-domain";
/** The lenses of a backend that only has the default one (samples, viewer). */
export const DEFAULT_LENSES: readonly LensInfo[] = [{ id: DEFAULT_LENS, label: "Layer × domain" }];

/** A lens id read from stored params: a non-empty string, else the default lens. */
export function parseLens(v: unknown): string {
  return typeof v === "string" && v !== "" ? v : DEFAULT_LENS;
}

/** The lens actually drawn for `id`: itself when the backend offers it, else the default lens
 * (a layout saved with a lens this backend lacks). Until the backend's list is `known`, `id`
 * itself: a layout saved with another lens waits for that list instead of drawing the
 * default lens first. */
export function resolveLens(id: string, available: readonly LensInfo[], known = true): string {
  return !known || available.some((l) => l.id === id) ? id : DEFAULT_LENS;
}

/** The lens selector is shown only when there is a choice. */
export function lensSelectorVisible(available: readonly LensInfo[]): boolean {
  return available.length > 1;
}

/** Label of lens `id` (the id itself when unknown). */
export function lensLabel(id: string, available: readonly LensInfo[]): string {
  return available.find((l) => l.id === id)?.label ?? id;
}

/** `imp` with its cells replaced by the ones of another lens (null: not known yet, so none). */
export function withLensCells(imp: Impact, cells: LensCells | null): Impact {
  return { ...imp, changed_cells: cells?.changed_cells ?? [], impacted_cells: cells?.impacted_cells ?? [] };
}

/** The analysis a matrix comes from: its commit (null: the working tree) and generation. */
export function snapshotId(head: string | null, generation: number): string {
  return `${head ?? ""}\u0000${generation}`;
}

/** The analyses a view can still show: the working tree's (`work`) and the commit snapshot of
 * each impact listed (an impact without a snapshot matrix is drawn on the working tree). */
export function liveSnapshots(work: Matrix | null, impacts: readonly Impact[]): Set<string> {
  const live = new Set<string>();
  if (work) live.add(snapshotId(null, work.generation ?? 0));
  for (const i of impacts) if (i.matrix) live.add(snapshotId(i.head, i.matrix.generation ?? 0));
  return live;
}

/**
 * The matrix a lens panel draws: the lens matrix when loaded; while the next one of the same
 * lens loads (a commit snapshot picked, rules reloaded), the last one drawn, so the matrix view
 * stays mounted and keeps its open groups and zoom; nothing on an error or another lens.
 */
export class HeldMatrix {
  private last: { lens: string; matrix: Matrix } | null = null;

  /** What to draw for `matrix` (null while loading) of `lens`, and whether it is a held one. */
  show(lens: string, matrix: Matrix | null, error: string): { matrix: Matrix | null; loading: boolean } {
    if (matrix) {
      this.last = { lens, matrix };
      return { matrix, loading: false };
    }
    if (error || this.last?.lens !== lens) {
      this.last = null;
      return { matrix: null, loading: false };
    }
    return { matrix: this.last.matrix, loading: true };
  }
}
