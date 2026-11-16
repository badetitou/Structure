import type { Matrix } from "../api";
import { shownMatrix } from "../matrixsel";
import { analysis } from "./analysis.svelte";
import { impact } from "./impact.svelte";

/** What the matrix shows, derived from the analysis and the impact: the commit snapshot's
 * matrix when the impact carries one, else the working tree. Read-only. */
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
};
