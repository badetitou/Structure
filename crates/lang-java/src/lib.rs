//! Java tree-sitter frontend: symbols, ranges, annotations, supertype edges.
//!
//! Ranges are 0-based and end-exclusive. Columns are tree-sitter byte offsets
//! within the line (identical to UTF-16/char columns for ASCII sources).
//!
//! Ids: type = fully qualified name (nested: `Outer$Inner`), method =
//! `<type>#<name>(<arity>)`, constructor = `<type>#<init>(<arity>)`,
//! field = `<type>#<name>`. Call edges are not resolved here (SCIP does that).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use rayon::prelude::*;
use tree_sitter::{Node, Parser};

mod rpc;

pub use rpc::{RawRpcCall, Receiver};

use structure_ir::ir::{
    Edge, EdgeKind, Graph, HttpEndpoint, HttpVerb, Language, Range, RpcCall, Symbol, SymbolKind,
};

/// Result of parsing one Java source file.
#[derive(Debug, Clone, Default)]
pub struct FileSymbols {
    pub symbols: Vec<Symbol>,
    /// Non-static imports as written: `a.b.C` or `a.b.*`.
    pub imports: Vec<String>,
    pub package: Option<String>,
    /// (type symbol id, simple type name) for field types and constructor
    /// parameter types, generics/arrays stripped, primitives skipped.
    pub uses: Vec<(String, String)>,
    /// Method id -> its endpoints with no class-level prefix (inheritance input).
    pub routes: HashMap<String, Vec<HttpEndpoint>>,
    /// Type id -> its own class-level prefixes, only when it declares a mapping.
    pub type_prefixes: HashMap<String, Vec<String>>,
    /// Calls that may be GWT RPC calls, resolved once the project is parsed.
    pub rpc_raw: Vec<RawRpcCall>,
}

/// Names never reported as `Uses` targets (java.lang basics and common collections).
const IGNORED_TYPES: &[&str] = &[
    "String",
    "Integer",
    "Long",
    "Short",
    "Byte",
    "Double",
    "Float",
    "Boolean",
    "Character",
    "Number",
    "Object",
    "Void",
    "Class",
    "Enum",
    "Exception",
    "RuntimeException",
    "Throwable",
    "Iterable",
    "CharSequence",
    "List",
    "Map",
    "Set",
    "Collection",
    "Optional",
    "ArrayList",
    "HashMap",
    "HashSet",
    "LinkedList",
    "Date",
    "BigDecimal",
    "BigInteger",
];

fn range_of(n: Node) -> Range {
    let (s, e) = (n.start_position(), n.end_position());
    Range {
        start_line: s.row as u32,
        start_col: s.column as u32,
        end_line: e.row as u32,
        end_col: e.column as u32,
    }
}

fn text<'a>(n: Node, src: &'a str) -> &'a str {
    src.get(n.start_byte()..n.end_byte()).unwrap_or("")
}

/// Simple name of a type node, without generics, qualifiers or array suffix.
/// None for primitives and other non-class types.
fn type_name(n: Node, src: &str) -> Option<String> {
    match n.kind() {
        "type_identifier" => Some(text(n, src).to_string()),
        "generic_type" => n.named_child(0).and_then(|c| type_name(c, src)),
        "scoped_type_identifier" => {
            let mut cur = n.walk();
            let last = n
                .named_children(&mut cur)
                .filter(|c| c.kind() == "type_identifier")
                .last();
            last.map(|c| text(c, src).to_string())
        }
        "array_type" => n
            .child_by_field_name("element")
            .and_then(|c| type_name(c, src)),
        _ => None,
    }
}

fn annotations_of(n: Node, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = n.walk();
    for m in n.children(&mut cur).filter(|c| c.kind() == "modifiers") {
        let mut c2 = m.walk();
        for a in m.children(&mut c2) {
            if matches!(a.kind(), "annotation" | "marker_annotation")
                && let Some(name) = a.child_by_field_name("name")
            {
                let t = text(name, src);
                out.push(t.rsplit('.').next().unwrap_or(t).to_string());
            }
        }
    }
    out
}

fn annotation_nodes<'t>(n: Node<'t>, src: &str) -> Vec<(String, Node<'t>)> {
    let mut out = Vec::new();
    let mut cur = n.walk();
    for m in n.children(&mut cur).filter(|c| c.kind() == "modifiers") {
        let mut c2 = m.walk();
        for a in m.children(&mut c2) {
            if matches!(a.kind(), "annotation" | "marker_annotation")
                && let Some(name) = a.child_by_field_name("name")
            {
                let t = text(name, src);
                out.push((t.rsplit('.').next().unwrap_or(t).to_string(), a));
            }
        }
    }
    out
}

/// Value expressions of annotation argument `keys` (or the positional one if
/// `positional`); an array initializer is flattened.
fn annotation_values<'t>(a: Node<'t>, src: &str, keys: &[&str], positional: bool) -> Vec<Node<'t>> {
    let Some(args) = a.child_by_field_name("arguments") else {
        return Vec::new();
    };
    let mut found = None;
    let mut cur = args.walk();
    for c in args.named_children(&mut cur) {
        if c.kind() == "element_value_pair" {
            let k = c.child_by_field_name("key").map_or("", |k| text(k, src));
            if keys.contains(&k) {
                found = c.child_by_field_name("value");
            }
        } else if positional && found.is_none() {
            found = Some(c);
        }
    }
    let Some(v) = found else {
        return Vec::new();
    };
    if matches!(
        v.kind(),
        "element_value_array_initializer" | "array_initializer"
    ) {
        let mut c = v.walk();
        v.named_children(&mut c).collect()
    } else {
        vec![v]
    }
}

/// Literal string value, or `?` for anything that is not a plain literal.
fn literal_or_unknown(v: Node, src: &str) -> String {
    let t = text(v, src);
    if v.kind() == "string_literal" && !t.starts_with("\"\"\"") && t.len() >= 2 {
        t[1..t.len() - 1].to_string()
    } else {
        "?".to_string()
    }
}

/// Paths of a mapping annotation; one empty path when none is given.
fn annotation_paths(a: Node, src: &str, positional: bool) -> Vec<String> {
    let paths: Vec<String> = annotation_values(a, src, &["value", "path"], positional)
        .into_iter()
        .map(|v| literal_or_unknown(v, src))
        .collect();
    if paths.is_empty() {
        vec![String::new()]
    } else {
        paths
    }
}

fn join_paths(prefix: &str, path: &str) -> String {
    if prefix == "?" || path == "?" {
        return "?".to_string();
    }
    let parts: Vec<&str> = prefix
        .split('/')
        .chain(path.split('/'))
        .filter(|p| !p.is_empty())
        .collect();
    format!("/{}", parts.join("/"))
}

/// Own class-level prefixes, or `None` when the class declares no mapping.
fn class_prefixes(class: Node, src: &str) -> Option<Vec<String>> {
    let anns = annotation_nodes(class, src);
    // HTTP clients (Feign, `@HttpExchange`) call endpoints, they do not serve
    // them: no prefix means `method_endpoints` emits nothing.
    if anns
        .iter()
        .any(|(n, _)| matches!(n.as_str(), "FeignClient" | "HttpExchange"))
    {
        return Some(Vec::new());
    }
    for (name, a) in anns {
        if matches!(name.as_str(), "RequestMapping" | "Path") {
            return Some(annotation_paths(a, src, true));
        }
    }
    None
}

fn verb_of(name: &str) -> Option<HttpVerb> {
    Some(match name {
        "GET" | "Get" => HttpVerb::Get,
        "POST" | "Post" => HttpVerb::Post,
        "PUT" | "Put" => HttpVerb::Put,
        "DELETE" | "Delete" => HttpVerb::Delete,
        "PATCH" | "Patch" => HttpVerb::Patch,
        _ => return None,
    })
}

/// Endpoints served by a method: Spring mappings or JAX-RS `@Path` + verb.
fn method_endpoints(m: Node, src: &str, prefixes: &[String]) -> Vec<HttpEndpoint> {
    let anns = annotation_nodes(m, src);
    let mut routes: Vec<(HttpVerb, Vec<String>)> = Vec::new();
    let jaxrs_path = anns.iter().find(|(n, _)| n == "Path").map(|(_, a)| *a);
    for (name, a) in &anns {
        if let Some(v) = name.strip_suffix("Mapping").and_then(verb_of) {
            routes.push((v, annotation_paths(*a, src, true)));
        } else if name == "RequestMapping" {
            let methods = annotation_values(*a, src, &["method"], false);
            // Absent `method` means any verb; values that are not a known
            // verb (constant, HEAD, OPTIONS) yield no endpoint, not a guess.
            let verbs: Vec<HttpVerb> = if methods.is_empty() {
                vec![HttpVerb::Any]
            } else {
                methods
                    .into_iter()
                    .filter_map(|v| {
                        let t = text(v, src);
                        verb_of(t.rsplit('.').next().unwrap_or(t))
                    })
                    .collect()
            };
            let paths = annotation_paths(*a, src, true);
            for v in verbs {
                routes.push((v, paths.clone()));
            }
        } else if name.chars().all(|c| c.is_ascii_uppercase())
            && let Some(v) = verb_of(name)
        {
            let paths =
                jaxrs_path.map_or_else(|| vec![String::new()], |p| annotation_paths(p, src, true));
            routes.push((v, paths));
        }
    }
    let mut out: Vec<HttpEndpoint> = Vec::new();
    for (verb, paths) in routes {
        for pre in prefixes {
            for p in &paths {
                let e = HttpEndpoint {
                    verb,
                    path: join_paths(pre, p),
                };
                if !out.contains(&e) {
                    out.push(e);
                }
            }
        }
    }
    out
}

fn collect_types(n: Node, src: &str, out: &mut Vec<String>) {
    if let Some(name) = type_name(n, src) {
        out.push(name);
        return;
    }
    let mut cur = n.walk();
    for c in n.named_children(&mut cur) {
        collect_types(c, src, out);
    }
}

fn arity(params: Option<Node>) -> usize {
    params.map_or(0, |p| {
        let mut cur = p.walk();
        p.named_children(&mut cur)
            .filter(|c| matches!(c.kind(), "formal_parameter" | "spread_parameter"))
            .count()
    })
}

/// Names of the type parameters declared on a class/method/constructor node.
fn type_params_of(n: Node, src: &str) -> Vec<String> {
    let Some(tp) = n.child_by_field_name("type_parameters") else {
        return Vec::new();
    };
    let mut cur = tp.walk();
    tp.named_children(&mut cur)
        .filter(|c| c.kind() == "type_parameter")
        .filter_map(|c| c.named_child(0))
        .map(|c| text(c, src).to_string())
        .collect()
}

struct Ctx<'a> {
    file: &'a str,
    src: &'a str,
    package: Option<String>,
    out: FileSymbols,
    /// Class-level path prefixes (Spring `@RequestMapping`, JAX-RS `@Path`).
    prefixes: Vec<String>,
    /// Field receivers of the enclosing types, outermost first.
    fields: Vec<rpc::Env>,
    /// Inside a top-level interface (GWT service/async twins): no RPC calls.
    in_interface: bool,
}

impl Ctx<'_> {
    fn symbol(
        &mut self,
        id: String,
        kind: SymbolKind,
        node: Node,
        name_node: Node,
        container: Option<&str>,
    ) -> usize {
        self.out.symbols.push(Symbol {
            id,
            kind,
            name: text(name_node, self.src).to_string(),
            container: container.map(Into::into),
            file: self.file.to_string(),
            range: range_of(node),
            name_range: range_of(name_node),
            annotations: annotations_of(node, self.src),
            supertypes: vec![],
            package: self.package.clone(),
            http: vec![],
            http_calls: vec![],
            rpc_calls: vec![],
        });
        self.out.symbols.len() - 1
    }

    fn rpc(&mut self, node: Node, caller: &str) {
        if !self.in_interface {
            rpc::collect(node, self.src, caller, &self.fields, &mut self.out.rpc_raw);
        }
    }

    fn type_decl(&mut self, node: Node, container: Option<&str>, outer_tp: &[String]) {
        let kind = match node.kind() {
            "class_declaration" | "record_declaration" => SymbolKind::Class,
            "interface_declaration" | "annotation_type_declaration" => SymbolKind::Interface,
            "enum_declaration" => SymbolKind::Enum,
            _ => return,
        };
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = text(name_node, self.src);
        let id = match (container, &self.package) {
            (Some(c), _) => format!("{c}${name}"),
            (None, Some(p)) => format!("{p}.{name}"),
            (None, None) => name.to_string(),
        };
        let idx = self.symbol(id.clone(), kind, node, name_node, container);
        let own_prefixes = class_prefixes(node, self.src);
        // An empty list marks an HTTP client (Feign...): it serves nothing and
        // must not feed inheritance.
        if let Some(p) = own_prefixes.as_ref().filter(|p| !p.is_empty()) {
            self.out.type_prefixes.insert(id.clone(), p.clone());
        }
        let saved_prefixes = std::mem::replace(
            &mut self.prefixes,
            own_prefixes.unwrap_or_else(|| vec![String::new()]),
        );

        let mut supers = Vec::new();
        let mut cur = node.walk();
        for c in node.children(&mut cur) {
            if matches!(
                c.kind(),
                "superclass" | "super_interfaces" | "extends_interfaces"
            ) {
                collect_types(c, self.src, &mut supers);
            }
        }
        self.out.symbols[idx].supertypes = supers;

        let mut tp = outer_tp.to_vec();
        tp.extend(type_params_of(node, self.src));

        let saved_iface = self.in_interface;
        if container.is_none() {
            self.in_interface = kind == SymbolKind::Interface;
        }
        let mut rec_arity = 0;
        if node.kind() == "record_declaration"
            && let Some(params) = node.child_by_field_name("parameters")
        {
            let mut c2 = params.walk();
            for p in params.named_children(&mut c2) {
                if !matches!(p.kind(), "formal_parameter" | "spread_parameter") {
                    continue;
                }
                rec_arity += 1;
                let name_node = p.child_by_field_name("name").or_else(|| {
                    let mut c3 = p.walk();
                    p.named_children(&mut c3)
                        .find(|c| c.kind() == "variable_declarator")
                        .and_then(|d| d.child_by_field_name("name"))
                });
                if let Some(n) = name_node {
                    let fid = format!("{id}#{}", text(n, self.src));
                    self.symbol(fid, SymbolKind::Field, p, n, Some(&id));
                }
                self.param_use(p, &id, &tp);
            }
        }

        if let Some(body) = node.child_by_field_name("body") {
            self.fields.push(rpc::field_env(body, self.src));
            self.members(body, &id, &tp, rec_arity);
            self.fields.pop();
        }
        self.in_interface = saved_iface;
        self.prefixes = saved_prefixes;
    }

    /// Record a `Uses` entry for the type of a (formal or spread) parameter.
    fn param_use(&mut self, p: Node, tid: &str, tp: &[String]) {
        let ty = if p.kind() == "spread_parameter" {
            let mut cur = p.walk();
            p.named_children(&mut cur)
                .find_map(|c| type_name(c, self.src))
        } else {
            p.child_by_field_name("type")
                .and_then(|t| type_name(t, self.src))
        };
        if let Some(t) = ty
            && !tp.contains(&t)
        {
            self.out.uses.push((tid.to_string(), t));
        }
    }

    fn members(&mut self, body: Node, tid: &str, tp: &[String], rec_arity: usize) {
        let mut cur = body.walk();
        for m in body.named_children(&mut cur) {
            match m.kind() {
                "enum_body_declarations" => self.members(m, tid, tp, rec_arity),
                "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
                | "annotation_type_declaration" => self.type_decl(m, Some(tid), tp),
                "method_declaration" => {
                    if let Some(n) = m.child_by_field_name("name") {
                        let id = format!(
                            "{tid}#{}({})",
                            text(n, self.src),
                            arity(m.child_by_field_name("parameters"))
                        );
                        self.rpc(m, &id);
                        let idx = self.symbol(id, SymbolKind::Method, m, n, Some(tid));
                        self.out.symbols[idx].http = method_endpoints(m, self.src, &self.prefixes);
                        let raw = method_endpoints(m, self.src, &[String::new()]);
                        if !raw.is_empty() && !self.prefixes.is_empty() {
                            let mid = self.out.symbols[idx].id.clone();
                            self.out.routes.insert(mid, raw);
                        }
                    }
                }
                "annotation_type_element_declaration" => {
                    if let Some(n) = m.child_by_field_name("name") {
                        let id = format!("{tid}#{}(0)", text(n, self.src));
                        self.symbol(id, SymbolKind::Method, m, n, Some(tid));
                    }
                }
                "compact_constructor_declaration" => {
                    if let Some(n) = m.child_by_field_name("name") {
                        let id = format!("{tid}#<init>({rec_arity})");
                        self.symbol(id, SymbolKind::Constructor, m, n, Some(tid));
                    }
                }
                "constructor_declaration" => {
                    if let Some(n) = m.child_by_field_name("name") {
                        let params = m.child_by_field_name("parameters");
                        let id = format!("{tid}#<init>({})", arity(params));
                        self.rpc(m, &id);
                        self.symbol(id, SymbolKind::Constructor, m, n, Some(tid));
                        let mut ctp = tp.to_vec();
                        ctp.extend(type_params_of(m, self.src));
                        if let Some(p) = params {
                            let mut c2 = p.walk();
                            for prm in p.named_children(&mut c2) {
                                self.param_use(prm, tid, &ctp);
                            }
                        }
                    }
                }
                "field_declaration" | "constant_declaration" => {
                    let ty = m
                        .child_by_field_name("type")
                        .and_then(|t| type_name(t, self.src))
                        .filter(|t| !tp.contains(t));
                    let mut c2 = m.walk();
                    for d in m.children_by_field_name("declarator", &mut c2) {
                        if let Some(n) = d.child_by_field_name("name") {
                            let id = format!("{tid}#{}", text(n, self.src));
                            self.symbol(id, SymbolKind::Field, m, n, Some(tid));
                            if let Some(t) = &ty {
                                self.out.uses.push((tid.to_string(), t.clone()));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Parse one Java source. `file` is the path stored in each symbol.
pub fn parse_source(file: &str, src: &str) -> Result<FileSymbols> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .context("loading Java grammar")?;
    let tree = parser
        .parse(src, None)
        .ok_or_else(|| anyhow!("tree-sitter failed to parse {file}"))?;
    let root = tree.root_node();

    let mut package = None;
    let mut imports = Vec::new();
    let mut cur = root.walk();
    for n in root.children(&mut cur) {
        match n.kind() {
            "package_declaration" => {
                let mut c = n.walk();
                package = n
                    .named_children(&mut c)
                    .find(|x| matches!(x.kind(), "scoped_identifier" | "identifier"))
                    .map(|x| text(x, src).to_string());
            }
            "import_declaration" => {
                let mut c = n.walk();
                let is_static = n.children(&mut c).any(|x| x.kind() == "static");
                let mut c = n.walk();
                let has_star = n.children(&mut c).any(|x| x.kind() == "asterisk");
                let mut c = n.walk();
                let path = n
                    .named_children(&mut c)
                    .find(|x| matches!(x.kind(), "scoped_identifier" | "identifier"))
                    .map(|x| text(x, src).to_string());
                if let (false, Some(p)) = (is_static, path) {
                    imports.push(if has_star { format!("{p}.*") } else { p });
                }
            }
            _ => {}
        }
    }

    let mut ctx = Ctx {
        file,
        src,
        package: package.clone(),
        out: FileSymbols::default(),
        prefixes: vec![String::new()],
        fields: vec![],
        in_interface: false,
    };
    let mut cur = root.walk();
    for n in root.children(&mut cur) {
        ctx.type_decl(n, None, &[]);
    }
    let mut out = ctx.out;
    out.package = package;
    out.imports = imports;
    Ok(out)
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn read_and_parse(root: &Path, path: &Path) -> Result<FileSymbols> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse_source(&rel_path(root, path), &String::from_utf8_lossy(&bytes))
}

/// Type lookup tables used to resolve simple names to project type ids.
struct TypeIndex {
    /// dotted name (`a.b.Outer.Inner`) -> id
    dotted: HashMap<String, String>,
    /// simple name -> ids
    simple: HashMap<String, Vec<String>>,
    kinds: HashMap<String, SymbolKind>,
}

impl TypeIndex {
    fn new(symbols: &[Symbol]) -> Self {
        let mut idx = TypeIndex {
            dotted: HashMap::new(),
            simple: HashMap::new(),
            kinds: HashMap::new(),
        };
        for s in symbols.iter().filter(|s| {
            matches!(
                s.kind,
                SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum
            )
        }) {
            idx.dotted.insert(s.id.replace('$', "."), s.id.clone());
            idx.simple
                .entry(s.name.clone())
                .or_default()
                .push(s.id.clone());
            idx.kinds.insert(s.id.clone(), s.kind);
        }
        idx
    }

    fn resolve(&self, fs: &FileSymbols, name: &str) -> Option<String> {
        let suffix = format!(".{name}");
        let mut explicit = false;
        for imp in fs.imports.iter().filter(|i| i.ends_with(&suffix)) {
            explicit = true;
            if let Some(id) = self.dotted.get(imp) {
                return Some(id.clone());
            }
        }
        if explicit {
            // Explicitly imported non-project class: external, never guess.
            return None;
        }
        match &fs.package {
            Some(p) => {
                if let Some(id) = self.dotted.get(&format!("{p}{suffix}")) {
                    return Some(id.clone());
                }
            }
            None => {
                if let Some(id) = self.dotted.get(name) {
                    return Some(id.clone());
                }
            }
        }
        for imp in fs.imports.iter().filter_map(|i| i.strip_suffix(".*")) {
            if let Some(id) = self.dotted.get(&format!("{imp}{suffix}")) {
                return Some(id.clone());
            }
        }
        match self.simple.get(name).map(Vec::as_slice) {
            Some([only]) => Some(only.clone()),
            _ => None,
        }
    }
}

/// Parse every main-source `.java` file under `root` into a `Graph`
/// (symbols, `Extends`/`Implements` and `Uses` edges; no calls).
pub fn parse_project(root: &Path) -> Result<Graph> {
    parse_project_with_warnings(root, &mut Vec::new())
}

/// Component pairs marking build-generated sources, walked for endpoints only.
fn in_generated(rel: &Path) -> bool {
    let comps: Vec<_> = rel.components().map(|c| c.as_os_str()).collect();
    comps.windows(2).any(|w| {
        (w[0] == "target" && w[1] == "generated-sources")
            || (w[0] == "build" && w[1] == "generated")
    })
}

/// `.java` files under `target/generated-sources/**` and `build/generated/**`.
fn generated_files(root: &Path) -> Vec<std::path::PathBuf> {
    let walk_root = root.to_path_buf();
    let walker = ignore::WalkBuilder::new(root)
        .standard_filters(false)
        .filter_entry(move |e| {
            if e.depth() == 0 || !e.file_type().is_some_and(|t| t.is_dir()) {
                return true;
            }
            let n = e.file_name().to_string_lossy();
            if matches!(n.as_ref(), "node_modules" | ".git" | ".gradle") {
                return false;
            }
            let rel = e.path().strip_prefix(&walk_root).unwrap_or(e.path());
            // Main sources are walked elsewhere; only generated trees matter here.
            in_generated(rel) || n != "src"
        })
        .build();
    let mut files: Vec<_> = walker
        .flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .map(|e| e.into_path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "java")
                && in_generated(p.strip_prefix(root).unwrap_or(p))
        })
        .collect();
    files.sort();
    files
}

/// Endpoints (no class prefix) of `tid#suffix` found on a supertype, transitively.
fn inherited_routes<'a>(
    supers: &HashMap<String, Vec<String>>,
    routes: &'a HashMap<String, Vec<HttpEndpoint>>,
    tid: &str,
    suffix: &str,
    seen: &mut HashSet<String>,
) -> Option<&'a Vec<HttpEndpoint>> {
    for sup in supers.get(tid)? {
        if !seen.insert(sup.clone()) {
            continue;
        }
        if let Some(r) = routes.get(&format!("{sup}#{suffix}")) {
            return Some(r);
        }
        if let Some(r) = inherited_routes(supers, routes, sup, suffix, seen) {
            return Some(r);
        }
    }
    None
}

/// Prefixes in force for `tid`: its own mapping, else the nearest supertype's.
fn effective_prefixes(
    supers: &HashMap<String, Vec<String>>,
    prefixes: &HashMap<String, Vec<String>>,
    tid: &str,
    seen: &mut HashSet<String>,
) -> Option<Vec<String>> {
    if let Some(p) = prefixes.get(tid) {
        return Some(p.clone());
    }
    for sup in supers.get(tid)? {
        if seen.insert(sup.clone())
            && let Some(p) = effective_prefixes(supers, prefixes, sup, seen)
        {
            return Some(p);
        }
    }
    None
}

/// Like [`parse_project`], also reporting soft problems (unresolved `*Api` supertypes).
pub fn parse_project_with_warnings(root: &Path, warnings: &mut Vec<String>) -> Result<Graph> {
    let walk_root = root.to_path_buf();
    let walker = ignore::WalkBuilder::new(root)
        .require_git(false)
        .filter_entry(move |e| {
            if e.depth() == 0 {
                return true;
            }
            let n = e.file_name().to_string_lossy();
            let is_dir = e.file_type().is_some_and(|t| t.is_dir());
            // Explicit: keeps Gradle's working dir out even if `.hidden(false)` is set later.
            if is_dir && n == ".gradle" {
                return false;
            }
            if !(is_dir && matches!(n.as_ref(), "target" | "build" | "node_modules")) {
                return true;
            }
            // Keep such dirs when they sit under a `src/` component (packages).
            let rel = e.path().strip_prefix(&walk_root).unwrap_or(e.path());
            rel.components().any(|c| c.as_os_str() == "src")
        })
        .build();
    let mut files = Vec::new();
    for entry in walker {
        let entry = entry?;
        let p = entry.path();
        let is_file = entry.file_type().is_some_and(|t| t.is_file());
        if is_file
            && p.extension().is_some_and(|e| e == "java")
            && !rel_path(root, p).contains("src/test/")
        {
            files.push(p.to_path_buf());
        }
    }
    files.sort();

    let parsed: Vec<FileSymbols> = files
        .par_iter()
        .map(|p| read_and_parse(root, p))
        .collect::<Result<_>>()?;

    // Generated sources (OpenAPI server interfaces...) feed endpoint
    // inheritance only: their symbols never reach the graph.
    let generated: Vec<FileSymbols> = generated_files(root)
        .par_iter()
        .filter_map(|p| read_and_parse(root, p).ok())
        .collect();
    assemble(&root.to_string_lossy(), parsed, generated, warnings)
}

/// Whether the directory walk would reach `rel`: no hidden component, and no
/// `target` / `build` / `node_modules` directory unless under a `src/` one.
fn walk_reaches(rel: &str) -> bool {
    let comps: Vec<&str> = rel.split('/').collect();
    let dirs = &comps[..comps.len().saturating_sub(1)];
    !comps.iter().any(|c| c.starts_with('.'))
        && !dirs.iter().enumerate().any(|(i, d)| {
            matches!(*d, "target" | "build" | "node_modules") && !dirs[..i].contains(&"src")
        })
}

/// Build the graph from in-memory `(relative path, source text)` pairs, with no
/// file system access. Applies the walk's filters (`.java`, not under
/// `src/test/`, no hidden or build directory), `.gitignore` aside. No generated
/// sources are taken: endpoints are not inherited from them.
pub fn parse_sources(
    root_label: &str,
    sources: &[(String, String)],
    warnings: &mut Vec<String>,
) -> Result<Graph> {
    let mut kept: Vec<&(String, String)> = sources
        .iter()
        .filter(|(rel, _)| {
            rel.ends_with(".java") && !rel.contains("src/test/") && walk_reaches(rel)
        })
        .collect();
    // Same order as the walk's sorted `PathBuf`s: component by component.
    kept.sort_by(|a, b| a.0.split('/').cmp(b.0.split('/')));
    let parsed: Vec<FileSymbols> = kept
        .par_iter()
        .map(|(rel, src)| parse_source(rel, src))
        .collect::<Result<_>>()?;
    assemble(root_label, parsed, Vec::new(), warnings)
}

/// Shared assembly: type index, edges, RPC and endpoint inheritance, sorting.
fn assemble(
    root_label: &str,
    parsed: Vec<FileSymbols>,
    generated: Vec<FileSymbols>,
    warnings: &mut Vec<String>,
) -> Result<Graph> {
    let all: Vec<Symbol> = parsed.iter().flat_map(|f| f.symbols.clone()).collect();
    let index = TypeIndex::new(&all);

    let gen_syms: Vec<Symbol> = generated.iter().flat_map(|f| f.symbols.clone()).collect();
    let gen_index = TypeIndex::new(&gen_syms);

    let mut edges = Vec::new();
    let mut seen: HashSet<(String, String, u8)> = HashSet::new();
    let mut push = |from: &str, to: String, kind: EdgeKind| {
        if seen.insert((from.to_string(), to.clone(), kind as u8)) {
            edges.push(Edge {
                from: from.to_string(),
                to,
                kind,
            });
        }
    };
    for fs in &parsed {
        for s in fs
            .symbols
            .iter()
            .filter(|s| index.kinds.contains_key(&s.id))
        {
            for sup in &s.supertypes {
                if let Some(target) = index.resolve(fs, sup) {
                    let kind = if s.kind != SymbolKind::Interface
                        && index.kinds.get(&target) == Some(&SymbolKind::Interface)
                    {
                        EdgeKind::Implements
                    } else {
                        EdgeKind::Extends
                    };
                    push(&s.id, target, kind);
                }
            }
        }
        for (from, name) in &fs.uses {
            match index.resolve(fs, name) {
                Some(t) if &t != from => push(from, t, EdgeKind::Uses),
                Some(_) => {}
                None if IGNORED_TYPES.contains(&name.as_str()) => {}
                None => push(from, format!("ext:{name}"), EdgeKind::Uses),
            }
        }
    }

    let mut all = all;
    attach_rpc(&mut all, &parsed, &index);
    inherit_endpoints(&mut all, &parsed, &generated, &index, &gen_index, warnings);
    all.sort_by(|a, b| a.id.cmp(&b.id));
    edges.sort_by(|a, b| (&a.from, &a.to, a.kind as u8).cmp(&(&b.from, &b.to, b.kind as u8)));

    Ok(Graph {
        root: root_label.to_string(),
        language: Language::Java,
        symbols: all,
        edges,
    })
}

/// Interfaces that extend `RemoteService`, directly or through parsed interfaces.
fn remote_services(parsed: &[FileSymbols], index: &TypeIndex) -> HashSet<String> {
    let mut remote: HashSet<String> = HashSet::new();
    let mut supers: Vec<(&str, Vec<String>)> = Vec::new();
    for fs in parsed {
        for s in fs
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Interface && index.kinds.contains_key(&s.id))
        {
            if s.supertypes.iter().any(|t| t == "RemoteService") {
                remote.insert(s.id.clone());
            } else {
                let resolved = s
                    .supertypes
                    .iter()
                    .filter_map(|t| index.resolve(fs, t))
                    .collect();
                supers.push((&s.id, resolved));
            }
        }
    }
    loop {
        let before = remote.len();
        for (id, sups) in &supers {
            if !remote.contains(*id) && sups.iter().any(|s| remote.contains(s)) {
                remote.insert((*id).to_string());
            }
        }
        if remote.len() == before {
            return remote;
        }
    }
}

/// Turn the raw RPC-shaped calls of each file into `Symbol::rpc_calls`.
fn attach_rpc(all: &mut [Symbol], parsed: &[FileSymbols], index: &TypeIndex) {
    if parsed.iter().all(|f| f.rpc_raw.is_empty()) {
        return;
    }
    let remote = remote_services(parsed, index);
    let by_id: HashMap<String, usize> = all
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.clone(), i))
        .collect();
    for fs in parsed {
        for raw in &fs.rpc_raw {
            // (service, maybe)
            let service = match &raw.recv {
                Receiver::Async(n) => match index.resolve(fs, n) {
                    Some(id) => id
                        .strip_suffix("Async")
                        .filter(|sync| remote.contains(*sync))
                        .map(|sync| (sync.to_string(), false)),
                    // `XAsync` from another root: keep `X`, the merged link decides.
                    None => n.strip_suffix("Async").map(|x| (x.to_string(), false)),
                },
                Receiver::Sync { name, strict } => match index.resolve(fs, name) {
                    Some(id) => remote.contains(&id).then_some((id, false)),
                    // Unknown `X`: plausible only if an `XAsync` type is parsed.
                    None => (!strict).then(|| {
                        let twin = index.simple.contains_key(&format!("{name}Async"));
                        (name.clone(), !twin)
                    }),
                },
            };
            if let (Some((service, maybe)), Some(&i)) = (service, by_id.get(&raw.caller)) {
                all[i].rpc_calls.push(RpcCall {
                    service,
                    method: raw.method.clone(),
                    arity: raw.arity,
                    maybe,
                });
            }
        }
    }
}

/// Give each method with no mapping of its own the endpoints of the supertype
/// method it overrides (same name and arity), as Spring does. Spring takes the
/// type-level mapping from the nearest type that has one, so the implementing
/// class's own `@RequestMapping` replaces the interface's rather than adding to it.
fn inherit_endpoints(
    all: &mut [Symbol],
    parsed: &[FileSymbols],
    generated: &[FileSymbols],
    index: &TypeIndex,
    gen_index: &TypeIndex,
    warnings: &mut Vec<String>,
) {
    let mut supers: HashMap<String, Vec<String>> = HashMap::new();
    let mut routes: HashMap<String, Vec<HttpEndpoint>> = HashMap::new();
    let mut prefixes: HashMap<String, Vec<String>> = HashMap::new();
    let mut unresolved_api = 0;
    // Parsed code first: it wins over a generated twin with the same id.
    for (is_parsed, fs) in parsed
        .iter()
        .map(|f| (true, f))
        .chain(generated.iter().map(|f| (false, f)))
    {
        for s in fs.symbols.iter().filter(|s| {
            matches!(s.kind, SymbolKind::Class | SymbolKind::Interface) && !s.supertypes.is_empty()
        }) {
            let mut resolved = Vec::new();
            let mut missing_api = false;
            for sup in &s.supertypes {
                match index
                    .resolve(fs, sup)
                    .or_else(|| gen_index.resolve(fs, sup))
                {
                    Some(t) => resolved.push(t),
                    None => missing_api |= sup.ends_with("Api"),
                }
            }
            if is_parsed && s.kind == SymbolKind::Class && missing_api {
                unresolved_api += 1;
            }
            supers.entry(s.id.clone()).or_insert(resolved);
        }
        for (k, v) in &fs.routes {
            routes.entry(k.clone()).or_insert_with(|| v.clone());
        }
        for (k, v) in &fs.type_prefixes {
            prefixes.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    for s in all.iter_mut().filter(|s| s.kind == SymbolKind::Method) {
        let (Some(tid), Some((_, suffix))) = (s.container.as_deref(), s.id.split_once('#')) else {
            continue;
        };
        // Own mapping, else the overridden one; either way the prefix is the
        // nearest type-level mapping (own class, then supertypes).
        let raw = match routes.get(&s.id) {
            Some(r) => r,
            None if s.http.is_empty() => {
                match inherited_routes(&supers, &routes, tid, suffix, &mut HashSet::new()) {
                    Some(r) => r,
                    None => continue,
                }
            }
            None => continue,
        };
        let pres = effective_prefixes(&supers, &prefixes, tid, &mut HashSet::new())
            .unwrap_or_else(|| vec![String::new()]);
        let mut out: Vec<HttpEndpoint> = Vec::new();
        for e in raw {
            for pre in &pres {
                let n = HttpEndpoint {
                    verb: e.verb,
                    path: join_paths(pre, &e.path),
                };
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        }
        s.http = out;
    }
    if unresolved_api > 0 {
        warnings.push(format!(
            "{unresolved_api} class(es) implement an unresolved `*Api` interface; \
             their HTTP endpoints are unknown (generated sources missing? run the build)"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradle_dir_is_not_parsed() {
        let dir =
            std::env::temp_dir().join(format!("structure-gradle-parse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src/main/java")).unwrap();
        std::fs::create_dir_all(dir.join(".gradle/src/main/java")).unwrap();
        std::fs::write(dir.join("src/main/java/A.java"), "class A {}").unwrap();
        std::fs::write(dir.join(".gradle/src/main/java/B.java"), "class B {}").unwrap();
        std::fs::create_dir_all(dir.join("sub/.gradle")).unwrap();
        std::fs::write(dir.join("sub/.gradle/B.java"), "class B {}").unwrap();
        let g = parse_project(&dir).unwrap();
        assert!(g.symbols.iter().any(|s| s.name == "A"));
        assert!(!g.symbols.iter().any(|s| s.name == "B"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    const SRC: &str = r#"package com.acme.shop;

import org.springframework.stereotype.Service;
import com.acme.base.*;

@Service
@org.foo.Transactional
public class OrderService extends BaseService implements Runnable, Repo<Foo, Long> {
    private final JdbcTemplate jdbc;
    private String name, other;
    private List<Item> items;

    public OrderService(JdbcTemplate jdbc, int n) { this.jdbc = jdbc; }

    public void run() {}
    int add(int a, String b) { return 0; }

    static class Inner { void go() {} }
}

interface Repo<T, ID> extends JpaRepository<T, ID> { T find(ID id); }
enum Color { RED, GREEN; Color() {} void paint() {} }
record Point(int x, int y) implements Shape { double len() { return 0; } }
@interface Marker {}
"#;

    fn get<'a>(fs: &'a FileSymbols, id: &str) -> &'a Symbol {
        fs.symbols
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("missing {id}"))
    }

    #[test]
    fn parses_symbols_ids_and_ranges() {
        let fs = parse_source("com/acme/shop/OrderService.java", SRC).unwrap();
        assert_eq!(fs.package.as_deref(), Some("com.acme.shop"));
        assert_eq!(
            fs.imports,
            ["org.springframework.stereotype.Service", "com.acme.base.*"]
        );

        let c = get(&fs, "com.acme.shop.OrderService");
        assert_eq!(c.kind, SymbolKind::Class);
        assert_eq!(c.annotations, ["Service", "Transactional"]);
        assert_eq!(c.supertypes, ["BaseService", "Runnable", "Repo"]);
        assert_eq!(c.range.start_line, 5);
        assert_eq!(c.range.end_line, 18);
        assert_eq!(
            c.name_range,
            Range {
                start_line: 7,
                start_col: 13,
                end_line: 7,
                end_col: 25
            }
        );
        assert_eq!(c.package.as_deref(), Some("com.acme.shop"));

        let m = get(&fs, "com.acme.shop.OrderService#add(2)");
        assert_eq!(m.kind, SymbolKind::Method);
        assert_eq!(m.container.as_deref(), Some("com.acme.shop.OrderService"));
        assert_eq!(m.name_range.start_line, 15);

        let k = get(&fs, "com.acme.shop.OrderService#<init>(2)");
        assert_eq!(k.kind, SymbolKind::Constructor);
        assert_eq!(
            get(&fs, "com.acme.shop.OrderService#name").kind,
            SymbolKind::Field
        );
        get(&fs, "com.acme.shop.OrderService#other");
        get(&fs, "com.acme.shop.OrderService#run(0)");

        let inner = get(&fs, "com.acme.shop.OrderService$Inner");
        assert_eq!(
            inner.container.as_deref(),
            Some("com.acme.shop.OrderService")
        );
        get(&fs, "com.acme.shop.OrderService$Inner#go(0)");
    }

    #[test]
    fn interface_enum_record_annotation() {
        let fs = parse_source("A.java", SRC).unwrap();
        let r = get(&fs, "com.acme.shop.Repo");
        assert_eq!(r.kind, SymbolKind::Interface);
        assert_eq!(r.supertypes, ["JpaRepository"]);
        get(&fs, "com.acme.shop.Repo#find(1)");
        assert_eq!(get(&fs, "com.acme.shop.Color").kind, SymbolKind::Enum);
        get(&fs, "com.acme.shop.Color#<init>(0)");
        get(&fs, "com.acme.shop.Color#paint(0)");
        let p = get(&fs, "com.acme.shop.Point");
        assert_eq!(p.kind, SymbolKind::Class);
        assert_eq!(p.supertypes, ["Shape"]);
        get(&fs, "com.acme.shop.Point#len(0)");
        assert_eq!(get(&fs, "com.acme.shop.Marker").kind, SymbolKind::Interface);
    }

    #[test]
    fn collects_uses() {
        let fs = parse_source("A.java", SRC).unwrap();
        let u: Vec<&str> = fs
            .uses
            .iter()
            .filter(|(f, _)| f == "com.acme.shop.OrderService")
            .map(|(_, t)| t.as_str())
            .collect();
        assert!(u.contains(&"JdbcTemplate"));
        assert!(!u.contains(&"int"));
    }

    #[test]
    fn default_package() {
        let fs = parse_source("A.java", "class A { void m() {} }").unwrap();
        assert_eq!(fs.symbols[0].id, "A");
        assert_eq!(fs.symbols[1].id, "A#m(0)");
        assert!(fs.package.is_none());
    }

    #[test]
    fn project_edges() {
        let dir = std::env::temp_dir().join(format!("structure_java_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("src/main/java/p");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(dir.join("src/test/java/p")).unwrap();
        std::fs::write(src.join("Base.java"), "package p; public class Base {}").unwrap();
        std::fs::write(src.join("Api.java"), "package p; public interface Api {}").unwrap();
        std::fs::write(
            src.join("Impl.java"),
            "package p; class Impl extends Base implements Api, Ext { Base b; EntityManager em; Impl(Api a, String s) {} }",
        )
        .unwrap();
        std::fs::write(dir.join("src/test/java/p/T.java"), "package p; class T {}").unwrap();
        let g = parse_project(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(g.symbols.iter().all(|s| s.id != "p.T"));
        let has = |to: &str, k: EdgeKind| {
            g.edges
                .iter()
                .any(|e| e.from == "p.Impl" && e.to == to && e.kind == k)
        };
        assert!(has("p.Base", EdgeKind::Extends));
        assert!(has("p.Api", EdgeKind::Implements));
        assert!(has("p.Base", EdgeKind::Uses));
        assert!(has("p.Api", EdgeKind::Uses));
        assert!(has("ext:EntityManager", EdgeKind::Uses));
        assert!(
            !g.edges
                .iter()
                .any(|e| e.to == "ext:String" || e.to == "ext:Ext")
        );
        assert_eq!(
            g.symbols.iter().find(|s| s.id == "p.Impl").unwrap().file,
            "src/main/java/p/Impl.java"
        );
    }

    #[test]
    fn constants_annotation_elements_records_varargs_typevars() {
        let src = "package p;\n\
            interface K { int MAX = 3; }\n\
            @interface A { String value(); }\n\
            record R<T>(Foo foo, T t, Bar... bars) { R {} }\n\
            class G<T> { T val; Baz baz; <U> G(T t, U u, Qux... q) {} }\n";
        let fs = parse_source("p/K.java", src).unwrap();
        assert_eq!(get(&fs, "p.K#MAX").kind, SymbolKind::Field);
        assert_eq!(get(&fs, "p.A#value(0)").kind, SymbolKind::Method);
        assert_eq!(get(&fs, "p.R#foo").kind, SymbolKind::Field);
        assert_eq!(get(&fs, "p.R#bars").kind, SymbolKind::Field);
        assert_eq!(get(&fs, "p.R#<init>(3)").kind, SymbolKind::Constructor);
        assert_eq!(get(&fs, "p.G#<init>(3)").kind, SymbolKind::Constructor);
        let uses = |t: &str| -> Vec<&str> {
            fs.uses
                .iter()
                .filter(|(f, _)| f == t)
                .map(|(_, n)| n.as_str())
                .collect()
        };
        assert_eq!(uses("p.R"), ["Foo", "Bar"]);
        assert_eq!(uses("p.G"), ["Baz", "Qux"]);
    }

    #[test]
    fn generics_stripped_and_end_ranges() {
        let src = "class A extends B<C<D>, E> {\n  Map<String, F> m;\n  void f() {\n  }\n}\n";
        let fs = parse_source("A.java", src).unwrap();
        assert_eq!(get(&fs, "A").supertypes, ["B"]);
        let f = get(&fs, "A#f(0)");
        assert_eq!(
            (f.range.start_line, f.range.end_line, f.range.end_col),
            (2, 3, 3)
        );
        let m = get(&fs, "A#m");
        assert_eq!(m.range.end_line, 1);
        assert_eq!(m.range.end_col, 19);
        assert_eq!(m.name_range.start_col, 17);
        assert_eq!(m.name_range.end_col, 18);
        assert_eq!(fs.uses, [("A".to_string(), "Map".to_string())]);
    }

    fn write(dir: &Path, rel: &str, content: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    #[test]
    fn resolution_rules_and_pruning() {
        let dir = std::env::temp_dir().join(format!("structure_java_res_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let m = "src/main/java/";
        write(
            &dir,
            &format!("{m}q/Widget.java"),
            "package q; class Widget {}",
        );
        write(
            &dir,
            &format!("{m}w/Gadget.java"),
            "package w; class Gadget {}",
        );
        write(
            &dir,
            &format!("{m}z/Gadget.java"),
            "package z; class Gadget {}",
        );
        write(
            &dir,
            &format!("{m}y/Thing.java"),
            "package y; class Thing {}",
        );
        write(
            &dir,
            &format!("{m}x/Thing.java"),
            "package x; class Thing {}",
        );
        write(
            &dir,
            &format!("{m}org/x/build/B.java"),
            "package org.x.build; class B {}",
        );
        write(&dir, "build/gen/Gen.java", "package gen; class Gen {}");
        write(&dir, "target/gen/Tgt.java", "package gen; class Tgt {}");
        write(
            &dir,
            &format!("{m}u/User.java"),
            "package u; import org.ext.Widget; import w.*;\n\
             class User { Widget a; Gadget b; Thing c; Date d; String e; int f; }",
        );
        let g = parse_project(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let uses: Vec<&str> = g
            .edges
            .iter()
            .filter(|e| e.from == "u.User" && e.kind == EdgeKind::Uses)
            .map(|e| e.to.as_str())
            .collect();
        assert_eq!(uses, ["ext:Thing", "ext:Widget", "w.Gadget"]);
        let ids: Vec<&str> = g.symbols.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"org.x.build.B"));
        assert!(!ids.contains(&"gen.Gen") && !ids.contains(&"gen.Tgt"));
        assert!(ids.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn sources_match_project() {
        let files: Vec<(String, String)> = [
            (
                "src/main/java/p/Base.java",
                "package p; public class Base {}",
            ),
            (
                "src/main/java/p/Api.java",
                "package p; public interface Api {}",
            ),
            (
                "src/main/java/p/Impl.java",
                "package p; class Impl extends Base implements Api { Base b; Ext e; }",
            ),
            ("src/test/java/p/T.java", "package p; class T {}"),
            ("README.md", "not java"),
            // String order and path order differ for these two.
            ("src/main/java/a/X.java", "package a; class X {}"),
            ("src/main/java/a-b/Y.java", "package ab; class Y {}"),
            // Skipped by the walk: hidden and build directories; kept under `src/`.
            (".mvn/wrapper/W.java", "class W {}"),
            ("target/gen/G.java", "package g; class G {}"),
            (
                "src/main/java/p/build/K.java",
                "package p.build; class K {}",
            ),
        ]
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .into();
        let dir = std::env::temp_dir().join(format!("structure_java_mem_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (rel, src) in &files {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, src).unwrap();
        }
        let (mut w1, mut w2) = (Vec::new(), Vec::new());
        let disk = parse_project_with_warnings(&dir, &mut w1).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let mem = parse_sources("mem", &files, &mut w2).unwrap();
        assert_eq!(mem.root, "mem");
        assert!(mem.edges.iter().any(|e| e.kind == EdgeKind::Implements));
        assert!(mem.edges.iter().any(|e| e.kind == EdgeKind::Uses));
        assert!(!mem.symbols.iter().any(|s| s.id == "p.T"));
        assert_eq!(w1, w2);
        assert_eq!(
            Graph {
                root: String::new(),
                ..disk
            },
            Graph {
                root: String::new(),
                ..mem
            }
        );
    }
}
