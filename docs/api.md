# Tauri API (UI contract)

The UI talks to the Rust side through two commands and one event. JSON field
names are snake_case; enums are snake_case strings. Rust definitions live in
`crates/core/src/model.rs` and `ir.rs`. Samples: `docs/samples/matrix.json`,
`docs/samples/impact.json`.

## Commands

```ts
invoke<Matrix>("analyze", { path: string });
invoke<Impact>("impact", { path: string, base: string | null, head: string | null });
```

- `analyze(path)` analyzes the Java/TypeScript project at `path` and returns the matrix.
- `impact(path, base, head)`: `base = null` means HEAD vs working tree;
  otherwise `base..head` (`head = null` means working tree). Impact cell ids
  and symbol ids refer to the ids in the `Matrix` from `analyze`.

## Event

`analysis-progress`, payload `Progress`, emitted during `analyze`/`impact`.

## Types

```ts
type Language = "java" | "typescript";
type Layer =
  | "ui" | "state" | "api_client" | "entry" | "business"
  | "persistence" | "model" | "util" | "unknown"; // matrix rows, top to bottom

interface Matrix {
  project: string;
  language: Language;
  layers: Layer[];   // only the layers present, in Layer order
  domains: string[];
  cells: Cell[];
  edges: CellEdge[];
  stats: Stats;
}
interface Cell {
  id: string; // "<layer>:<domain>", e.g. "business:account"
  layer: Layer;
  domain: string;
  size: number;
  symbols: CellSymbol[];
}
interface CellSymbol { id: string; name: string; file: string }
interface CellEdge { from: string; to: string; weight: number } // cell ids
interface Stats { files: number; symbols: number; edges: number; elapsed_ms: number }

interface Impact {
  base: string;
  head: string | null;
  changed_files: string[];
  changed_symbols: string[];
  changed_cells: string[];
  impacted_cells: ImpactedCell[];     // distance = hops from a changed cell
  impacted_symbols: ImpactedSymbol[];
}
interface ImpactedCell { id: string; distance: number }
interface ImpactedSymbol { id: string; distance: number }

interface Progress { step: string; pct: number } // pct 0..100
```

Granularity: symbol ids in `Matrix.cells[].symbols` and in `Impact` are class-level (top-level symbol) ids.
