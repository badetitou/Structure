//! TypeScript tree-sitter frontend: symbols, ranges, decorators, supertype
//! and `Uses` edges. Call edges come from SCIP, not from here.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::Context;
use rayon::prelude::*;
use tree_sitter::{Node, Parser};

use structure_ir::ir::{Edge, EdgeKind, Graph, Language, Range, Symbol, SymbolKind};

mod http;

/// Result of parsing a single file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileSymbols {
    pub symbols: Vec<Symbol>,
    /// (local name, imported name, module specifier)
    pub imports: Vec<(String, String, String)>,
    /// Top-level `type X = ...` names (not symbols; skipped as Uses targets).
    pub type_aliases: Vec<String>,
    /// (from id, type name)
    pub uses: Vec<(String, String)>,
    /// (from id, supertype name, Extends | Implements); needed to type the
    /// supertype edges, since `Symbol::supertypes` merges both.
    pub heritage: Vec<(String, String, EdgeKind)>,
}

const IGNORED_TYPES: &[&str] = &[
    "string",
    "number",
    "boolean",
    "any",
    "unknown",
    "void",
    "never",
    "null",
    "undefined",
    "object",
    "symbol",
    "bigint",
    "Array",
    "ReadonlyArray",
    "Promise",
    "Observable",
    "Record",
    "Partial",
    "Required",
    "Readonly",
    "Pick",
    "Omit",
    "Map",
    "Set",
    "Date",
    "Function",
    "Object",
    "String",
    "Number",
    "Boolean",
    "RegExp",
    "Error",
    "HTMLElement",
    "HTMLInputElement",
    "HTMLDivElement",
    "HTMLFormElement",
    "Element",
    "Node",
    "Event",
    "MouseEvent",
    "KeyboardEvent",
    "File",
    "Blob",
    "FormData",
    "Window",
    "Document",
    "WeakMap",
    "WeakSet",
    "Uint8Array",
    "JSON",
    "Console",
    "URL",
    "Response",
    "Request",
];

fn text<'a>(n: Node, src: &'a str) -> &'a str {
    n.utf8_text(src.as_bytes()).unwrap_or("")
}

fn rng(n: Node) -> Range {
    let (s, e) = (n.start_position(), n.end_position());
    Range {
        start_line: s.row as u32,
        start_col: s.column as u32,
        end_line: e.row as u32,
        end_col: e.column as u32,
    }
}

fn decorator_name(d: Node, src: &str) -> Option<String> {
    let mut c = d.walk();
    let inner = d.named_children(&mut c).next()?;
    let target = if inner.kind() == "call_expression" {
        inner.child_by_field_name("function")?
    } else {
        inner
    };
    let id = if target.kind() == "member_expression" {
        target.child_by_field_name("property")?
    } else {
        target
    };
    Some(text(id, src).to_string())
}

/// Type names referenced by a type node (generic arguments included).
fn collect_types(n: Node, src: &str, skip: &HashSet<String>, out: &mut Vec<String>) {
    if n.kind() == "type_identifier" {
        let t = text(n, src);
        if !IGNORED_TYPES.contains(&t) && !skip.contains(t) {
            out.push(t.to_string());
        }
        return;
    }
    let mut c = n.walk();
    for ch in n.named_children(&mut c) {
        collect_types(ch, src, skip, out);
    }
}

/// `inject(X)` calls anywhere under `n`.
fn collect_inject(n: Node, src: &str, out: &mut Vec<String>) {
    if n.kind() == "call_expression"
        && let Some(f) = n.child_by_field_name("function")
        && text(f, src) == "inject"
        && let Some(args) = n.child_by_field_name("arguments")
    {
        let mut c = args.walk();
        if let Some(a) = args.named_children(&mut c).next()
            && a.kind() == "identifier"
        {
            let t = text(a, src);
            if !IGNORED_TYPES.contains(&t) {
                out.push(t.to_string());
            }
        }
    }
    let mut c = n.walk();
    for ch in n.named_children(&mut c) {
        collect_inject(ch, src, out);
    }
}

/// Base name of a supertype expression (generics stripped).
fn heritage_name(n: Node, src: &str) -> Option<String> {
    match n.kind() {
        "generic_type" => heritage_name(n.child_by_field_name("name")?, src),
        "type_identifier" | "identifier" => Some(text(n, src).to_string()),
        "nested_type_identifier" | "member_expression" => {
            let t = text(n, src);
            Some(t.rsplit('.').next().unwrap_or(t).to_string())
        }
        _ => None,
    }
}

struct Ctx<'a> {
    file: &'a str,
    src: &'a str,
    package: Option<String>,
    out: FileSymbols,
    seen_ids: HashSet<String>,
    /// Receiver names typed or injected as `HttpClient` in the current class.
    clients: HashSet<String>,
}

struct NewSymbol<'n> {
    id: String,
    kind: SymbolKind,
    container: Option<String>,
    node: Node<'n>,
    name_node: Node<'n>,
    annotations: Vec<String>,
    supertypes: Vec<String>,
}

impl Ctx<'_> {
    fn push(&mut self, s: NewSymbol) {
        // Ids must be unique (get/set pairs, overloads, declaration merging).
        if !self.seen_ids.insert(s.id.clone()) {
            return;
        }
        self.out.symbols.push(Symbol {
            id: s.id,
            kind: s.kind,
            name: text(s.name_node, self.src).to_string(),
            container: s.container,
            file: self.file.to_string(),
            range: rng(s.node),
            name_range: rng(s.name_node),
            annotations: s.annotations,
            supertypes: s.supertypes,
            http: vec![],
            http_calls: vec![],
            rpc_calls: vec![],
            package: self.package.clone(),
        });
    }

    /// Record the HTTP calls made under `node` on symbol `id`.
    fn attach_calls(&mut self, id: &str, node: Node) {
        let mut calls = Vec::new();
        http::http_calls(node, self.src, &self.clients, &mut calls);
        if calls.is_empty() {
            return;
        }
        if let Some(s) = self.out.symbols.iter_mut().rev().find(|s| s.id == id) {
            s.http_calls.extend(calls);
        }
    }

    fn simple(&mut self, kind: SymbolKind, node: Node, name: Node) {
        let id = format!("{}#{}", self.file, text(name, self.src));
        self.push(NewSymbol {
            id,
            kind,
            container: None,
            node,
            name_node: name,
            annotations: vec![],
            supertypes: vec![],
        });
    }

    fn decorators(&self, nodes: &[Node]) -> Vec<String> {
        nodes
            .iter()
            .filter_map(|d| decorator_name(*d, self.src))
            .collect()
    }

    fn top_level(&mut self, node: Node, outer_decorators: &[Node]) {
        match node.kind() {
            "export_statement" => {
                let mut c = node.walk();
                let decs: Vec<Node> = node
                    .children(&mut c)
                    .filter(|n| n.kind() == "decorator")
                    .collect();
                if let Some(d) = node.child_by_field_name("declaration") {
                    self.top_level(d, &decs);
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                self.class(node, outer_decorators)
            }
            "interface_declaration" => self.interface(node),
            "enum_declaration" => {
                if let Some(name) = node.child_by_field_name("name") {
                    self.simple(SymbolKind::Enum, node, name);
                }
            }
            "function_declaration" | "generator_function_declaration" => {
                if let Some(name) = node.child_by_field_name("name") {
                    self.simple(SymbolKind::Function, node, name);
                    self.function_calls(node, name);
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                let mut c = node.walk();
                for d in node.named_children(&mut c) {
                    if d.kind() != "variable_declarator" {
                        continue;
                    }
                    let (Some(name), Some(val)) = (
                        d.child_by_field_name("name"),
                        d.child_by_field_name("value"),
                    ) else {
                        continue;
                    };
                    if name.kind() == "identifier"
                        && matches!(
                            val.kind(),
                            "arrow_function" | "function_expression" | "function"
                        )
                    {
                        self.simple(SymbolKind::Function, node, name);
                        self.function_calls(val, name);
                    }
                }
            }
            "import_statement" => self.import(node),
            "type_alias_declaration" => {
                if let Some(name) = node.child_by_field_name("name") {
                    self.out.type_aliases.push(text(name, self.src).to_string());
                    // A named type shape: modelled like an interface.
                    self.simple(SymbolKind::Interface, node, name);
                }
            }
            _ => {}
        }
    }

    /// Calls of a top-level function, with its own `HttpClient` variables.
    fn function_calls(&mut self, body: Node, name: Node) {
        let id = format!("{}#{}", self.file, text(name, self.src));
        self.clients.clear();
        http::http_client_names(body, self.src, &mut self.clients);
        self.attach_calls(&id, body);
        self.clients.clear();
    }

    fn import(&mut self, node: Node) {
        let Some(source) = node.child_by_field_name("source") else {
            return;
        };
        let spec = text(source, self.src)
            .trim_matches(|c| c == '"' || c == '\'' || c == '`')
            .to_string();
        let mut c = node.walk();
        for clause in node.named_children(&mut c) {
            if clause.kind() != "import_clause" {
                continue;
            }
            let mut c2 = clause.walk();
            for part in clause.named_children(&mut c2) {
                match part.kind() {
                    "identifier" => {
                        let n = text(part, self.src).to_string();
                        self.out.imports.push((n.clone(), n, spec.clone()));
                    }
                    "namespace_import" => {
                        let mut c3 = part.walk();
                        if let Some(id) = part.named_children(&mut c3).next() {
                            let n = text(id, self.src).to_string();
                            self.out.imports.push((n, "*".to_string(), spec.clone()));
                        }
                    }
                    "named_imports" => {
                        let mut c3 = part.walk();
                        for sp in part.named_children(&mut c3) {
                            let imported = sp.child_by_field_name("name");
                            let local = sp.child_by_field_name("alias").or(imported);
                            if let (Some(l), Some(i)) = (local, imported) {
                                self.out.imports.push((
                                    text(l, self.src).to_string(),
                                    text(i, self.src).to_string(),
                                    spec.clone(),
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn interface(&mut self, node: Node) {
        let Some(name) = node.child_by_field_name("name") else {
            return;
        };
        let id = format!("{}#{}", self.file, text(name, self.src));
        let mut supers = Vec::new();
        let mut c = node.walk();
        for ch in node.children(&mut c) {
            if ch.kind() == "extends_type_clause" {
                let mut c2 = ch.walk();
                for t in ch.named_children(&mut c2) {
                    if let Some(n) = heritage_name(t, self.src) {
                        self.out
                            .heritage
                            .push((id.clone(), n.clone(), EdgeKind::Extends));
                        supers.push(n);
                    }
                }
            }
        }
        self.push(NewSymbol {
            id,
            kind: SymbolKind::Interface,
            container: None,
            node,
            name_node: name,
            annotations: vec![],
            supertypes: supers,
        });
    }

    fn class(&mut self, node: Node, outer_decorators: &[Node]) {
        let Some(name) = node.child_by_field_name("name") else {
            return;
        };
        let id = format!("{}#{}", self.file, text(name, self.src));
        let mut decs: Vec<Node> = outer_decorators.to_vec();
        let mut supers = Vec::new();
        let mut c = node.walk();
        for ch in node.children(&mut c) {
            match ch.kind() {
                "decorator" => decs.push(ch),
                "class_heritage" => {
                    let mut c2 = ch.walk();
                    for clause in ch.named_children(&mut c2) {
                        let kind = match clause.kind() {
                            "extends_clause" => EdgeKind::Extends,
                            "implements_clause" => EdgeKind::Implements,
                            _ => continue,
                        };
                        let mut c3 = clause.walk();
                        for t in clause.named_children(&mut c3) {
                            if let Some(n) = heritage_name(t, self.src) {
                                self.out.heritage.push((id.clone(), n.clone(), kind));
                                supers.push(n);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let annotations = self.decorators(&decs);
        self.push(NewSymbol {
            id: id.clone(),
            kind: SymbolKind::Class,
            container: None,
            node,
            name_node: name,
            annotations,
            supertypes: supers,
        });

        let outer_clients = std::mem::take(&mut self.clients);
        http::http_client_names(node, self.src, &mut self.clients);
        let mut tparams = HashSet::new();
        if let Some(tp) = node.child_by_field_name("type_parameters") {
            let mut c = tp.walk();
            for p in tp.named_children(&mut c) {
                if let Some(n) = p.child_by_field_name("name") {
                    tparams.insert(text(n, self.src).to_string());
                }
            }
        }
        if let Some(body) = node.child_by_field_name("body") {
            self.members(body, &id, &tparams);
        }
        self.clients = outer_clients;
    }

    fn uses_in_type(&mut self, from: &str, n: Node, skip: &HashSet<String>) {
        let mut names = Vec::new();
        collect_types(n, self.src, skip, &mut names);
        for t in names {
            self.out.uses.push((from.to_string(), t));
        }
    }

    fn members(&mut self, body: Node, class_id: &str, tparams: &HashSet<String>) {
        let mut pending: Vec<Node> = Vec::new();
        let mut c = body.walk();
        for m in body.named_children(&mut c) {
            if m.kind() == "decorator" {
                pending.push(m);
                continue;
            }
            let mut decs = std::mem::take(&mut pending);
            let Some(name) = m.child_by_field_name("name") else {
                continue;
            };
            let mname = text(name, self.src).to_string();
            match m.kind() {
                "method_definition" | "abstract_method_signature" => {
                    let is_ctor = mname == "constructor" && m.kind() == "method_definition";
                    let mut c2 = m.walk();
                    decs.extend(m.children(&mut c2).filter(|n| n.kind() == "decorator"));
                    let kind = if is_ctor {
                        SymbolKind::Constructor
                    } else {
                        SymbolKind::Method
                    };
                    let annotations = self.decorators(&decs);
                    self.push(NewSymbol {
                        id: format!("{class_id}.{mname}"),
                        kind,
                        container: Some(class_id.to_string()),
                        node: m,
                        name_node: name,
                        annotations,
                        supertypes: vec![],
                    });
                    if is_ctor && let Some(params) = m.child_by_field_name("parameters") {
                        let mut c3 = params.walk();
                        for p in params.named_children(&mut c3) {
                            if let Some(t) = p.child_by_field_name("type") {
                                self.uses_in_type(class_id, t, tparams);
                            }
                        }
                    }
                    if let Some(b) = m.child_by_field_name("body") {
                        self.attach_calls(&format!("{class_id}.{mname}"), b);
                        let mut names = Vec::new();
                        collect_inject(b, self.src, &mut names);
                        for n in names {
                            self.out.uses.push((class_id.to_string(), n));
                        }
                    }
                }
                "public_field_definition" => {
                    let mut c2 = m.walk();
                    decs.extend(m.children(&mut c2).filter(|n| n.kind() == "decorator"));
                    let annotations = self.decorators(&decs);
                    self.push(NewSymbol {
                        id: format!("{class_id}.{mname}"),
                        kind: SymbolKind::Field,
                        container: Some(class_id.to_string()),
                        node: m,
                        name_node: name,
                        annotations,
                        supertypes: vec![],
                    });
                    if let Some(t) = m.child_by_field_name("type") {
                        self.uses_in_type(class_id, t, tparams);
                    }
                    if let Some(v) = m.child_by_field_name("value") {
                        self.attach_calls(&format!("{class_id}.{mname}"), v);
                        let mut names = Vec::new();
                        collect_inject(v, self.src, &mut names);
                        for n in names {
                            self.out.uses.push((class_id.to_string(), n));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn dir_of(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(d, _)| d)
}

/// Parse one TypeScript (or TSX) source text.
pub fn parse_source(file: &str, src: &str, tsx: bool) -> anyhow::Result<FileSymbols> {
    let mut parser = Parser::new();
    let lang = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    };
    parser
        .set_language(&lang.into())
        .context("loading TypeScript grammar")?;
    let tree = parser
        .parse(src, None)
        .with_context(|| format!("parsing {file}"))?;
    let dir = dir_of(file);
    let mut ctx = Ctx {
        file,
        src,
        package: if dir.is_empty() {
            None
        } else {
            Some(dir.to_string())
        },
        out: FileSymbols::default(),
        seen_ids: HashSet::new(),
        clients: HashSet::new(),
    };
    let root = tree.root_node();
    let mut c = root.walk();
    for n in root.named_children(&mut c) {
        ctx.top_level(n, &[]);
    }
    Ok(ctx.out)
}

fn normalize(dir: &str, spec: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for seg in spec.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

struct Resolver<'a> {
    ids: &'a HashSet<&'a str>,
    files: &'a HashSet<&'a str>,
    aliases: &'a HashSet<String>,
    imports: &'a HashMap<String, Vec<(String, String, String)>>,
}

enum Resolved {
    Project(String),
    External(String),
    /// Relative import that does not resolve to a project symbol (barrels...).
    Unknown,
}

impl Resolver<'_> {
    fn resolve(&self, file: &str, name: &str) -> Resolved {
        let local = format!("{file}#{name}");
        if self.ids.contains(local.as_str()) {
            return Resolved::Project(local);
        }
        if self.aliases.contains(&local) {
            return Resolved::Unknown;
        }
        let imp = self
            .imports
            .get(file)
            .and_then(|v| v.iter().find(|(l, _, _)| l == name))
            .map(|(_, i, s)| (i.as_str(), s.as_str()));
        match imp {
            Some((orig, s)) if s.starts_with("./") || s.starts_with("../") => {
                let s = s
                    .strip_suffix(".js")
                    .or_else(|| s.strip_suffix(".jsx"))
                    .or_else(|| s.strip_suffix(".mjs"))
                    .unwrap_or(s);
                let Some(base) = normalize(dir_of(file), s) else {
                    return Resolved::Unknown;
                };
                let name = orig;
                for cand in [
                    format!("{base}.ts"),
                    format!("{base}.tsx"),
                    format!("{base}/index.ts"),
                    format!("{base}/index.tsx"),
                ] {
                    let id = format!("{cand}#{name}");
                    if self.aliases.contains(&id) {
                        return Resolved::Unknown;
                    }
                    if self.files.contains(cand.as_str()) && self.ids.contains(id.as_str()) {
                        return Resolved::Project(id);
                    }
                }
                Resolved::Unknown
            }
            Some((orig, _)) => Resolved::External(orig.to_string()),
            None => Resolved::External(name.to_string()),
        }
    }
}

/// Directories never descended into (below the root) when looking for TS sources.
pub const IGNORED_DIRS: &[&str] = &["node_modules", "dist", ".angular", ".git"];

/// Parse every TypeScript file under `root` and assemble a [`Graph`].
pub fn parse_project(root: &Path) -> anyhow::Result<Graph> {
    let mut paths = Vec::new();
    let walker = ignore::WalkBuilder::new(root)
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !(e.depth() > 0
                && e.file_type().is_some_and(|t| t.is_dir())
                && IGNORED_DIRS.contains(&n.as_ref()))
        })
        .build();
    for entry in walker {
        let entry = entry?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        let is_ts = name.ends_with(".ts") || name.ends_with(".tsx");
        let skipped = name.ends_with(".d.ts")
            || name.ends_with(".spec.ts")
            || name.ends_with(".test.ts")
            || name.ends_with(".test.tsx");
        if is_ts && !skipped {
            paths.push(entry.into_path());
        }
    }
    paths.sort();

    let parsed: Vec<(String, FileSymbols)> = paths
        .par_iter()
        .map(|p| -> anyhow::Result<(String, FileSymbols)> {
            let rel = p
                .strip_prefix(root)
                .unwrap_or(p)
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let src = std::fs::read_to_string(p).with_context(|| format!("reading {rel}"))?;
            let tsx = rel.ends_with(".tsx");
            let fs = parse_source(&rel, &src, tsx).with_context(|| rel.clone())?;
            Ok((rel, fs))
        })
        .collect::<anyhow::Result<_>>()?;

    let ids: HashSet<&str> = parsed
        .iter()
        .flat_map(|(_, f)| f.symbols.iter().map(|s| s.id.as_str()))
        .collect();
    let files: HashSet<&str> = parsed.iter().map(|(r, _)| r.as_str()).collect();
    let aliases: HashSet<String> = parsed
        .iter()
        .flat_map(|(r, f)| f.type_aliases.iter().map(move |a| format!("{r}#{a}")))
        .collect();
    let imports: HashMap<String, Vec<(String, String, String)>> = parsed
        .iter()
        .map(|(r, f)| (r.clone(), f.imports.clone()))
        .collect();
    let resolver = Resolver {
        ids: &ids,
        files: &files,
        aliases: &aliases,
        imports: &imports,
    };

    let mut edges = Vec::new();
    let mut seen: BTreeSet<(String, String, u8)> = BTreeSet::new();
    let mut add = |from: &str, to: String, kind: EdgeKind| {
        if seen.insert((from.to_string(), to.clone(), kind as u8)) {
            edges.push(Edge {
                from: from.to_string(),
                to,
                kind,
            });
        }
    };
    for (rel, f) in &parsed {
        for (from, name, kind) in &f.heritage {
            if let Resolved::Project(to) = resolver.resolve(rel, name) {
                add(from, to, *kind);
            }
        }
        for (from, name) in &f.uses {
            match resolver.resolve(rel, name) {
                Resolved::Project(to) => add(from, to, EdgeKind::Uses),
                Resolved::External(n) => add(from, format!("ext:{n}"), EdgeKind::Uses),
                Resolved::Unknown => {}
            }
        }
    }

    let mut symbols: Vec<Symbol> = parsed.into_iter().flat_map(|(_, f)| f.symbols).collect();
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    edges.sort_by(|a, b| (&a.from, &a.to, a.kind as u8).cmp(&(&b.from, &b.to, b.kind as u8)));
    Ok(Graph {
        root: root.to_string_lossy().into_owned(),
        language: Language::TypeScript,
        symbols,
        edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(f: &'a FileSymbols, id: &str) -> &'a Symbol {
        f.symbols.iter().find(|s| s.id == id).unwrap_or_else(|| {
            panic!(
                "missing {id}: {:?}",
                f.symbols.iter().map(|s| &s.id).collect::<Vec<_>>()
            )
        })
    }

    #[test]
    fn angular_component() {
        let src = "import { Component } from '@angular/core';\n@Component({ selector: 'a' })\nexport class AppComponent implements OnInit {\n  title = 'x';\n  ngOnInit() {}\n}\n";
        let f = parse_source("src/app/app.component.ts", src, false).unwrap();
        let c = find(&f, "src/app/app.component.ts#AppComponent");
        assert_eq!(c.kind, SymbolKind::Class);
        assert_eq!(c.annotations, vec!["Component"]);
        assert_eq!(c.supertypes, vec!["OnInit"]);
        assert_eq!(c.package.as_deref(), Some("src/app"));
        assert_eq!((c.name_range.start_line, c.name_range.start_col), (2, 13));
        assert_eq!(c.name_range.end_col, 25);
        let m = find(&f, "src/app/app.component.ts#AppComponent.ngOnInit");
        assert_eq!(m.kind, SymbolKind::Method);
        assert_eq!(
            m.container.as_deref(),
            Some("src/app/app.component.ts#AppComponent")
        );
        assert_eq!(
            find(&f, "src/app/app.component.ts#AppComponent.title").kind,
            SymbolKind::Field
        );
        assert_eq!(
            f.imports,
            vec![(
                "Component".to_string(),
                "Component".to_string(),
                "@angular/core".to_string()
            )]
        );
    }

    #[test]
    fn service_di_and_inject() {
        let src = "@Injectable({providedIn:'root'})\nexport class UserService {\n  private http2 = inject(HttpClient);\n  items: Item[] = [];\n  count: number = 0;\n  constructor(private http: HttpClient, private jwt: JwtService, n: string) {}\n}\n";
        let f = parse_source("u.ts", src, false).unwrap();
        let c = find(&f, "u.ts#UserService");
        assert_eq!(c.annotations, vec!["Injectable"]);
        assert_eq!(
            find(&f, "u.ts#UserService.constructor").kind,
            SymbolKind::Constructor
        );
        let used: Vec<&str> = f.uses.iter().map(|(_, t)| t.as_str()).collect();
        assert!(used.contains(&"HttpClient"));
        assert!(used.contains(&"JwtService"));
        assert!(used.contains(&"Item"));
        assert!(!used.contains(&"number") && !used.contains(&"string"));
        assert!(f.uses.iter().all(|(from, _)| from == "u.ts#UserService"));
    }

    #[test]
    fn interface_and_implements() {
        let src = "export interface Repo<T> extends Base<T> { get(): T }\nclass Impl extends Parent<string> implements Repo<User>, Other {}\nenum E { A }\n";
        let f = parse_source("r.ts", src, false).unwrap();
        let i = find(&f, "r.ts#Repo");
        assert_eq!(i.kind, SymbolKind::Interface);
        assert_eq!(i.supertypes, vec!["Base"]);
        let c = find(&f, "r.ts#Impl");
        assert_eq!(c.supertypes, vec!["Parent", "Repo", "Other"]);
        assert!(
            f.heritage
                .contains(&("r.ts#Impl".into(), "Repo".into(), EdgeKind::Implements))
        );
        assert!(
            f.heritage
                .contains(&("r.ts#Impl".into(), "Parent".into(), EdgeKind::Extends))
        );
        assert_eq!(find(&f, "r.ts#E").kind, SymbolKind::Enum);
    }

    #[test]
    fn export_default_abstract_and_decorator_before_export() {
        let f = parse_source(
            "d.ts",
            "@Injectable()\nexport abstract class Base {}\nexport default class Def {\n  get x() { return 1; }\n  set x(v) {}\n  m() { const h = inject(HttpClient); }\n  constructor() { inject(Foo); }\n}\ntype Alias = string;\n",
            false,
        )
        .unwrap();
        assert_eq!(find(&f, "d.ts#Base").annotations, vec!["Injectable"]);
        let d = find(&f, "d.ts#Def");
        assert_eq!(
            (
                d.range.start_line,
                d.range.start_col,
                d.range.end_line,
                d.range.end_col
            ),
            (2, 15, 7, 1)
        );
        let m = find(&f, "d.ts#Def.m");
        assert_eq!(
            (m.range.start_line, m.range.start_col, m.range.end_line),
            (5, 2, 5)
        );
        assert_eq!(f.symbols.iter().filter(|s| s.id == "d.ts#Def.x").count(), 1);
        let used: Vec<&str> = f.uses.iter().map(|(_, t)| t.as_str()).collect();
        assert!(used.contains(&"HttpClient") && used.contains(&"Foo"));
        assert_eq!(f.type_aliases, vec!["Alias"]);
    }

    #[test]
    fn aliased_import_recorded() {
        let f = parse_source("a.ts", "import { Foo as Bar } from './foo';", false).unwrap();
        assert_eq!(
            f.imports,
            vec![("Bar".into(), "Foo".into(), "./foo".into())]
        );
    }

    #[test]
    fn arrow_functions_and_tsx() {
        let f = parse_source(
            "a.ts",
            "export const add = (a: number) => a + 1;\nexport function sub() {}\nconst k = 3;\n",
            false,
        )
        .unwrap();
        assert_eq!(find(&f, "a.ts#add").kind, SymbolKind::Function);
        assert_eq!(find(&f, "a.ts#sub").kind, SymbolKind::Function);
        assert!(!f.symbols.iter().any(|s| s.name == "k"));
        let t = parse_source(
            "ui/B.tsx",
            "export const Button = () => <div>hi</div>;\nexport function Card() { return <p/>; }\n",
            true,
        )
        .unwrap();
        assert_eq!(find(&t, "ui/B.tsx#Button").kind, SymbolKind::Function);
        assert_eq!(find(&t, "ui/B.tsx#Card").kind, SymbolKind::Function);
    }
}
