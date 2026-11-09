//! Links HTTP calls (`Symbol::http_calls`, frontend) to the endpoints that
//! serve them (`Symbol::http`, backend) over a merged graph.

use std::collections::BTreeSet;

use crate::ir::{Edge, EdgeKind, Graph, HttpVerb};

/// Outcome of [`link_http`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpLinkStats {
    /// Calls found (every `http_calls` entry).
    pub calls: usize,
    /// Calls that became an edge.
    pub linked: usize,
    /// Calls whose path is `?`; never linked.
    pub unknown_path: usize,
    /// Calls whose best score is shared by several handlers; no edge.
    pub ambiguous: usize,
    /// Calls with no matching endpoint, as "caller VERB path".
    pub unmatched: Vec<String>,
}

fn segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

fn is_placeholder(seg: &str) -> bool {
    seg.contains('{')
}

fn verbs_match(a: HttpVerb, b: HttpVerb) -> bool {
    a == b || a == HttpVerb::Any || b == HttpVerb::Any
}

/// Score of `endpoint` against the trailing segments of `call`: the number of
/// equal literal segments (then, as tie-break, the number of segments that are
/// placeholders on both sides), or `None` when they do not match. The call may
/// carry extra leading segments; the endpoint may not.
fn score(call: &[&str], endpoint: &[&str]) -> Option<(usize, usize)> {
    if endpoint.is_empty() || endpoint.len() > call.len() {
        return None;
    }
    let tail = &call[call.len() - endpoint.len()..];
    let (mut literal, mut both) = (0, 0);
    for (c, e) in tail.iter().zip(endpoint) {
        if is_placeholder(c) || is_placeholder(e) {
            if is_placeholder(c) && is_placeholder(e) {
                both += 1;
            }
            continue;
        }
        if c != e {
            return None;
        }
        literal += 1;
    }
    // Placeholders alone (e.g. `/{apiUrl}`) must not match everything.
    (literal >= 1).then_some((literal, both))
}

/// `Http` edges from each calling symbol to the handler it reaches, plus stats.
pub fn link_http(graph: &Graph) -> (Vec<Edge>, HttpLinkStats) {
    let endpoints: Vec<(&str, HttpVerb, Vec<&str>)> = graph
        .symbols
        .iter()
        .flat_map(|s| {
            s.http
                .iter()
                .map(move |h| (s.id.as_str(), h.verb, segments(&h.path)))
        })
        .collect();
    let mut stats = HttpLinkStats::default();
    let mut pairs: BTreeSet<(&str, &str)> = BTreeSet::new();
    for caller in &graph.symbols {
        for call in &caller.http_calls {
            stats.calls += 1;
            if call.path == "?" {
                stats.unknown_path += 1;
                continue;
            }
            let segs = segments(&call.path);
            let mut best: Option<(usize, usize)> = None;
            let mut handlers: BTreeSet<&str> = BTreeSet::new();
            for (id, verb, ep) in &endpoints {
                if !verbs_match(call.verb, *verb) {
                    continue;
                }
                let Some(sc) = score(&segs, ep) else {
                    continue;
                };
                match best {
                    Some(b) if sc < b => {}
                    Some(b) if sc == b => {
                        handlers.insert(id);
                    }
                    _ => {
                        best = Some(sc);
                        handlers.clear();
                        handlers.insert(id);
                    }
                }
            }
            match handlers.len() {
                0 => stats.unmatched.push(format!(
                    "{} {} {}",
                    caller.id,
                    format!("{:?}", call.verb).to_uppercase(),
                    call.path
                )),
                1 => {
                    stats.linked += 1;
                    if let Some(h) = handlers.first() {
                        pairs.insert((caller.id.as_str(), h));
                    }
                }
                _ => stats.ambiguous += 1,
            }
        }
    }
    let edges = pairs
        .into_iter()
        .map(|(f, t)| Edge {
            from: f.to_string(),
            to: t.to_string(),
            kind: EdgeKind::Http,
        })
        .collect();
    (edges, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{HttpEndpoint, Language, Range, Symbol, SymbolKind};

    fn ep(verb: HttpVerb, path: &str) -> HttpEndpoint {
        HttpEndpoint {
            verb,
            path: path.into(),
        }
    }

    fn sym(id: &str, http: Vec<HttpEndpoint>, calls: Vec<HttpEndpoint>) -> Symbol {
        let r = Range {
            start_line: 0,
            start_col: 0,
            end_line: 1,
            end_col: 0,
        };
        Symbol {
            id: id.into(),
            kind: SymbolKind::Method,
            name: id.into(),
            container: None,
            file: "f".into(),
            range: r,
            name_range: r,
            annotations: vec![],
            supertypes: vec![],
            package: None,
            http,
            http_calls: calls,
        }
    }

    fn graph(symbols: Vec<Symbol>) -> Graph {
        Graph {
            root: ".".into(),
            language: Language::Java,
            symbols,
            edges: vec![],
        }
    }

    fn pairs(edges: &[Edge]) -> Vec<(&str, &str)> {
        edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect()
    }

    use HttpVerb::*;

    #[test]
    fn placeholders_match_on_both_sides() {
        let g = graph(vec![
            sym("h", vec![ep(Get, "/articles/{slug}/comments")], vec![]),
            sym("c", vec![], vec![ep(Get, "/articles/${slug}/comments")]),
            sym("c2", vec![], vec![ep(Get, "/articles/foo/comments")]),
        ]);
        let (e, s) = link_http(&g);
        assert_eq!(pairs(&e), [("c", "h"), ("c2", "h")]);
        assert_eq!((s.calls, s.linked), (2, 2));
    }

    #[test]
    fn any_verb_matches_either_side() {
        let g = graph(vec![
            sym("h1", vec![ep(Any, "/a")], vec![]),
            sym("h2", vec![ep(Post, "/b")], vec![]),
            sym("c1", vec![], vec![ep(Delete, "/a")]),
            sym("c2", vec![], vec![ep(Any, "/b")]),
            sym("c3", vec![], vec![ep(Get, "/b")]),
        ]);
        let (e, s) = link_http(&g);
        assert_eq!(pairs(&e), [("c1", "h1"), ("c2", "h2")]);
        assert_eq!(s.unmatched, ["c3 GET /b"]);
    }

    #[test]
    fn base_url_prefix_is_tolerated() {
        let g = graph(vec![
            sym("h", vec![ep(Get, "/user")], vec![]),
            sym("c1", vec![], vec![ep(Get, "/api/user")]),
            sym("c2", vec![], vec![ep(Get, "{apiUrl}/user")]),
        ]);
        let (e, s) = link_http(&g);
        assert_eq!(pairs(&e), [("c1", "h"), ("c2", "h")]);
        assert_eq!(s.linked, 2);
    }

    #[test]
    fn a_match_needs_one_literal_segment() {
        let g = graph(vec![
            sym("h", vec![ep(Get, "/tags")], vec![]),
            sym("c1", vec![], vec![ep(Get, "/{apiUrl}")]),
            sym("c2", vec![], vec![ep(Get, "/{base}/tags")]),
        ]);
        let (e, s) = link_http(&g);
        assert_eq!(pairs(&e), [("c2", "h")]);
        assert_eq!(s.unmatched, ["c1 GET /{apiUrl}"]);
    }

    #[test]
    fn endpoint_longer_than_call_is_rejected() {
        let g = graph(vec![
            sym("h", vec![ep(Get, "/ctx/user")], vec![]),
            sym("c", vec![], vec![ep(Get, "/user")]),
        ]);
        let (e, s) = link_http(&g);
        assert!(e.is_empty());
        assert_eq!(s.unmatched, ["c GET /user"]);
    }

    #[test]
    fn most_specific_wins_and_ties_are_ambiguous() {
        let g = graph(vec![
            sym("lit", vec![ep(Get, "/articles/feed")], vec![]),
            sym("var", vec![ep(Get, "/articles/{slug}")], vec![]),
            sym("var2", vec![ep(Get, "/articles/{id}")], vec![]),
            sym("c1", vec![], vec![ep(Get, "/articles/feed")]),
            sym("c2", vec![], vec![ep(Get, "/articles/other")]),
        ]);
        let (e, s) = link_http(&g);
        assert_eq!(pairs(&e), [("c1", "lit")]);
        assert_eq!((s.linked, s.ambiguous), (1, 1));
    }

    #[test]
    fn placeholder_on_both_sides_beats_literal_endpoint() {
        let g = graph(vec![
            sym("lit", vec![ep(Get, "/articles/feed")], vec![]),
            sym("var", vec![ep(Get, "/articles/{slug}")], vec![]),
            sym("c", vec![], vec![ep(Get, "/articles/{slug}")]),
        ]);
        let (e, s) = link_http(&g);
        assert_eq!(pairs(&e), [("c", "var")]);
        assert_eq!(s.ambiguous, 0);
    }

    #[test]
    fn unknown_paths_are_counted_not_linked() {
        let g = graph(vec![
            sym("h", vec![ep(Get, "/a")], vec![]),
            sym("c", vec![], vec![ep(Get, "?")]),
        ]);
        let (e, s) = link_http(&g);
        assert!(e.is_empty());
        assert_eq!((s.calls, s.unknown_path, s.linked), (1, 1, 0));
        assert!(s.unmatched.is_empty());
    }

    #[test]
    fn edges_are_deduplicated_per_pair() {
        let g = graph(vec![
            sym("h", vec![ep(Get, "/a"), ep(Get, "/b")], vec![]),
            sym("c", vec![], vec![ep(Get, "/a"), ep(Get, "/b")]),
        ]);
        let (e, s) = link_http(&g);
        assert_eq!(pairs(&e), [("c", "h")]);
        assert_eq!(s.linked, 2);
    }
}
