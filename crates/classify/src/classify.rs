//! Declarative layer/domain classification of top-level symbols.
//!
//! Rules live in TOML files (`rules/*.toml`). A rule matches when ALL of its
//! given matchers match; each matcher is "any of" its list. The first matching
//! rule (file order) decides the layer. Without a match, a structural fallback
//! applies (DTO-like class -> `model`, else `unknown`).

use crate::matrix::{id_index, top_level_indices};
use anyhow::Context;
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use structure_ir::ir::{EdgeKind, Graph, Symbol, SymbolKind};
use structure_ir::model::{Classification, DomainNode, Layer, Placement};

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
    /// Per symbol like `name_root` (or `path_segment` when `after` is set);
    /// project-wide, switches to feature domains when the code is
    /// package-by-feature (`app.billing.{controller,service}`, see
    /// `feature_modules`), else to module domains (package / directory) when
    /// at least 75% of the symbols using the default spec are alone in their
    /// domain. An explicit `name_root` never switches.
    Auto,
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
    /// Role prefixes removed from the name (after the suffix), only at a word
    /// boundary: `MyBatisArticle` -> `Article`, but `Defaulted` is kept.
    #[serde(default)]
    pub strip_prefixes: Vec<String>,
    /// Applies wherever the name root is used (the `name_root` strategy and the
    /// fallback of the segment strategies): `Comments` -> `Comment`,
    /// `Categories` -> `Category`, `Boxes` -> `Box`. Common singular nouns in
    /// `s` (`News`, `Series`, `Status`, `Address`, ...) are kept.
    #[serde(default)]
    pub singularize: bool,
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
            strip_prefixes: vec![],
            singularize: false,
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

    /// Rules from embedded TOML text; `name` labels parse errors.
    pub fn from_embedded(toml: &str, name: &str) -> anyhow::Result<RuleSet> {
        Self::from_toml_str(toml).context(name.to_string())
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

fn rule_matches(r: &Rule, f: &Facts, path: &str, pkg: Option<&str>) -> bool {
    let s = f.sym;
    let any = |list: &[String], pred: &dyn Fn(&str) -> bool| {
        list.is_empty() || list.iter().any(|x| pred(x))
    };
    (r.kind.is_empty() || r.kind.contains(&s.kind))
        && any(&r.any_annotation, &|a| s.annotations.iter().any(|x| x == a))
        && any(&r.any_supertype, &|a| {
            s.supertypes.iter().any(|x| simple(x) == a)
        })
        && any(&r.name_suffix, &|x| s.name.ends_with(x))
        && any(&r.name_prefix, &|x| s.name.starts_with(x))
        && any(&r.name_contains, &|x| s.name.contains(x))
        && any(&r.path_contains, &|x| path.contains(x))
        && any(&r.package_contains, &|x| pkg.is_some_and(|p| p.contains(x)))
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

fn strip_prefix<'a>(name: &'a str, prefixes: &[String]) -> &'a str {
    for p in prefixes {
        if let Some(rest) = name.strip_prefix(p.as_str())
            && rest.chars().next().is_some_and(char::is_uppercase)
        {
            return rest;
        }
    }
    name
}

/// Singular nouns that end in `s`; matched on the last camel-case word.
const SINGULAR_S: [&str; 11] = [
    "news", "alias", "canvas", "atlas", "bias", "series", "lens", "gas", "status", "address",
    "analysis",
];

/// Naive English singular of the last word: `ies` -> `y` (stem > 2 letters),
/// `es` after `x`/`ch`/`sh`, else a trailing `s` (not after `s`, `u`, `i`).
fn singular(name: &str) -> String {
    let lower = name.to_lowercase();
    let at_word = |w: &str| {
        lower.ends_with(w)
            && (name.len() == w.len()
                || name[name.len() - w.len()..]
                    .chars()
                    .next()
                    .is_some_and(char::is_uppercase))
    };
    if SINGULAR_S.iter().any(|w| at_word(w)) {
        return name.to_string();
    }
    let cut = |n: usize| name[..name.len() - n].to_string();
    if lower.ends_with("ies") && name.len() - 3 > 2 {
        return format!("{}y", cut(3));
    }
    if ["xes", "ches", "shes"].iter().any(|e| lower.ends_with(e)) {
        return cut(2);
    }
    match lower.strip_suffix('s').and_then(|r| r.chars().last()) {
        Some(prev) if name.len() > 3 && !matches!(prev, 's' | 'u' | 'i') => cut(1),
        _ => name.to_string(),
    }
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
    let name_root = || {
        let root = strip_suffix(&s.name, &spec.strip_suffixes);
        let root = strip_prefix(root, &spec.strip_prefixes);
        if spec.singularize {
            singular(root)
        } else {
            root.to_string()
        }
    };
    let after = spec.after.as_deref();
    let raw = match spec.strategy {
        Strategy::NameRoot => Some(name_root()),
        Strategy::Auto if after.is_none() => Some(name_root()),
        Strategy::PackageSegment => s
            .package
            .as_deref()
            .and_then(|p| segment(&format!("{p}."), '.', after, &spec.skip_segments)),
        Strategy::PathSegment | Strategy::Auto => s
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
pub fn classify(graph: &Graph, rules: &RuleSet, module_separator: char) -> Classification {
    let by_id = id_index(graph);

    let mut facts_by_id: BTreeMap<&str, Facts> = BTreeMap::new();
    for s in graph
        .symbols
        .iter()
        .filter(|s| s.container.is_none() && is_top_kind(s.kind))
    {
        facts_by_id.insert(
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
            && let Some(f) = s.container.as_deref().and_then(|c| facts_by_id.get_mut(c))
        {
            f.members.push(s);
        }
    }
    // Facts in id order, addressed by slot from here on.
    let (fact_ids, mut facts): (Vec<&str>, Vec<Facts>) = facts_by_id.into_iter().unzip();
    let slot_of_id: HashMap<&str, usize> = fact_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect();
    // Facts slot of each symbol's top-level ancestor (computed once).
    let slot_of_symbol: Vec<Option<usize>> = top_level_indices(graph, &by_id)
        .into_iter()
        .map(|t| t.and_then(|t| slot_of_id.get(graph.symbols[t].id.as_str()).copied()))
        .collect();
    for e in &graph.edges {
        let Some(&from) = by_id.get(e.from.as_str()) else {
            continue;
        };
        let Some(slot) = slot_of_symbol[from] else {
            continue;
        };
        let f = &mut facts[slot];
        match e.kind {
            EdgeKind::Call => f.has_calls = true,
            EdgeKind::Uses => {
                if let Some(x) = e.to.strip_prefix("ext:") {
                    f.uses.insert(x);
                } else if let Some(&t) = by_id.get(e.to.as_str()) {
                    f.uses.insert(graph.symbols[t].name.as_str());
                }
            }
            EdgeKind::Extends | EdgeKind::Implements => {}
        }
    }

    let default_spec = rules.domain.clone().unwrap_or_default();
    // First matching rule per fact (matching is independent per fact).
    let hits: Vec<Option<&Rule>> = facts
        .par_iter()
        .map(|f| {
            let path = format!("/{}", f.sym.file);
            let pkg = f.sym.package.as_deref().map(|p| format!(".{p}."));
            rules
                .rules
                .iter()
                .find(|r| rule_matches(r, f, &path, pkg.as_deref()))
        })
        .collect();
    let mut placements: BTreeMap<String, Placement> = facts
        .iter()
        .zip(&hits)
        .zip(&fact_ids)
        .map(|((f, hit), id)| {
            let layer = match hit {
                Some(r) => r.layer,
                None if is_dto(f) => Layer::Model,
                None => Layer::Unknown,
            };
            let spec = hit.and_then(|r| r.domain.as_ref()).unwrap_or(&default_spec);
            let domain = domain_of(spec, f.sym);
            (id.to_string(), Placement { layer, domain })
        })
        .collect();
    let mut domain_tree = None;
    if default_spec.strategy == Strategy::Auto {
        let auto_ids: Vec<&str> = hits
            .iter()
            .zip(&fact_ids)
            .filter(|(hit, _)| hit.is_none_or(|r| r.domain.is_none()))
            .map(|(_, id)| *id)
            .collect();
        let mut per_domain: HashMap<&str, usize> = HashMap::new();
        for id in &auto_ids {
            if let Some(p) = placements.get(*id) {
                *per_domain.entry(p.domain.as_str()).or_default() += 1;
            }
        }
        let alone = per_domain.values().filter(|n| **n == 1).count();
        let sep = module_separator;
        let members: Vec<(&str, Vec<String>)> = auto_ids
            .iter()
            .filter_map(|id| {
                slot_of_id
                    .get(id)
                    .map(|&i| (*id, module_path(facts[i].sym, sep)))
            })
            .collect();
        let name_root = |id: &str| placements.get(id).map(|p| p.domain.clone());
        let features = feature_modules(&members, &name_root);
        let fragmented = !auto_ids.is_empty() && alone * 4 >= auto_ids.len() * 3;
        if features.is_some() || fragmented {
            let members = features.unwrap_or(members);
            let (domains, tree) = module_domains(&members, sep, &default_spec.fallback);
            for (id, d) in domains {
                if let Some(p) = placements.get_mut(id) {
                    p.domain = d;
                }
            }
            domain_tree = Some(tree);
        }
    }
    Classification {
        placements,
        domain_tree,
    }
}

/// Module of a symbol: its package (Java) or file directory (TS).
fn module_path(s: &Symbol, sep: char) -> Vec<String> {
    // Java: the package (a class without one is in the default, empty module);
    // TS: the file's directory. Split on that language's separator only.
    let raw = match sep {
        '.' => s.package.as_deref().unwrap_or(""),
        _ => s.file.rsplit_once('/').map(|(d, _)| d).unwrap_or(""),
    };
    raw.split(sep)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect()
}

/// A module's last segment found under at least this many distinct parents is
/// a role (`controller`, `service`, `mapper`...), not a domain.
const MIN_ROLE_PARENTS: usize = 3;

/// Fewest features for feature mode: with fewer, a handful of layer packages
/// over recurring entity names (`web.article`, `repo.article`) looks the same.
const MIN_FEATURES: usize = 8;

/// Name key that ignores case, separators and plural `s`/`x` anywhere:
/// `bordereaux_preparatoires` and `BordereauPreparatoire` share one.
fn loose_key(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .filter(|c| !matches!(c, 's' | 'x'))
        .collect()
}

/// `m` without its trailing role segments (never empty when `m` is not).
fn strip_roles<'m>(m: &'m [String], roles: &HashSet<&str>) -> &'m [String] {
    let mut m = m;
    while m.len() > 1 && roles.contains(m[m.len() - 1].as_str()) {
        m = &m[..m.len() - 1];
    }
    m
}

/// Package-by-feature detection (`app.billing.{controller,service,mapper}`).
///
/// Role segments are module last segments found under `MIN_ROLE_PARENTS`
/// parents or more. A feature is a module minus its trailing role segments,
/// unless it is a container (ancestor of two or more other features). Applies
/// only with at least `MIN_FEATURES` features and twice as many features as
/// roles, which keeps layer-then-feature code (`core.article`, `api.article`)
/// out. Returns each
/// member's domain module: its feature, else the feature its name root
/// matches (`entity.Invoice` -> `billing.invoices`), else its own module.
fn feature_modules<'a>(
    members: &[(&'a str, Vec<String>)],
    name_root: &dyn Fn(&str) -> Option<String>,
) -> Option<Vec<(&'a str, Vec<String>)>> {
    let modules: BTreeSet<&[String]> = members.iter().map(|(_, m)| m.as_slice()).collect();
    let mut parents: HashMap<&str, BTreeSet<&[String]>> = HashMap::new();
    for m in &modules {
        if let Some((last, parent)) = m.split_last()
            && !parent.is_empty()
        {
            parents.entry(last.as_str()).or_default().insert(parent);
        }
    }
    let roles: HashSet<&str> = parents
        .iter()
        .filter(|(_, p)| p.len() >= MIN_ROLE_PARENTS)
        .map(|(s, _)| *s)
        .collect();
    if roles.is_empty() {
        return None;
    }
    let candidates: BTreeSet<&[String]> = modules
        .iter()
        .filter_map(|m| {
            let f = strip_roles(m, &roles);
            (f.len() < m.len()).then_some(f)
        })
        .collect();
    let mut below: HashMap<&[String], usize> = HashMap::new();
    for c in &candidates {
        for k in 1..c.len() {
            *below.entry(&c[..k]).or_default() += 1;
        }
    }
    let features: BTreeSet<&[String]> = candidates
        .into_iter()
        .filter(|c| below.get(c).copied().unwrap_or(0) < 2)
        .collect();
    if features.len() < MIN_FEATURES || features.len() < 2 * roles.len() {
        return None;
    }
    let mut by_key: HashMap<String, Option<&[String]>> = HashMap::new();
    for f in &features {
        if let Some(last) = f.last() {
            let key = loose_key(last);
            by_key
                .entry(key)
                .and_modify(|e| *e = None)
                .or_insert(Some(f));
        }
    }
    let out = members
        .iter()
        .map(|(id, m)| {
            let stripped = strip_roles(m, &roles);
            if features.contains(stripped) {
                return (*id, stripped.to_vec());
            }
            let joined = name_root(id)
                .map(|r| loose_key(&r))
                .filter(|k| k.len() >= 3)
                .and_then(|k| by_key.get(&k).copied().flatten());
            (*id, joined.unwrap_or(m.as_slice()).to_vec())
        })
        .collect();
    Some(out)
}

#[derive(Default)]
struct Trie {
    own: usize,
    domain: Option<String>,
    children: BTreeMap<String, Trie>,
}

impl Trie {
    fn total(&self) -> usize {
        self.own + self.children.values().map(Trie::total).sum::<usize>()
    }
}

fn convert(label: String, t: &Trie, sep: char) -> DomainNode {
    // Collapse a chain of single-child nodes without own classes.
    if t.own == 0
        && t.children.len() == 1
        && let Some((l, c)) = t.children.iter().next()
    {
        return convert(format!("{label}{sep}{l}"), c, sep);
    }
    let mut children: Vec<DomainNode> = vec![];
    if t.own > 0 && !t.children.is_empty() {
        let last = label.rsplit(['.', '/']).next().unwrap_or(&label);
        children.push(DomainNode {
            label: last.to_string(),
            domain: t.domain.clone(),
            size: t.own,
            children: vec![],
        });
    }
    children.extend(t.children.iter().map(|(l, c)| convert(l.clone(), c, sep)));
    DomainNode {
        domain: if children.is_empty() {
            t.domain.clone()
        } else {
            None
        },
        label,
        size: t.total(),
        children,
    }
}

/// Assigns each member its module domain (shortest unique suffix of the module
/// path, lower-cased) and builds the collapsed module tree.
fn module_domains<'a>(
    members: &[(&'a str, Vec<String>)],
    sep: char,
    fallback: &str,
) -> (Vec<(&'a str, String)>, Vec<DomainNode>) {
    let modules: BTreeSet<&Vec<String>> = members.iter().map(|(_, m)| m).collect();
    // Lower-cased, since domains are: `a.Utils` and `b.utils` must not collide.
    let suffix = |m: &[String], k: usize| {
        m[m.len().saturating_sub(k)..]
            .join(&sep.to_string())
            .to_lowercase()
    };
    // For each suffix length, how many distinct modules share each suffix.
    let longest = modules.iter().map(|m| m.len()).max().unwrap_or(0);
    let shared: Vec<HashMap<String, usize>> = (1..=longest)
        .map(|k| {
            let mut c: HashMap<String, usize> = HashMap::new();
            for m in &modules {
                *c.entry(suffix(m, k)).or_default() += 1;
            }
            c
        })
        .collect();
    let name_of = |m: &Vec<String>| -> String {
        if m.is_empty() {
            return fallback.to_string();
        }
        let k = (1..=m.len())
            .find(|k| shared[k - 1].get(&suffix(m, *k)).is_some_and(|n| *n == 1))
            .unwrap_or(m.len());
        suffix(m, k)
    };
    let names: HashMap<&Vec<String>, String> = modules.iter().map(|m| (*m, name_of(m))).collect();
    let mut root = Trie::default();
    let mut out = vec![];
    for (id, m) in members {
        let name = names
            .get(m)
            .cloned()
            .unwrap_or_else(|| fallback.to_string());
        let mut node = &mut root;
        for seg in m {
            node = node.children.entry(seg.clone()).or_default();
        }
        node.own += 1;
        node.domain = Some(name.clone());
        out.push((*id, name));
    }
    let mut tree: Vec<DomainNode> = root
        .children
        .iter()
        .map(|(l, c)| convert(l.clone(), c, sep))
        .collect();
    if root.own > 0 {
        tree.insert(
            0,
            DomainNode {
                label: fallback.to_string(),
                domain: root.domain.clone(),
                size: root.own,
                children: vec![],
            },
        );
    }
    (out, tree)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin(lang: Language) -> anyhow::Result<RuleSet> {
        let (toml, name, _) = match lang {
            Language::Java => (
                include_str!("../../../rules/java.toml"),
                "rules/java.toml",
                '.',
            ),
            Language::TypeScript => (
                include_str!("../../../rules/typescript.toml"),
                "rules/typescript.toml",
                '/',
            ),
        };
        RuleSet::from_embedded(toml, name)
    }

    fn classify(graph: &Graph, rules: &RuleSet) -> Classification {
        let sep = match graph.language {
            Language::Java => '.',
            Language::TypeScript => '/',
        };
        super::classify(graph, rules, sep)
    }
    use structure_ir::ir::{Edge, Language, Range};

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
            http: vec![],
            http_calls: vec![],
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
        let c = classify(&g, &builtin(Language::Java).unwrap());
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
        let c = classify(&g, &builtin(Language::TypeScript).unwrap());
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
        let c = classify(&g, &builtin(Language::Java).unwrap());
        assert_eq!(c.placements["x.Dao"].layer, Layer::Persistence);
        assert!(RuleSet::from_toml_str("[[rule]]\nlayer=\"nope\"\n").is_err());
        assert!(RuleSet::from_toml_str("[[rule]]\nlayer=\"ui\"\nbogus=1\n").is_err());
    }

    #[test]
    fn builtin_rules_parse() {
        assert!(!builtin(Language::Java).unwrap().rules.is_empty());
        assert!(!builtin(Language::TypeScript).unwrap().rules.is_empty());
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
            &builtin(Language::Java).unwrap(),
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
        let rs = builtin(Language::Java).unwrap();
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
        let c = classify(&g, &builtin(Language::TypeScript).unwrap());
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
    fn spring_ddd_rules() {
        let rs = builtin(Language::Java).unwrap();
        let base = "io.acme";
        let mut with_logic = java(
            "Article",
            SymbolKind::Class,
            &format!("{base}.core.article"),
        );
        let work = method(&with_logic, "publish");
        with_logic.annotations = vec![];
        let syms = vec![
            with_logic,
            work,
            java(
                "ArticleRepository",
                SymbolKind::Interface,
                &format!("{base}.core.article"),
            ),
            java("OrderRepository", SymbolKind::Interface, "x.domain"),
            ann(
                java(
                    "ArticleDatafetcher",
                    SymbolKind::Class,
                    &format!("{base}.graphql"),
                ),
                &["DgsComponent"],
            ),
            java(
                "JwtService",
                SymbolKind::Interface,
                &format!("{base}.core.service"),
            ),
            java("Clock", SymbolKind::Class, &format!("{base}.core.service")),
            java("Encoder", SymbolKind::Class, &format!("{base}.core")),
            java("Score", SymbolKind::Enum, &format!("{base}.core.user")),
            java(
                "FooException",
                SymbolKind::Class,
                &format!("{base}.core.exception"),
            ),
            java(
                "StringUtils",
                SymbolKind::Class,
                &format!("{base}.core.util"),
            ),
            java(
                "StringUtil",
                SymbolKind::Class,
                &format!("{base}.core.util"),
            ),
            java("CoreConfig", SymbolKind::Class, &format!("{base}.core")),
            java(
                "CoreConfiguration",
                SymbolKind::Class,
                &format!("{base}.core"),
            ),
            ann(
                java("UserMutation", SymbolKind::Class, "io.spring.graphql"),
                &["DgsComponent"],
            ),
        ];
        let c = classify(&graph(Language::Java, syms, vec![]), &rs);
        let l = |id: &str| c.placements[id].layer;
        assert_eq!(l("io.acme.core.article.Article"), Layer::Model);
        assert_eq!(
            l("io.acme.core.article.ArticleRepository"),
            Layer::Persistence
        );
        // interface named *Repository wins over the `.domain.` model package
        assert_eq!(l("x.domain.OrderRepository"), Layer::Persistence);
        assert_eq!(l("io.acme.graphql.ArticleDatafetcher"), Layer::Entry);
        // `core.service` stays business; the core rule only takes classes
        assert_eq!(l("io.acme.core.service.JwtService"), Layer::Business);
        assert_eq!(l("io.acme.core.service.Clock"), Layer::Business);
        assert_eq!(l("io.acme.core.Encoder"), Layer::Model);
        assert_eq!(l("io.acme.core.user.Score"), Layer::Unknown);
        for n in [
            "exception.FooException",
            "util.StringUtils",
            "util.StringUtil",
            "CoreConfig",
            "CoreConfiguration",
        ] {
            assert_eq!(l(&format!("io.acme.core.{n}")), Layer::Util, "{n}");
        }
        assert_eq!(l("io.spring.graphql.UserMutation"), Layer::Entry);
    }

    #[test]
    fn role_suffixes_prefixes_and_plurals() {
        let rs = builtin(Language::Java).unwrap();
        let names = [
            ("ArticleQueryService", "article"),
            ("ArticleCommandService", "article"),
            ("UserReadService", "user"),
            ("CommentsApi", "comment"),
            ("ArticleApi", "article"),
            ("ArticleDatafetcher", "article"),
            ("UserMutation", "user"),
            ("MyBatisArticleRepository", "article"),
            ("DefaultJwtService", "jwt"),
            ("JpaOrderRepository", "order"),
            // not a word boundary / guarded plural / whole-name: kept
            ("DefaultsService", "default"),
            ("Defaulted", "defaulted"),
            ("Address", "address"),
            ("Status", "status"),
            ("Api", "api"),
            ("Is", "is"),
        ];
        let syms: Vec<Symbol> = names
            .iter()
            .map(|(n, _)| java(n, SymbolKind::Class, "x"))
            .collect();
        let c = classify(&graph(Language::Java, syms, vec![]), &rs);
        for (n, d) in names {
            assert_eq!(c.placements[&format!("x.{n}")].domain, d, "{n}");
        }
    }

    #[test]
    fn strip_prefix_and_singular_helpers() {
        let pre = vec!["MyBatis".to_string()];
        assert_eq!(strip_prefix("MyBatisFoo", &pre), "Foo");
        assert_eq!(strip_prefix("MyBatis", &pre), "MyBatis");
        assert_eq!(strip_prefix("MyBatisfoo", &pre), "MyBatisfoo");
        assert_eq!(singular("Comments"), "Comment");
        assert_eq!(singular("Class"), "Class");
        assert_eq!(singular("Bus"), "Bus");
        assert_eq!(singular("Ids"), "Ids");
        assert_eq!(singular("Id"), "Id");
        for (i, o) in [
            ("News", "News"),
            ("Alias", "Alias"),
            ("Series", "Series"),
            ("Status", "Status"),
            ("Address", "Address"),
            ("UserNews", "UserNews"),
            ("Categories", "Category"),
            ("Ies", "Ies"),
            ("Boxes", "Box"),
            ("Searches", "Search"),
            ("Dishes", "Dish"),
            ("Articles", "Article"),
            ("Comments", "Comment"),
        ] {
            assert_eq!(singular(i), o, "{i}");
        }
        // opt-in: default spec leaves plurals and prefixes alone
        let s = java("MyBatisUsers", SymbolKind::Class, "x");
        assert_eq!(domain_of(&DomainSpec::default(), &s), "mybatisusers");
    }

    #[test]
    fn user_domain_spec_without_new_fields_parses() {
        let rs = RuleSet::from_toml_str(
            "[domain]\nstrategy = \"name_root\"\nstrip_suffixes = [\"Service\"]\n",
        )
        .unwrap();
        let spec = rs.domain.unwrap();
        assert!(!spec.singularize);
        assert!(spec.strip_prefixes.is_empty());
        assert_eq!(
            domain_of(&spec, &java("Users", SymbolKind::Class, "x")),
            "users"
        );
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

    fn module_graph() -> Graph {
        let mut syms = vec![];
        for n in ["A", "B", "C"] {
            syms.push(java(n, SymbolKind::Class, "org.x.model.famix.traits"));
        }
        for n in ["D", "E"] {
            syms.push(java(n, SymbolKind::Class, "org.x.model.famix.entities"));
        }
        syms.push(java("F", SymbolKind::Class, "fr.y.ext"));
        syms.push(java("G", SymbolKind::Class, "fr.y.ext.utils"));
        syms.push(java("H", SymbolKind::Class, "fr.y.visitors.utils"));
        graph(Language::Java, syms, vec![])
    }

    fn leaves(n: &DomainNode, out: &mut Vec<String>) {
        match &n.domain {
            Some(d) => out.push(d.clone()),
            None => n.children.iter().for_each(|c| leaves(c, out)),
        }
    }

    #[test]
    fn auto_switches_to_module_domains_with_tree() {
        let g = module_graph();
        let mut rules = builtin(Language::Java).unwrap();
        // names are all distinct: every class alone -> modules
        let c = classify(&g, &rules);
        assert_eq!(p(&c, "org.x.model.famix.traits.A").1, "traits");
        assert_eq!(p(&c, "org.x.model.famix.entities.D").1, "entities");
        assert_eq!(p(&c, "fr.y.ext.F").1, "ext");
        assert_eq!(p(&c, "fr.y.ext.utils.G").1, "ext.utils");
        assert_eq!(p(&c, "fr.y.visitors.utils.H").1, "visitors.utils");
        let tree = c.domain_tree.clone().unwrap();
        let labels: Vec<_> = tree.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, ["fr.y", "org.x.model.famix"]);
        assert_eq!(tree[1].size, 5);
        assert_eq!(tree[1].children.len(), 2);
        let ext = &tree[0].children[0];
        assert_eq!(ext.label, "ext");
        assert_eq!(ext.children.len(), 2);
        assert_eq!(ext.children[0].domain.as_deref(), Some("ext"));
        let mut ls = vec![];
        tree.iter().for_each(|n| leaves(n, &mut ls));
        let mut doms: Vec<_> = c.placements.values().map(|p| p.domain.clone()).collect();
        doms.sort();
        doms.dedup();
        ls.sort();
        assert_eq!(ls, doms);

        // explicit name_root never switches
        rules.domain.as_mut().unwrap().strategy = Strategy::NameRoot;
        let c = classify(&g, &rules);
        assert!(c.domain_tree.is_none());
        assert_eq!(p(&c, "org.x.model.famix.traits.A").1, "a");
    }

    #[test]
    fn auto_keeps_name_roots_when_not_fragmented() {
        let mut syms = vec![];
        for pkg in ["a.one", "a.two", "a.three"] {
            syms.push(java("UserService", SymbolKind::Class, pkg));
        }
        let c = classify(
            &graph(Language::Java, syms, vec![]),
            &builtin(Language::Java).unwrap(),
        );
        assert!(c.domain_tree.is_none());
        assert_eq!(p(&c, "a.one.UserService").1, "user");
    }

    #[test]
    fn auto_switches_at_exactly_75_percent_alone() {
        let rules = builtin(Language::Java).unwrap();
        // `n` distinct names alone, plus two classes sharing one name root.
        let run = |alone: &[&str]| {
            let mut syms: Vec<Symbol> = alone
                .iter()
                .map(|n| java(n, SymbolKind::Class, &format!("p.{}", n.to_lowercase())))
                .collect();
            syms.push(java("Order", SymbolKind::Class, "p.q"));
            syms.push(java("OrderService", SymbolKind::Class, "p.r"));
            classify(&graph(Language::Java, syms, vec![]), &rules)
        };
        // 6 alone of 8 = 75% -> switch; 5 of 7 ~ 71% -> keep.
        assert!(run(&["A", "B", "C", "D", "E", "F"]).domain_tree.is_some());
        assert!(run(&["A", "B", "C", "D", "E"]).domain_tree.is_none());
    }

    #[test]
    fn module_domains_differing_only_by_case_stay_distinct() {
        let syms = vec![
            java("A", SymbolKind::Class, "a.Utils"),
            java("B", SymbolKind::Class, "b.utils"),
            java("C", SymbolKind::Class, ""),
        ];
        let mut c_sym = syms;
        c_sym[2].package = None;
        let c = classify(
            &graph(Language::Java, c_sym, vec![]),
            &builtin(Language::Java).unwrap(),
        );
        assert_eq!(p(&c, "a.Utils.A").1, "a.utils");
        assert_eq!(p(&c, "b.utils.B").1, "b.utils");
        // no package -> default module (fallback), not the file directory
        let tree = c.domain_tree.unwrap();
        assert_eq!(tree[0].domain.as_deref(), Some("misc"));
    }

    /// Several rules match: the earliest decides layer and domain spec, per
    /// symbol, whatever the rest.
    #[test]
    fn earliest_matching_rule_decides_per_symbol() {
        let rs = RuleSet::from_toml_str(
            "[[rule]]\nlayer=\"entry\"\nname_suffix=[\"Api\"]\n\
             [[rule]]\nlayer=\"business\"\nname_contains=[\"Foo\"]\n\
             [[rule]]\nlayer=\"util\"\npath_contains=[\"/a/\"]\n",
        )
        .unwrap();
        let syms = vec![
            java("FooApi", SymbolKind::Class, "a"),
            java("FooBar", SymbolKind::Class, "a"),
            java("Other", SymbolKind::Class, "a"),
            java("Other2", SymbolKind::Class, "z"),
        ];
        let c = classify(&graph(Language::Java, syms, vec![]), &rs);
        assert_eq!(p(&c, "a.FooApi").0, Layer::Entry);
        assert_eq!(p(&c, "a.FooBar").0, Layer::Business);
        assert_eq!(p(&c, "a.Other").0, Layer::Util);
        assert_ne!(p(&c, "z.Other2").0, Layer::Util);
    }

    fn mods(paths: &[&[&str]]) -> Vec<(&'static str, Vec<String>)> {
        paths
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let id: &'static str = Box::leak(format!("m{i}").into_boxed_str());
                (id, m.iter().map(|x| x.to_string()).collect())
            })
            .collect()
    }

    /// Shortest suffix that is unique among modules; a module that is a
    /// suffix of another keeps its full (shorter) path.
    #[test]
    fn module_domains_shared_unique_and_nested_suffixes() {
        let members = mods(&[&["x"], &["a", "x"], &["b", "y"], &["a", "b", "y"]]);
        let (out, _) = module_domains(&members, '.', "misc");
        let names: Vec<&str> = out.iter().map(|(_, d)| d.as_str()).collect();
        // `x` is also the last segment of `a.x`; `b.y` is a suffix of `a.b.y`.
        assert_eq!(names, ["x", "a.x", "b.y", "a.b.y"]);
        let members = mods(&[&["a", "x"], &["b", "x"], &["c", "y"]]);
        let (out, _) = module_domains(&members, '.', "misc");
        let names: Vec<&str> = out.iter().map(|(_, d)| d.as_str()).collect();
        assert_eq!(names, ["a.x", "b.x", "y"]);
    }

    /// `app.<feature>.{controller,service,mapper}` for eight features, plus
    /// package-by-layer `app.entity` / `app.repository` classes.
    fn feature_graph() -> Graph {
        let mut syms = vec![];
        for f in [
            "billing",
            "orders",
            "shipping",
            "users",
            "stock",
            "invoices",
            "payments",
            "bordereaux_preparatoires",
        ] {
            let cap = f[..1].to_uppercase() + &f[1..];
            syms.push(java(
                &format!("{cap}Ctl"),
                SymbolKind::Class,
                &format!("app.{f}.controller"),
            ));
            syms.push(java(
                &format!("{cap}Svc"),
                SymbolKind::Class,
                &format!("app.{f}.service"),
            ));
            syms.push(java(
                &format!("{cap}Conv"),
                SymbolKind::Class,
                &format!("app.{f}.mapper"),
            ));
        }
        syms.push(java("Order", SymbolKind::Class, "app.entity"));
        syms.push(java(
            "BordereauPreparatoire",
            SymbolKind::Class,
            "app.entity",
        ));
        syms.push(java("Audit", SymbolKind::Class, "app.entity"));
        syms.push(java("OrderRepository", SymbolKind::Class, "app.repository"));
        syms.push(java("App", SymbolKind::Class, "app"));
        graph(Language::Java, syms, vec![])
    }

    #[test]
    fn auto_groups_package_by_feature_code() {
        let c = classify(&feature_graph(), &builtin(Language::Java).unwrap());
        // the three layers of a feature share its domain
        for id in [
            "app.billing.controller.BillingCtl",
            "app.billing.service.BillingSvc",
            "app.billing.mapper.BillingConv",
        ] {
            assert_eq!(p(&c, id).1, "billing");
        }
        // layer packages join the feature named like the class (plural, `_`)
        assert_eq!(p(&c, "app.entity.Order").1, "orders");
        assert_eq!(p(&c, "app.repository.OrderRepository").1, "orders");
        assert_eq!(
            p(&c, "app.entity.BordereauPreparatoire").1,
            "bordereaux_preparatoires"
        );
        // no matching feature: own module
        assert_eq!(p(&c, "app.entity.Audit").1, "entity");
        assert_eq!(p(&c, "app.App").1, "app");
        let tree = c.domain_tree.clone().unwrap();
        let mut ls = vec![];
        tree.iter().for_each(|n| leaves(n, &mut ls));
        let mut doms: Vec<_> = c.placements.values().map(|p| p.domain.clone()).collect();
        doms.sort();
        doms.dedup();
        ls.sort();
        assert_eq!(ls, doms);
        assert_eq!(doms.len(), 10);

        // explicit name_root never switches
        let mut rules = builtin(Language::Java).unwrap();
        rules.domain.as_mut().unwrap().strategy = Strategy::NameRoot;
        let c = classify(&feature_graph(), &rules);
        assert!(c.domain_tree.is_none());
    }

    #[test]
    fn feature_mode_needs_more_features_than_roles() {
        // layer-then-feature: `article`/`user` recur under 3 layers, but there
        // are only 3 "features" (the layers) for 2 roles.
        let m = mods(&[
            &["app", "core", "article"],
            &["app", "core", "user"],
            &["app", "api", "article"],
            &["app", "api", "user"],
            &["app", "infra", "article"],
            &["app", "infra", "user"],
        ]);
        assert!(feature_modules(&m, &|_| None).is_none());
        // 4 layers x 2 entities: the ratio holds, but too few "features"
        let mut paths: Vec<Vec<&str>> = vec![];
        for layer in ["web", "service", "repo", "dto"] {
            for entity in ["article", "user"] {
                paths.push(vec!["app", layer, entity]);
            }
        }
        let refs: Vec<&[&str]> = paths.iter().map(|p| p.as_slice()).collect();
        assert!(feature_modules(&mods(&refs), &|_| None).is_none());
        // a single name under 3 layers
        let m = mods(&[
            &["app", "core", "article"],
            &["app", "api", "article"],
            &["app", "infra", "article"],
        ]);
        assert!(feature_modules(&m, &|_| None).is_none());
        // no role segment at all
        let m = mods(&[&["a", "x"], &["b", "y"]]);
        assert!(feature_modules(&m, &|_| None).is_none());
    }

    #[test]
    fn container_of_features_is_not_a_feature() {
        // `app.config` strips to `app`, which holds the other features.
        let mut paths: Vec<Vec<&str>> = vec![];
        for f in ["a", "b", "c", "d", "e", "f", "g", "h"] {
            paths.push(vec!["app", f, "config"]);
            paths.push(vec!["app", f, "web"]);
        }
        paths.push(vec!["app", "config"]);
        let refs: Vec<&[&str]> = paths.iter().map(|p| p.as_slice()).collect();
        let m = mods(&refs);
        let out = feature_modules(&m, &|_| None).unwrap();
        let last = out.last().unwrap();
        assert_eq!(last.1, ["app", "config"]);
        assert_eq!(out[0].1, ["app", "a"]);
    }

    #[test]
    fn loose_key_ignores_case_separators_and_plurals() {
        assert_eq!(
            loose_key("bordereaux_preparatoires"),
            loose_key("BordereauPreparatoire")
        );
        assert_eq!(
            loose_key("organismesregroupement"),
            loose_key("OrganismeRegroupement")
        );
        assert_ne!(loose_key("orders"), loose_key("users"));
    }
}
