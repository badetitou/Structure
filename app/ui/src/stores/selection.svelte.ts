import * as api from "../api";
import { PAGE, appendPage } from "../cellsymbols";
import type { SymbolList } from "../cellsymbols";
import type { ViewCell, ViewMatrix } from "../domaintree";
import { DEFAULT_LENS } from "../lens";
import { analysis } from "./analysis.svelte";
import { shown } from "./shown";

/** The selected cell of the matrix shown and its symbols, fetched a page at a time. */
export class SelectionStore {
  /** Selected (view) cell. */
  selected = $state.raw<ViewCell | null>(null);
  /** Lens of the panel the cell was selected in: its ids (and members) are that lens' cells. */
  lens = $state(DEFAULT_LENS);
  symbols = $state.raw<SymbolList | null>(null);
  /** The aggregated matrix currently drawn (set by MatrixView; tracks expansion). */
  view = $state.raw<ViewMatrix | null>(null);
  private symReq = 0;

  /** No selection; late symbol pages are ignored. */
  reset() {
    this.symReq++;
    this.selected = null;
    this.lens = DEFAULT_LENS;
    this.symbols = null;
  }

  /** Select a view cell (or none) of a panel showing `lens`, and list its symbols. */
  select(cell: ViewCell | null, lens: string = DEFAULT_LENS) {
    this.selected = cell;
    this.lens = lens;
    void this.loadSymbols(false);
  }

  /** Keeps the selection in step with the cells drawn: dropped when its cell left the view,
   * replaced by the view's cell of the same id when that one differs (after a rules reload). */
  sync(cells: ViewCell[]) {
    const sel = this.selected;
    if (!sel) return;
    const next = cells.find((c) => c.id === sel.id);
    if (!next) this.select(null);
    else if (next.size !== sel.size || next.members.join("\n") !== sel.members.join("\n")) this.select(next, this.lens);
  }

  /** First page of the selected cell's symbols (in the matrix shown, through the selection's
   * lens: its matrix shares the generation of the matrix shown), or the next one when `more`. */
  async loadSymbols(more: boolean): Promise<void> {
    const cell = this.selected;
    const m = shown.matrix;
    const req = ++this.symReq;
    if (!cell || !m) {
      this.symbols = null;
      return;
    }
    const prev = this.symbols;
    const base: SymbolList =
      more && prev?.cell === cell.id
        ? { ...prev, loading: true }
        : { cell: cell.id, total: cell.size, items: [], loading: true };
    this.symbols = base;
    try {
      const lens = this.lens === DEFAULT_LENS ? [] : [this.lens];
      const page = await api.cellSymbols(analysis.paths, shown.head, m.generation ?? 0, cell.members, base.items.length, PAGE, ...lens);
      if (req !== this.symReq) return;
      this.symbols = appendPage(base, page);
    } catch (e) {
      if (req !== this.symReq) return;
      this.symbols = { ...base, loading: false, error: String(e) };
    }
  }
}

export const selection = new SelectionStore();
