# Tauri API (UI contract)

The UI talks to the Rust side through three commands and one event. JSON field
names are snake_case; enums are snake_case strings. Rust definitions live in
`crates/core/src/model.rs` and `ir.rs`. Samples: `docs/samples/matrix.json`,
`docs/samples/impact.json`.

## Commands

```ts
invoke<Matrix>("analyze", { path: string });
invoke<Impact>("impact", { path: string, base: string | null, head: string | null });
invoke<History>("history", { path: string, limit: number | null });
```

- `analyze(path)` analyzes the Java/TypeScript project at `path` and returns the matrix.
- `impact(path, base, head)`: `base = null` means HEAD vs working tree;
  otherwise `base..head` (`head = null` means working tree). Impact cell ids
  and symbol ids refer to the ids in the `Matrix` from `analyze`.
- `history(path, limit)`: up to `limit` (default 200) commits reachable from
  HEAD, newest first. `dirty` = the working tree has changes under the
  project root that `impact(path, null, null)` would show (tracked edits and
  non-ignored untracked files; `index.scip` ignored). Pass `commit.base` / `commit.id` as `impact(path, base, head)` to
  show one commit's diff (`base` is the first parent, or the empty tree for a
  root commit). Errors when the path is not a repository or has no commits.

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
  warnings?: string[]; // only when non-empty: indexing failed, stale index...
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
  warnings?: string[];
}
interface ImpactedCell { id: string; distance: number }
interface ImpactedSymbol { id: string; distance: number }

interface History { dirty: boolean; commits: Commit[] } // newest first
interface Commit {
  id: string; short: string;
  parents: string[];  // empty for a root commit, 2+ for a merge
  base: string;       // first parent, or the empty tree id for a root commit
  subject: string; author: string;
  date: string;       // ISO 8601
  refs: string[];     // branches/tags pointing at it
}

interface Progress { step: string; pct: number } // pct 0..100
```

Granularity: symbol ids in `Matrix.cells[].symbols` and in `Impact` are class-level (top-level symbol) ids.

`analyze` re-runs the indexer when a source file is newer than `index.scip`.
`impact` always re-parses the project (fresh symbol ranges) and reuses the
index, warning when it is stale.
