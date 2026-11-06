import type { CellSymbol, CellSymbols } from "./api";

/** Symbols of the selected (view) cell, loaded page by page. */
export interface SymbolList {
  cell: string;
  total: number;
  items: CellSymbol[];
  loading: boolean;
  error?: string;
}

export const PAGE = 500;

/** Append a fetched page to `list` (ignored when it is not the next page). */
export function appendPage(list: SymbolList, page: CellSymbols): SymbolList {
  if (page.offset !== list.items.length) return { ...list, loading: false };
  return {
    ...list,
    total: page.total,
    items: [...list.items, ...page.symbols],
    loading: false,
    error: undefined,
  };
}

export const hasMore = (l: SymbolList) => l.items.length < l.total;
