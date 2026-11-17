// Coordination between the stores: each function here is the one place where an action
// touches several of them (a load resets them all, an impact changes the matrix shown and so
// the selection, a rules reload re-runs the impact…). Single-store actions are called on the
// store directly.
import type { Impact, Matrix } from "../api";
import { selectionDiff } from "../history";
import { DEFAULT_LENS } from "../lens";
import type { Selection } from "../history";
import type { ViewCell } from "../domaintree";
import { retainSelection } from "../matrixsel";
import { clickSelection, sameSelection } from "../overlays";
import { analysis } from "./analysis.svelte";
import { git } from "./git.svelte";
import { impact } from "./impact.svelte";
import { lenses } from "./lenses.svelte";
import { selection } from "./selection.svelte";
import { shown } from "./shown";

/** Analyses `paths`, then loads its history (single project only). Rejects when the analysis fails. */
export async function load(input: string | string[]): Promise<void> {
  const paths = typeof input === "string" ? [input] : input;
  reset();
  const req = analysis.loadId;
  if (!(await analysis.load(paths))) return;
  void lenses.refresh();
  if (analysis.multi) return; // no git history across several repos
  await git.loadHistory(paths[0], () => req === analysis.loadId);
}

/** Back to the empty state; late answers of a previous load are ignored. */
export function reset() {
  analysis.reset();
  impact.reset();
  selection.reset();
  git.reset();
  lenses.reset();
}

/** Runs `change`; when it changes the matrix shown, the selection is dropped (cell ids of
 * another snapshot are not comparable) or, with `keep` (only the rules changed), carried over
 * (see `keepSelection`). */
function changeShown(change: () => void, keep: boolean) {
  const before = shown.matrix;
  const sel = selection.selected;
  const lens = selection.lens;
  change();
  pruneLenses();
  const now = shown.matrix;
  if (now === before) return;
  if (keep && sel && now) keepSelection(sel, lens, now);
  else selection.select(null);
}

/** Drops the lens matrices and cells no view can show any more (see `LensStore.prune`). */
function pruneLenses() {
  lenses.prune(
    analysis.work,
    impact.overlays.map((o) => o.impact),
  );
}

/** Carries `sel`, selected in `lens`, over to `now` (the new matrix shown, same snapshot with
 * reloaded rules), in its lens: retained against that lens' matrix of `now`, or kept as is
 * while that one loads (the panel of that lens then keeps it in step, see `SelectionStore.sync`). */
function keepSelection(sel: ViewCell, lens: string, now: Matrix) {
  if (lens === DEFAULT_LENS) return selection.select(retainSelection(sel, now));
  const lensed = lenses.matrix(lens, now, shown.head);
  selection.select(lensed ? retainSelection(sel, lensed) : sel, lens);
}

/** Diff of two refs (null = HEAD / working tree); shows the commit snapshot when there is one.
 * The result becomes the selected overlay (see `ImpactOverlays.add`). Ignored while another
 * impact runs. With `keepSelection` (re-run after a rules reload), a selected cell that still
 * exists in the new matrix stays selected. Once it is in, the next stale overlay shown, if
 * any, is computed again (see `refreshStale`). */
export async function runImpact(base: string | null, head: string | null, keepSelection = false): Promise<void> {
  if (impact.busy || analysis.multi) return;
  const historySel = git.historySel;
  let applied = false;
  await impact.run(analysis.project, { base, head }, (res: Impact) => {
    applied = true;
    changeShown(() => impact.add({ base, head }, res, historySel), keepSelection);
  });
  // Only after a success: a failing run would otherwise be retried forever.
  if (applied) refreshStale();
}

/** Computes again the first overlay shown that is stale (computed with rules reloaded since),
 * in selection order; `runImpact` then goes on with the next one. */
function refreshStale() {
  const o = impact.selectedIds.map((id) => impact.overlays.find((x) => x.id === id)).find((x) => x?.stale);
  if (o) void runImpact(o.range.base, o.range.head, true);
}

/** Drops the impact shown (pinned overlays stay listed), the history selection and the diff. */
export function clearImpact() {
  git.historySel = null;
  git.clearDiff();
  changeShown(() => impact.clear(), false);
}

/** The history selection and the diff follow the first overlay shown (none shown: both are
 * dropped); overlays shown that were computed before a rules reload are computed again. */
function followOverlay() {
  const o = impact.selected;
  if (!o) {
    git.historySel = null;
    git.clearDiff();
    return;
  }
  git.historySel = o.historySel;
  if (git.diff?.base !== o.range.base || git.diff?.head !== o.range.head) void git.loadDiff(o.range.base, o.range.head);
  refreshStale();
}

/** Shows overlay `id` alone, or with `toggle` (cmd/ctrl-click) adds it to the overlays shown or
 * takes it out (see `clickSelection`); several shown are drawn together on the working-tree
 * matrix. The matrix, details, history selection and diff follow (see `followOverlay`).
 * Ignored while an impact runs. */
export function selectOverlay(id: string, toggle = false) {
  if (impact.busy || !impact.overlays.some((o) => o.id === id)) return;
  if (sameSelection(clickSelection(impact.selectedIds, id, toggle), impact.selectedIds)) return;
  changeShown(() => impact.select(id, toggle), false);
  followOverlay();
}

/** Retries what lens `lens` could not load. The overlays shown whose impact it could not map
 * (or whose commit snapshot it could not draw), typically because the backend dropped that
 * snapshot's analysis to make room for newer ones, are computed again: the new impact comes
 * with its snapshot analysed afresh, and is mapped in the lens once in. Else the failed lens
 * matrices and cells are requested again. */
export function retryLens(lens: string) {
  const ids = impact.selectedOverlays.filter((o) => lenses.impactError(lens, o.impact) !== "").map((o) => o.id);
  const sel = impact.selected;
  if (sel && !impact.multi && shown.head !== null && lenses.matrixError(lens, shown.matrix, shown.head) !== "") {
    if (!ids.includes(sel.id)) ids.push(sel.id);
  }
  if (ids.length === 0) {
    lenses.retry();
    return;
  }
  impact.markStale(ids);
  refreshStale();
}

/** Keeps the overlays shown in the list when the next impact runs. */
export function keepOverlay() {
  for (const o of impact.selectedOverlays) if (!o.pinned) impact.pin(o.id);
}

/** Drops overlay `id`; when it was the only one shown, the most recent one left is shown
 * instead. Ignored while an impact runs. */
export function removeOverlay(id: string) {
  if (impact.busy) return;
  const was = impact.selectedId;
  changeShown(() => impact.remove(id), false);
  if (impact.selectedId !== was) followOverlay();
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
  const lens = selection.lens;
  const showsWork = shown.matrix === analysis.work;
  analysis.work = m;
  // The old generation is gone: list the kept cell's symbols again.
  if (showsWork) {
    if (sel) keepSelection(sel, lens, m);
    else selection.select(null);
  }
  pruneLenses();
  const range = impact.range;
  impact.markStale();
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
