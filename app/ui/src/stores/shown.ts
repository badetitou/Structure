import type { Matrix } from "../api";
import { shownMatrix } from "../matrixsel";
import { cellsNotIn } from "../multioverlay";
import { analysis } from "./analysis.svelte";
import { impact } from "./impact.svelte";

/** What the matrix shows, derived from the analysis and the impact: the commit snapshot's
 * matrix when the impact drawn alone carries one, else the working tree (always so when
 * several overlays are drawn together). Read-only. */
export const shown = {
  get matrix(): Matrix | null {
    return analysis.work ? shownMatrix(analysis.work, impact.current) : null;
  },
  get warnings(): string[] {
    return impact.current?.warnings ?? this.matrix?.warnings ?? [];
  },
  /** Commit whose snapshot is shown, null for the working tree. */
  get head(): string | null {
    return this.matrix === analysis.work ? null : (impact.current?.head ?? null);
  },
  /** Several overlays drawn: the cells they hit that the matrix shown (the working tree) does
   * not have, so cannot be drawn; 0 otherwise. */
  get missingCells(): number {
    const m = this.matrix;
    return impact.multi && m ? cellsNotIn(impact.selectedOverlays.map((o) => o.impact), m) : 0;
  },
};
