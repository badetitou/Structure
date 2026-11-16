//! What highlighting needs from a graph, independent of any request.

use std::collections::HashMap;
use structure_ir::ir::Graph;

/// Built once per graph and reused by every highlight on it. Symbols are
/// indices into `Graph::symbols`; on duplicate ids the last symbol stands for
/// the id. Edges are lifted to top-level symbols.
#[derive(Debug, Clone, Default)]
pub struct HighlightIndex {
    /// Top-level ancestor of each symbol's id; `None` on a container cycle.
    pub(crate) top: Vec<Option<u32>>,
    /// Symbol standing for each id.
    pub(crate) by_id: HashMap<String, u32>,
    /// Symbols of each file, in graph order.
    pub(crate) by_file: HashMap<String, Vec<u32>>,
    /// `callers[caller_start[t]..caller_start[t + 1]]`: top-level symbols with
    /// an edge to top-level symbol `t` (sorted, deduplicated, never `t`).
    caller_start: Vec<u32>,
    callers: Vec<u32>,
    /// Same layout: top-level symbols `t` has an edge to.
    callee_start: Vec<u32>,
    callees: Vec<u32>,
}

impl HighlightIndex {
    pub fn new(graph: &Graph) -> HighlightIndex {
        let n = graph.symbols.len();
        let by_id: HashMap<&str, u32> = graph
            .symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.as_str(), i as u32))
            .collect();
        let lift = |id: &str| -> Option<u32> {
            let mut cur = *by_id.get(id)?;
            for _ in 0..=n {
                match graph.symbols[cur as usize]
                    .container
                    .as_deref()
                    .and_then(|c| by_id.get(c))
                {
                    Some(&p) => cur = p,
                    None => return Some(cur),
                }
            }
            None
        };
        let top: Vec<Option<u32>> = graph.symbols.iter().map(|s| lift(&s.id)).collect();
        let top_of = |id: &str| by_id.get(id).and_then(|&i| top[i as usize]);

        let mut by_file: HashMap<String, Vec<u32>> = HashMap::new();
        for (i, s) in graph.symbols.iter().enumerate() {
            match by_file.get_mut(s.file.as_str()) {
                Some(v) => v.push(i as u32),
                None => {
                    by_file.insert(s.file.clone(), vec![i as u32]);
                }
            }
        }

        // (caller, callee) pairs between distinct top-level symbols.
        let pairs: Vec<(u32, u32)> = graph
            .edges
            .iter()
            .filter_map(|e| match (top_of(&e.from), top_of(&e.to)) {
                (Some(f), Some(t)) if f != t => Some((f, t)),
                _ => None,
            })
            .collect();
        let (caller_start, callers) = adjacency(n, pairs.iter().map(|&(f, t)| (t, f)).collect());
        let (callee_start, callees) = adjacency(n, pairs);
        HighlightIndex {
            top,
            by_id: by_id.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            by_file,
            caller_start,
            callers,
            callee_start,
            callees,
        }
    }

    pub(crate) fn callers_of(&self, t: u32) -> &[u32] {
        slice(&self.caller_start, &self.callers, t)
    }

    pub(crate) fn callees_of(&self, t: u32) -> &[u32] {
        slice(&self.callee_start, &self.callees, t)
    }
}

/// `(key, value)` pairs over `0..n` laid out per key (CSR): sorted and
/// deduplicated `values[start[k]..start[k + 1]]` for each key `k`.
fn adjacency(n: usize, mut pairs: Vec<(u32, u32)>) -> (Vec<u32>, Vec<u32>) {
    pairs.sort_unstable();
    pairs.dedup();
    let mut start = vec![0u32; n + 1];
    for &(k, _) in &pairs {
        start[k as usize + 1] += 1;
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    (start, pairs.into_iter().map(|(_, v)| v).collect())
}

fn slice<'a>(start: &[u32], values: &'a [u32], t: u32) -> &'a [u32] {
    match (start.get(t as usize), start.get(t as usize + 1)) {
        (Some(&a), Some(&b)) => &values[a as usize..b as usize],
        _ => &[],
    }
}
