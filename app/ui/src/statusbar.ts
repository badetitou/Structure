import { layerLabel } from "./aligned";
import type { Impact, LayerDef, Progress, Stats } from "./api";
import { shortSha } from "./diffview";
import type { ViewCell } from "./domaintree";

/** A git ref for display (see `shortSha`); null is the working tree. */
export function shortRef(ref: string | null): string {
  return ref === null ? "working tree" : shortSha(ref);
}

/** "1,234 files · 5,678 symbols · … · 1.2 s". */
export function statsLabel(st: Stats): string {
  const n = (v: number) => v.toLocaleString("en-US");
  const time = st.elapsed_ms < 1000 ? `${st.elapsed_ms} ms` : `${(st.elapsed_ms / 1000).toFixed(1)} s`;
  return `${n(st.files)} files · ${n(st.symbols)} symbols · ${n(st.edges)} edges · ${time}`;
}

/** Which state the matrix shows: the working tree, or the commit snapshot of the impact. */
export function snapshotLabel(impact: Impact | null): string {
  return impact?.matrix ? `snapshot ${shortRef(impact.head)}` : "working tree";
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

/** Impact summary, its progress while it runs, or null when there is none. */
export function impactLabel(impact: Impact | null, busy: Progress | null): string | null {
  if (busy) return `Impact: ${busy.step} (${busy.pct}%)`;
  if (!impact) return null;
  return (
    `Impact ${shortRef(impact.base)}..${shortRef(impact.head)}: ` +
    `${plural(impact.changed_cells.length, "changed cell")}, ${plural(impact.impacted_cells.length, "impacted cell")}`
  );
}

/** "layer / domain · 12 symbols", or null when no cell is selected. */
export function selectionLabel(cell: ViewCell | null, layers?: readonly LayerDef[]): string | null {
  return cell ? `${layerLabel(layers, cell.layer)} / ${cell.label} · ${plural(cell.size, "symbol")}` : null;
}

/** Several overlays drawn together: how many, and the cells they hit that the working tree
 * does not have (see `cellsNotIn`). */
export function multiImpactLabel(count: number, missing: number): string {
  const base = `${count} overlays on the working tree`;
  return missing > 0 ? `${base} · ${plural(missing, "cell")} not in working tree` : base;
}

/** "2 changed cells · 5 impacted cells" of one overlay. */
export function cellCountsLabel(impact: Impact): string {
  return `${plural(impact.changed_cells.length, "changed cell")} · ${plural(impact.impacted_cells.length, "impacted cell")}`;
}
