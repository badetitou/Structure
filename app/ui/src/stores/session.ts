// Coordination between the stores: each function here is the one place where an action
// touches several of them (a load resets them all, an impact changes the matrix shown and so
// the selection, a rules reload re-runs the impact…). Single-store actions are called on the
// store directly.
import type { Impact, Matrix } from "../api";
import { selectionDiff } from "../history";
import type { Selection } from "../history";
import { retainSelection } from "../matrixsel";
import { analysis } from "./analysis.svelte";
import { git } from "./git.svelte";
import { impact } from "./impact.svelte";
import { selection } from "./selection.svelte";
import { shown } from "./shown";

/** Analyses `paths`, then loads its history (single project only). Rejects when the analysis fails. */
export async function load(input: string | string[]): Promise<void> {
  const paths = typeof input === "string" ? [input] : input;
  reset();
  const req = analysis.loadId;
  if (!(await analysis.load(paths))) return;
  if (analysis.multi) return; // no git history across several repos
  await git.loadHistory(paths[0], () => req === analysis.loadId);
}

/** Back to the empty state; late answers of a previous load are ignored. */
export function reset() {
  analysis.reset();
  impact.clear();
  selection.reset();
  git.reset();
}

/** Runs `change`; when it changes the matrix shown, the selection is dropped (cell ids of
 * another snapshot are not comparable) or, with `keep` (only the rules changed), carried over. */
function changeShown(change: () => void, keep: boolean) {
  const before = shown.matrix;
  const sel = selection.selected;
  change();
  const now = shown.matrix;
  if (now !== before) selection.select(keep && sel && now ? retainSelection(sel, now) : null);
}

/** Diff of two refs (null = HEAD / working tree); shows the commit snapshot when there is one.
 * Ignored while another impact runs. With `keepSelection` (re-run after a rules reload), a
 * selected cell that still exists in the new matrix stays selected. */
export async function runImpact(base: string | null, head: string | null, keepSelection = false): Promise<void> {
  if (impact.busy || analysis.multi) return;
  await impact.run(analysis.project, { base, head }, (res: Impact) =>
    changeShown(() => (impact.current = res), keepSelection),
  );
}

/** Drops the impact, the history selection and the diff. */
export function clearImpact() {
  git.historySel = null;
  git.clearDiff();
  changeShown(() => impact.clear(), false);
}

/** Select history entry `sel`; its diff drives the impact. */
export function pickHistory(sel: Selection) {
  if (!git.history || impact.busy) return;
  const d = selectionDiff(git.history.commits, sel);
  if (!d) return;
  git.historySel = sel;
  void git.loadDiff(d.base, d.head);
  void runImpact(d.base, d.head);
}

/** A typed diff is not a sidebar selection. */
export function runTypedImpact(base: string | null, head: string | null) {
  if (impact.busy) return;
  git.historySel = null;
  void git.loadDiff(base, head);
  void runImpact(base, head);
}

/** Installs a reclassified working-tree matrix: the selection is kept when its cell still
 * exists and the impact shown (or running) is computed again with the new rules. */
function applyReclassified(m: Matrix) {
  const sel = selection.selected;
  const showsWork = shown.matrix === analysis.work;
  analysis.work = m;
  // The old generation is gone: list the kept cell's symbols again.
  if (showsWork) selection.select(sel ? retainSelection(sel, m) : null);
  const range = impact.range;
  if (range) {
    // Computed with the old classification (a running one too): run it again.
    impact.abandon();
    void runImpact(range.base, range.head, true);
  }
}

/** Re-applies the project's rules without re-parsing it (see `AnalysisStore.reloadRules`). */
export function reloadRules(): Promise<void> {
  return analysis.reloadRules(applyReclassified);
}

/** Reloads the rules when their files changed since last checked. */
export function checkRules(): Promise<void> {
  return analysis.checkRules(applyReclassified);
}

/** Polls the rules files while a project is open; returns the function that stops it. */
export function watchRules(): () => void {
  return analysis.watchRules(applyReclassified);
}
