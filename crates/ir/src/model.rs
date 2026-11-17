//! Matrix, classification and impact output contracts.

use crate::highlight::{Highlight, Role};
use crate::ir::Language;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Layer identifier, the snake_case name used in rules, cells and cell ids
/// (e.g. `business`). The known layers and their order are [`LayerDef`]s.
/// A newtype: serde reads and writes it as the bare string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LayerId(pub String);

impl LayerId {
    /// Id of the implicit layer of unclassified symbols, always last.
    pub const UNKNOWN: &'static str = "unknown";

    pub fn new(id: impl Into<String>) -> LayerId {
        LayerId(id.into())
    }

    /// The implicit `unknown` layer.
    pub fn unknown() -> LayerId {
        LayerId::new(Self::UNKNOWN)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_unknown(&self) -> bool {
        self.0 == Self::UNKNOWN
    }
}

impl From<&str> for LayerId {
    fn from(s: &str) -> LayerId {
        LayerId::new(s)
    }
}

impl std::fmt::Display for LayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Definition of a layer: matrix rows are sorted by `order`, top to bottom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LayerDef {
    pub id: LayerId,
    /// Text shown for the layer.
    pub label: String,
    pub order: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub layer: LayerId,
    pub domain: String,
}

/// Key = top-level symbol id (class granularity).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Classification {
    pub placements: BTreeMap<String, Placement>,
    /// Module tree, present when domains come from modules (see `Strategy::Auto`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_tree: Option<Vec<DomainNode>>,
}

/// Node of the module tree: a leaf is one matrix domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DomainNode {
    pub label: String,
    /// Matrix domain, on leaves only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub domain: Option<String>,
    /// Number of classes below this node.
    pub size: usize,
    pub children: Vec<DomainNode>,
}

/// One analysed root of a multi-root matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Project {
    pub name: String,
    pub root: String,
    pub language: Language,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Matrix {
    /// With several roots: the first root's name (see `projects`).
    pub project: String,
    /// With several roots: the first root's language (see `projects`).
    pub language: Language,
    /// Analysed roots when there are at least two (ids and domains are then
    /// prefixed by the project name); omitted for a single root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<Project>,
    /// Definitions of the layers present, sorted by `order`.
    pub layers: Vec<LayerDef>,
    pub domains: Vec<String>,
    pub cells: Vec<Cell>,
    /// Grouped by source cell, indices into `cells` (see docs/api.md).
    pub edges: Vec<EdgeGroup>,
    /// Same shape as `edges`, from HTTP edges only (frontend -> backend);
    /// omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub http_edges: Vec<EdgeGroup>,
    /// Same shape as `edges`, from GWT RPC edges only (client -> server
    /// implementation); omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rpc_edges: Vec<EdgeGroup>,
    pub stats: Stats,
    /// Things the user should know about this analysis (indexing failed,
    /// stale index...). Omitted from JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Id of the cached analysis this matrix comes from, set by the app so
    /// `cell_symbols` can detect a replaced analysis; omitted when 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub generation: u64,
    /// Module tree whose leaves are `domains`; omitted when domains are name roots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub domain_tree: Option<Vec<DomainNode>>,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Cell {
    /// `[<lens prefix>]<layer>:<domain>`: "business:account" in the default
    /// lens (`layer-domain`, empty prefix), "package/business:org.foo" in
    /// `layer-package` (see `classify::lens`). Read `layer` and `domain`
    /// instead of parsing the id.
    pub id: String,
    pub layer: LayerId,
    /// The column: an inferred domain, or a module path in `layer-package`.
    pub domain: String,
    /// Number of top-level symbols; the list is `matrix::cell_symbols_in`
    /// (the `cell_symbols` command).
    pub size: usize,
}

impl Cell {
    pub fn id_for(layer: &LayerId, domain: &str) -> String {
        format!("{}:{}", layer.as_str(), domain)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CellSymbol {
    pub id: String,
    pub name: String,
    pub file: String,
}

/// One page of the symbols of some cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CellSymbols {
    pub total: usize,
    pub offset: usize,
    pub symbols: Vec<CellSymbol>,
}

/// Edges leaving one cell; indices refer to `Matrix::cells`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EdgeGroup {
    pub from: usize,
    /// `[cell index, weight]`, sorted by index.
    pub targets: Vec<(usize, u32)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Stats {
    pub files: usize,
    pub symbols: usize,
    pub edges: usize,
    pub elapsed_ms: u64,
}

/// One commit of the project history (`pipeline::history`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Commit {
    pub id: String,
    pub short: String,
    pub parents: Vec<String>,
    /// Diff base for this commit: first parent, or the empty tree for a root commit.
    pub base: String,
    pub subject: String,
    pub author: String,
    /// ISO 8601 author date.
    pub date: String,
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct History {
    /// Uncommitted changes present in the working tree.
    pub dirty: bool,
    pub commits: Vec<Commit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Impact {
    pub base: String,
    pub head: Option<String>,
    pub changed_files: Vec<String>,
    pub changed_symbols: Vec<String>,
    pub changed_cells: Vec<String>,
    pub impacted_cells: Vec<ImpactedCell>,
    pub impacted_symbols: Vec<ImpactedSymbol>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Matrix of the analysed snapshot; set only when `head` was analysed in
    /// its own checkout, so the UI can show the cells that exist at that commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub matrix: Option<Matrix>,
}

impl Impact {
    /// The single adapter from a git-diff [`Highlight`] to the `Impact` wire
    /// shape: symbols and cells of role [`Role::Changed`] become
    /// `changed_symbols` / `changed_cells` (sorted by id); every other role
    /// becomes `impacted_symbols` / `impacted_cells` with its distance (sorted
    /// by distance, then id). The highlight's warnings are kept; `matrix` is
    /// left unset for the caller.
    pub fn from_highlight(
        base: String,
        head: Option<String>,
        mut changed_files: Vec<String>,
        highlight: Highlight,
    ) -> Impact {
        changed_files.sort();
        changed_files.dedup();
        let mut changed_symbols = Vec::new();
        let mut impacted_symbols = Vec::new();
        for s in highlight.symbols {
            match s.role {
                Role::Changed => changed_symbols.push(s.id),
                Role::Reached => impacted_symbols.push(ImpactedSymbol {
                    id: s.id,
                    distance: s.distance,
                }),
            }
        }
        let mut changed_cells = Vec::new();
        let mut impacted_cells = Vec::new();
        for c in highlight.cells {
            match c.role {
                Role::Changed => changed_cells.push(c.id),
                Role::Reached => impacted_cells.push(ImpactedCell {
                    id: c.id,
                    distance: c.distance,
                }),
            }
        }
        changed_symbols.sort();
        changed_cells.sort();
        impacted_symbols.sort_by(|a, b| (a.distance, &a.id).cmp(&(b.distance, &b.id)));
        impacted_cells.sort_by(|a, b| (a.distance, &a.id).cmp(&(b.distance, &b.id)));
        Impact {
            base,
            head,
            changed_files,
            changed_symbols,
            changed_cells,
            impacted_cells,
            impacted_symbols,
            warnings: highlight.warnings,
            matrix: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ImpactedCell {
    pub id: String,
    pub distance: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ImpactedSymbol {
    pub id: String,
    pub distance: u32,
}

/// A lens: one way of quotienting the graph into matrix cells (rows are
/// layers in every lens; columns differ). See `structure_classify::lens`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LensInfo {
    /// Stable id, e.g. `layer-domain` (the default) or `layer-package`.
    pub id: String,
    /// Text shown for the lens.
    pub label: String,
}

/// The cells of an impact in one lens: `changed_cells` and `impacted_cells`
/// of [`Impact`], recomputed from its symbols with that lens' cell mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LensCells {
    pub changed_cells: Vec<String>,
    pub impacted_cells: Vec<ImpactedCell>,
}

/// Textual diff of `base..head` (or the working tree), file by file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Diff {
    pub base: String,
    pub head: Option<String>,
    pub files: Vec<FileDiff>,
    /// Files or lines were left out to bound the payload.
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FileDiff {
    /// Path on the new side (the old path for a deleted file), relative to the project.
    pub path: String,
    /// Set only for a rename.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub binary: bool,
    pub additions: u32,
    pub deletions: u32,
    pub hunks: Vec<Hunk>,
    /// Some hunk lines of this file were left out (counts stay exact).
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Hunk {
    /// The `@@ -a,b +c,d @@ context` line.
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Context,
    Add,
    Del,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DiffLine {
    pub kind: LineKind,
    /// 1-based line number on the old side (absent for an added line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub old: Option<u32>,
    /// 1-based line number on the new side (absent for a removed line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub new: Option<u32>,
    /// Line content without the leading marker.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Progress {
    pub step: String,
    pub pct: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATRIX: &str = include_str!("../../../docs/samples/matrix.json");
    const IMPACT: &str = include_str!("../../../docs/samples/impact.json");
    const HISTORY: &str = include_str!("../../../docs/samples/history.json");

    fn roundtrip<T: Serialize + for<'a> Deserialize<'a> + PartialEq + std::fmt::Debug>(s: &str) {
        let v: T = serde_json::from_str(s).unwrap();
        let back = serde_json::to_value(&v).unwrap();
        let orig: serde_json::Value = serde_json::from_str(s).unwrap();
        assert_eq!(back, orig);
        let again: T = serde_json::from_value(back).unwrap();
        assert_eq!(v, again);
    }

    #[test]
    fn matrix_sample_roundtrips() {
        roundtrip::<Matrix>(MATRIX);
        let m: Matrix = serde_json::from_str(MATRIX).unwrap();
        assert!(m.domain_tree.is_none());
        roundtrip::<Matrix>(include_str!("../../../docs/samples/matrix-modules.json"));
    }

    #[test]
    fn impact_sample_roundtrips() {
        roundtrip::<Impact>(IMPACT);
        roundtrip::<History>(HISTORY);
    }

    #[test]
    fn progress_detail_is_optional() {
        let p: Progress = serde_json::from_str(r#"{"step":"parsing","pct":35}"#).unwrap();
        assert_eq!(p.detail, None);
        assert_eq!(
            serde_json::to_string(&p).unwrap(),
            r#"{"step":"parsing","pct":35}"#
        );
        let p = Progress {
            detail: Some("> Task :x · 3s".into()),
            ..p
        };
        let back: Progress = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn layer_id_is_a_plain_string() {
        let l = LayerId::new("api_client");
        assert_eq!(serde_json::to_string(&l).unwrap(), "\"api_client\"");
        assert_eq!(
            serde_json::from_str::<LayerId>("\"ui\"").unwrap(),
            "ui".into()
        );
        assert_eq!(l.as_str(), "api_client");
        assert!(LayerId::unknown().is_unknown());
        assert_eq!(
            Cell::id_for(&"business".into(), "account"),
            "business:account"
        );
    }

    #[test]
    fn layer_defs_carry_label_and_order() {
        let m: Matrix = serde_json::from_str(MATRIX).unwrap();
        let ids: Vec<&str> = m.layers.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["entry", "business", "persistence", "model", "util"]);
        assert!(m.layers.windows(2).all(|w| w[0].order < w[1].order));
        assert!(m.layers.iter().all(|d| d.label == d.id.as_str()));
    }

    #[test]
    fn sample_cell_ids_match() {
        let m: Matrix = serde_json::from_str(MATRIX).unwrap();
        for c in &m.cells {
            assert_eq!(c.id, Cell::id_for(&c.layer, &c.domain));
        }
    }

    #[test]
    fn impact_from_highlight_splits_by_role() {
        use crate::highlight::{HighlightedCell, HighlightedSymbol, SourceInfo};
        let sym = |id: &str, role, distance| HighlightedSymbol {
            id: id.into(),
            role,
            distance,
        };
        let cell = |id: &str, role, distance| HighlightedCell {
            id: id.into(),
            role,
            distance,
        };
        let h = Highlight {
            source: SourceInfo {
                kind: "git-diff".into(),
                label: "HEAD..working tree".into(),
            },
            symbols: vec![
                sym("B", Role::Changed, 0),
                sym("A", Role::Changed, 0),
                sym("Z", Role::Reached, 1),
                sym("Y", Role::Reached, 2),
                sym("X", Role::Reached, 1),
            ],
            cells: vec![
                cell("entry:a", Role::Reached, 2),
                cell("model:a", Role::Changed, 0),
                cell("business:a", Role::Reached, 1),
            ],
            unresolved: vec![],
            warnings: vec!["w".into()],
        };
        let files = vec!["b.java".into(), "a.java".into(), "b.java".into()];
        let i = Impact::from_highlight("HEAD".into(), None, files, h);
        assert_eq!(i.base, "HEAD");
        assert_eq!(i.head, None);
        assert_eq!(i.changed_files, ["a.java", "b.java"]);
        assert_eq!(i.changed_symbols, ["A", "B"]);
        assert_eq!(i.changed_cells, ["model:a"]);
        let syms: Vec<_> = i
            .impacted_symbols
            .iter()
            .map(|s| (s.id.as_str(), s.distance))
            .collect();
        assert_eq!(syms, [("X", 1), ("Z", 1), ("Y", 2)]);
        let cells: Vec<_> = i
            .impacted_cells
            .iter()
            .map(|c| (c.id.as_str(), c.distance))
            .collect();
        assert_eq!(cells, [("business:a", 1), ("entry:a", 2)]);
        assert_eq!(i.warnings, ["w"]);
        assert!(i.matrix.is_none());
    }
}
