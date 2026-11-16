//! Language frontends: everything that differs per language (detection,
//! indexer, sources, parsing, default classification rules, module separator)
//! behind one trait. A new language needs:
//! - a `Language` variant in `structure-ir`;
//! - a parser crate and one module here implementing [`LanguageFrontend`];
//! - one `static` and one line in [`all`], and one arm in [`frontend`].
//!
//! The `registry_is_consistent` test fails if a variant is missing from [`all`].

mod java;
mod pharo;
mod ts_args;
mod typescript;

use std::path::{Path, PathBuf};
use std::process::Command;

use structure_ir::ir::{Graph, Language};

pub use java::SCIP_JAVA_VERSION;
pub use ts_args::{DEFAULT_NODE_HEAP_MB, discover_tsconfigs, node_options, ts_index_args};
pub use typescript::SCIP_TS_VERSION;

pub trait LanguageFrontend: Sync {
    /// Serialized identifier of the language.
    fn language(&self) -> Language;
    /// Whether `root` looks like a project of this language (marker files).
    fn detect(&self, root: &Path) -> bool;
    /// Human description of the detection markers, for error messages.
    fn markers(&self) -> &'static str;
    /// Source file extensions (without dot).
    fn extensions(&self) -> &'static [&'static str];
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
    /// Embedded default rules: `(toml, display name)`.
    fn default_rules(&self) -> (&'static str, &'static str);
    /// Separator of module paths in symbol ids (domain inference).
    fn module_separator(&self) -> char;
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
        for f in all() {
            assert_eq!(frontend(f.language()).language(), f.language());
            assert!(!f.extensions().is_empty());
            let (toml, name) = f.default_rules();
            assert!(crate::classify::RuleSet::from_embedded(toml, name).is_ok());
        }
    }
}
