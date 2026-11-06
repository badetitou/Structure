//! Matrix, classification and impact output contracts.

use crate::ir::Language;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Declaration order = matrix rows, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Ui,
    State,
    ApiClient,
    Entry,
    Business,
    Persistence,
    Model,
    Util,
    Unknown,
}

impl Layer {
    /// snake_case name, as used in serde and cell ids.
    pub fn as_str(&self) -> &'static str {
        match self {
            Layer::Ui => "ui",
            Layer::State => "state",
            Layer::ApiClient => "api_client",
            Layer::Entry => "entry",
            Layer::Business => "business",
            Layer::Persistence => "persistence",
            Layer::Model => "model",
            Layer::Util => "util",
            Layer::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub layer: Layer,
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
pub struct DomainNode {
    pub label: String,
    /// Matrix domain, on leaves only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Number of classes below this node.
    pub size: usize,
    pub children: Vec<DomainNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Matrix {
    pub project: String,
    pub language: Language,
    pub layers: Vec<Layer>,
    pub domains: Vec<String>,
    pub cells: Vec<Cell>,
    /// Grouped by source cell, indices into `cells` (see docs/api.md).
    pub edges: Vec<EdgeGroup>,
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
    pub domain_tree: Option<Vec<DomainNode>>,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    /// "<layer>:<domain>", e.g. "business:account".
    pub id: String,
    pub layer: Layer,
    pub domain: String,
    /// Number of top-level symbols; the list is `matrix::cell_symbols`.
    pub size: usize,
}

impl Cell {
    pub fn id_for(layer: Layer, domain: &str) -> String {
        format!("{}:{}", layer.as_str(), domain)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellSymbol {
    pub id: String,
    pub name: String,
    pub file: String,
}

/// One page of the symbols of some cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellSymbols {
    pub total: usize,
    pub offset: usize,
    pub symbols: Vec<CellSymbol>,
}

/// Edges leaving one cell; indices refer to `Matrix::cells`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeGroup {
    pub from: usize,
    /// `[cell index, weight]`, sorted by index.
    pub targets: Vec<(usize, u32)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub files: usize,
    pub symbols: usize,
    pub edges: usize,
    pub elapsed_ms: u64,
}

/// One commit of the project history (`pipeline::history`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
pub struct History {
    /// Uncommitted changes present in the working tree.
    pub dirty: bool,
    pub commits: Vec<Commit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub matrix: Option<Matrix>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImpactedCell {
    pub id: String,
    pub distance: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImpactedSymbol {
    pub id: String,
    pub distance: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    pub step: String,
    pub pct: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    fn layer_as_str_matches_serde() {
        use Layer::*;
        for l in [
            Ui,
            State,
            ApiClient,
            Entry,
            Business,
            Persistence,
            Model,
            Util,
            Unknown,
        ] {
            assert_eq!(serde_json::to_value(l).unwrap(), l.as_str());
        }
    }

    #[test]
    fn layer_order_and_serde() {
        assert!(Layer::Ui < Layer::State);
        assert!(Layer::Entry < Layer::Business);
        assert!(Layer::Model < Layer::Util && Layer::Util < Layer::Unknown);
        assert_eq!(
            serde_json::to_string(&Layer::ApiClient).unwrap(),
            "\"api_client\""
        );
        assert_eq!(Cell::id_for(Layer::Business, "account"), "business:account");
    }

    #[test]
    fn sample_cell_ids_match() {
        let m: Matrix = serde_json::from_str(MATRIX).unwrap();
        for c in &m.cells {
            assert_eq!(c.id, Cell::id_for(c.layer, &c.domain));
        }
    }
}
