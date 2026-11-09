//! Links GWT RPC calls (`Symbol::rpc_calls`, client) to the methods of the
//! classes implementing the service (server) over a graph, merged or not.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::ir::{Edge, EdgeKind, Graph, Symbol, SymbolKind};

/// Outcome of [`link_rpc`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RpcLinkStats {
    /// Calls found (every `rpc_calls` entry).
    pub calls: usize,
    /// Calls that became at least one edge.
    pub linked: usize,
    /// Calls to a service that is neither in the graph nor implemented by name.
    pub unresolved_service: usize,
    /// Calls to a known service with no implementation, or none having the method.
    pub no_impl: usize,
    /// Calls on a service nothing says exists (`Foo.Util.getInstance()` with no
    /// `FooAsync` type, no `RemoteService` Foo in the graph): not RPC as far as
    /// we know, left out of every other counter.
    pub maybe_rpc: usize,
    /// Calls not linked, as "caller -> Service#method(arity)".
    pub unmatched: Vec<String>,
}

struct Index<'g> {
    by_id: HashMap<&'g str, &'g Symbol>,
    /// type -> its direct subtypes
    subs: HashMap<&'g str, Vec<&'g str>>,
    /// type -> its direct supertypes
    supers: HashMap<&'g str, Vec<&'g str>>,
}

impl<'g> Index<'g> {
    fn new(graph: &'g Graph) -> Self {
        let mut subs: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut supers: HashMap<&str, Vec<&str>> = HashMap::new();
        for e in &graph.edges {
            if matches!(e.kind, EdgeKind::Extends | EdgeKind::Implements) {
                subs.entry(&e.to).or_default().push(&e.from);
                supers.entry(&e.from).or_default().push(&e.to);
            }
        }
        Index {
            by_id: graph.symbols.iter().map(|s| (s.id.as_str(), s)).collect(),
            subs,
            supers,
        }
    }

    /// Interfaces named by `service`: its id, the id inside a project of a
    /// merged graph (`<project>::<id>`), or any interface of that simple name.
    fn interfaces(&self, graph: &'g Graph, service: &str) -> Vec<&'g str> {
        if let Some((id, _)) = self.by_id.get_key_value(service) {
            return vec![*id];
        }
        let suffix = format!("::{service}");
        let simple = (!service.contains('.')).then_some(service);
        graph
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Interface)
            .filter(|s| s.id.ends_with(&suffix) || simple.is_some_and(|n| s.name == n))
            .map(|s| s.id.as_str())
            .collect()
    }

    /// Whether the interface extends `RemoteService`, through parsed interfaces.
    fn is_remote(&self, id: &str) -> bool {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut todo: Vec<&str> = vec![id];
        while let Some(t) = todo.pop() {
            if !seen.insert(t) {
                continue;
            }
            if self
                .by_id
                .get(t)
                .is_some_and(|s| s.supertypes.iter().any(|x| x == "RemoteService"))
            {
                return true;
            }
            todo.extend(self.supers.get(t).into_iter().flatten());
        }
        false
    }

    /// Classes implementing any of `ifaces`, transitively, plus classes whose
    /// unresolved supertype is named like the service. That fallback is for an
    /// interface living in another project (or a library): it never applies to
    /// a class of the same project as a found interface, where an unresolved
    /// supertype of that name is some other type.
    fn implementations(
        &self,
        graph: &'g Graph,
        ifaces: &[&'g str],
        service: &str,
    ) -> BTreeSet<&'g str> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut todo: Vec<&str> = ifaces.to_vec();
        let mut out = BTreeSet::new();
        while let Some(t) = todo.pop() {
            if !seen.insert(t) {
                continue;
            }
            if self
                .by_id
                .get(t)
                .is_some_and(|s| s.kind == SymbolKind::Class)
            {
                out.insert(t);
            }
            todo.extend(self.subs.get(t).into_iter().flatten());
        }
        let project = |id: &str| id.split_once("::").map(|(p, _)| p.to_string());
        let projects: Vec<Option<String>> = ifaces.iter().map(|i| project(i)).collect();
        let simple = service.rsplit("::").next().unwrap_or(service);
        let simple = simple.rsplit(['.', '$']).next().unwrap_or(simple);
        for s in graph
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class && s.supertypes.iter().any(|t| t == simple))
        {
            let resolved = self
                .supers
                .get(s.id.as_str())
                .into_iter()
                .flatten()
                .any(|t| self.by_id.get(t).is_some_and(|x| x.name == simple));
            let same_project = projects.contains(&project(&s.id));
            if !resolved && (!same_project || ifaces.is_empty()) {
                out.insert(s.id.as_str());
            }
        }
        out
    }

    /// `class#method(arity)` declared by the class or inherited from a
    /// superclass (interface declarations do not count).
    fn find_method(&self, class: &'g str, method: &str, arity: u32) -> Option<&'g str> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut todo: Vec<&str> = vec![class];
        while let Some(c) = todo.pop() {
            if !seen.insert(c) {
                continue;
            }
            if self
                .by_id
                .get(c)
                .is_none_or(|s| s.kind != SymbolKind::Class)
            {
                continue;
            }
            let id = format!("{c}#{method}({arity})");
            if let Some((k, _)) = self.by_id.get_key_value(id.as_str()) {
                return Some(k);
            }
            todo.extend(self.supers.get(c).into_iter().flatten());
        }
        None
    }
}

/// `Rpc` edges from each calling symbol to the implementation methods it
/// reaches, plus stats. The `AsyncCallback` parameter is dropped from the
/// arity first (`Impl#m(n-1)`), then kept (`Impl#m(n)`, methods returning a `Request`).
pub fn link_rpc(graph: &Graph) -> (Vec<Edge>, RpcLinkStats) {
    let mut stats = RpcLinkStats::default();
    if graph.symbols.iter().all(|s| s.rpc_calls.is_empty()) {
        return (vec![], stats);
    }
    let idx = Index::new(graph);
    // (service, maybe) -> (interface found, implementations)
    let mut services: HashMap<(&str, bool), (bool, BTreeSet<&str>)> = HashMap::new();
    let mut pairs: BTreeSet<(&str, &str)> = BTreeSet::new();
    for caller in &graph.symbols {
        for call in &caller.rpc_calls {
            let (found, impls) = services
                .entry((call.service.as_str(), call.maybe))
                .or_insert_with(|| {
                    let mut ifaces = idx.interfaces(graph, &call.service);
                    if call.maybe {
                        ifaces.retain(|i| idx.is_remote(i));
                    }
                    let impls = if call.maybe && ifaces.is_empty() {
                        BTreeSet::new()
                    } else {
                        idx.implementations(graph, &ifaces, &call.service)
                    };
                    (!ifaces.is_empty(), impls)
                });
            if call.maybe && !*found {
                stats.maybe_rpc += 1;
                continue;
            }
            stats.calls += 1;
            let mut targets: BTreeSet<&str> = BTreeSet::new();
            for c in impls.iter() {
                let t = call
                    .arity
                    .checked_sub(1)
                    .and_then(|a| idx.find_method(c, &call.method, a))
                    .or_else(|| idx.find_method(c, &call.method, call.arity));
                targets.extend(t);
            }
            if targets.is_empty() {
                if *found || !impls.is_empty() {
                    stats.no_impl += 1;
                } else {
                    stats.unresolved_service += 1;
                }
                stats.unmatched.push(format!(
                    "{} -> {}#{}({})",
                    caller.id, call.service, call.method, call.arity
                ));
            } else {
                stats.linked += 1;
                pairs.extend(targets.into_iter().map(|t| (caller.id.as_str(), t)));
            }
        }
    }
    let edges = pairs
        .into_iter()
        .map(|(f, t)| Edge {
            from: f.to_string(),
            to: t.to_string(),
            kind: EdgeKind::Rpc,
        })
        .collect();
    (edges, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Language, Range, RpcCall};

    fn sym(id: &str, kind: SymbolKind, container: Option<&str>) -> Symbol {
        let r = Range {
            start_line: 0,
            start_col: 0,
            end_line: 1,
            end_col: 0,
        };
        Symbol {
            id: id.into(),
            kind,
            name: id.rsplit(['.', '$', '#', ':']).next().unwrap_or(id).into(),
            container: container.map(Into::into),
            file: "f".into(),
            range: r,
            name_range: r,
            annotations: vec![],
            supertypes: vec![],
            package: None,
            http: vec![],
            http_calls: vec![],
            rpc_calls: vec![],
        }
    }

    fn call(service: &str, method: &str, arity: u32) -> RpcCall {
        RpcCall {
            service: service.into(),
            method: method.into(),
            arity,
            maybe: false,
        }
    }

    fn edge(from: &str, to: &str, kind: EdgeKind) -> Edge {
        Edge {
            from: from.into(),
            to: to.into(),
            kind,
        }
    }

    fn graph(symbols: Vec<Symbol>, edges: Vec<Edge>) -> Graph {
        Graph {
            root: ".".into(),
            language: Language::Java,
            symbols,
            edges,
        }
    }

    fn pairs(edges: &[Edge]) -> Vec<(&str, &str)> {
        edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect()
    }

    fn base() -> Vec<Symbol> {
        vec![
            sym("p.X", SymbolKind::Interface, None),
            sym("p.XImpl", SymbolKind::Class, None),
            sym("p.XImpl#go(1)", SymbolKind::Method, Some("p.XImpl")),
        ]
    }

    #[test]
    fn async_arity_drops_the_callback() {
        let mut syms = base();
        let mut c = sym("c.Cl#run(0)", SymbolKind::Method, None);
        c.rpc_calls = vec![call("p.X", "go", 2)];
        syms.push(c);
        let g = graph(syms, vec![edge("p.XImpl", "p.X", EdgeKind::Implements)]);
        let (e, s) = link_rpc(&g);
        assert_eq!(pairs(&e), [("c.Cl#run(0)", "p.XImpl#go(1)")]);
        assert_eq!((s.calls, s.linked), (1, 1));
        assert!(e.iter().all(|e| e.kind == EdgeKind::Rpc));
    }

    #[test]
    fn falls_back_to_the_full_arity() {
        let mut syms = base();
        syms.push(sym("p.XImpl#req(1)", SymbolKind::Method, Some("p.XImpl")));
        let mut c = sym("cl", SymbolKind::Method, None);
        c.rpc_calls = vec![call("p.X", "req", 1)];
        syms.push(c);
        let g = graph(syms, vec![edge("p.XImpl", "p.X", EdgeKind::Implements)]);
        assert_eq!(pairs(&link_rpc(&g).0), [("cl", "p.XImpl#req(1)")]);
    }

    #[test]
    fn several_implementations_get_one_edge_each() {
        let mut syms = base();
        syms.push(sym("p.Other", SymbolKind::Class, None));
        syms.push(sym("p.Other#go(1)", SymbolKind::Method, Some("p.Other")));
        // implements through a sub-interface
        syms.push(sym("p.Y", SymbolKind::Interface, None));
        let mut c = sym("cl", SymbolKind::Method, None);
        c.rpc_calls = vec![call("p.X", "go", 2), call("p.X", "go", 2)];
        syms.push(c);
        let g = graph(
            syms,
            vec![
                edge("p.XImpl", "p.X", EdgeKind::Implements),
                edge("p.Y", "p.X", EdgeKind::Extends),
                edge("p.Other", "p.Y", EdgeKind::Implements),
            ],
        );
        let (e, s) = link_rpc(&g);
        assert_eq!(
            pairs(&e),
            [("cl", "p.Other#go(1)"), ("cl", "p.XImpl#go(1)")]
        );
        assert_eq!((s.calls, s.linked), (2, 2));
    }

    #[test]
    fn inherited_implementation_method_is_found() {
        let mut syms = vec![
            sym("p.X", SymbolKind::Interface, None),
            sym("p.Base", SymbolKind::Class, None),
            sym("p.Base#go(0)", SymbolKind::Method, Some("p.Base")),
            sym("p.Impl", SymbolKind::Class, None),
        ];
        let mut c = sym("cl", SymbolKind::Method, None);
        c.rpc_calls = vec![call("p.X", "go", 1)];
        syms.push(c);
        let g = graph(
            syms,
            vec![
                edge("p.Impl", "p.X", EdgeKind::Implements),
                edge("p.Impl", "p.Base", EdgeKind::Extends),
            ],
        );
        assert_eq!(pairs(&link_rpc(&g).0), [("cl", "p.Base#go(0)")]);
    }

    #[test]
    fn unresolved_service_and_missing_impl_are_reported() {
        let mut syms = base();
        let mut c = sym("cl", SymbolKind::Method, None);
        c.rpc_calls = vec![
            call("Nowhere", "go", 1),
            call("p.X", "missing", 1),
            call("p.X", "go", 1),
        ];
        syms.push(c);
        let g = graph(syms, vec![edge("p.XImpl", "p.X", EdgeKind::Implements)]);
        let (e, s) = link_rpc(&g);
        assert_eq!(pairs(&e), [("cl", "p.XImpl#go(1)")]);
        assert_eq!((s.calls, s.linked), (3, 1));
        assert_eq!((s.unresolved_service, s.no_impl), (1, 1));
        assert_eq!(s.unmatched, ["cl -> Nowhere#go(1)", "cl -> p.X#missing(1)"]);
    }

    #[test]
    fn merged_graph_resolves_prefixed_ids_and_unresolved_supertypes() {
        let mut imp = sym("srv::q.Impl", SymbolKind::Class, None);
        imp.supertypes = vec!["X".into()];
        let mut c = sym("web::cl", SymbolKind::Method, None);
        c.rpc_calls = vec![call("p.X", "go", 2)];
        let g = graph(
            vec![
                sym("web::p.X", SymbolKind::Interface, None),
                imp,
                sym("srv::q.Impl#go(1)", SymbolKind::Method, Some("srv::q.Impl")),
                c,
            ],
            vec![],
        );
        let (e, s) = link_rpc(&g);
        assert_eq!(pairs(&e), [("web::cl", "srv::q.Impl#go(1)")]);
        assert_eq!(s.linked, 1);
    }

    #[test]
    fn same_simple_name_fallback_skips_unrelated_types_of_the_project() {
        // `p.X` is found; `o.Other` implements some other `X` (unresolved,
        // same project) and must not become an implementation.
        let mut other = sym("o.Other", SymbolKind::Class, None);
        other.supertypes = vec!["X".into()];
        let mut syms = base();
        syms.push(other);
        syms.push(sym("o.Other#go(1)", SymbolKind::Method, Some("o.Other")));
        let mut c = sym("cl", SymbolKind::Method, None);
        c.rpc_calls = vec![call("p.X", "go", 2)];
        syms.push(c);
        let g = graph(syms, vec![edge("p.XImpl", "p.X", EdgeKind::Implements)]);
        assert_eq!(pairs(&link_rpc(&g).0), [("cl", "p.XImpl#go(1)")]);
    }

    #[test]
    fn maybe_calls_need_a_remote_service_in_the_graph() {
        let mut maybe = sym("cl", SymbolKind::Method, None);
        maybe.rpc_calls = vec![RpcCall {
            maybe: true,
            ..call("X", "go", 2)
        }];
        // `X` exists but is no RemoteService: dropped, counted apart.
        let mut syms = base();
        syms.push(maybe.clone());
        let g = graph(syms, vec![edge("p.XImpl", "p.X", EdgeKind::Implements)]);
        let (e, s) = link_rpc(&g);
        assert!(e.is_empty());
        assert_eq!(
            (s.calls, s.maybe_rpc, s.unresolved_service, s.no_impl),
            (0, 1, 0, 0)
        );
        assert!(s.unmatched.is_empty());
        // Same graph, `X` extends RemoteService: linked.
        let mut syms = base();
        syms[0].supertypes = vec!["RemoteService".into()];
        syms.push(maybe);
        let g = graph(syms, vec![edge("p.XImpl", "p.X", EdgeKind::Implements)]);
        let (e, s) = link_rpc(&g);
        assert_eq!(pairs(&e), [("cl", "p.XImpl#go(1)")]);
        assert_eq!((s.calls, s.linked, s.maybe_rpc), (1, 1, 0));
    }

    #[test]
    fn no_calls_no_work() {
        let g = graph(base(), vec![]);
        let (e, s) = link_rpc(&g);
        assert!(e.is_empty());
        assert_eq!(s, RpcLinkStats::default());
    }
}
