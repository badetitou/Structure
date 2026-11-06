use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ir::{Graph, Language};
use crate::java;

use super::{LanguageFrontend, find_on_path};

pub const SCIP_JAVA_VERSION: &str = "0.12.3";

const GRADLE_FILES: [&str; 4] = [
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
];

fn find_cs() -> Option<PathBuf> {
    find_on_path("cs").or_else(|| {
        let p = PathBuf::from(std::env::var_os("HOME")?).join(".local/bin/cs");
        p.is_file().then_some(p)
    })
}

pub struct JavaFrontend;

impl LanguageFrontend for JavaFrontend {
    fn language(&self) -> Language {
        Language::Java
    }

    fn detect(&self, root: &Path) -> bool {
        let has = |n: &str| root.join(n).is_file();
        has("pom.xml") || GRADLE_FILES.iter().any(|f| has(f))
    }

    fn markers(&self) -> &'static str {
        "pom.xml/build.gradle/build.gradle.kts/settings.gradle/settings.gradle.kts (Java)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn indexer_command(&self, _root: &Path) -> Option<Command> {
        if let Some(p) = find_on_path("scip-java") {
            let mut c = Command::new(p);
            c.args(["index", "--output", "index.scip"]);
            Some(c)
        } else if let Some(cs) = find_cs() {
            let mut c = Command::new(cs);
            c.arg("launch")
                .arg(format!(
                    "com.sourcegraph:scip-java_2.13:{SCIP_JAVA_VERSION}"
                ))
                .args(["--", "index", "--output", "index.scip"]);
            Some(c)
        } else {
            None
        }
    }

    fn indexer_hint(&self, root: &Path) -> Option<String> {
        let has = |n: &str| root.join(n).is_file();
        if has("pom.xml") {
            return Some("Maven project: check ~/.m2/settings.xml (unreachable repository?) and try `mvn -q compile` in the project".into());
        }
        if GRADLE_FILES.iter().any(|f| has(f)) {
            let cmd = if has("gradlew") {
                "./gradlew help"
            } else {
                "gradle help"
            };
            return Some(format!(
                "Gradle project: run `{cmd}` in the project to see the build error (toolchain/plugin resolution?)"
            ));
        }
        None
    }

    fn parse_project(&self, root: &Path) -> anyhow::Result<Graph> {
        java::parse_project(root)
    }

    fn default_rules(&self) -> (&'static str, &'static str) {
        (
            include_str!("../../../../rules/java.toml"),
            "rules/java.toml",
        )
    }

    fn module_separator(&self) -> char {
        '.'
    }
}
