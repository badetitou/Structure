// Lenses (pure): the ways the backend quotients the graph into matrix cells (see
// `structure_classify::lens`). Each matrix panel picks one (its `lens` display option); the
// rendering stays the panel's. The reactive cache of lens matrices and impact cells is
// `stores/lenses.svelte.ts`.
import type { Impact, LensCells, LensInfo } from "./api";

/** Id of the default lens (layers × inferred domains): the matrix `analyze` returns. */
export const DEFAULT_LENS = "layer-domain";
/** The lenses of a backend that only has the default one (samples, viewer). */
export const DEFAULT_LENSES: readonly LensInfo[] = [{ id: DEFAULT_LENS, label: "Layer × domain" }];

/** A lens id read from stored params: a non-empty string, else the default lens. */
export function parseLens(v: unknown): string {
  return typeof v === "string" && v !== "" ? v : DEFAULT_LENS;
}

/** The lens actually drawn for `id`: itself when the backend offers it, else the default lens
 * (a layout saved with a lens this backend lacks). */
export function resolveLens(id: string, available: readonly LensInfo[]): string {
  return available.some((l) => l.id === id) ? id : DEFAULT_LENS;
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
