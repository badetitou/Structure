//! Cross-project linkers: one per protocol, each turns the ports of a graph
//! (`Symbol::ports`) into `EdgeKind::Remote { protocol }` edges. The pipeline
//! runs every registered linker ([`all`]) through [`run`]; a new protocol is a
//! new linker here plus the frontends that emit its ports, with no IR change.

use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

use crate::http_link::{HttpLinkStats, HttpLinker};
use crate::ir::{Graph, protocol};
use crate::rpc_link::{RpcLinkStats, RpcLinker};

/// What a linker reports about one run.
pub trait LinkerStats: Any + Debug + Send + Sync {
    /// Whether the run is worth reporting (e.g. the graph had calls of the
    /// protocol); unreported runs are left out of [`Analysis`](crate::pipeline::Analysis).
    fn reported(&self) -> bool;
    /// Human summary, one line per entry (CLI output).
    fn summary(&self) -> Vec<String>;
}

/// Matches the ports of one protocol over a graph.
pub trait Linker: Send + Sync {
    /// The protocol of the ports it reads and of the edges it makes.
    fn protocol(&self) -> &'static str;
    /// Whether it also runs on the graph of a single root; otherwise only on
    /// the merged graph of a multi-root analysis.
    fn single_root(&self) -> bool;
    /// `Remote { protocol }` edges, plus stats.
    fn link(&self, graph: &Graph) -> (Vec<crate::ir::Edge>, Arc<dyn LinkerStats>);
}

/// The registered linkers, in report order.
pub fn all() -> &'static [&'static dyn Linker] {
    &[&HttpLinker, &RpcLinker]
}

/// Stats of one linker run.
#[derive(Debug, Clone)]
pub struct LinkerRun {
    pub protocol: &'static str,
    pub stats: Arc<dyn LinkerStats>,
}

/// Runs the linkers that apply (all of them on a `merged` graph, the
/// single-root ones otherwise), adds their edges to `graph` and returns the
/// reported runs in registry order.
pub fn run(graph: &mut Graph, merged: bool) -> Vec<LinkerRun> {
    let mut runs = Vec::new();
    for l in all().iter().filter(|l| merged || l.single_root()) {
        let (edges, stats) = l.link(graph);
        graph.edges.extend(edges);
        if stats.reported() {
            runs.push(LinkerRun {
                protocol: l.protocol(),
                stats,
            });
        }
    }
    runs
}

/// Stats of the `protocol` run, if reported and of type `T`.
pub fn stats_of<'a, T: LinkerStats>(runs: &'a [LinkerRun], protocol: &str) -> Option<&'a T> {
    runs.iter()
        .filter(|r| r.protocol == protocol)
        .find_map(|r| (&*r.stats as &dyn Any).downcast_ref::<T>())
}

/// [`stats_of`] the HTTP linker.
pub fn http_stats(runs: &[LinkerRun]) -> Option<&HttpLinkStats> {
    stats_of(runs, protocol::HTTP)
}

/// [`stats_of`] the GWT RPC linker.
pub fn rpc_stats(runs: &[LinkerRun]) -> Option<&RpcLinkStats> {
    stats_of(runs, protocol::GWT_RPC)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        EdgeKind, HttpEndpoint, HttpVerb, Language, Port, PortKey, PortRole, Range, RpcCall,
        Symbol, SymbolKind,
    };

    fn sym(id: &str, kind: SymbolKind, ports: Vec<Port>) -> Symbol {
        let r = Range {
            start_line: 0,
            start_col: 0,
            end_line: 1,
            end_col: 0,
        };
        Symbol {
            id: id.into(),
            kind,
            name: id.rsplit(['.', '#']).next().unwrap_or(id).into(),
            container: None,
            file: "f".into(),
            range: r,
            name_range: r,
            annotations: vec![],
            supertypes: vec![],
            package: None,
            ports,
        }
    }

    fn http(role: PortRole, path: &str) -> Port {
        Port::new(
            protocol::HTTP,
            role,
            PortKey::Endpoint(HttpEndpoint {
                verb: HttpVerb::Get,
                path: path.into(),
            }),
        )
    }

    fn graph() -> Graph {
        let rpc = Port::new(
            protocol::GWT_RPC,
            PortRole::Requires,
            PortKey::ServiceMethod(RpcCall {
                service: "Nowhere".into(),
                method: "go".into(),
                arity: 1,
                maybe: false,
            }),
        );
        Graph {
            root: ".".into(),
            language: Language::Java,
            symbols: vec![
                sym(
                    "h",
                    SymbolKind::Method,
                    vec![http(PortRole::Provides, "/a")],
                ),
                sym(
                    "c",
                    SymbolKind::Method,
                    vec![http(PortRole::Requires, "/a"), rpc],
                ),
            ],
            edges: vec![],
        }
    }

    #[test]
    fn registry_has_one_linker_per_protocol() {
        let p: Vec<&str> = all().iter().map(|l| l.protocol()).collect();
        assert_eq!(p, [protocol::HTTP, protocol::GWT_RPC]);
    }

    #[test]
    fn single_root_runs_skip_merged_only_linkers() {
        let mut g = graph();
        let runs = run(&mut g, false);
        assert!(g.edges.is_empty());
        assert!(http_stats(&runs).is_none());
        assert_eq!(rpc_stats(&runs).map(|s| s.unresolved_service), Some(1));
    }

    #[test]
    fn merged_runs_add_remote_edges_tagged_by_protocol() {
        let mut g = graph();
        let runs = run(&mut g, true);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].kind, EdgeKind::remote(protocol::HTTP));
        assert_eq!(
            (g.edges[0].from.as_str(), g.edges[0].to.as_str()),
            ("c", "h")
        );
        assert_eq!(http_stats(&runs).map(|s| s.linked), Some(1));
        assert!(rpc_stats(&runs).is_some());
        // Wrong type for the protocol: none.
        assert!(stats_of::<RpcLinkStats>(&runs, protocol::HTTP).is_none());
    }

    #[test]
    fn unreported_runs_are_dropped() {
        let mut g = graph();
        g.symbols[1].ports.retain(|p| p.protocol == protocol::HTTP);
        let runs = run(&mut g, true);
        assert_eq!(
            runs.iter().map(|r| r.protocol).collect::<Vec<_>>(),
            [protocol::HTTP]
        );
    }
}
