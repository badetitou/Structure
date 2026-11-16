//! Pharo frontend: a hand-written Tonel reader (no maintained tree-sitter
//! grammar exists). Produces class and method symbols, superclass edges and
//! `Uses` edges for capitalised identifiers in method bodies. Message sends
//! (`Call` edges) are not resolved here.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use anyhow::Context;

use structure_ir::ir::{Edge, EdgeKind, Graph, Language, Range, Symbol, SymbolKind};

/// Result of parsing a single Tonel file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileSymbols {
    pub symbols: Vec<Symbol>,
    /// (from id, superclass name)
    pub heritage: Vec<(String, String)>,
    /// (from id, capitalised identifier referenced in the body)
    pub uses: Vec<(String, String)>,
    /// (class, name) of class variables and pools: not global references.
    pub scoped: Vec<(String, String)>,
    /// The file ended early or had an unknown shape; what was read is kept.
    pub warning: Option<String>,
}

/// Kernel globals that carry no architectural information.
const IGNORED_GLOBALS: &[&str] = &[
    "Smalltalk",
    "Transcript",
    "Object",
    "String",
    "Symbol",
    "Array",
    "OrderedCollection",
    "Dictionary",
    "Set",
    "Bag",
    "Number",
    "Integer",
    "Float",
    "Boolean",
    "True",
    "False",
    "Character",
    "UndefinedObject",
    "Error",
    "Exception",
    "Date",
    "Time",
];

// ---- lexical helpers (all delimiters are ASCII, so byte offsets are safe) ----

/// End of the string, comment or character literal starting at `i`, if any.
fn skip_literal(b: &[u8], i: usize) -> Option<usize> {
    match b.get(i)? {
        b'\'' => Some(
            b[i + 1..]
                .iter()
                .position(|&c| c == b'\'')
                .map_or(b.len(), |p| i + 1 + p + 1),
        ),
        b'"' => Some(
            b[i + 1..]
                .iter()
                .position(|&c| c == b'"')
                .map_or(b.len(), |p| i + 1 + p + 1),
        ),
        b'$' => Some((i + 2).min(b.len())),
        _ => None,
    }
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b.get(i) == Some(&b'"') {
            i = skip_literal(b, i).unwrap_or(b.len());
        } else {
            return i;
        }
    }
}

/// Index just after the bracket closing the one at `start`.
fn skip_balanced(b: &[u8], start: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = start;
    while i < b.len() {
        if let Some(n) = skip_literal(b, i) {
            i = n;
            continue;
        }
        if b[i] == open {
            depth += 1;
        } else if b[i] == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    None
}

/// First `[` outside strings and comments, from `i`.
fn find_open_bracket(b: &[u8], mut i: usize) -> Option<usize> {
    while i < b.len() {
        if let Some(n) = skip_literal(b, i) {
            i = n;
            continue;
        }
        if b[i] == b'[' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Capitalised identifiers of a method body, in order, duplicates kept.
fn capitalised_identifiers(body: &str) -> Vec<String> {
    let b = body.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if let Some(n) = skip_literal(b, i) {
            i = n;
            continue;
        }
        let c = b[i];
        if c == b'#' {
            // Symbol (`#Foo`, `#at:put:`): not a reference. `#'..'` is a string.
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b':') {
                i += 1;
            }
        } else if c.is_ascii_alphabetic() || c == b'_' || c.is_ascii_digit() {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            if b[s].is_ascii_uppercase() && b.get(i) != Some(&b':') {
                out.push(body[s..i].to_string());
            }
        } else {
            i += 1;
        }
    }
    out
}

// ---- STON header ----

/// `#key : value` pairs of a Tonel header body (between the braces).
fn header_pairs(h: &str) -> Vec<(String, String)> {
    let b = h.as_bytes();
    let mut items: Vec<&str> = Vec::new();
    let (mut start, mut depth, mut i) = (0, 0i32, 0);
    while i < b.len() {
        if let Some(n) = skip_literal(b, i) {
            i = n;
            continue;
        }
        match b[i] {
            b'[' | b'{' | b'(' => depth += 1,
            b']' | b'}' | b')' => depth -= 1,
            b',' if depth == 0 => {
                items.push(&h[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    items.push(&h[start..]);
    items
        .into_iter()
        .filter_map(|it| {
            let (k, v) = it.split_once(':')?;
            Some((k.trim().trim_start_matches('#').to_string(), ston_value(v)))
        })
        .collect()
}

/// `#Foo`, `#'Foo-Bar'`, `'Foo'` or a bare word, unquoted.
fn ston_value(v: &str) -> String {
    let v = v.trim();
    if v.starts_with('[') {
        return v.to_string();
    }
    let v = v.trim_start_matches('#');
    match v.strip_prefix('\'') {
        Some(rest) => {
            let end = rest.rfind('\'').unwrap_or(rest.len());
            rest[..end].replace("''", "'")
        }
        None => v.split_whitespace().next().unwrap_or("").to_string(),
    }
}

// ---- positions ----

struct Lines(Vec<usize>);

impl Lines {
    fn new(src: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(src.match_indices('\n').map(|(i, _)| i + 1));
        Lines(starts)
    }

    fn pos(&self, off: usize) -> (u32, u32) {
        let line = self.0.partition_point(|&s| s <= off) - 1;
        (line as u32, (off - self.0[line]) as u32)
    }

    fn range(&self, a: usize, b: usize) -> Range {
        let (start_line, start_col) = self.pos(a);
        let (end_line, end_col) = self.pos(b);
        Range {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }
}

/// Whitespace-separated tokens of `s` with their absolute `(start, end)`.
fn tokens(s: &str, base: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in s.char_indices() {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(st)) => {
                out.push((base + st, base + i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(st) = start {
        out.push((base + st, base + s.len()));
    }
    out
}

/// Selector and its absolute extent from a method signature's tokens.
fn selector(src: &str, toks: &[(usize, usize)]) -> Option<(String, usize, usize)> {
    let (s0, e0) = *toks.first()?;
    let first = &src[s0..e0];
    if !first.ends_with(':') {
        return Some((first.to_string(), s0, e0));
    }
    let kws: Vec<&(usize, usize)> = toks
        .iter()
        .filter(|(s, e)| src[*s..*e].ends_with(':'))
        .collect();
    let name: String = kws.iter().map(|(s, e)| &src[*s..*e]).collect();
    let end = kws.last().map_or(e0, |(_, e)| *e);
    Some((name, s0, end))
}

/// Parse one Tonel file. `dir_package` is the package directory name, used
/// when the header does not name one and for extension files.
pub fn parse_source(file: &str, src: &str, dir_package: &str) -> FileSymbols {
    let mut out = FileSymbols::default();
    let b = src.as_bytes();
    let lines = Lines::new(src);

    let i = skip_ws(b, 0);
    let mut j = i;
    while j < b.len() && b[j].is_ascii_alphabetic() {
        j += 1;
    }
    let word = &src[i..j];
    if word == "Package" {
        return out;
    }
    let is_type = matches!(word, "Class" | "Trait");
    if !is_type && word != "Extension" {
        out.warning = Some(format!(
            "{file}: not a Tonel file (no Class/Extension header)"
        ));
        return out;
    }
    let ob = skip_ws(b, j);
    let end = if b.get(ob) == Some(&b'{') {
        skip_balanced(b, ob, b'{', b'}')
    } else {
        None
    };
    let Some(end) = end else {
        out.warning = Some(format!("{file}: unterminated or missing header"));
        return out;
    };
    let pairs = header_pairs(&src[ob + 1..end - 1]);
    let get = |k: &str| {
        pairs
            .iter()
            .find(|(key, v)| key == k && !v.is_empty())
            .map(|(_, v)| v.as_str())
    };
    let Some(class_name) = get("name") else {
        out.warning = Some(format!("{file}: header without #name"));
        return out;
    };

    let package = if is_type {
        match (get("package"), get("tag")) {
            (Some(p), Some(t)) => format!("{p}-{t}"),
            (Some(p), None) => p.to_string(),
            _ => get("category").unwrap_or(dir_package).to_string(),
        }
    } else {
        dir_package.to_string()
    };
    let package = (!package.is_empty()).then_some(package);

    let mk = |id: String,
              kind: SymbolKind,
              name: &str,
              container: Option<String>,
              range: Range,
              name_range: Range,
              supertypes: Vec<String>| Symbol {
        id,
        kind,
        name: name.to_string(),
        container,
        file: file.to_string(),
        range,
        name_range,
        annotations: vec![],
        supertypes,
        package: package.clone(),
        http: vec![],
        http_calls: vec![],
        rpc_calls: vec![],
    };

    let mut class_id = None;
    if is_type {
        let cs = skip_ws(b, 0);
        // Search after the `#name` key, not in the `Class`/`Trait` keyword.
        let key = src[cs..end].find("#name").map_or(cs, |p| cs + p + 5);
        let name_off = src[key..end].find(class_name).map_or(cs, |p| key + p);
        for k in ["classVars", "pools", "sharedPools"] {
            for (_, v) in pairs.iter().filter(|(key, _)| key == k) {
                for w in v.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
                    if w.starts_with(|c: char| c.is_uppercase()) {
                        out.scoped.push((class_name.to_string(), w.to_string()));
                    }
                }
            }
        }
        let sup = get("superclass").filter(|s| *s != "nil");
        if let Some(s) = sup {
            out.heritage.push((class_name.to_string(), s.to_string()));
        }
        let kind = if word == "Trait" {
            SymbolKind::Interface
        } else {
            SymbolKind::Class
        };
        // The class extent is its header; methods are separate symbols.
        let r = lines.range(cs, end);
        out.symbols.push(mk(
            class_name.to_string(),
            kind,
            class_name,
            None,
            r,
            lines.range(name_off, name_off + class_name.len()),
            sup.map(str::to_string).into_iter().collect(),
        ));
        class_id = Some(class_name.to_string());
    }

    let mut i = end;
    let mut seen: HashSet<String> = HashSet::new();
    loop {
        i = skip_ws(b, i);
        if i >= b.len() {
            break;
        }
        if b[i] == b'{' {
            match skip_balanced(b, i, b'{', b'}') {
                Some(n) => {
                    i = n;
                    continue;
                }
                None => {
                    out.warning = Some(format!("{file}: unterminated method pragma"));
                    break;
                }
            }
        }
        let hstart = i;
        let Some(open) = find_open_bracket(b, i) else {
            out.warning = Some(format!("{file}: method without body at byte {hstart}"));
            break;
        };
        let body_end = skip_balanced(b, open, b'[', b']');
        let stop = body_end.unwrap_or(b.len());
        let header = &src[hstart..open];
        let Some(arrow) = header.find(">>") else {
            out.warning = Some(format!("{file}: method header without >> at byte {hstart}"));
            i = stop;
            continue;
        };
        let recv: Vec<&str> = header[..arrow].split_whitespace().collect();
        let sig_base = hstart + arrow + 2;
        let toks = tokens(&header[arrow + 2..], sig_base);
        if let (Some(cls), Some((sel, ns, ne))) = (recv.first(), selector(src, &toks)) {
            let class_side = recv.get(1) == Some(&"class");
            let owner = if class_side {
                format!("{cls} class")
            } else {
                cls.to_string()
            };
            let id = format!("{owner}#{sel}");
            if seen.insert(id.clone()) {
                let container = class_id.clone().filter(|c| c == cls);
                out.symbols.push(mk(
                    id.clone(),
                    SymbolKind::Method,
                    &sel,
                    container,
                    lines.range(hstart, stop),
                    lines.range(ns, ne),
                    vec![],
                ));
                for name in capitalised_identifiers(&src[open..stop]) {
                    out.uses.push((id.clone(), name));
                }
            }
        } else {
            out.warning = Some(format!("{file}: unreadable method header at byte {hstart}"));
        }
        if body_end.is_none() {
            out.warning = Some(format!("{file}: unterminated method body"));
            break;
        }
        i = stop;
    }
    out
}

/// Directories never descended into.
pub const IGNORED_DIRS: &[&str] = &[".git", "pharo-local", "node_modules"];

/// Parse every Tonel file under `root` (`root/src` when it exists).
pub fn parse_project(root: &Path) -> anyhow::Result<Graph> {
    parse_project_with_warnings(root, &mut Vec::new())
}

/// [`parse_project`], pushing soft problems (malformed files) into `warnings`.
pub fn parse_project_with_warnings(
    root: &Path,
    warnings: &mut Vec<String>,
) -> anyhow::Result<Graph> {
    let base = if root.join("src").is_dir() {
        root.join("src")
    } else {
        root.to_path_buf()
    };
    let walker = ignore::WalkBuilder::new(&base)
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !(e.depth() > 0
                && e.file_type().is_some_and(|t| t.is_dir())
                && IGNORED_DIRS.contains(&n.as_ref()))
        })
        .build();
    let mut paths = Vec::new();
    for entry in walker {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy();
        if entry.file_type().is_some_and(|t| t.is_file())
            && name.ends_with(".st")
            && name != "package.st"
        {
            paths.push(entry.into_path());
        }
    }
    paths.sort();

    let mut parsed = Vec::new();
    for p in &paths {
        let rel = p
            .strip_prefix(root)
            .unwrap_or(p)
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let src = std::fs::read_to_string(p).with_context(|| format!("reading {rel}"))?;
        let dir = p
            .parent()
            .filter(|d| *d != base.as_path())
            .and_then(|d| d.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        parsed.push(parse_source(&rel, &src, &dir));
    }
    for f in &parsed {
        warnings.extend(f.warning.clone());
    }

    let classes: HashSet<String> = parsed
        .iter()
        .flat_map(|f| &f.symbols)
        .filter(|s| s.container.is_none() && s.kind != SymbolKind::Method)
        .map(|s| s.id.clone())
        .collect();
    let owner_of = |from: &str| -> String {
        from.split_once('#')
            .map_or(from, |(o, _)| o)
            .trim_end_matches(" class")
            .to_string()
    };

    let scoped: HashSet<(&str, &str)> = parsed
        .iter()
        .flat_map(|f| &f.scoped)
        .map(|(c, n)| (c.as_str(), n.as_str()))
        .collect();
    let mut edges = Vec::new();
    let mut seen: BTreeSet<(String, String, u8)> = BTreeSet::new();
    let mut add = |from: &str, to: String, kind: EdgeKind| {
        if from != to && seen.insert((from.to_string(), to.clone(), kind.rank())) {
            edges.push(Edge {
                from: from.to_string(),
                to,
                kind,
            });
        }
    };
    for f in &parsed {
        for (from, sup) in &f.heritage {
            let to = if classes.contains(sup) {
                sup.clone()
            } else {
                format!("ext:{sup}")
            };
            add(from, to, EdgeKind::Extends);
        }
        for (from, name) in &f.uses {
            let owner = owner_of(from);
            if *name == owner || scoped.contains(&(owner.as_str(), name.as_str())) {
                continue;
            }
            if classes.contains(name) {
                add(from, name.clone(), EdgeKind::Uses);
            } else if !IGNORED_GLOBALS.contains(&name.as_str()) {
                add(from, format!("ext:{name}"), EdgeKind::Uses);
            }
        }
    }

    // Methods of project classes hang under their class, wherever defined.
    let mut symbols: Vec<Symbol> = parsed.into_iter().flat_map(|f| f.symbols).collect();
    for s in symbols.iter_mut().filter(|s| s.kind == SymbolKind::Method) {
        let owner = owner_of(&s.id);
        if classes.contains(&owner) {
            s.container = Some(owner);
        }
    }
    let mut ids = HashSet::new();
    symbols.retain(|s| ids.insert(s.id.clone()));
    symbols.sort_by(|a, b| a.id.cmp(&b.id));
    edges.sort_by(|a, b| (&a.from, &a.to, a.kind.rank()).cmp(&(&b.from, &b.to, b.kind.rank())));
    Ok(Graph {
        root: root.to_string_lossy().into_owned(),
        language: Language::Pharo,
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

    const V3: &str = r#""
A comment with [brackets
"
Class {
	#name : #OrderService,
	#superclass : #Object,
	#traits : 'TLogging',
	#classTraits : 'TLogging classTrait',
	#instVars : [ 'repo', 'log' ],
	#package : 'MyApp-Core',
	#tag : 'Services'
}

{ #category : #accessing }
OrderService >> place: anOrder at: aDate [
	"closing ] in a comment"
	| s |
	s := 'it''s a ] string'.
	$] = $[ ifTrue: [ ^ #Foo ].
	^ Order new repo: OrderRepository default; yourself
]

{ #category : #'instance creation' }
OrderService class >> new [
	^ super new init
]

OrderService >> + other [
	^ Smalltalk at: #Missing ifAbsent: [ Widget new ]
]
"#;

    #[test]
    fn v3_class_and_methods() {
        let f = parse_source("src/MyApp-Core/OrderService.class.st", V3, "MyApp-Core");
        assert_eq!(f.warning, None);
        let c = find(&f, "OrderService");
        assert_eq!(c.kind, SymbolKind::Class);
        assert_eq!(c.package.as_deref(), Some("MyApp-Core-Services"));
        assert_eq!(c.supertypes, vec!["Object"]);
        assert_eq!(f.heritage, vec![("OrderService".into(), "Object".into())]);
        assert_eq!(c.range.start_line, 3);

        let m = find(&f, "OrderService#place:at:");
        assert_eq!(m.container.as_deref(), Some("OrderService"));
        assert_eq!(m.name, "place:at:");
        assert_eq!((m.range.start_line, m.range.start_col), (14, 0));
        assert_eq!(m.range.end_line, 20);
        assert_eq!((m.name_range.start_line, m.name_range.start_col), (14, 16));
        assert_eq!(m.name_range.end_col, 34);

        let cm = find(&f, "OrderService class#new");
        assert_eq!(cm.name, "new");
        find(&f, "OrderService#+");
        assert_eq!(f.symbols.len(), 4);
    }

    #[test]
    fn body_references() {
        let f = parse_source("a.class.st", V3, "P");
        let of = |id: &str| -> Vec<&str> {
            f.uses
                .iter()
                .filter(|(from, _)| from == id)
                .map(|(_, n)| n.as_str())
                .collect()
        };
        // `#Foo`, strings, comments and char literals are not references.
        assert_eq!(
            of("OrderService#place:at:"),
            vec!["Order", "OrderRepository"]
        );
        assert_eq!(of("OrderService#+"), vec!["Smalltalk", "Widget"]);
    }

    #[test]
    fn v1_header_and_category_fallback() {
        let src = "Class {\n\t#name : 'Foo',\n\t#superclass : 'Bar',\n\t#category : 'App-Model'\n}\n\nFoo >> x [ ^ 1 ]\n";
        let f = parse_source("Foo.class.st", src, "dir");
        let c = find(&f, "Foo");
        assert_eq!(c.package.as_deref(), Some("App-Model"));
        assert_eq!(f.heritage[0].1, "Bar");
        find(&f, "Foo#x");

        let src = "Class { #name : #Foo }\nFoo >> x [ ^ 1 ]";
        let f = parse_source("Foo.class.st", src, "dir");
        assert_eq!(find(&f, "Foo").package.as_deref(), Some("dir"));
    }

    #[test]
    fn trait_header() {
        let src = "Trait {\n\t#name : #TFoo,\n\t#package : 'App'\n}\n\nTFoo >> foo [ ^ Bar new ]\n";
        let f = parse_source("TFoo.trait.st", src, "App");
        assert_eq!(find(&f, "TFoo").kind, SymbolKind::Interface);
        assert!(f.heritage.is_empty());
        find(&f, "TFoo#foo");
    }

    #[test]
    fn extension_file() {
        let src = "Extension { #name : #String }\n\n{ #category : #'*MyApp' }\nString >> shout [\n\t^ self asUppercase, '!'\n]\n\nString class >> hello [ ^ Greeter new ]\n";
        let f = parse_source("src/MyApp-Ext/String.extension.st", src, "MyApp-Ext");
        assert_eq!(f.warning, None);
        assert_eq!(f.symbols.len(), 2);
        let m = find(&f, "String#shout");
        assert_eq!(m.package.as_deref(), Some("MyApp-Ext"));
        assert_eq!(m.container, None);
        assert_eq!(m.range.start_line, 3);
        find(&f, "String class#hello");
    }

    #[test]
    fn malformed_keeps_partial_result() {
        let src = "Class { #name : #Foo }\nFoo >> a [ ^ 1 ]\nFoo >> b [ ^ [ 2 ";
        let f = parse_source("Foo.class.st", src, "d");
        assert!(f.warning.is_some());
        find(&f, "Foo#a");
        assert!(parse_source("x.st", "hello", "d").warning.is_some());
    }

    #[test]
    fn class_vars_and_pools_are_not_references() {
        let src = "Class {\n\t#name : #C,\n\t#superclass : #Object,\n\t#classVars : [ 'Counter' ],\n\t#pools : [ #Default ],\n\t#category : 'App'\n}\n\nC class >> next [ ^ Counter + Default + Other ]\n";
        let f = parse_source("C.class.st", src, "d");
        let c = find(&f, "C");
        // Name located after `#name`, not inside the `Class` keyword.
        assert_eq!((c.name_range.start_line, c.name_range.start_col), (1, 10));
        let dir = std::env::temp_dir().join(format!("structure-pharo-cv-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src/App")).unwrap();
        std::fs::write(dir.join("src/App/C.class.st"), src).unwrap();
        std::fs::write(
            dir.join("src/App/C.extension.st"),
            "Extension { #name : #C }\nC >> peek [ ^ Counter ]\n",
        )
        .unwrap();
        let g = parse_project(&dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        let to: Vec<&str> = g.edges.iter().map(|e| e.to.as_str()).collect();
        assert_eq!(to, vec!["ext:Object", "ext:Other"], "{:?}", g.edges);
    }

    #[test]
    fn project_graph() {
        let dir = std::env::temp_dir().join(format!("structure-pharo-{}", std::process::id()));
        let pkg = dir.join("src/App-Core");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(dir.join("src/.properties"), "{ #format : #tonel }").unwrap();
        std::fs::write(pkg.join("package.st"), "Package { #name : 'App-Core' }").unwrap();
        std::fs::write(
            pkg.join("Base.class.st"),
            "Class { #name : #Base, #superclass : #Object, #category : 'App-Core' }\nBase >> go [ ^ Base new, Helper new, Morph new ]\n",
        )
        .unwrap();
        std::fs::write(
            pkg.join("Helper.class.st"),
            "Class { #name : #Helper, #superclass : #Base, #package : 'App-Core' }\n",
        )
        .unwrap();
        std::fs::write(
            pkg.join("Base.extension.st"),
            "Extension { #name : #Helper }\nHelper >> extra [ ^ 1 ]\n",
        )
        .unwrap();
        let mut warnings = Vec::new();
        let g = parse_project_with_warnings(&dir, &mut warnings).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(g.language, Language::Pharo);
        let ids: Vec<&str> = g.symbols.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["Base", "Base#go", "Helper", "Helper#extra"]);
        assert_eq!(
            g.symbol("Helper#extra")
                .and_then(|s| s.container.as_deref()),
            Some("Helper")
        );
        let e = |from: &str, to: &str, kind| Edge {
            from: from.into(),
            to: to.into(),
            kind,
        };
        assert_eq!(
            g.edges,
            vec![
                e("Base", "ext:Object", EdgeKind::Extends),
                e("Base#go", "Helper", EdgeKind::Uses),
                e("Base#go", "ext:Morph", EdgeKind::Uses),
                e("Helper", "Base", EdgeKind::Extends),
            ]
        );
    }
}
