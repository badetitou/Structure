//! Highlight contracts: what to highlight (seeds, propagation) and the
//! highlighted symbols and cells, whatever produced them (git diff, stack
//! trace, symbol list). Computed by `structure-highlight`.

use serde::{Deserialize, Serialize};

/// Where a seed points in the code.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Anchor {
    /// A graph symbol, by exact id.
    Symbol { id: String },
    /// Line ranges of a file (relative to the graph root): 0-based,
    /// end-exclusive; empty `lines` means the whole file.
    Location {
        file: String,
        lines: Vec<(u32, u32)>,
    },
    /// A name as a tool prints it (e.g. a stack frame): a symbol id, else a
    /// top-level symbol name, narrowed by `file_hint` (path or path suffix)
    /// and, among several candidates, by the `line` (0-based) they contain.
    Qualified {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        file_hint: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        line: Option<u32>,
    },
}

/// Why a symbol or cell is highlighted. Declaration order = precedence: when
/// several roles meet on one symbol or cell, the first one wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The seed itself changed (e.g. a git diff touches it).
    Changed,
    /// Reached from a seed by propagation (or seeded as such).
    Reached,
}

/// One starting point of a highlight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Seed {
    pub anchor: Anchor,
    pub role: Role,
}

/// What produced a highlight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SourceInfo {
    /// Machine name of the source, e.g. `git-diff`.
    pub kind: String,
    /// Human label, e.g. `HEAD..working tree`.
    pub label: String,
}

/// Which edges a highlight follows from its seeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Symbols that depend on a seed (what a change can break).
    Callers,
    /// Symbols a seed depends on.
    Callees,
    /// The seeds only.
    None,
}

/// Propagation from the seeds: breadth-first in `direction`, at most
/// `max_depth` edges away (no limit when absent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Propagation {
    pub direction: Direction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_depth: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HighlightRequest {
    pub source: SourceInfo,
    pub seeds: Vec<Seed>,
    pub propagate: Propagation,
}

/// A highlighted top-level symbol; `distance` 0 for a seed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HighlightedSymbol {
    pub id: String,
    pub role: Role,
    pub distance: u32,
}

/// A highlighted cell (the id of a matrix cell of the lens it was
/// aggregated for): the strongest role and the smallest distance of its
/// highlighted symbols.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HighlightedCell {
    pub id: String,
    pub role: Role,
    pub distance: u32,
}

/// Result of a [`HighlightRequest`]. `symbols` and `cells` are sorted by
/// (distance, role, id); `unresolved` keeps the request order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Highlight {
    pub source: SourceInfo,
    pub symbols: Vec<HighlightedSymbol>,
    pub cells: Vec<HighlightedCell>,
    /// Seed anchors that matched no symbol.
    pub unresolved: Vec<Anchor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}
