import type { Impact, Matrix } from "./api";

/** The matrix to display: the commit snapshot's when the impact carries one,
 * otherwise the working-tree matrix. */
export function shownMatrix(work: Matrix, impact: Impact | null): Matrix {
  return impact?.matrix ?? work;
}
