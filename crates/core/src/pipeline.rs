//! End-to-end pipeline: detect language, index (SCIP), parse, link,
//! classify, build the matrix, compute impact.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{Context, bail};

use crate::classify::{RuleSet, classify};
use crate::impact;
use crate::ir::{Graph, Language};
use crate::link::{LinkStats, link};
use crate::matrix;
use crate::model::{Classification, Impact, Matrix, Progress};
use crate::{java, scip, ts};

pub const SCIP_JAVA_VERSION: &str = "0.12.3";
pub const SCIP_TS_VERSION: &str = "0.6.1";

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Always re-run the indexer.
    pub reindex: bool,
    /// Re-run the indexer when a source file is newer than `index.scip`.
    /// When false, a stale index is reused with a warning.
    pub reindex_if_stale: bool,
    pub use_scip: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            reindex: false,
            reindex_if_stale: true,
            use_scip: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub graph: Graph,
    pub classification: Classification,
    pub matrix: Matrix,
    pub link_stats: Option<LinkStats>,
    pub warnings: Vec<String>,
}

pub fn detect_language(root: &Path) -> anyhow::Result<Language> {
    let has = |n: &str| root.join(n).is_file();
    if has("pom.xml") || has("build.gradle") || has("build.gradle.kts") {
        Ok(Language::Java)
    } else if has("tsconfig.json") || has("package.json") {
        Ok(Language::TypeScript)
    } else {
        bail!(
            "cannot detect language in {}: expected pom.xml/build.gradle (Java) or tsconfig.json/package.json (TypeScript)",
            root.display()
        )
    }
}

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

fn run_indexer(root: &Path, mut cmd: Command) -> Result<(), String> {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::Duration;
    let mut child = cmd
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start {:?}: {e}", cmd.get_program()))?;
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let deadline = Instant::now() + Duration::from_secs(INDEX_TIMEOUT_SECS);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "timed out after {} min; check ~/.m2/settings.xml (unreachable repository?) or run the indexer manually",
                    INDEX_TIMEOUT_SECS / 60
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(format!("waiting for indexer: {e}")),
        }
    };
    if status.success() {
        return Ok(());
    }
    let buf = reader.join().unwrap_or_default();
    let err = String::from_utf8_lossy(&buf);
    let mut tail: Vec<&str> = err.lines().rev().take(8).collect();
    tail.reverse();
    Err(format!(
        "{} ({status}); for Java also check ~/.m2/settings.xml",
        tail.join(" | ")
    ))
}

const INDEX_TIMEOUT_SECS: u64 = 600;

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
        .filter_entry(|e| {
            !matches!(
                e.file_name().to_str(),
                Some("node_modules" | "target" | "build" | "dist" | ".git")
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
                .args(["index", "--output", "index.scip"]);
            c
        }),
    };
    let Some(cmd) = cmd else {
        warn("no SCIP indexer found (scip-java/cs/npx); using tree-sitter only".into());
        return Ok(None);
    };
    match run_indexer(root, cmd) {
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

fn project_name(root: &Path) -> String {
    let canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    canon
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into())
}

pub fn analyze(
    root: &Path,
    opts: &Options,
    progress: &dyn Fn(Progress),
) -> anyhow::Result<Analysis> {
    let started = Instant::now();
    let step = |s: &str, pct: u8| {
        progress(Progress {
            step: s.into(),
            pct,
        })
    };
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }
    let lang = detect_language(root)?;
    let mut warnings: Vec<String> = Vec::new();

    step("indexing", 5);
    let index_path = if opts.use_scip {
        ensure_index(root, lang, opts, &mut |w| warnings.push(w))?
    } else {
        None
    };

    step("parsing", 35);
    let graph = match lang {
        Language::Java => java::parse_project(root),
        Language::TypeScript => ts::parse_project(root),
    }
    .with_context(|| format!("parsing {}", root.display()))?;

    step("linking", 60);
    let (graph, link_stats) = match index_path {
        Some(p) => match scip::load(&p) {
            Ok(idx) => {
                let (g, st) = link(graph, &idx);
                (g, Some(st))
            }
            Err(e) => {
                warnings.push(format!(
                    "cannot read {}: {e:#}; using tree-sitter only",
                    p.display()
                ));
                (graph, None)
            }
        },
        None => (graph, None),
    };

    step("classifying", 80);
    let mut rules = RuleSet::builtin(lang)?;
    let custom = root.join(".structure/rules.toml");
    if custom.is_file() {
        let text = std::fs::read_to_string(&custom)
            .with_context(|| format!("reading {}", custom.display()))?;
        let user = RuleSet::from_toml_str(&text)
            .with_context(|| format!("parsing {}", custom.display()))?;
        rules = rules.merge(user);
    }
    let classification = classify(&graph, &rules);

    step("building matrix", 95);
    let elapsed = started.elapsed().as_millis() as u64;
    let mut matrix = matrix::build(&graph, &classification, &project_name(root), elapsed);
    matrix.warnings = warnings.clone();
    step("done", 100);

    Ok(Analysis {
        graph,
        classification,
        matrix,
        link_stats,
        warnings,
    })
}

pub fn impact(
    root: &Path,
    analysis: &Analysis,
    base: Option<&str>,
    head: Option<&str>,
) -> anyhow::Result<Impact> {
    let mut changes = impact::changed_lines(root, base, head)?;
    // The SCIP index we write into the project is a build artefact, not a change.
    changes.retain(|c| c.file != "index.scip");
    let mut result = impact::compute(
        &analysis.graph,
        &analysis.classification,
        &changes,
        base.unwrap_or("HEAD"),
        head,
    );
    result.warnings = analysis.warnings.clone();
    if let Some(h) = head {
        result.warnings.push(format!(
            "symbol positions come from the working tree; results for {h} are exact only if it is checked out"
        ));
    }
    Ok(result)
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
