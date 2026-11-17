//! The HTTP linker: links HTTP calls (`Requires` ports of protocol `http`,
//! frontend) to the endpoints that serve them (`Provides` ports, backend)
//! over a merged graph.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use crate::ir::{Edge, EdgeKind, Graph, HttpEndpoint, HttpVerb, PortRole, Symbol, protocol};
use crate::linker::{Linker, LinkerStats};

/// Outcome of [`link_http`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpLinkStats {
    /// Calls found (every `http` `Requires` port).
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

impl LinkerStats for HttpLinkStats {
    /// Always: HTTP only runs on several roots, where the line is expected.
    fn reported(&self) -> bool {
        true
    }

    fn summary(&self) -> Vec<String> {
        let mut out = vec![format!(
            "http: {}/{} linked, {} unknown path, {} ambiguous",
            self.linked, self.calls, self.unknown_path, self.ambiguous
        )];
        out.extend(self.unmatched.iter().map(|u| format!("  unmatched: {u}")));
        out
    }
}

/// [`link_http`] as a [`Linker`]; merged graphs only.
pub struct HttpLinker;

impl Linker for HttpLinker {
    fn protocol(&self) -> &'static str {
        protocol::HTTP
    }

    fn single_root(&self) -> bool {
        false
    }

    fn link(&self, graph: &Graph) -> (Vec<Edge>, Arc<dyn LinkerStats>) {
        let (edges, stats) = link_http(graph);
        (edges, Arc::new(stats))
    }
}

/// HTTP endpoints of `s` with `role`.
fn http_ports(s: &Symbol, role: PortRole) -> impl Iterator<Item = &HttpEndpoint> {
    s.ports(protocol::HTTP, role)
        .filter_map(|k| k.as_endpoint())
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

/// Supertype methods overridden (same name and arity) by a subtype method that
/// has endpoints: only the implementation handles the request, whatever the
/// class-level prefixes.
fn overridden_endpoints(graph: &Graph) -> HashSet<&str> {
    let mut supers: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in &graph.edges {
        if matches!(e.kind, EdgeKind::Extends | EdgeKind::Implements) {
            supers.entry(&e.from).or_default().push(&e.to);
        }
    }
    let with_http: HashMap<&str, &Symbol> = graph
        .symbols
        .iter()
        .filter(|s| http_ports(s, PortRole::Provides).next().is_some())
        .map(|s| (s.id.as_str(), s))
        .collect();
    let mut out = HashSet::new();
    for s in with_http.values() {
        let (Some(tid), Some((_, suffix))) = (s.container.as_deref(), s.id.split_once('#')) else {
            continue;
        };
        let mut seen: HashSet<&str> = HashSet::new();
        let mut todo: Vec<&str> = supers.get(tid).cloned().unwrap_or_default();
        while let Some(t) = todo.pop() {
            if !seen.insert(t) {
                continue;
            }
            todo.extend(supers.get(t).into_iter().flatten());
            let mid = format!("{t}#{suffix}");
            if let Some(m) = with_http.get(mid.as_str()) {
                out.insert(m.id.as_str());
            }
        }
    }
    out
}

/// `Remote { http }` edges from each calling symbol to the handler it reaches, plus stats.
pub fn link_http(graph: &Graph) -> (Vec<Edge>, HttpLinkStats) {
    let shadowed = overridden_endpoints(graph);
    let endpoints: Vec<(&str, HttpVerb, Vec<&str>)> = graph
        .symbols
        .iter()
        .flat_map(|s| {
            let shadowed = &shadowed;
            http_ports(s, PortRole::Provides)
                .filter(move |_| !shadowed.contains(s.id.as_str()))
                .map(move |h| (s.id.as_str(), h.verb, segments(&h.path)))
        })
        .collect();
    let mut stats = HttpLinkStats::default();
    let mut pairs: BTreeSet<(&str, &str)> = BTreeSet::new();
    for caller in &graph.symbols {
        for call in http_ports(caller, PortRole::Requires) {
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
            kind: EdgeKind::remote(protocol::HTTP),
        })
        .collect();
    (edges, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Language, Port, PortKey, Range, Symbol, SymbolKind};

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
            ports: http
                .into_iter()
                .map(|e| (PortRole::Provides, e))
                .chain(calls.into_iter().map(|e| (PortRole::Requires, e)))
                .map(|(r, e)| Port::new(protocol::HTTP, r, PortKey::Endpoint(e)))
                .collect(),
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
    fn implementation_preferred_over_interface() {
        let mut i = sym("p.Api#go(0)", vec![ep(Get, "/a/go")], vec![]);
        i.container = Some("p.Api".into());
        let mut c = sym("p.Ctl#go(0)", vec![ep(Get, "/a/go")], vec![]);
        c.container = Some("p.Ctl".into());
        let mut g = graph(vec![i, c, sym("fe", vec![], vec![ep(Get, "/a/go")])]);
        g.edges.push(Edge {
            from: "p.Ctl".into(),
            to: "p.Api".into(),
            kind: EdgeKind::Implements,
        });
        let (e, s) = link_http(&g);
        assert_eq!(pairs(&e), [("fe", "p.Ctl#go(0)")]);
        assert_eq!((s.linked, s.ambiguous), (1, 0));
        g.edges.clear();
        assert_eq!(link_http(&g).1.ambiguous, 1);
    }

    #[test]
    fn implementation_preferred_with_different_prefixes() {
        let mut i = sym("p.Api#go(0)", vec![ep(Get, "/I/go")], vec![]);
        i.container = Some("p.Api".into());
        let mut c = sym("p.Ctl#go(0)", vec![ep(Get, "/C/go")], vec![]);
        c.container = Some("p.Ctl".into());
        let mut g = graph(vec![i, c, sym("fe", vec![], vec![ep(Get, "/{base}/go")])]);
        g.edges.push(Edge {
            from: "p.Ctl".into(),
            to: "p.Api".into(),
            kind: EdgeKind::Implements,
        });
        let (e, s) = link_http(&g);
        assert_eq!(pairs(&e), [("fe", "p.Ctl#go(0)")]);
        assert_eq!(s.ambiguous, 0);
        g.edges.clear();
        assert_eq!(link_http(&g).1.ambiguous, 1);
    }

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
    fn edges_are_remote_http_and_ports_of_other_protocols_are_ignored() {
        let mut c = sym("c", vec![], vec![ep(Get, "/a")]);
        c.ports.push(Port::new(
            "other",
            PortRole::Requires,
            PortKey::Endpoint(ep(Get, "/b")),
        ));
        let mut h = sym("h", vec![ep(Get, "/a")], vec![]);
        h.ports.push(Port::new(
            "other",
            PortRole::Provides,
            PortKey::Endpoint(ep(Get, "/b")),
        ));
        let (e, s) = link_http(&graph(vec![h, c]));
        assert_eq!(pairs(&e), [("c", "h")]);
        assert_eq!(e[0].kind, EdgeKind::remote(protocol::HTTP));
        assert_eq!((s.calls, s.linked), (1, 1));
    }

    #[test]
    fn summary_matches_the_cli_lines() {
        let g = graph(vec![
            sym("h", vec![ep(Get, "/a")], vec![]),
            sym(
                "c",
                vec![],
                vec![ep(Get, "/a"), ep(Post, "/z"), ep(Get, "?")],
            ),
        ]);
        let (_, s) = link_http(&g);
        assert!(s.reported());
        assert_eq!(
            s.summary(),
            [
                "http: 1/3 linked, 1 unknown path, 0 ambiguous",
                "  unmatched: c POST /z"
            ]
        );
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
