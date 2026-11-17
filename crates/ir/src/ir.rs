//! Language-neutral intermediate representation: symbols and edges.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Java,
    #[serde(rename = "typescript")]
    TypeScript,
    Pharo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Class,
    Interface,
    Enum,
    Method,
    Constructor,
    Field,
    Function,
}

/// 0-based, end exclusive (same convention as SCIP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    /// Frontend-local unique id.
    pub id: String,
    pub kind: SymbolKind,
    /// Simple name.
    pub name: String,
    /// Id of the enclosing symbol; None for top-level.
    pub container: Option<String>,
    /// Path relative to project root, '/' separators.
    pub file: String,
    /// Full extent.
    pub range: Range,
    /// Identifier only; used to match SCIP definition occurrences.
    pub name_range: Range,
    /// Annotation/decorator simple names without '@'.
    pub annotations: Vec<String>,
    /// As written in source: extends + implements.
    pub supertypes: Vec<String>,
    /// Java package or TS file's directory path.
    pub package: Option<String>,
    /// What the symbol offers to (`Provides`) or needs from (`Requires`)
    /// another project, per protocol; linked into `Remote` edges by the
    /// protocol's linker (`structure-core` `linker`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<Port>,
}

impl Symbol {
    /// Keys of the ports of `protocol` with `role`, in declaration order.
    pub fn ports<'a>(
        &'a self,
        protocol: &'a str,
        role: PortRole,
    ) -> impl Iterator<Item = &'a PortKey> + 'a {
        self.ports
            .iter()
            .filter(move |p| p.role == role && p.protocol == protocol)
            .map(|p| &p.key)
    }
}

/// Well-known protocol names of [`Port::protocol`] and [`EdgeKind::Remote`].
/// Plain strings: a new protocol only needs a new name, not an IR change.
pub mod protocol {
    /// REST/HTTP: Java handlers provide `{verb, path}`, TS calls require it.
    pub const HTTP: &str = "http";
    /// GWT RPC: client calls require a service method (providers are found
    /// from the type hierarchy by the linker).
    pub const GWT_RPC: &str = "gwt-rpc";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortRole {
    /// Served by the symbol (e.g. an HTTP handler).
    Provides,
    /// Called by the symbol (e.g. an HTTP request, an RPC call).
    Requires,
}

/// Protocol data of a port, by shape so protocols can share a key shape.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortKey {
    /// `{verb, path}` (HTTP).
    Endpoint(HttpEndpoint),
    /// A method of a named service (GWT RPC).
    ServiceMethod(RpcCall),
}

impl PortKey {
    pub fn as_endpoint(&self) -> Option<&HttpEndpoint> {
        match self {
            PortKey::Endpoint(e) => Some(e),
            PortKey::ServiceMethod(_) => None,
        }
    }

    pub fn as_service_method(&self) -> Option<&RpcCall> {
        match self {
            PortKey::ServiceMethod(c) => Some(c),
            PortKey::Endpoint(_) => None,
        }
    }
}

/// One end of a cross-project link, matched by the linker of `protocol`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Port {
    /// See [`protocol`].
    pub protocol: String,
    pub role: PortRole,
    pub key: PortKey,
}

impl Port {
    pub fn new(protocol: &str, role: PortRole, key: PortKey) -> Self {
        Port {
            protocol: protocol.to_string(),
            role,
            key,
        }
    }
}

/// A call to a method of a GWT RPC service, as seen from the client.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RpcCall {
    /// FQN of the sync interface (`X extends RemoteService`), or its simple
    /// name when it is not in the parsed code.
    pub service: String,
    pub method: String,
    /// Argument count at the call site (async: includes the `AsyncCallback`).
    pub arity: u32,
    /// The service is not in the parsed code and nothing says it is one (the
    /// receiver was `X.Util.getInstance()` or a `GWT.create` cast, no `XAsync`
    /// type): linked when the merged graph has such a service, else dropped.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub maybe: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HttpVerb {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Any,
}

/// A `{verb, path}` pair; neutral so it can describe served endpoints and calls.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HttpEndpoint {
    pub verb: HttpVerb,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Call,
    Implements,
    Extends,
    Uses,
    /// Cross-project link made by the linker of `protocol` (see
    /// [`protocol`]): an HTTP call to the handler it reaches, a GWT RPC call
    /// to the implementation method, ...
    Remote {
        protocol: String,
    },
}

impl EdgeKind {
    /// Stable order used wherever edges are sorted or keyed.
    pub fn rank(&self) -> u8 {
        match self {
            EdgeKind::Call => 0,
            EdgeKind::Implements => 1,
            EdgeKind::Extends => 2,
            EdgeKind::Uses => 3,
            EdgeKind::Remote { .. } => 4,
        }
    }

    pub fn remote(protocol: &str) -> Self {
        EdgeKind::Remote {
            protocol: protocol.to_string(),
        }
    }

    /// Protocol of a `Remote` edge.
    pub fn protocol(&self) -> Option<&str> {
        match self {
            EdgeKind::Remote { protocol } => Some(protocol),
            _ => None,
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, EdgeKind::Remote { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub root: String,
    pub language: Language,
    pub symbols: Vec<Symbol>,
    pub edges: Vec<Edge>,
}

/// A named counter reported by a language parser (e.g. Pharo's resolved
/// message sends). Consecutive stats of one `group` render together as
/// `group: value name, value name` (see `Metrics::parse_stats_lines`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseStat {
    pub group: &'static str,
    pub name: &'static str,
    pub value: usize,
}

impl Graph {
    pub fn symbol(&self, id: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|s| s.id == id)
    }

    /// Follow `container` to the outermost symbol.
    pub fn top_level_of(&self, id: &str) -> Option<&Symbol> {
        let mut cur = self.symbol(id)?;
        // Bounded by symbol count to survive cyclic containers.
        for _ in 0..=self.symbols.len() {
            match cur.container.as_deref().and_then(|c| self.symbol(c)) {
                Some(parent) => cur = parent,
                None => return Some(cur),
            }
        }
        None
    }

    pub fn symbols_in_file<'a>(&'a self, file: &'a str) -> impl Iterator<Item = &'a Symbol> + 'a {
        self.symbols.iter().filter(move |s| s.file == file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            name: id.into(),
            container: container.map(Into::into),
            file: "A.java".into(),
            range: r,
            name_range: r,
            annotations: vec![],
            supertypes: vec![],
            package: None,
            ports: vec![],
        }
    }

    #[test]
    fn top_level_of_follows_containers() {
        let g = Graph {
            root: ".".into(),
            language: Language::Java,
            symbols: vec![
                sym("A", SymbolKind::Class, None),
                sym("A.B", SymbolKind::Class, Some("A")),
                sym("A.B#m", SymbolKind::Method, Some("A.B")),
            ],
            edges: vec![],
        };
        assert_eq!(g.top_level_of("A.B#m").map(|s| s.id.as_str()), Some("A"));
        assert_eq!(g.top_level_of("A").map(|s| s.id.as_str()), Some("A"));
        assert!(g.top_level_of("nope").is_none());
        assert_eq!(g.symbols_in_file("A.java").count(), 3);
    }

    #[test]
    fn ports_filter_by_protocol_and_role() {
        let ep = |path: &str| {
            PortKey::Endpoint(HttpEndpoint {
                verb: HttpVerb::Get,
                path: path.into(),
            })
        };
        let mut s = sym("A#m(0)", SymbolKind::Method, Some("A"));
        s.ports = vec![
            Port::new(protocol::HTTP, PortRole::Provides, ep("/a")),
            Port::new(protocol::HTTP, PortRole::Requires, ep("/b")),
            Port::new("other", PortRole::Provides, ep("/c")),
            Port::new(protocol::HTTP, PortRole::Provides, ep("/d")),
        ];
        let paths: Vec<&str> = s
            .ports(protocol::HTTP, PortRole::Provides)
            .filter_map(PortKey::as_endpoint)
            .map(|e| e.path.as_str())
            .collect();
        assert_eq!(paths, ["/a", "/d"]);
        assert!(ep("/a").as_service_method().is_none());
    }

    #[test]
    fn remote_edges_carry_their_protocol() {
        let k = EdgeKind::remote(protocol::GWT_RPC);
        assert!(k.is_remote() && !EdgeKind::Call.is_remote());
        assert_eq!(k.protocol(), Some("gwt-rpc"));
        assert_eq!(EdgeKind::Uses.protocol(), None);
        assert_eq!(k.rank(), 4);
        assert!(EdgeKind::Uses < k && EdgeKind::Call < EdgeKind::Implements);
        assert_eq!(
            serde_json::to_string(&k).unwrap(),
            r#"{"remote":{"protocol":"gwt-rpc"}}"#
        );
        assert_eq!(serde_json::to_string(&EdgeKind::Call).unwrap(), r#""call""#);
    }
}
