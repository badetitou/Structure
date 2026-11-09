# Tauri API (UI contract)

The UI talks to the Rust side through five commands and one event. JSON field
names are snake_case; enums are snake_case strings. Rust definitions live in
`crates/core/src/model.rs` and `ir.rs`. Samples: `docs/samples/matrix.json`,
`docs/samples/impact.json`.

## Commands

```ts
invoke<Matrix>("analyze", { path: string });
invoke<Impact>("impact", { path: string, base: string | null, head: string | null });
invoke<History>("history", { path: string, limit: number | null });
invoke<Diff>("diff", { path: string, base: string | null, head: string | null });
invoke<CellSymbols>("cell_symbols", {
  path: string, head: string | null, generation: number, cells: string[],
  offset: number | null, limit: number | null,
});
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
- `cell_symbols(path, head, generation, cells, offset, limit)`: the top-level symbols of
  the given cell ids (the matrix carries counts only), sorted by (name, id),
  concatenated in the order of `cells`. `head = null` reads the working-tree
  analysis, otherwise the snapshot of that commit: pass the full SHA of
  `Impact.head` (the backend resolves any `head` you gave `impact` to a full
  SHA, and snapshots are cached under it). `generation` is `Matrix.generation`
  of the matrix on screen; if the cached analysis was replaced since (for
  example `impact` re-parsed an edited working tree), the call fails with
  `analysis changed, re-run Analyze`. Paged: `offset` default 0, `limit` default 500, at most 2000.
  Reads the analyses cached by `analyze` / `impact`; same error when none
  exists. The cache keeps each project's working-tree analysis and at most 2
  commit snapshots (vscode: ~3.5 GB each); snapshots never evict working trees.
  `docs/samples/cell-symbols.json` maps cell ids to their symbols, the data
  the browser-only dev mode serves for `cell_symbols`.
- `impact` reuses the cached analysis instead of re-analysing: a commit
  snapshot is immutable, so `impact(path, base, head)` analyses `head` once;
  for the working tree the cached analysis is reused while the working tree
  is unchanged (same HEAD, same `git status`, same mtimes of the changed files).
- `history(path, limit)`: up to `limit` (default 200) commits reachable from
  HEAD in topological order (`git log --topo-order`: children before parents,
  each merged line of history contiguous), newest first. `dirty` = the working tree has changes under the
  project root that `impact(path, null, null)` would show (tracked edits and
  non-ignored untracked files; `index.scip` ignored). Pass `commit.base` / `commit.id` as `impact(path, base, head)` to
  show one commit's diff (`base` is the first parent, or the empty tree for a
  root commit). Errors when the path is not a repository or has no commits.

- `diff(path, base, head)`: the textual diff shown by the Diff panel, with the
  same `base` / `head` meaning as `impact` (`git diff -M --relative`, so only
  files under the project root). With `base = null` (working tree vs HEAD),
  untracked non-ignored files are listed as added (regular files only, at most 1 MiB read
  of each; symlinks and directories show as binary). A `base` / `head` starting with `-` is rejected. Bounded: at most 1000 files,
  2000 lines per file, 20 000 lines in all and 1000 characters per line;
  `FileDiff.truncated` / `Diff.truncated` flag what was cut (`additions` /
  `deletions` stay exact). Fast (no analysis); `docs/samples/diff.json` is the
  browser-only dev mode's answer.

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
  project: string;   // several roots: the first root's name
  language: Language; // several roots: the first root's language
  // Present only with >= 2 roots (CLI `analyze a b`, `pipeline::analyze_many`):
  // symbol ids are `<name>::<id>` and domains `<name>/<domain>`.
  projects?: { name: string; root: string; language: Language }[];
  layers: Layer[];   // only the layers present, in Layer order
  domains: string[];
  cells: Cell[];     // non-empty cells only, sorted by (layer, domain)
  edges: EdgeGroup[]; // cell-to-cell calls/uses, grouped by source cell
  stats: Stats;
  generation?: number; // set by the app: id of the cached analysis; pass to `cell_symbols`
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
  size: number;   // number of top-level symbols; list them with `cell_symbols`
}
interface CellSymbol { id: string; name: string; file: string }
interface CellSymbols {
  total: number;   // symbols in the requested cells
  offset: number;
  symbols: CellSymbol[]; // at most `limit`, from `offset`
}
// Edges refer to cells by index into `Matrix.cells` (a cell id is ~40 bytes,
// repeated 130 000 times on vscode). `targets` = [[cell index, weight], ...],
// sorted; no self-loops. The UI expands them to {from, to, weight} with cell ids.
interface EdgeGroup { from: number; targets: [number, number][] }
interface Stats { files: number; symbols: number; edges: number; elapsed_ms: number }

interface Impact {
  base: string;
  head: string | null; // full SHA when a commit was analysed (as resolved from the request)
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

interface Diff {
  base: string;          // as requested ("HEAD" when base was null)
  head: string | null;
  files: FileDiff[];
  truncated: boolean;    // some files or lines were left out
}
interface FileDiff {
  path: string;          // new side (old path for a deleted file), relative to the project
  old_path?: string;     // renames only
  status: "added" | "deleted" | "modified" | "renamed";
  binary: boolean;       // no hunks then
  additions: number; deletions: number;
  hunks: Hunk[];
  truncated: boolean;    // some of this file's lines were left out
}
interface Hunk { header: string; lines: DiffLine[] } // header = "@@ -a,b +c,d @@ ..."
interface DiffLine {
  kind: "context" | "add" | "del";
  old?: number; new?: number; // 1-based line numbers; absent on the side the line is not on
  text: string;               // without the +/-/space marker
}

interface Progress { step: string; pct: number; detail?: string } // pct 0..100
```

Granularity: symbol ids in `CellSymbols` and in `Impact` are class-level (top-level symbol) ids.

Payload size (vscode, 2 501 cells, 133 514 cell edges): 33 MB pretty-printed /
24.7 MB compact before (symbol lists 13 MB, string-id edges 11.5 MB); counts and
index-based grouped edges bring it to about 1.7 MB.

`analyze` re-runs the indexer when a source file is newer than `index.scip`.
`impact` re-parses the working tree only when it changed since the cached
analysis (fresh symbol ranges), reusing the index and warning when it is stale.
