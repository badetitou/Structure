// Rules of the highlight overlay list (pure; the store in `stores/impact.svelte.ts` applies them).
// The list holds at most one unpinned overlay, the one the next impact replaces; pinned ones
// ("Keep") stay until removed. New overlays go at the end, so the last one is the most recent.
import type { Impact } from "./api";
import { rangeLabel } from "./diffview";
import type { Selection } from "./history";

/** A git range; null stands for HEAD / the working tree. */
export interface Range {
  base: string | null;
  head: string | null;
}

/** Number of overlay colour tokens (`--overlay-0` … in `app.css`). */
export const OVERLAY_COLOURS = 6;

/** One named highlight of the matrix. */
export interface Overlay {
  id: string;
  /** The range, e.g. `b6a8d35..a94f1b7` or `working tree`. */
  label: string;
  source: "git-diff";
  range: Range;
  impact: Impact;
  /** Theme token of its colour, e.g. `--overlay-2`. */
  colour: string;
  /** Kept when the next impact runs; else replaced by it. */
  pinned: boolean;
  /** Computed with rules that were reloaded since: selecting it computes it again. */
  stale: boolean;
  /** History selection it was run from, restored when it is selected again. */
  historySel: Selection | null;
}

const sameRange = (a: Range, b: Range) => a.base === b.base && a.head === b.head;

/** First colour token not used by `list` (cycling once all are taken). */
export function nextColour(list: readonly Overlay[]): string {
  const used = new Set(list.map((o) => o.colour));
  for (let i = 0; i < OVERLAY_COLOURS; i++) if (!used.has(`--overlay-${i}`)) return `--overlay-${i}`;
  return `--overlay-${list.length % OVERLAY_COLOURS}`;
}

/** The list once `impact` of `range` is added, and the id of its overlay (to select).
 * Same range as an overlay of the list: that overlay gets the new result in place (it keeps
 * its id, colour, pin and history selection). Else the unpinned overlay, if any, is replaced
 * by a new one at the end. */
export function addOverlay(
  list: readonly Overlay[],
  range: Range,
  impact: Impact,
  historySel: Selection | null,
  newId: string,
): { list: Overlay[]; id: string } {
  const same = list.find((o) => sameRange(o.range, range));
  if (same) {
    return { list: list.map((o) => (o === same ? { ...o, impact, stale: false } : o)), id: same.id };
  }
  const kept = list.filter((o) => o.pinned);
  const overlay: Overlay = {
    id: newId,
    label: rangeLabel(range.base, range.head),
    source: "git-diff",
    range,
    impact,
    colour: nextColour(kept),
    pinned: false,
    stale: false,
    historySel,
  };
  return { list: [...kept, overlay], id: newId };
}

/** The selection once `removed` is gone from the list (now `list`): unchanged when another
 * overlay (or none) was selected, else the most recent overlay left, null when none. */
export function selectionAfterRemoval(list: readonly Overlay[], selected: string | null, removed: string): string | null {
  if (selected !== removed) return selected;
  return list.at(-1)?.id ?? null;
}

/** The selector is shown only when there is a choice to make: two overlays or more, or kept
 * overlays left with none shown (after "Clear impact"), which would otherwise be unreachable. */
export function selectorVisible(list: readonly Overlay[], selected: string | null): boolean {
  return list.length >= 2 || (list.length === 1 && selected === null);
}
