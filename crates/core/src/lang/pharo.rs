use std::path::Path;
use std::process::Command;

use crate::pharo;
use structure_ir::ir::{Graph, Language};

use super::LanguageFrontend;

pub struct PharoFrontend;

impl LanguageFrontend for PharoFrontend {
    fn language(&self) -> Language {
        Language::Pharo
    }

    fn detect(&self, root: &Path) -> bool {
        let src = root.join("src");
        if src.join(".properties").is_file() {
            return true;
        }
        if !src.is_dir() {
            return false;
        }
        // Tonel layout: `src/<Package>/<Class>.class.st`.
        ignore::WalkBuilder::new(&src)
            .max_depth(Some(3))
            .build()
            .filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().ends_with(".class.st"))
    }

    fn markers(&self) -> &'static str {
        "src/.properties or src/**/*.class.st (Pharo Tonel)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["st"]
    }

    fn indexer_command(&self, _root: &Path) -> Option<Command> {
        None
    }

    fn indexer_hint(&self, _root: &Path) -> Option<String> {
        None
    }

    fn parse_project(&self, root: &Path) -> anyhow::Result<Graph> {
        pharo::parse_project(root)
    }

    fn parse_project_warned(
        &self,
        root: &Path,
        warnings: &mut Vec<String>,
    ) -> anyhow::Result<Graph> {
        pharo::parse_project_with_warnings(root, warnings)
    }

    fn default_rules(&self) -> (&'static str, &'static str) {
        (
            include_str!("../../../../rules/pharo.toml"),
            "rules/pharo.toml",
        )
    }

    fn module_separator(&self) -> char {
        '-'
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tonel_layouts() {
        let base = std::env::temp_dir().join(format!("structure-pharo-det-{}", std::process::id()));
        let a = base.join("a/src");
        let b = base.join("b/src/App-Core");
        let c = base.join("c/src");
        for d in [&a, &b, &c] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(a.join(".properties"), "{}").unwrap();
        std::fs::write(b.join("Foo.class.st"), "").unwrap();
        std::fs::write(c.join("x.txt"), "").unwrap();
        let f = PharoFrontend;
        assert!(f.detect(&base.join("a")));
        assert!(f.detect(&base.join("b")));
        assert!(!f.detect(&base.join("c")));
        assert!(!f.detect(&base.join("nope")));
        std::fs::remove_dir_all(&base).ok();
    }
}
