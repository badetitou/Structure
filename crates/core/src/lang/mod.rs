//! Native language frontends: the per-language parts that need a process or
//! the filesystem (marker detection, SCIP indexer, parsing a project from
//! disk). The pure parts (extensions, in-memory parsing, default rules, module
//! separator) live in `structure-lang` ([`pure`]), shared with the wasm build.
//! A new language needs:
//! - a `Language` variant in `structure-ir` and a frontend in `structure-lang`;
//! - one module here implementing [`LanguageFrontend`];
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

use structure_ir::ir::{Graph, Language};
pub use structure_lang::Frontend;
/// Pure frontend of a language (`structure_lang::frontend`).
pub use structure_lang::frontend as pure;

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
    /// SCIP indexer invocation, or `None` if no indexer is available.
    fn indexer_command(&self, root: &Path) -> Option<Command>;
    /// Build-tool specific advice appended to indexer failures.
    fn indexer_hint(&self, root: &Path) -> Option<String>;
    /// Parse the project's sources into a graph (tree-sitter only).
    fn parse_project(&self, root: &Path) -> anyhow::Result<Graph>;
    /// [`Self::parse_project`], pushing soft problems into `warnings`.
    fn parse_project_warned(
        &self,
        root: &Path,
        _warnings: &mut Vec<String>,
    ) -> anyhow::Result<Graph> {
        self.parse_project(root)
    }
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
}
