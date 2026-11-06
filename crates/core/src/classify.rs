//! Declarative layer/domain classification of top-level symbols.
//!
//! Rules live in TOML files (`rules/*.toml`). A rule matches when ALL of its
//! given matchers match; each matcher is "any of" its list. The first matching
//! rule (file order) decides the layer. Without a match, a structural fallback
//! applies (DTO-like class -> `model`, else `unknown`).

use crate::ir::{EdgeKind, Graph, Language, Symbol, SymbolKind};
use crate::model::{Classification, Layer, Placement};
use anyhow::Context;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};

const JAVA_RULES: &str = include_str!("../../../rules/java.toml");
const TS_RULES: &str = include_str!("../../../rules/typescript.toml");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub layer: Layer,
    /// Annotation/decorator simple names.
    #[serde(default)]
    pub any_annotation: Vec<String>,
    /// Supertype names (last path segment compared).
    #[serde(default)]
    pub any_supertype: Vec<String>,
    #[serde(default)]
    pub name_suffix: Vec<String>,
    #[serde(default)]
    pub name_prefix: Vec<String>,
    #[serde(default)]
    pub name_contains: Vec<String>,
    /// Substrings of the file path (matched against `/<file>`).
    #[serde(default)]
    pub path_contains: Vec<String>,
    /// Substrings of the package (matched against `.<package>.`).
    #[serde(default)]
    pub package_contains: Vec<String>,
    /// Type names used (`ext:X` edges, or project symbols named X).
    #[serde(default)]
    pub uses: Vec<String>,
    #[serde(default)]
    pub kind: Vec<SymbolKind>,
    /// Overrides the rule set's default domain spec.
    #[serde(default)]
    pub domain: Option<DomainSpec>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    /// Symbol name minus `strip_suffixes`.
    #[default]
    NameRoot,
    /// First package segment after `after`, skipping `skip_segments`.
    PackageSegment,
    /// First directory segment after `after`, skipping `skip_segments`.
    PathSegment,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainSpec {
    #[serde(default)]
    pub strategy: Strategy,
    #[serde(default)]
    pub strip_suffixes: Vec<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub skip_segments: Vec<String>,
    /// canonical domain -> raw names mapped onto it (case-insensitive).
    #[serde(default)]
    pub aliases: BTreeMap<String, Vec<String>>,
    #[serde(default = "default_fallback")]
    pub fallback: String,
}

fn default_fallback() -> String {
    "misc".into()
}

impl Default for DomainSpec {
    fn default() -> Self {
        DomainSpec {
            strategy: Strategy::NameRoot,
            strip_suffixes: vec![],
            after: None,
            skip_segments: vec![],
            aliases: BTreeMap::new(),
            fallback: default_fallback(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSet {
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub domain: Option<DomainSpec>,
}

impl RuleSet {
    pub fn from_toml_str(s: &str) -> anyhow::Result<RuleSet> {
        toml::from_str(s).context("invalid classification rules")
    }

    /// Embedded rules for a language.
    pub fn builtin(lang: Language) -> anyhow::Result<RuleSet> {
        match lang {
            Language::Java => Self::from_toml_str(JAVA_RULES).context("rules/java.toml"),
            Language::TypeScript => Self::from_toml_str(TS_RULES).context("rules/typescript.toml"),
        }
    }

    /// `other` takes priority: its rules come first, and its domain spec
    /// replaces ours when present.
    pub fn merge(mut self, other: RuleSet) -> RuleSet {
        let mut rules = other.rules;
        rules.append(&mut self.rules);
        RuleSet {
            rules,
            domain: other.domain.or(self.domain),
        }
    }
}

/// Per top-level symbol facts needed by the matchers.
struct Facts<'a> {
    sym: &'a Symbol,
    uses: HashSet<&'a str>,
    /// Direct member methods/constructors.
    members: Vec<&'a Symbol>,
    has_calls: bool,
}

fn is_top_kind(k: SymbolKind) -> bool {
    matches!(
        k,
        SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum | SymbolKind::Function
    )
}

fn simple(s: &str) -> &str {
    s.rsplit('.').next().unwrap_or(s)
}

fn rule_matches(r: &Rule, f: &Facts) -> bool {
    let s = f.sym;
    let any = |list: &[String], pred: &dyn Fn(&str) -> bool| {
        list.is_empty() || list.iter().any(|x| pred(x))
    };
    let path = format!("/{}", s.file);
    let pkg = s.package.as_deref().map(|p| format!(".{p}."));
    (r.kind.is_empty() || r.kind.contains(&s.kind))
        && any(&r.any_annotation, &|a| s.annotations.iter().any(|x| x == a))
        && any(&r.any_supertype, &|a| {
            s.supertypes.iter().any(|x| simple(x) == a)
        })
        && any(&r.name_suffix, &|x| s.name.ends_with(x))
        && any(&r.name_prefix, &|x| s.name.starts_with(x))
        && any(&r.name_contains, &|x| s.name.contains(x))
        && any(&r.path_contains, &|x| path.contains(x))
        && any(&r.package_contains, &|x| {
            pkg.as_deref().is_some_and(|p| p.contains(x))
        })
        && any(&r.uses, &|x| f.uses.contains(x))
}

fn is_dto_method(class: &str, m: &Symbol) -> bool {
    if m.kind == SymbolKind::Constructor || m.name == class || m.name == "constructor" {
        return true;
    }
    let n = m.name.as_str();
    let accessor = ["get", "set", "is"].iter().any(|p| {
        n.strip_prefix(p)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|c| c.is_uppercase())
    });
    accessor || matches!(n, "equals" | "hashCode" | "toString")
}

fn is_dto(f: &Facts) -> bool {
    f.sym.kind == SymbolKind::Class
        && !f.has_calls
        && f.members.iter().all(|m| is_dto_method(&f.sym.name, m))
}

fn strip_suffix<'a>(name: &'a str, suffixes: &[String]) -> &'a str {
    for s in suffixes {
        if name.len() > s.len() && name.ends_with(s.as_str()) {
            return &name[..name.len() - s.len()];
        }
    }
    name
}

/// First non-skipped segment of `s` (split on `sep`) after `after`.
fn segment(s: &str, sep: char, after: Option<&str>, skip: &[String]) -> Option<String> {
    let rest = match after {
        Some(a) => &s[s.find(a)? + a.len()..],
        None => s,
    };
    rest.split(sep)
        .filter(|seg| !seg.is_empty())
        .find(|seg| !skip.iter().any(|k| k.eq_ignore_ascii_case(seg)))
        .map(str::to_string)
}

fn domain_of(spec: &DomainSpec, s: &Symbol) -> String {
    let name_root = || strip_suffix(&s.name, &spec.strip_suffixes).to_string();
    let after = spec.after.as_deref();
    let raw = match spec.strategy {
        Strategy::NameRoot => Some(name_root()),
        Strategy::PackageSegment => s
            .package
            .as_deref()
            .and_then(|p| segment(&format!("{p}."), '.', after, &spec.skip_segments)),
        Strategy::PathSegment => s
            .file
            .rsplit_once('/')
            .and_then(|(dir, _)| segment(&format!("{dir}/"), '/', after, &spec.skip_segments)),
    };
    let raw = raw.unwrap_or_else(name_root).to_lowercase();
    if raw.is_empty() {
        return spec.fallback.clone();
    }
    for (canon, names) in &spec.aliases {
        if canon.to_lowercase() == raw || names.iter().any(|n| n.to_lowercase() == raw) {
            return canon.to_lowercase();
        }
    }
    raw
}

/// One placement per top-level symbol (kind Class/Interface/Enum/Function).
pub fn classify(graph: &Graph, rules: &RuleSet) -> Classification {
    let by_id: HashMap<&str, &Symbol> = graph.symbols.iter().map(|s| (s.id.as_str(), s)).collect();
    let top_of = |id: &str| -> Option<&str> {
        let mut cur = *by_id.get(id)?;
        for _ in 0..=graph.symbols.len() {
            match cur.container.as_deref().and_then(|c| by_id.get(c)) {
                Some(p) => cur = p,
                None => return Some(cur.id.as_str()),
            }
        }
        None
    };

    let mut facts: BTreeMap<&str, Facts> = BTreeMap::new();
    for s in graph
        .symbols
        .iter()
        .filter(|s| s.container.is_none() && is_top_kind(s.kind))
    {
        facts.insert(
            s.id.as_str(),
            Facts {
                sym: s,
                uses: HashSet::new(),
                members: vec![],
                has_calls: false,
            },
        );
    }
    for s in &graph.symbols {
        if matches!(s.kind, SymbolKind::Method | SymbolKind::Constructor)
            && let Some(f) = s.container.as_deref().and_then(|c| facts.get_mut(c))
        {
            f.members.push(s);
        }
    }
    for e in &graph.edges {
        let Some(top) = top_of(&e.from) else { continue };
        let Some(f) = facts.get_mut(top) else {
            continue;
        };
        match e.kind {
            EdgeKind::Call => f.has_calls = true,
            EdgeKind::Uses => {
                if let Some(x) = e.to.strip_prefix("ext:") {
                    f.uses.insert(x);
                } else if let Some(t) = by_id.get(e.to.as_str()) {
                    f.uses.insert(t.name.as_str());
                }
            }
            EdgeKind::Extends | EdgeKind::Implements => {}
        }
    }

    let default_spec = rules.domain.clone().unwrap_or_default();
    let placements = facts
        .into_iter()
        .map(|(id, f)| {
            let hit = rules.rules.iter().find(|r| rule_matches(r, &f));
            let layer = match hit {
                Some(r) => r.layer,
                None if is_dto(&f) => Layer::Model,
                None => Layer::Unknown,
            };
            let spec = hit.and_then(|r| r.domain.as_ref()).unwrap_or(&default_spec);
            let domain = domain_of(spec, f.sym);
            (id.to_string(), Placement { layer, domain })
        })
        .collect();
    Classification { placements }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Edge, Range};

    const R: Range = Range {
        start_line: 0,
        start_col: 0,
        end_line: 1,
        end_col: 0,
    };

    fn sym(id: &str, kind: SymbolKind, name: &str, file: &str, pkg: Option<&str>) -> Symbol {
        Symbol {
            id: id.into(),
            kind,
            name: name.into(),
            container: None,
            file: file.into(),
            range: R,
            name_range: R,
            annotations: vec![],
            supertypes: vec![],
            package: pkg.map(Into::into),
        }
    }

    fn java(name: &str, kind: SymbolKind, pkg: &str) -> Symbol {
        let file = format!("{}/{name}.java", pkg.replace('.', "/"));
        sym(&format!("{pkg}.{name}"), kind, name, &file, Some(pkg))
    }

    fn method(owner: &Symbol, name: &str) -> Symbol {
        let mut m = sym(
            &format!("{}#{name}", owner.id),
            SymbolKind::Method,
            name,
            &owner.file,
            owner.package.as_deref(),
        );
        m.container = Some(owner.id.clone());
        m
    }

    fn edge(from: &str, to: &str, kind: EdgeKind) -> Edge {
        Edge {
            from: from.into(),
            to: to.into(),
            kind,
        }
    }

    fn graph(lang: Language, symbols: Vec<Symbol>, edges: Vec<Edge>) -> Graph {
        Graph {
            root: ".".into(),
            language: lang,
            symbols,
            edges,
        }
    }

    fn p(c: &Classification, id: &str) -> (Layer, String) {
        let x = &c.placements[id];
        (x.layer, x.domain.clone())
    }

    #[test]
    fn jpetstore_like() {
        let base = "org.mybatis.jpetstore";
        let mut syms = vec![];
        for n in ["Account", "Catalog", "Cart", "Order"] {
            let mut s = java(
                &format!("{n}ActionBean"),
                SymbolKind::Class,
                &format!("{base}.web.actions"),
            );
            s.supertypes = vec!["AbstractActionBean".into()];
            syms.push(s);
        }
        for n in ["Account", "Catalog", "Order"] {
            let s = java(
                &format!("{n}Service"),
                SymbolKind::Class,
                &format!("{base}.service"),
            );
            syms.push(method(&s, "doWork"));
            syms.push(s);
        }
        for n in [
            "Account", "Category", "Item", "LineItem", "Order", "Product", "Sequence",
        ] {
            syms.push(java(
                &format!("{n}Mapper"),
                SymbolKind::Interface,
                &format!("{base}.mapper"),
            ));
        }
        for n in [
            "Account", "Cart", "CartItem", "Category", "Item", "LineItem", "Order", "Product",
            "Sequence",
        ] {
            syms.push(java(n, SymbolKind::Class, &format!("{base}.domain")));
        }
        let mut stripes = java("BaseBean", SymbolKind::Class, &format!("{base}.web"));
        stripes.supertypes = vec!["ActionBean".into()];
        syms.push(stripes);
        let g = graph(Language::Java, syms, vec![]);
        let c = classify(&g, &RuleSet::builtin(Language::Java).unwrap());
        let id = |n: &str, sub: &str| format!("{base}.{sub}.{n}");
        assert_eq!(
            p(&c, &id("AccountActionBean", "web.actions")),
            (Layer::Entry, "account".into())
        );
        assert_eq!(
            p(&c, &id("CartActionBean", "web.actions")),
            (Layer::Entry, "cart".into())
        );
        assert_eq!(
            p(&c, &id("CatalogActionBean", "web.actions")),
            (Layer::Entry, "catalog".into())
        );
        assert_eq!(
            p(&c, &id("OrderService", "service")),
            (Layer::Business, "order".into())
        );
        assert_eq!(
            p(&c, &id("CatalogService", "service")),
            (Layer::Business, "catalog".into())
        );
        assert_eq!(
            p(&c, &id("LineItemMapper", "mapper")),
            (Layer::Persistence, "order".into())
        );
        assert_eq!(
            p(&c, &id("ItemMapper", "mapper")),
            (Layer::Persistence, "catalog".into())
        );
        assert_eq!(
            p(&c, &id("CartItem", "domain")),
            (Layer::Model, "cart".into())
        );
        assert_eq!(
            p(&c, &id("Sequence", "domain")),
            (Layer::Model, "order".into())
        );
        assert_eq!(
            p(&c, &id("Category", "domain")),
            (Layer::Model, "catalog".into())
        );
        assert_eq!(p(&c, &id("BaseBean", "web")).0, Layer::Entry);
        // methods are not placed
        assert_eq!(c.placements.len(), 4 + 3 + 7 + 9 + 1);
    }

    #[test]
    fn angular_like() {
        let ts = |name: &str, file: &str, kind, ann: &[&str]| {
            let mut s = sym(
                &format!("{file}#{name}"),
                kind,
                name,
                file,
                file.rsplit_once('/').map(|x| x.0),
            );
            s.annotations = ann.iter().map(|a| a.to_string()).collect();
            s
        };
        let cmp = ts(
            "CartComponent",
            "src/app/features/cart/cart.component.ts",
            SymbolKind::Class,
            &["Component"],
        );
        let api = ts(
            "CartApiService",
            "src/app/features/cart/cart-api.service.ts",
            SymbolKind::Class,
            &["Injectable"],
        );
        let store = ts(
            "CartStore",
            "src/app/features/cart/cart.store.ts",
            SymbolKind::Class,
            &["Injectable"],
        );
        let biz = ts(
            "PricingService",
            "src/app/core/pricing.service.ts",
            SymbolKind::Class,
            &["Injectable"],
        );
        let model = ts(
            "Product",
            "src/app/features/catalog/models/product.ts",
            SymbolKind::Interface,
            &[],
        );
        let guard = ts(
            "AuthGuard",
            "src/app/core/auth.guard.ts",
            SymbolKind::Class,
            &["Injectable"],
        );
        let edges = vec![edge(&api.id, "ext:HttpClient", EdgeKind::Uses)];
        let ids: Vec<String> = [&cmp, &api, &store, &biz, &model, &guard]
            .iter()
            .map(|s| s.id.clone())
            .collect();
        let g = graph(
            Language::TypeScript,
            vec![cmp, api, store, biz, model, guard],
            edges,
        );
        let c = classify(&g, &RuleSet::builtin(Language::TypeScript).unwrap());
        assert_eq!(p(&c, &ids[0]), (Layer::Ui, "cart".into()));
        assert_eq!(p(&c, &ids[1]), (Layer::ApiClient, "cart".into()));
        assert_eq!(p(&c, &ids[2]), (Layer::State, "cart".into()));
        assert_eq!(p(&c, &ids[3]), (Layer::Business, "pricing".into()));
        assert_eq!(p(&c, &ids[4]), (Layer::Model, "catalog".into()));
        assert_eq!(p(&c, &ids[5]).0, Layer::Util);
    }

    #[test]
    fn dto_fallback_and_unknown() {
        let pkg = "a.b";
        let dto = java("Thing", SymbolKind::Class, pkg);
        let ms = vec![
            method(&dto, "getX"),
            method(&dto, "setX"),
            method(&dto, "isY"),
            method(&dto, "hashCode"),
        ];
        let worker = java("Worker", SymbolKind::Class, pkg);
        let wm = method(&worker, "run");
        let caller = java("Caller", SymbolKind::Class, pkg);
        let cm = method(&caller, "getX");
        let edges = vec![edge(&cm.id, "a.b.Thing#getX", EdgeKind::Call)];
        let (d, w, k) = (dto.id.clone(), worker.id.clone(), caller.id.clone());
        let mut syms = vec![dto, worker, wm, caller, cm];
        syms.extend(ms);
        let c = classify(&graph(Language::Java, syms, edges), &RuleSet::default());
        assert_eq!(p(&c, &d), (Layer::Model, "thing".into()));
        assert_eq!(p(&c, &w).0, Layer::Unknown);
        assert_eq!(p(&c, &k).0, Layer::Unknown);
    }

    #[test]
    fn first_match_wins_and_merge_priority() {
        let mut s = java("FooService", SymbolKind::Class, "a.service");
        s.annotations = vec!["Service".into(), "Controller".into()];
        let g = graph(Language::Java, vec![s], vec![]);
        let rs = RuleSet::from_toml_str(
            "[[rule]]\nlayer=\"entry\"\nany_annotation=[\"Controller\"]\n[[rule]]\nlayer=\"business\"\nany_annotation=[\"Service\"]\n",
        )
        .unwrap();
        assert_eq!(
            classify(&g, &rs).placements["a.service.FooService"].layer,
            Layer::Entry
        );
        let over =
            RuleSet::from_toml_str("[[rule]]\nlayer=\"util\"\nany_annotation=[\"Service\"]\n")
                .unwrap();
        let merged = rs.merge(over);
        assert_eq!(
            classify(&g, &merged).placements["a.service.FooService"].layer,
            Layer::Util
        );
    }

    #[test]
    fn alias_and_strip_suffixes() {
        let rs = RuleSet::from_toml_str(
            r#"
[[rule]]
layer = "persistence"
name_suffix = ["Mapper"]
[domain]
strip_suffixes = ["Mapper"]
aliases = { order = ["LineItem"] }
"#,
        )
        .unwrap();
        let g = graph(
            Language::Java,
            vec![
                java("LineItemMapper", SymbolKind::Interface, "x"),
                java("Mapper", SymbolKind::Interface, "x"),
            ],
            vec![],
        );
        let c = classify(&g, &rs);
        assert_eq!(
            p(&c, "x.LineItemMapper"),
            (Layer::Persistence, "order".into())
        );
        // suffix equal to whole name is not stripped
        assert_eq!(p(&c, "x.Mapper").1, "mapper");
    }

    #[test]
    fn uses_matches_project_symbols_and_rejects_bad_toml() {
        let mut a = java("Dao", SymbolKind::Class, "x");
        a.annotations = vec![];
        let t = java("SqlSession", SymbolKind::Interface, "y");
        let g = graph(
            Language::Java,
            vec![a, t],
            vec![edge("x.Dao", "y.SqlSession", EdgeKind::Uses)],
        );
        let c = classify(&g, &RuleSet::builtin(Language::Java).unwrap());
        assert_eq!(c.placements["x.Dao"].layer, Layer::Persistence);
        assert!(RuleSet::from_toml_str("[[rule]]\nlayer=\"nope\"\n").is_err());
        assert!(RuleSet::from_toml_str("[[rule]]\nlayer=\"ui\"\nbogus=1\n").is_err());
    }

    #[test]
    fn builtin_rules_parse() {
        assert!(!RuleSet::builtin(Language::Java).unwrap().rules.is_empty());
        assert!(
            !RuleSet::builtin(Language::TypeScript)
                .unwrap()
                .rules
                .is_empty()
        );
    }

    fn ann(mut s: Symbol, anns: &[&str]) -> Symbol {
        s.annotations = anns.iter().map(|a| a.to_string()).collect();
        s
    }

    fn one(rules: &str, s: Symbol) -> Placement {
        let id = s.id.clone();
        let g = graph(Language::Java, vec![s], vec![]);
        classify(&g, &RuleSet::from_toml_str(rules).unwrap()).placements[&id].clone()
    }

    #[test]
    fn jpetstore_spring_mvc() {
        let base = "org.mybatis.jpetstore";
        let mut syms = vec![];
        for n in ["Account", "Cart", "Catalog", "Order", "Root"] {
            syms.push(ann(
                java(
                    &format!("{n}Controller"),
                    SymbolKind::Class,
                    &format!("{base}.web.controllers"),
                ),
                &["Controller", "RequestMapping"],
            ));
        }
        for n in ["Account", "Catalog", "Order"] {
            syms.push(ann(
                java(
                    &format!("{n}Service"),
                    SymbolKind::Class,
                    &format!("{base}.service"),
                ),
                &["Service"],
            ));
        }
        for n in [
            "Account", "Category", "Item", "LineItem", "Order", "Product", "Sequence",
        ] {
            syms.push(java(
                &format!("{n}Mapper"),
                SymbolKind::Interface,
                &format!("{base}.mapper"),
            ));
        }
        for n in [
            "Account", "Cart", "CartItem", "Category", "Item", "LineItem", "Order", "Product",
            "Sequence",
        ] {
            syms.push(java(n, SymbolKind::Class, &format!("{base}.domain")));
        }
        let mut l = ann(
            java(
                "StaxFactoryInitializationListener",
                SymbolKind::Class,
                &format!("{base}.web.configuration"),
            ),
            &["WebListener"],
        );
        l.supertypes = vec!["ServletContextListener".into()];
        syms.push(l);
        let c = classify(
            &graph(Language::Java, syms, vec![]),
            &RuleSet::builtin(Language::Java).unwrap(),
        );
        let id = |sub: &str, n: &str| format!("{base}.{sub}.{n}");
        for (n, d) in [
            ("Account", "account"),
            ("Cart", "cart"),
            ("Catalog", "catalog"),
            ("Order", "order"),
        ] {
            assert_eq!(
                p(&c, &id("web.controllers", &format!("{n}Controller"))),
                (Layer::Entry, d.into())
            );
        }
        assert_eq!(
            p(&c, &id("web.controllers", "RootController")).0,
            Layer::Entry
        );
        assert_eq!(
            p(&c, &id("service", "OrderService")),
            (Layer::Business, "order".into())
        );
        assert_eq!(
            p(&c, &id("service", "AccountService")),
            (Layer::Business, "account".into())
        );
        assert_eq!(
            p(&c, &id("mapper", "SequenceMapper")),
            (Layer::Persistence, "order".into())
        );
        assert_eq!(
            p(&c, &id("mapper", "ProductMapper")),
            (Layer::Persistence, "catalog".into())
        );
        assert_eq!(
            p(&c, &id("domain", "CartItem")),
            (Layer::Model, "cart".into())
        );
        assert_eq!(
            p(&c, &id("domain", "LineItem")),
            (Layer::Model, "order".into())
        );
        assert_eq!(
            p(&c, &id("domain", "Item")),
            (Layer::Model, "catalog".into())
        );
        assert_eq!(
            p(
                &c,
                &id("web.configuration", "StaxFactoryInitializationListener")
            )
            .0,
            Layer::Util
        );
    }

    #[test]
    fn mapper_annotation_and_component_ordering() {
        let rs = RuleSet::builtin(Language::Java).unwrap();
        let g = graph(
            Language::Java,
            vec![
                ann(java("AMapper", SymbolKind::Interface, "x"), &["Mapper"]),
                ann(java("Convert", SymbolKind::Class, "x"), &["Mapper"]),
                ann(
                    java("Helper", SymbolKind::Class, "x.service"),
                    &["Component"],
                ),
                ann(java("Helper2", SymbolKind::Class, "x"), &["Component"]),
            ],
            vec![],
        );
        let c = classify(&g, &rs);
        assert_eq!(c.placements["x.AMapper"].layer, Layer::Persistence);
        assert_eq!(c.placements["x.Convert"].layer, Layer::Util);
        assert_eq!(c.placements["x.service.Helper"].layer, Layer::Business);
        assert_eq!(c.placements["x.Helper2"].layer, Layer::Util);
    }

    #[test]
    fn typescript_word_boundaries_nest_pipes() {
        let ts = |name: &str, file: &str, kind, anns: &[&str]| {
            ann(sym(&format!("{file}#{name}"), kind, name, file, None), anns)
        };
        let syms = vec![
            ts(
                "StatementService",
                "src/app/a/statement.service.ts",
                SymbolKind::Class,
                &["Injectable"],
            ),
            ts(
                "RestoreService",
                "src/app/a/restore.service.ts",
                SymbolKind::Class,
                &["Injectable"],
            ),
            ts(
                "AppStore",
                "src/app/a/app.store.ts",
                SymbolKind::Class,
                &["Injectable"],
            ),
            ts(
                "UsersController",
                "src/users/users.controller.ts",
                SymbolKind::Class,
                &["Controller"],
            ),
            ts(
                "UserEntity",
                "src/users/user.entity.ts",
                SymbolKind::Class,
                &["Entity"],
            ),
            ts(
                "MoneyPipe",
                "src/app/a/money.pipe.ts",
                SymbolKind::Class,
                &["Pipe"],
            ),
            ts(
                "HighlightDirective",
                "src/app/a/h.directive.ts",
                SymbolKind::Class,
                &["Directive"],
            ),
        ];
        let ids: Vec<String> = syms.iter().map(|s| s.id.clone()).collect();
        let g = graph(Language::TypeScript, syms, vec![]);
        let c = classify(&g, &RuleSet::builtin(Language::TypeScript).unwrap());
        let layers: Vec<Layer> = ids.iter().map(|i| c.placements[i].layer).collect();
        assert_eq!(
            layers,
            [
                Layer::Business,
                Layer::Business,
                Layer::State,
                Layer::Entry,
                Layer::Model,
                Layer::Util,
                Layer::Util
            ]
        );
        // no `src/app/` in path: name_root fallback
        assert_eq!(c.placements[&ids[3]].domain, "users");
    }

    #[test]
    fn matchers_and_semantics() {
        let rs = r#"
[[rule]]
layer = "ui"
name_prefix = ["Pre"]
[[rule]]
layer = "state"
name_contains = ["Mid"]
kind = ["enum"]
[[rule]]
layer = "util"
any_supertype = ["Base"]
name_suffix = ["Impl"]
"#;
        assert_eq!(
            one(rs, java("PreThing", SymbolKind::Class, "x")).layer,
            Layer::Ui
        );
        assert_eq!(
            one(rs, java("AMidB", SymbolKind::Enum, "x")).layer,
            Layer::State
        );
        // kind mismatch: no match -> unknown (enum with no methods would be Enum, not DTO)
        assert_eq!(
            one(rs, java("AMidB", SymbolKind::Class, "x")).layer,
            Layer::Model
        );
        let mut q = java("FooImpl", SymbolKind::Class, "x");
        q.supertypes = vec!["com.acme.Base".into()];
        assert_eq!(one(rs, q).layer, Layer::Util);
        // AND: supertype ok, suffix missing
        let mut q = java("Foo", SymbolKind::Interface, "x");
        q.supertypes = vec!["Base".into()];
        assert_eq!(one(rs, q).layer, Layer::Unknown);
    }

    #[test]
    fn package_segment_strategy() {
        let rs = r#"
[[rule]]
layer = "business"
[domain]
strategy = "package_segment"
after = "com.acme."
skip_segments = ["Shared"]
fallback = "misc"
aliases = { billing = ["invoices"] }
"#;
        assert_eq!(
            one(
                rs,
                java("A", SymbolKind::Class, "com.acme.shared.invoices.impl")
            )
            .domain,
            "billing"
        );
        assert_eq!(
            one(rs, java("Zed", SymbolKind::Class, "com.acme.core")).domain,
            "core"
        );
        // `after` absent from the package: falls back to the name root
        assert_eq!(
            one(rs, java("Zed", SymbolKind::Class, "other.pkg")).domain,
            "zed"
        );
        // no package at all
        let s = sym("Q", SymbolKind::Class, "", "Q.java", None);
        assert_eq!(one(rs, s).domain, "misc");
    }

    #[test]
    fn dto_edge_cases() {
        let rs = RuleSet::default();
        let g = |kind, methods: &[&str]| {
            let c = java("T", kind, "x");
            let mut v: Vec<Symbol> = methods.iter().map(|m| method(&c, m)).collect();
            v.push(c);
            classify(&graph(Language::Java, v, vec![]), &rs).placements["x.T"].layer
        };
        assert_eq!(g(SymbolKind::Class, &["getA", "setA", "isB"]), Layer::Model);
        assert_eq!(g(SymbolKind::Interface, &[]), Layer::Unknown);
        assert_eq!(g(SymbolKind::Enum, &[]), Layer::Unknown);
        assert_eq!(g(SymbolKind::Class, &["getA", "settle"]), Layer::Unknown);
        assert_eq!(g(SymbolKind::Class, &["issue"]), Layer::Unknown);
        assert_eq!(g(SymbolKind::Class, &["setup"]), Layer::Unknown);
        assert_eq!(g(SymbolKind::Class, &["compute"]), Layer::Unknown);
    }
}
