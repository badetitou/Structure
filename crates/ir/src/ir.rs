//! Language-neutral intermediate representation: symbols and edges.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Java,
    #[serde(rename = "typescript")]
    TypeScript,
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
    /// HTTP endpoints served by this symbol (Java methods: Spring, JAX-RS).
    /// `path` is `?` when it is not a literal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub http: Vec<HttpEndpoint>,
    /// HTTP requests made by this symbol. `path` is `?` when it is not a literal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub http_calls: Vec<HttpEndpoint>,
    /// GWT RPC calls made by this symbol (Java methods).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rpc_calls: Vec<RpcCall>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Call,
    Implements,
    Extends,
    Uses,
    /// Frontend HTTP call to the backend handler method it reaches.
    Http,
    /// Client call to the server implementation of a GWT RPC service method.
    Rpc,
}

impl EdgeKind {
    /// Stable order used wherever edges are sorted or keyed.
    pub fn rank(self) -> u8 {
        match self {
            EdgeKind::Call => 0,
            EdgeKind::Implements => 1,
            EdgeKind::Extends => 2,
            EdgeKind::Uses => 3,
            EdgeKind::Http => 4,
            EdgeKind::Rpc => 5,
        }
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
            http: vec![],
            http_calls: vec![],
            rpc_calls: vec![],
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
}
