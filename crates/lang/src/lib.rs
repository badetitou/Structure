//! Pure language registry: what differs per language and needs neither a
//! process nor a filesystem (source recognition, parsing of in-memory sources,
//! embedded default rules, module separator). Shared by `structure-core`
//! (which adds the native parts: marker detection, indexer, parsing from disk)
//! and `structure-wasm`.
//!
//! A new language needs a `Language` variant in `structure-ir`, a parser crate,
//! one [`Frontend`] implementation here with its `rules/<lang>.toml`, one
//! `static` and one line in [`all`] and one arm in [`frontend`]; then
//! regenerate the browser extension's copy of the extensions
//! (`app/ui/src/extension/sources.generated.ts`, checked by a `structure-wasm`
//! test that says how).
//!
//! The default rules are kept as plain strings: validating them needs
//! `structure-classify`, which only dev-depends on this crate (its tests take
//! the rules from here), so the check lives on the classify side.

use anyhow::bail;
use structure_ir::ir::{Graph, Language, ParseStat};

/// Result of parsing a project: its graph and the parser's own counters
/// (empty for most languages; Pharo reports its resolved/unresolved sends).
#[derive(Debug, Clone, PartialEq)]
pub struct ParseOutput {
    pub graph: Graph,
    pub stats: Vec<ParseStat>,
}

impl ParseOutput {
    /// A graph without stats.
    pub fn new(graph: Graph) -> Self {
        ParseOutput {
            graph,
            stats: Vec::new(),
        }
    }
}

impl From<structure_lang_pharo::Parsed> for ParseOutput {
    fn from(p: structure_lang_pharo::Parsed) -> Self {
        let stat = |name, value| ParseStat {
            group: "sends",
            name,
            value,
        };
        ParseOutput {
            graph: p.graph,
            stats: vec![
                stat("resolved", p.stats.resolved),
                stat("unresolved", p.stats.unresolved),
            ],
        }
    }
}

pub trait Frontend: Sync {
    /// Serialized identifier of the language.
    fn language(&self) -> Language;
    /// Source file extensions (without dot).
    fn extensions(&self) -> &'static [&'static str];
    /// Whether `path` (`/`-separated) counts as a source of this language
    /// when detecting the language of a set of paths ([`detect_by_paths`]).
    fn is_source(&self, path: &str) -> bool;
    /// Parse in-memory `(path, content)` sources of `project` into a graph
    /// and stats, pushing soft problems into `warnings`.
    fn parse_sources(
        &self,
        project: &str,
        files: &[(String, String)],
        warnings: &mut Vec<String>,
    ) -> anyhow::Result<ParseOutput>;
    /// Embedded default rules: `(toml, display name)`.
    fn default_rules(&self) -> (&'static str, &'static str);
    /// Separator of module paths in symbol ids (domain inference).
    fn module_separator(&self) -> char;
}

pub struct JavaFrontend;

impl Frontend for JavaFrontend {
    fn language(&self) -> Language {
        Language::Java
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn is_source(&self, path: &str) -> bool {
        path.ends_with(".java")
    }

    fn parse_sources(
        &self,
        project: &str,
        files: &[(String, String)],
        warnings: &mut Vec<String>,
    ) -> anyhow::Result<ParseOutput> {
        structure_lang_java::parse_sources(project, files, warnings).map(ParseOutput::new)
    }

    fn default_rules(&self) -> (&'static str, &'static str) {
        (include_str!("../rules/java.toml"), "rules/java.toml")
    }

    fn module_separator(&self) -> char {
        '.'
    }
}

pub struct TypeScriptFrontend;

impl Frontend for TypeScriptFrontend {
    fn language(&self) -> Language {
        Language::TypeScript
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx"]
    }

    fn is_source(&self, path: &str) -> bool {
        path.ends_with(".ts") || path.ends_with(".tsx")
    }

    fn parse_sources(
        &self,
        project: &str,
        files: &[(String, String)],
        _warnings: &mut Vec<String>,
    ) -> anyhow::Result<ParseOutput> {
        structure_lang_ts::parse_sources(project, files).map(ParseOutput::new)
    }

    fn default_rules(&self) -> (&'static str, &'static str) {
        (
            include_str!("../rules/typescript.toml"),
            "rules/typescript.toml",
        )
    }

    fn module_separator(&self) -> char {
        '/'
    }
}

pub struct PharoFrontend;

impl Frontend for PharoFrontend {
    fn language(&self) -> Language {
        Language::Pharo
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["st"]
    }

    fn is_source(&self, path: &str) -> bool {
        structure_lang_pharo::is_tonel_source(path)
    }

    fn parse_sources(
        &self,
        project: &str,
        files: &[(String, String)],
        warnings: &mut Vec<String>,
    ) -> anyhow::Result<ParseOutput> {
        Ok(structure_lang_pharo::parse_sources(project, files, warnings).into())
    }

    fn default_rules(&self) -> (&'static str, &'static str) {
        (include_str!("../rules/pharo.toml"), "rules/pharo.toml")
    }

    fn module_separator(&self) -> char {
        '-'
    }
}

static JAVA: JavaFrontend = JavaFrontend;
static TYPESCRIPT: TypeScriptFrontend = TypeScriptFrontend;
static PHARO: PharoFrontend = PharoFrontend;

/// All frontends, in detection priority order.
pub fn all() -> &'static [&'static dyn Frontend] {
    static ALL: [&dyn Frontend; 3] = [&JAVA, &TYPESCRIPT, &PHARO];
    &ALL
}

/// Frontend of `lang` (the one place that maps the enum to a frontend).
pub fn frontend(lang: Language) -> &'static dyn Frontend {
    match lang {
        Language::Java => &JAVA,
        Language::TypeScript => &TYPESCRIPT,
        Language::Pharo => &PHARO,
    }
}

/// Serialized id of `lang` (`java`, `typescript`, `pharo`), as in JSON.
pub fn language_id(lang: Language) -> &'static str {
    match lang {
        Language::Java => "java",
        Language::TypeScript => "typescript",
        Language::Pharo => "pharo",
    }
}

/// Language of a set of paths: the one with the most sources
/// ([`Frontend::is_source`], a path counting for the first frontend of [`all`]
/// that accepts it); ties go to the earlier one in [`all`] (Java, then
/// TypeScript, then Pharo). Fails when no path is a source.
pub fn detect_by_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> anyhow::Result<Language> {
    let frontends = all();
    let mut counts = vec![0usize; frontends.len()];
    for p in paths {
        if let Some(i) = frontends.iter().position(|f| f.is_source(p)) {
            counts[i] += 1;
        }
    }
    // First maximum: `max_by_key` keeps the last one, so compare by hand.
    let mut best: Option<(usize, usize)> = None;
    for (i, &n) in counts.iter().enumerate() {
        if n > 0 && best.is_none_or(|(_, m)| n > m) {
            best = Some((i, n));
        }
    }
    match best {
        Some((i, _)) => Ok(frontends[i].language()),
        None => bail!(
            "no supported sources: expected {} files",
            expected_extensions()
        ),
    }
}

/// `.java, .ts, .tsx or .st`: every extension of [`all`], in order.
fn expected_extensions() -> String {
    let exts: Vec<String> = all()
        .iter()
        .flat_map(|f| f.extensions().iter().map(|e| format!(".{e}")))
        .collect();
    match exts.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_consistent() {
        // Exhaustive: adding a variant breaks compilation here until listed.
        for lang in [Language::Java, Language::TypeScript, Language::Pharo] {
            match lang {
                Language::Java | Language::TypeScript | Language::Pharo => {}
            }
            assert!(
                all().iter().any(|f| f.language() == lang),
                "{lang:?} missing from all()"
            );
        }
        for f in all() {
            assert_eq!(frontend(f.language()).language(), f.language());
            assert!(!f.extensions().is_empty());
            for e in f.extensions() {
                assert!(f.is_source(&format!("src/A/x.{e}")), "{e}");
            }
            let (toml, name) = f.default_rules();
            assert!(!toml.trim().is_empty() && name.starts_with("rules/"));
        }
    }

    #[test]
    fn detection_picks_the_majority() {
        let p = [
            "src/P/A.class.st",
            "src/P/B.class.st",
            "src/P/package.st",
            "x.ts",
        ];
        assert_eq!(detect_by_paths(p).unwrap(), Language::Pharo);
        let p = ["a.ts", "b.tsx", "C.java"];
        assert_eq!(detect_by_paths(p).unwrap(), Language::TypeScript);
        let p = ["a.ts", "B.java", "C.java"];
        assert_eq!(detect_by_paths(p).unwrap(), Language::Java);
    }

    #[test]
    fn detection_ties_follow_priority() {
        assert_eq!(
            detect_by_paths(["a.ts", "B.java", "src/P/C.class.st"]).unwrap(),
            Language::Java
        );
        assert_eq!(
            detect_by_paths(["a.ts", "src/P/C.class.st"]).unwrap(),
            Language::TypeScript
        );
        assert_eq!(
            detect_by_paths(["src/P/C.class.st", "README.md"]).unwrap(),
            Language::Pharo
        );
    }

    #[test]
    fn detection_without_sources_fails() {
        let err = detect_by_paths(["README.md", "src/P/package.st"]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "no supported sources: expected .java, .ts, .tsx or .st files"
        );
    }

    #[test]
    fn parses_in_memory_sources() {
        let files = vec![(
            "src/main/java/p/A.java".to_string(),
            "package p;\npublic class A {}\n".to_string(),
        )];
        let out = frontend(Language::Java)
            .parse_sources("demo", &files, &mut Vec::new())
            .unwrap();
        assert_eq!(out.graph.language, Language::Java);
        assert!(out.graph.symbols.iter().any(|s| s.id == "p.A"));
        assert!(out.stats.is_empty());
    }

    #[test]
    fn pharo_reports_send_stats() {
        let files = vec![(
            "src/P/A.class.st".to_string(),
            "Class { #name : 'A', #superclass : 'Object', #category : 'P' }\n\
             A >> foo [ ^ self bar ]\n\
             A >> bar [ ^ x baz ]\n"
                .to_string(),
        )];
        let out = frontend(Language::Pharo)
            .parse_sources("demo", &files, &mut Vec::new())
            .unwrap();
        let stat = |name, value| ParseStat {
            group: "sends",
            name,
            value,
        };
        assert_eq!(out.stats, vec![stat("resolved", 1), stat("unresolved", 1)]);
    }
}
