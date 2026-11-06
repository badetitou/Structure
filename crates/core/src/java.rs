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

use crate::ir::{Edge, EdgeKind, Graph, Language, Range, Symbol, SymbolKind};

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

struct Ctx<'a> {
    file: &'a str,
    src: &'a str,
    package: Option<String>,
    out: FileSymbols,
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
        });
        self.out.symbols.len() - 1
    }

    fn type_decl(&mut self, node: Node, container: Option<&str>) {
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

        if let Some(body) = node.child_by_field_name("body") {
            self.members(body, &id);
        }
    }

    fn members(&mut self, body: Node, tid: &str) {
        let mut cur = body.walk();
        for m in body.named_children(&mut cur) {
            match m.kind() {
                "enum_body_declarations" => self.members(m, tid),
                "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
                | "annotation_type_declaration" => self.type_decl(m, Some(tid)),
                "method_declaration" => {
                    if let Some(n) = m.child_by_field_name("name") {
                        let id = format!(
                            "{tid}#{}({})",
                            text(n, self.src),
                            arity(m.child_by_field_name("parameters"))
                        );
                        self.symbol(id, SymbolKind::Method, m, n, Some(tid));
                    }
                }
                "constructor_declaration" => {
                    if let Some(n) = m.child_by_field_name("name") {
                        let params = m.child_by_field_name("parameters");
                        let id = format!("{tid}#<init>({})", arity(params));
                        self.symbol(id, SymbolKind::Constructor, m, n, Some(tid));
                        if let Some(p) = params {
                            let mut c2 = p.walk();
                            for prm in p.named_children(&mut c2) {
                                if let Some(t) = prm
                                    .child_by_field_name("type")
                                    .and_then(|t| type_name(t, self.src))
                                {
                                    self.out.uses.push((tid.to_string(), t));
                                }
                            }
                        }
                    }
                }
                "field_declaration" => {
                    let ty = m
                        .child_by_field_name("type")
                        .and_then(|t| type_name(t, self.src));
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
    };
    let mut cur = root.walk();
    for n in root.children(&mut cur) {
        ctx.type_decl(n, None);
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
        for imp in fs.imports.iter().filter(|i| i.ends_with(&suffix)) {
            if let Some(id) = self.dotted.get(imp) {
                return Some(id.clone());
            }
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
    let walker = ignore::WalkBuilder::new(root)
        .require_git(false)
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            let is_dir = e.file_type().is_some_and(|t| t.is_dir());
            !(is_dir && matches!(n.as_ref(), "target" | "build" | "node_modules"))
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

    let all: Vec<Symbol> = parsed.iter().flat_map(|f| f.symbols.clone()).collect();
    let index = TypeIndex::new(&all);

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

    Ok(Graph {
        root: root.to_string_lossy().into_owned(),
        language: Language::Java,
        symbols: all,
        edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
