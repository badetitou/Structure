use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ir::Language;

use super::Options;
use super::process::run_indexer;
use super::ts_args::{node_heap_mb, node_options, ts_index_args};

pub const SCIP_JAVA_VERSION: &str = "0.12.3";
pub const SCIP_TS_VERSION: &str = "0.6.1";

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

fn find_cs() -> Option<PathBuf> {
    find_on_path("cs").or_else(|| {
        let p = PathBuf::from(std::env::var_os("HOME")?).join(".local/bin/cs");
        p.is_file().then_some(p)
    })
}

/// Build-tool specific advice appended to indexer failures. Pure: only checks marker files in `root`.
fn indexer_hint(root: &Path, lang: Language) -> Option<String> {
    if !matches!(lang, Language::Java) {
        return None;
    }
    let has = |n: &str| root.join(n).is_file();
    if has("pom.xml") {
        return Some("Maven project: check ~/.m2/settings.xml (unreachable repository?) and try `mvn -q compile` in the project".into());
    }
    let gradle = [
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ];
    if gradle.iter().any(|f| has(f)) {
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

/// Number of source files modified after `index` was written.
fn newer_sources(root: &Path, lang: Language, index: &Path) -> usize {
    let Ok(indexed) = std::fs::metadata(index).and_then(|m| m.modified()) else {
        return 0;
    };
    let exts: &[&str] = match lang {
        Language::Java => &["java"],
        Language::TypeScript => &["ts", "tsx"],
    };
    ignore::WalkBuilder::new(root)
        // `.gradle` is explicit: keeps it out even if `.hidden(false)` is set later.
        .filter_entry(|e| {
            !matches!(
                e.file_name().to_str(),
                Some("node_modules" | "target" | "build" | "dist" | ".git" | ".gradle")
            )
        })
        .build()
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| exts.contains(&x))
        })
        .filter(|e| {
            e.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .is_some_and(|t| t > indexed)
        })
        .count()
}

/// Returns the path of an up-to-date `index.scip`, or `Ok(None)` (with a
/// warning through `warn`) if it could not be produced.
pub fn ensure_index(
    root: &Path,
    lang: Language,
    opts: &Options,
    warn: &mut dyn FnMut(String),
) -> anyhow::Result<Option<PathBuf>> {
    ensure_index_with_progress(root, lang, opts, warn, &mut |_, _| {})
}

/// Like [`ensure_index`], reporting `(pct, detail)` while the indexer runs.
pub fn ensure_index_with_progress(
    root: &Path,
    lang: Language,
    opts: &Options,
    warn: &mut dyn FnMut(String),
    progress: &mut dyn FnMut(u8, String),
) -> anyhow::Result<Option<PathBuf>> {
    let index = root.join("index.scip");
    if index.is_file() && !opts.reindex {
        let stale = newer_sources(root, lang, &index);
        if stale == 0 {
            return Ok(Some(index));
        }
        if !opts.reindex_if_stale {
            warn(format!(
                "index.scip is older than {stale} source file(s); call edges in those files may be off until the project is re-analyzed"
            ));
            return Ok(Some(index));
        }
    }
    let cmd = match lang {
        Language::Java => {
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
        Language::TypeScript => find_on_path("npx").map(|p| {
            let mut c = Command::new(p);
            c.args(["-y"])
                .arg(format!("@sourcegraph/scip-typescript@{SCIP_TS_VERSION}"))
                .args(["index", "--output", "index.scip"])
                .args(ts_index_args(root));
            let existing = std::env::var("NODE_OPTIONS").ok();
            c.env(
                "NODE_OPTIONS",
                node_options(existing.as_deref(), node_heap_mb()),
            );
            c
        }),
    };
    let Some(cmd) = cmd else {
        warn("no SCIP indexer found (scip-java/cs/npx); using tree-sitter only".into());
        return Ok(None);
    };
    match run_indexer(
        root,
        cmd,
        opts.indexer_timeout_secs,
        progress,
        indexer_hint(root, lang).as_deref(),
    ) {
        Ok(()) if index.is_file() => Ok(Some(index)),
        Ok(()) => {
            warn("indexer succeeded but produced no index.scip; using tree-sitter only".into());
            Ok(None)
        }
        Err(e) => {
            warn(format!("indexing failed, using tree-sitter only: {e}"));
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn touch(p: &Path, t: SystemTime) {
        std::fs::write(p, "x").unwrap();
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    fn hint_dir(tag: &str, files: &[&str]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("structure-hint-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for f in files {
            std::fs::write(dir.join(f), "").unwrap();
        }
        dir
    }

    #[test]
    fn indexer_hint_maven() {
        let dir = hint_dir("mvn", &["pom.xml"]);
        let h = indexer_hint(&dir, Language::Java).unwrap();
        assert!(
            h.contains("~/.m2/settings.xml") && h.contains("mvn -q compile"),
            "{h}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn indexer_hint_gradle_with_and_without_wrapper() {
        let dir = hint_dir("gw", &["build.gradle", "gradlew"]);
        let h = indexer_hint(&dir, Language::Java).unwrap();
        assert!(h.contains("./gradlew help") && !h.contains(".m2"), "{h}");
        let _ = std::fs::remove_dir_all(&dir);
        let dir = hint_dir("gn", &["settings.gradle.kts"]);
        let h = indexer_hint(&dir, Language::Java).unwrap();
        assert!(h.contains("`gradle help`"), "{h}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn indexer_hint_typescript_and_unknown_none() {
        let dir = hint_dir("ts", &["tsconfig.json"]);
        assert!(indexer_hint(&dir, Language::TypeScript).is_none());
        assert!(indexer_hint(&dir, Language::Java).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gradle_dir_not_counted_as_stale() {
        let dir =
            std::env::temp_dir().join(format!("structure-gradle-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".gradle/x")).unwrap();
        let t0 = SystemTime::now() - Duration::from_secs(100);
        let index = dir.join("index.scip");
        touch(&index, t0);
        touch(&dir.join(".gradle/x/A.java"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::Java, &index), 0);
        touch(&dir.join("B.java"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::Java, &index), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_index_detection_and_warning() {
        let dir = std::env::temp_dir().join(format!("structure-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        let t0 = SystemTime::now() - Duration::from_secs(100);
        let index = dir.join("index.scip");
        touch(&dir.join("src/old.ts"), t0);
        touch(&index, t0 + Duration::from_secs(10));
        touch(
            &dir.join("node_modules/dep.ts"),
            t0 + Duration::from_secs(50),
        );
        touch(&dir.join("src/notes.md"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::TypeScript, &index), 0);

        touch(&dir.join("src/new.ts"), t0 + Duration::from_secs(50));
        assert_eq!(newer_sources(&dir, Language::TypeScript, &index), 1);

        // Stale index reused with a warning when re-indexing is not wanted.
        let opts = Options {
            reindex_if_stale: false,
            ..Options::default()
        };
        let mut warnings = vec![];
        let got = ensure_index(&dir, Language::TypeScript, &opts, &mut |w| warnings.push(w));
        assert_eq!(got.unwrap(), Some(index));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("older than 1 source file"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
