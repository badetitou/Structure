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

/** The selection once overlay `id` is clicked. A plain click selects exactly that overlay; a
 * `toggle` click (cmd on macOS, ctrl elsewhere) adds it to the selection or takes it out, but
 * never empties it: toggling the only selected overlay keeps it. The order is the one of the
 * clicks, so the first selected overlay stays first (the diff follows it). */
export function clickSelection(selected: readonly string[], id: string, toggle: boolean): string[] {
  if (!toggle) return [id];
  if (!selected.includes(id)) return [...selected, id];
  return selected.length === 1 ? [...selected] : selected.filter((s) => s !== id);
}

/** Same selection, in the same order. */
export function sameSelection(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((x, i) => x === b[i]);
}

/** A click toggles an overlay in the selection with cmd on macOS, ctrl elsewhere. */
export function isToggleClick(ev: { metaKey: boolean; ctrlKey: boolean }, mac: boolean): boolean {
  return mac ? ev.metaKey : ev.ctrlKey;
}

/** macOS, from `navigator.platform` / `userAgent`. */
export function isMacPlatform(platform: string): boolean {
  return /mac|iphone|ipad/i.test(platform);
}

/** The selection once `removed` is gone from the list (now `list`): it only loses `removed`;
 * when that empties it, the most recent overlay left is selected (none when the list is
 * empty). An empty selection (nothing shown) stays empty. */
export function selectionAfterRemoval(list: readonly Overlay[], selected: readonly string[], removed: string): string[] {
  if (!selected.includes(removed)) return [...selected];
  const left = selected.filter((s) => s !== removed);
  if (left.length > 0) return left;
  const last = list.at(-1);
  return last ? [last.id] : [];
}

/** The selector is shown only when there is a choice to make: two overlays or more, or kept
 * overlays left with none shown (after "Clear impact"), which would otherwise be unreachable. */
export function selectorVisible(list: readonly Overlay[], selected: readonly string[]): boolean {
  return list.length >= 2 || (list.length === 1 && selected.length === 0);
}
