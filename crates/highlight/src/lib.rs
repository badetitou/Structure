//! Source-agnostic highlights over a graph: resolve seed anchors to top-level
//! symbols, propagate along callers or callees, aggregate into `layer:domain`
//! cells. Pure code (no process, filesystem or git): it also runs in wasm.

mod index;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use structure_ir::ir::Graph;
use structure_ir::model::{Cell, Classification};

pub use index::HighlightIndex;
pub use structure_ir::highlight::{
    Anchor, Direction, Highlight, HighlightRequest, HighlightedCell, HighlightedSymbol,
    Propagation, Role, Seed, SourceInfo,
};

/// Highlight of `request` on `graph`; `index` must have been built from `graph`.
/// The composition of [`resolve`], [`propagate`] and [`aggregate`].
pub fn highlight(
    graph: &Graph,
    cls: &Classification,
    index: &HighlightIndex,
    request: &HighlightRequest,
) -> Highlight {
    let mut seeds: BTreeMap<u32, Role> = BTreeMap::new();
    let mut unresolved = Vec::new();
    let mut warnings = Vec::new();
    for seed in &request.seeds {
        match resolve(graph, index, &seed.anchor) {
            Resolution::Symbols(found) => {
                for t in found {
                    let role = seeds.entry(t).or_insert(seed.role);
                    *role = (*role).min(seed.role);
                }
            }
            Resolution::Unresolved => unresolved.push(seed.anchor.clone()),
            Resolution::Ambiguous(n) => {
                if let Anchor::Qualified { name, .. } = &seed.anchor {
                    warnings.push(format!("ambiguous name {name}: {n} candidates"));
                }
                unresolved.push(seed.anchor.clone());
            }
        }
    }
    let id = |i: u32| graph.symbols[i as usize].id.clone();
    let seed_set: BTreeSet<u32> = seeds.keys().copied().collect();
    let reached = propagate(index, &seed_set, request.propagate);
    let mut symbols: Vec<HighlightedSymbol> = seeds
        .iter()
        .map(|(&i, &role)| HighlightedSymbol {
            id: id(i),
            role,
            distance: 0,
        })
        .chain(reached.into_iter().map(|(i, distance)| HighlightedSymbol {
            id: id(i),
            role: Role::Reached,
            distance,
        }))
        .collect();
    symbols.sort_by(|a, b| (a.distance, a.role, &a.id).cmp(&(b.distance, b.role, &b.id)));
    let cells = aggregate(cls, &symbols);
    Highlight {
        source: request.source.clone(),
        symbols,
        cells,
        unresolved,
        warnings,
    }
}

/// Top-level symbols (indices into `graph.symbols`) an anchor stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Never empty.
    Symbols(BTreeSet<u32>),
    Unresolved,
    /// A qualified name matching this many top-level symbols.
    Ambiguous(usize),
}

/// Resolve step: the top-level symbols an anchor stands for.
/// - `Symbol`: the symbol with that exact id, lifted to its top-level symbol.
/// - `Location`: the top-level symbols whose range overlaps a line range. No
///   lines, or a range outside every symbol of the file (package, imports,
///   file comments), mark every symbol of the file. An unknown file marks
///   nothing.
/// - `Qualified`: the symbol with that exact id, else the top-level symbols
///   named `name` in a file matching `file_hint` (equal, or ending with
///   `/hint`), narrowed to those containing `line` when several match.
pub fn resolve(graph: &Graph, index: &HighlightIndex, anchor: &Anchor) -> Resolution {
    let found = match anchor {
        Anchor::Symbol { id } => lift_id(index, id),
        Anchor::Location { file, lines } => resolve_location(graph, index, file, lines),
        Anchor::Qualified {
            name,
            file_hint,
            line,
        } => {
            let by_id = lift_id(index, name);
            if !by_id.is_empty() {
                by_id
            } else {
                let mut found = named(graph, index, name, file_hint.as_deref());
                if found.len() > 1
                    && let Some(l) = *line
                {
                    found.retain(|&t| {
                        let (st, en) = span(graph, t);
                        st <= l && l < en
                    });
                }
                if found.len() > 1 {
                    return Resolution::Ambiguous(found.len());
                }
                found
            }
        }
    };
    if found.is_empty() {
        Resolution::Unresolved
    } else {
        Resolution::Symbols(found)
    }
}

/// The top-level symbol standing for `id`, if any.
fn lift_id(index: &HighlightIndex, id: &str) -> BTreeSet<u32> {
    index
        .by_id
        .get(id)
        .and_then(|&i| index.top[i as usize])
        .into_iter()
        .collect()
}

fn resolve_location(
    graph: &Graph,
    index: &HighlightIndex,
    file: &str,
    lines: &[(u32, u32)],
) -> BTreeSet<u32> {
    let spans: Vec<(u32, u32, u32)> = index
        .by_file
        .get(file)
        .into_iter()
        .flatten()
        .map(|&i| {
            let (st, en) = span(graph, i);
            (i, st, en)
        })
        .collect();
    let whole = lines.is_empty()
        || lines
            .iter()
            .any(|&(a, b)| !spans.iter().any(|&(_, st, en)| st < b && a < en));
    let mut found = BTreeSet::new();
    for &(i, st, en) in &spans {
        if whole || lines.iter().any(|&(a, b)| st < b && a < en) {
            found.extend(index.top[i as usize]);
        }
    }
    found
}

/// Lines of symbol `i`: 0-based, end-exclusive, at least one.
fn span(graph: &Graph, i: u32) -> (u32, u32) {
    let r = &graph.symbols[i as usize].range;
    let st = r.start_line;
    (st, (r.end_line + u32::from(r.end_col > 0)).max(st + 1))
}

/// Top-level symbols named `name`, in files matching `file_hint` if given.
fn named(
    graph: &Graph,
    index: &HighlightIndex,
    name: &str,
    file_hint: Option<&str>,
) -> BTreeSet<u32> {
    let file_ok = |file: &str| match file_hint {
        None => true,
        Some(h) => file == h || file.strip_suffix(h).is_some_and(|p| p.ends_with('/')),
    };
    graph
        .symbols
        .iter()
        .enumerate()
        .filter(|(i, s)| {
            index.top.get(*i) == Some(&Some(*i as u32)) && s.name == name && file_ok(&s.file)
        })
        .map(|(i, _)| i as u32)
        .collect()
}

/// Propagate step: breadth-first from the top-level `seeds` along
/// `propagation.direction`, at most `max_depth` edges away. Returns every
/// reached top-level symbol that is not a seed, with its shortest distance
/// (>= 1) from any seed.
pub fn propagate(
    index: &HighlightIndex,
    seeds: &BTreeSet<u32>,
    propagation: Propagation,
) -> BTreeMap<u32, u32> {
    let next = |t: u32| match propagation.direction {
        Direction::Callers => index.callers_of(t),
        Direction::Callees => index.callees_of(t),
        Direction::None => &[],
    };
    let mut dist: BTreeMap<u32, u32> = seeds.iter().map(|&c| (c, 0)).collect();
    let mut queue: VecDeque<(u32, u32)> = seeds.iter().map(|&c| (c, 0)).collect();
    while let Some((t, d)) = queue.pop_front() {
        if propagation.max_depth.is_some_and(|m| d >= m) {
            continue;
        }
        for &n in next(t) {
            if let std::collections::btree_map::Entry::Vacant(e) = dist.entry(n) {
                e.insert(d + 1);
                queue.push_back((n, d + 1));
            }
        }
    }
    dist.retain(|i, _| !seeds.contains(i));
    dist
}

/// Aggregate step: symbols to `layer:domain` cells through the classification.
/// A cell takes the strongest role and the smallest distance of its symbols;
/// unplaced symbols have no cell. Sorted by (distance, role, id).
pub fn aggregate(cls: &Classification, symbols: &[HighlightedSymbol]) -> Vec<HighlightedCell> {
    let mut cells: BTreeMap<String, (Role, u32)> = BTreeMap::new();
    for s in symbols {
        if let Some(p) = cls.placements.get(&s.id) {
            let e = cells
                .entry(Cell::id_for(&p.layer, &p.domain))
                .or_insert((s.role, s.distance));
            *e = (e.0.min(s.role), e.1.min(s.distance));
        }
    }
    let mut out: Vec<HighlightedCell> = cells
        .into_iter()
        .map(|(id, (role, distance))| HighlightedCell { id, role, distance })
        .collect();
    out.sort_by(|a, b| (a.distance, a.role, &a.id).cmp(&(b.distance, b.role, &b.id)));
    out
}

#[cfg(test)]
mod tests;
