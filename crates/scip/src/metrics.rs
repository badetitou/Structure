//! Per-phase timings and cost-driving counts of one analysis.

use std::collections::BTreeMap;

use crate::scip::ScipIndex;
use structure_ir::ir::{EdgeKind, Graph};

#[derive(Debug, Clone, PartialEq)]
pub struct PhaseTiming {
    pub name: &'static str,
    pub ms: u64,
    /// Extra info, e.g. "reused index.scip" for the indexer.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metrics {
    pub phases: Vec<PhaseTiming>,
    pub scip_documents: usize,
    pub scip_occurrences: usize,
    /// Final edges by kind name, excluding targets `ext:*`.
    pub edges_by_kind: BTreeMap<&'static str, usize>,
    /// Final edges whose target is `ext:*`, by kind name.
    pub ext_edges_by_kind: BTreeMap<&'static str, usize>,
}

fn kind_name(k: EdgeKind) -> &'static str {
    match k {
        EdgeKind::Call => "call",
        EdgeKind::Implements => "implements",
        EdgeKind::Extends => "extends",
        EdgeKind::Uses => "uses",
        EdgeKind::Http => "http",
    }
}

impl Metrics {
    /// Records the time elapsed since `start` as phase `name`.
    pub fn phase(&mut self, name: &'static str, start: std::time::Instant, note: Option<&str>) {
        self.phases.push(PhaseTiming {
            name,
            ms: start.elapsed().as_millis() as u64,
            note: note.map(str::to_string),
        });
    }

    pub fn record_scip(&mut self, idx: &ScipIndex) {
        self.scip_documents = idx.documents.len();
        self.scip_occurrences = idx.documents.iter().map(|d| d.occurrences.len()).sum();
    }

    pub fn record_edges(&mut self, g: &Graph) {
        self.edges_by_kind.clear();
        self.ext_edges_by_kind.clear();
        for e in &g.edges {
            let map = if e.to.starts_with("ext:") {
                &mut self.ext_edges_by_kind
            } else {
                &mut self.edges_by_kind
            };
            *map.entry(kind_name(e.kind)).or_default() += 1;
        }
    }
}

/// Peak resident set size of this process in bytes, if the platform reports it.
pub fn peak_rss_bytes() -> Option<u64> {
    let mut ru = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `ru` is a valid, writable rusage; getrusage only writes to it
    // and it is read only when the call returns 0 (initialised).
    let ru = unsafe {
        if libc::getrusage(libc::RUSAGE_SELF, ru.as_mut_ptr()) != 0 {
            return None;
        }
        ru.assume_init()
    };
    let max = u64::try_from(ru.ru_maxrss).ok()?;
    // macOS reports bytes, Linux KiB.
    Some(if cfg!(target_os = "macos") {
        max
    } else {
        max * 1024
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use structure_ir::ir::{Edge, Language};

    #[test]
    fn counts_edges_by_kind_and_ext() {
        let e = |to: &str, kind| Edge {
            from: "a".into(),
            to: to.into(),
            kind,
        };
        let g = Graph {
            root: String::new(),
            language: Language::Java,
            symbols: vec![],
            edges: vec![
                e("b", EdgeKind::Call),
                e("ext:List", EdgeKind::Uses),
                e("ext:Map", EdgeKind::Uses),
            ],
        };
        let mut m = Metrics::default();
        m.record_edges(&g);
        assert_eq!(m.edges_by_kind["call"], 1);
        assert_eq!(m.ext_edges_by_kind["uses"], 2);
    }

    #[test]
    fn peak_rss_is_positive() {
        assert!(peak_rss_bytes().is_some_and(|b| b > 0));
    }
}
