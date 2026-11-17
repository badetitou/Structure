//! An analysis seen through a lens (`structure_classify::lens`): the
//! classification the lens quotients the graph with, and its matrix. The
//! default lens is the analysis itself; another one is built on first use from
//! the graph and the classification (no parsing, no rules) and cached on the
//! analysis, so a lens costs one quotient per analysis.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;

use super::Analysis;
use crate::highlight::{HighlightedSymbol, Role, aggregate_by};
use crate::lens::{Lens, package_classification};
use crate::matrix;
use crate::model::{Classification, ImpactedCell, ImpactedSymbol, LensCells, Matrix};
use structure_classify::merge::MergedClassification;

/// A non-default lens of an analysis.
#[derive(Debug)]
pub struct LensData {
    pub classification: Classification,
    pub matrix: Matrix,
}

/// Lens data built so far, by lens (never the default lens).
#[derive(Debug, Default)]
pub(super) struct LensCache(Mutex<BTreeMap<Lens, Arc<LensData>>>);

/// An analysis through a lens: borrowed for the default lens, built (and
/// shared with the cache) for another one.
pub enum LensView<'a> {
    Own(&'a Analysis),
    Built(Lens, Arc<LensData>),
}

impl LensView<'_> {
    pub fn lens(&self) -> Lens {
        match self {
            LensView::Own(_) => Lens::default(),
            LensView::Built(l, _) => *l,
        }
    }

    pub fn classification(&self) -> &Classification {
        match self {
            LensView::Own(a) => &a.classification,
            LensView::Built(_, d) => &d.classification,
        }
    }

    /// The lens' matrix; it carries the analysis' generation, project(s),
    /// warnings and stats.
    pub fn matrix(&self) -> &Matrix {
        match self {
            LensView::Own(a) => &a.matrix,
            LensView::Built(_, d) => &d.matrix,
        }
    }

    /// Cell id of a top-level symbol in this lens, `None` when unplaced.
    pub fn cell_of(&self, top_level_id: &str) -> Option<String> {
        let p = self.classification().placements.get(top_level_id)?;
        Some(self.lens().cell_id(&p.layer, &p.domain))
    }
}

impl Analysis {
    /// This analysis through `lens` (built on first use, then cached).
    pub fn lens(&self, lens: Lens) -> anyhow::Result<LensView<'_>> {
        if lens == Lens::default() {
            return Ok(LensView::Own(self));
        }
        let mut cache = self
            .lenses
            .0
            .lock()
            .map_err(|_| anyhow!("internal error: lens cache poisoned"))?;
        let data = match cache.get(&lens) {
            Some(d) => d.clone(),
            None => {
                let d = Arc::new(self.build_lens(lens));
                cache.insert(lens, d.clone());
                d
            }
        };
        Ok(LensView::Built(lens, data))
    }

    /// Number of non-default lenses built so far (tests).
    #[cfg(test)]
    pub(crate) fn lenses_built(&self) -> usize {
        self.lenses.0.lock().map(|c| c.len()).unwrap_or(0)
    }

    fn build_lens(&self, lens: Lens) -> LensData {
        let classification = self.lens_classification(lens);
        let old = &self.matrix;
        let mut m = matrix::build_in(
            &self.graph,
            &classification,
            &self.layers,
            &old.project,
            old.stats.elapsed_ms,
            lens.cell_prefix(),
        );
        m.projects = old.projects.clone();
        m.warnings = old.warnings.clone();
        m.generation = old.generation;
        LensData {
            classification,
            matrix: m,
        }
    }

    /// The classification of a non-default `lens`. Several projects: each
    /// one's part is quotiented with its own language's module separator and
    /// merged as [`super::analyze_many`] merges classifications (domains
    /// `<project>/<column>`, one tree node per project).
    fn lens_classification(&self, lens: Lens) -> Classification {
        let sep = |l| crate::lang::pure(l).module_separator();
        let projects = &self.matrix.projects;
        if projects.is_empty() {
            let own =
                lens.classification(&self.graph, &self.classification, sep(self.graph.language));
            return own.unwrap_or_else(|| self.classification.clone());
        }
        let mut merged = MergedClassification::default();
        for p in projects {
            let part = match lens {
                Lens::LayerDomain => continue,
                Lens::LayerPackage => package_classification(
                    &self.graph,
                    &self.classification,
                    sep(p.language),
                    &format!("{}::", p.name),
                ),
            };
            let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
            for pl in part.placements.values() {
                *sizes.entry(pl.domain.clone()).or_default() += 1;
            }
            merged.add(&p.name, part, sizes);
        }
        merged.finish()
    }
}

/// The cells of an impact's symbols in `view`'s lens: a cell holding a
/// changed symbol is changed, else impacted at the smallest distance of its
/// symbols (the aggregate step of `structure-highlight` with the lens' cell
/// mapping). Sorted as [`crate::model::Impact`] sorts its cells.
pub fn lens_cells(view: &LensView, changed: &[String], impacted: &[ImpactedSymbol]) -> LensCells {
    let symbols: Vec<HighlightedSymbol> = changed
        .iter()
        .map(|id| HighlightedSymbol {
            id: id.clone(),
            role: Role::Changed,
            distance: 0,
        })
        .chain(impacted.iter().map(|s| HighlightedSymbol {
            id: s.id.clone(),
            role: Role::Reached,
            distance: s.distance,
        }))
        .collect();
    let mut changed_cells = Vec::new();
    let mut impacted_cells = Vec::new();
    for c in aggregate_by(&symbols, |id| view.cell_of(id)) {
        match c.role {
            Role::Changed => changed_cells.push(c.id),
            Role::Reached => impacted_cells.push(ImpactedCell {
                id: c.id,
                distance: c.distance,
            }),
        }
    }
    changed_cells.sort();
    impacted_cells.sort_by(|a, b| (a.distance, &a.id).cmp(&(b.distance, &b.id)));
    LensCells {
        changed_cells,
        impacted_cells,
    }
}
