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
  When `head` is set, the commit is analysed exactly in a detached checkout
  (not the working tree) at `<git-common-dir>/structure/snapshots/<sha>`; at
  most 8 snapshots are kept (least recently used pruned). The first click on a
  commit may run the SCIP indexer (slow; `analysis-progress` starts with
  `checking out <short sha>`); later ones reuse the snapshot and its
  `index.scip`. The result then carries `matrix`, the snapshot's matrix: show
  it instead of the working-tree one (cells as of that commit; the impact ids
  refer to it). With `head = null`, `matrix` is absent.
- `history(path, limit)`: up to `limit` (default 200) commits reachable from
  HEAD in topological order (`git log --topo-order`: children before parents,
  each merged line of history contiguous), newest first. `dirty` = the working tree has changes under the
  project root that `impact(path, null, null)` would show (tracked edits and
  non-ignored untracked files; `index.scip` ignored). Pass `commit.base` / `commit.id` as `impact(path, base, head)` to
  show one commit's diff (`base` is the first parent, or the empty tree for a
  root commit). Errors when the path is not a repository or has no commits.

## Event

`analysis-progress`, payload `Progress`, emitted during `analyze`/`impact`.
While the SCIP indexer runs (Maven/Gradle can take minutes), `step` stays
`indexing` and events arrive about every second with `pct` in 5..34 (never
decreasing) and `detail` = latest build output line plus elapsed time
(e.g. `[INFO] Building foo 1.0 [3/12] · 42s`). `detail` is omitted otherwise.

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
  // Only when domains are modules (package / directory) instead of name roots:
  // module trie, single-child chains without own classes collapsed. Every entry
  // of `domains` is exactly one leaf. See docs/samples/matrix-modules.json.
  domain_tree?: DomainNode[];
}
interface DomainNode {
  label: string;     // collapsed path, e.g. "org.moosetechnology.model.famix"
  domain?: string;   // leaves only: the matrix domain
  size: number;      // classes below this node
  children: DomainNode[]; // a module with own classes and sub-modules gets a leaf child for its own classes
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
  matrix?: Matrix;                    // only when `head` was analysed in its snapshot
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

interface Progress { step: string; pct: number; detail?: string } // pct 0..100
```

Granularity: symbol ids in `Matrix.cells[].symbols` and in `Impact` are class-level (top-level symbol) ids.

`analyze` re-runs the indexer when a source file is newer than `index.scip`.
`impact` always re-parses the project (fresh symbol ranges) and reuses the
index, warning when it is stale.
