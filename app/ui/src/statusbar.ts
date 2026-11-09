import type { Impact, Progress, Stats } from "./api";
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
export function selectionLabel(cell: ViewCell | null): string | null {
  return cell ? `${cell.layer} / ${cell.label} · ${plural(cell.size, "symbol")}` : null;
}
