//! Native language frontends: the per-language parts that need a process or
//! the filesystem (marker detection, parsing a project from disk, and an
//! optional SCIP [`Indexer`]). The pure parts (extensions, in-memory parsing,
//! default rules, module separator) live in `structure-lang` ([`pure`]),
//! shared with the wasm build.
//! A new language needs:
//! - a `Language` variant in `structure-ir` and a frontend in `structure-lang`;
//! - one module here implementing [`LanguageFrontend`] (and [`Indexer`] when a
//!   SCIP indexer exists for it);
//! - one `static` and one line in [`all`], and one arm in [`frontend`].
//!
//! The `registry_is_consistent` test fails if a variant is missing from [`all`]
//! or if [`all`] and `structure_lang::all` disagree.

mod java;
mod pharo;
mod ts_args;
mod typescript;

use std::path::{Path, PathBuf};
use std::process::Command;

use structure_ir::ir::Language;
/// Pure frontend of a language (`structure_lang::frontend`).
pub use structure_lang::frontend as pure;
pub use structure_lang::{Frontend, ParseOutput};

pub use java::SCIP_JAVA_VERSION;
pub use ts_args::{DEFAULT_NODE_HEAP_MB, discover_tsconfigs, node_options, ts_index_args};
pub use typescript::SCIP_TS_VERSION;

pub trait LanguageFrontend: Sync {
    /// The pure part of this language (rules, separator, extensions).
    fn pure(&self) -> &'static dyn Frontend;
    /// Serialized identifier of the language.
    fn language(&self) -> Language {
        self.pure().language()
    }
    /// Whether `root` looks like a project of this language (marker files).
    fn detect(&self, root: &Path) -> bool;
    /// Human description of the detection markers, for error messages.
    fn markers(&self) -> &'static str;
    /// The language's SCIP indexer, `None` when it has none (the pipeline
    /// then uses the parsed graph alone).
    fn indexer(&self) -> Option<&dyn Indexer>;
    /// Parse the project's sources (no SCIP) into a graph and stats, pushing
    /// soft problems into `warnings`.
    fn parse_project(&self, root: &Path, warnings: &mut Vec<String>)
    -> anyhow::Result<ParseOutput>;
}

/// A SCIP indexer of one language, run in the project root to write the
/// index file ([`index_file`]).
pub trait Indexer: Sync {
    /// Key of the SCIP documents this indexer produces in the document cache
    /// (tool and version: a new version invalidates the cache).
    fn cache_tag(&self) -> String;
    /// Invocation writing the index to `output` (a file name relative to
    /// `root`), or `None` if the tool is not available here.
    fn command(&self, root: &Path, output: &str) -> Option<Command>;
    /// Build-tool specific advice appended to indexer failures.
    fn hint(&self, _root: &Path) -> Option<String> {
        None
    }
}

/// Document cache key of a language without an [`Indexer`].
pub const NO_INDEXER_TAG: &str = "none";

/// [`Indexer::cache_tag`] of `lang`, or [`NO_INDEXER_TAG`].
pub fn scip_cache_tag(lang: Language) -> String {
    frontend(lang)
        .indexer()
        .map_or_else(|| NO_INDEXER_TAG.to_string(), |i| i.cache_tag())
}

static JAVA: java::JavaFrontend = java::JavaFrontend;
static TYPESCRIPT: typescript::TypeScriptFrontend = typescript::TypeScriptFrontend;
static PHARO: pharo::PharoFrontend = pharo::PharoFrontend;

/// All frontends, in detection priority order.
pub fn all() -> &'static [&'static dyn LanguageFrontend] {
    static ALL: [&dyn LanguageFrontend; 3] = [&JAVA, &TYPESCRIPT, &PHARO];
    &ALL
}

/// Frontend of `lang` (the one place that maps the enum to a frontend).
pub fn frontend(lang: Language) -> &'static dyn LanguageFrontend {
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

/// File name of the SCIP index of `lang` in a root: `index.scip` when the
/// root holds one language, `index.<language>.scip` when it holds several
/// (each indexer writes its own file).
pub fn index_file(lang: Language, shared_root: bool) -> String {
    if shared_root {
        format!("index.{}.scip", language_id(lang))
    } else {
        "index.scip".to_string()
    }
}

/// Dependency and VCS directories never searched by [`has_sources`].
const SKIPPED_SOURCE_DIRS: &[&str] = &["node_modules", ".git", ".gradle"];

/// Build output directories skipped by [`has_sources`] unless under a `src`
/// directory (where they are package names, e.g. `src/main/java/acme/build`),
/// as the Java frontend does.
const SKIPPED_OUTPUT_DIRS: &[&str] = &["target", "build", "dist"];

/// Whether `root` holds at least one source file of `lang` (stops at the
/// first; honours `.gitignore`, skips dependency directories and build
/// output outside `src`).
pub fn has_sources(root: &Path, lang: Language) -> bool {
    let pure = pure(lang);
    let base = root.to_path_buf();
    ignore::WalkBuilder::new(root)
        .filter_entry(move |e| {
            if e.depth() == 0 || !e.file_type().is_some_and(|t| t.is_dir()) {
                return true;
            }
            let Some(name) = e.file_name().to_str() else {
                return true;
            };
            if SKIPPED_SOURCE_DIRS.contains(&name) {
                return false;
            }
            !SKIPPED_OUTPUT_DIRS.contains(&name)
                || e.path()
                    .strip_prefix(&base)
                    .unwrap_or(e.path())
                    .parent()
                    .is_some_and(|p| p.components().any(|c| c.as_os_str() == "src"))
        })
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .any(|e| {
            let rel = e.path().strip_prefix(root).unwrap_or(e.path());
            let rel: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            pure.is_source(&rel.join("/"))
        })
}

pub(crate) fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
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
        let pure: Vec<Language> = structure_lang::all().iter().map(|f| f.language()).collect();
        let native: Vec<Language> = all().iter().map(|f| f.language()).collect();
        assert_eq!(native, pure, "same languages in the same priority order");
        for f in all() {
            assert_eq!(frontend(f.language()).language(), f.language());
            assert_eq!(f.pure().language(), f.language());
            let (toml, name) = f.pure().default_rules();
            assert!(crate::classify::RuleSet::from_embedded(toml, name).is_ok());
        }
    }

    #[test]
    fn cache_tags_name_the_indexer_version() {
        assert_eq!(
            scip_cache_tag(Language::Java),
            format!("scip-java-{SCIP_JAVA_VERSION}")
        );
        assert_eq!(
            scip_cache_tag(Language::TypeScript),
            format!("scip-typescript-{SCIP_TS_VERSION}")
        );
        assert!(frontend(Language::Pharo).indexer().is_none());
        assert_eq!(scip_cache_tag(Language::Pharo), "none");
    }

    #[test]
    fn language_ids_match_serde() {
        for f in all() {
            let json = format!("\"{}\"", language_id(f.language()));
            let back: Language = serde_json::from_str(&json).unwrap();
            assert_eq!(back, f.language());
        }
        assert_eq!(index_file(Language::Java, false), "index.scip");
        assert_eq!(
            index_file(Language::TypeScript, true),
            "index.typescript.scip"
        );
        assert!(structure_git::is_scip_index(&index_file(
            Language::Pharo,
            true
        )));
    }

    #[test]
    fn has_sources_finds_only_that_language() {
        let d = std::env::temp_dir().join(format!("structure-has-src-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("node_modules/x")).unwrap();
        std::fs::create_dir_all(d.join("src/main")).unwrap();
        std::fs::write(d.join("node_modules/x/a.ts"), "").unwrap();
        std::fs::write(d.join("src/main/A.java"), "").unwrap();
        assert!(has_sources(&d, Language::Java));
        assert!(
            !has_sources(&d, Language::TypeScript),
            "node_modules skipped"
        );
        std::fs::write(d.join("src/b.ts"), "").unwrap();
        assert!(has_sources(&d, Language::TypeScript));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn has_sources_keeps_build_packages_under_src() {
        let d = std::env::temp_dir().join(format!("structure-has-build-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("target/classes")).unwrap();
        std::fs::write(d.join("target/classes/Gen.java"), "").unwrap();
        assert!(!has_sources(&d, Language::Java), "root target/ skipped");
        std::fs::create_dir_all(d.join("src/main/java/acme/build")).unwrap();
        std::fs::write(d.join("src/main/java/acme/build/A.java"), "").unwrap();
        assert!(has_sources(&d, Language::Java), "package named build kept");
        std::fs::create_dir_all(d.join("src/build")).unwrap();
        std::fs::write(d.join("src/build/b.ts"), "").unwrap();
        assert!(has_sources(&d, Language::TypeScript), "src/build kept");
        let _ = std::fs::remove_dir_all(&d);
    }
}
