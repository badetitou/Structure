//! End-to-end pipeline: detect language, index (SCIP), parse, link,
//! classify, build the matrix, compute impact.

mod git;
mod history;
mod indexer;
mod process;
mod progress;
mod snapshot;
#[cfg(test)]
mod test_support;
mod ts_args;

pub use git::{resolve_base, resolve_commit, worktree_fingerprint};
pub use history::{history, impact};
pub use indexer::{SCIP_JAVA_VERSION, SCIP_TS_VERSION, ensure_index, ensure_index_with_progress};
pub use snapshot::{MAX_SNAPSHOTS, analyze_snapshot, impact_at, impact_in_snapshot, snapshot};
pub use ts_args::{DEFAULT_NODE_HEAP_MB, discover_tsconfigs, node_options, ts_index_args};

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, bail};

use crate::classify::{RuleSet, classify};
use crate::ir::{Graph, Language};
use crate::link::{LinkStats, link};
use crate::matrix;
use crate::metrics::Metrics;
use crate::model::{Classification, Matrix, Progress};
use crate::{java, scip, ts};

pub const DEFAULT_INDEXER_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Always re-run the indexer.
    pub reindex: bool,
    /// Re-run the indexer when a source file is newer than `index.scip`.
    /// When false, a stale index is reused with a warning.
    pub reindex_if_stale: bool,
    pub use_scip: bool,
    /// Indexer wall-clock limit in seconds; 0 = no limit.
    pub indexer_timeout_secs: u64,
}

/// Timeout from `STRUCTURE_INDEXER_TIMEOUT` (seconds, 0 = none), else the default.
pub fn default_indexer_timeout() -> u64 {
    std::env::var("STRUCTURE_INDEXER_TIMEOUT")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(DEFAULT_INDEXER_TIMEOUT_SECS)
}

impl Default for Options {
    fn default() -> Self {
        Options {
            reindex: false,
            reindex_if_stale: true,
            use_scip: true,
            indexer_timeout_secs: default_indexer_timeout(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub graph: Graph,
    pub classification: Classification,
    pub matrix: Matrix,
    pub link_stats: Option<LinkStats>,
    pub metrics: Metrics,
    pub warnings: Vec<String>,
}

pub fn detect_language(root: &Path) -> anyhow::Result<Language> {
    let has = |n: &str| root.join(n).is_file();
    if has("pom.xml")
        || has("build.gradle")
        || has("build.gradle.kts")
        || has("settings.gradle")
        || has("settings.gradle.kts")
    {
        Ok(Language::Java)
    } else if has("tsconfig.json") || has("package.json") {
        Ok(Language::TypeScript)
    } else {
        bail!(
            "cannot detect language in {}: expected pom.xml/build.gradle/build.gradle.kts/settings.gradle/settings.gradle.kts (Java) or tsconfig.json/package.json (TypeScript)",
            root.display()
        )
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
            detail: None,
        })
    };
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }
    let lang = detect_language(root)?;
    let mut warnings: Vec<String> = Vec::new();
    let mut metrics = Metrics::default();
    let ran_indexer = std::cell::Cell::new(false);

    step("indexing", 5);
    let t = Instant::now();
    let index_path = if opts.use_scip {
        ensure_index_with_progress(
            root,
            lang,
            opts,
            &mut |w| warnings.push(w),
            &mut |pct, detail| {
                ran_indexer.set(true);
                progress(Progress {
                    step: "indexing".into(),
                    pct,
                    detail: Some(detail),
                })
            },
        )?
    } else {
        None
    };

    let note = match (opts.use_scip, index_path.is_some(), ran_indexer.get()) {
        (false, _, _) => "skipped (--no-scip)",
        (true, false, _) => "no index (tree-sitter only)",
        (true, true, true) => "ran indexer",
        (true, true, false) => "reused index.scip",
    };
    metrics.phase("indexer", t, Some(note));

    step("parsing", 35);
    let t = Instant::now();
    let graph = match lang {
        Language::Java => java::parse_project(root),
        Language::TypeScript => ts::parse_project(root),
    }
    .with_context(|| format!("parsing {}", root.display()))?;
    metrics.phase("parse", t, None);

    step("linking", 60);
    let t = Instant::now();
    let loaded = index_path.map(|p| (scip::load(&p), p));
    metrics.phase("scip load", t, None);
    let t = Instant::now();
    let (graph, link_stats) = match loaded {
        Some((Ok(idx), _)) => {
            metrics.record_scip(&idx);
            let (g, st) = link(graph, &idx);
            (g, Some(st))
        }
        Some((Err(e), p)) => {
            warnings.push(format!(
                "cannot read {}: {e:#}; using tree-sitter only",
                p.display()
            ));
            (graph, None)
        }
        None => (graph, None),
    };
    metrics.phase("link", t, None);
    metrics.record_edges(&graph);

    step("classifying", 80);
    let t = Instant::now();
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
    metrics.phase("classify", t, None);

    step("building matrix", 95);
    let t = Instant::now();
    let elapsed = started.elapsed().as_millis() as u64;
    let mut matrix = matrix::build(&graph, &classification, &project_name(root), elapsed);
    matrix.warnings = warnings.clone();
    metrics.phase("matrix", t, None);
    step("done", 100);

    Ok(Analysis {
        graph,
        classification,
        matrix,
        link_stats,
        metrics,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradle_settings_detected_as_java() {
        for f in ["settings.gradle", "settings.gradle.kts"] {
            let dir =
                std::env::temp_dir().join(format!("structure-detect-{}-{f}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(f), "").unwrap();
            assert!(matches!(detect_language(&dir).unwrap(), Language::Java));
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn build_gradle_kts_detected_as_java() {
        let dir = std::env::temp_dir().join(format!("structure-detect-kts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("build.gradle.kts"), "").unwrap();
        assert!(matches!(detect_language(&dir).unwrap(), Language::Java));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_language_error_lists_gradle_files() {
        let dir = std::env::temp_dir().join(format!("structure-detect-err-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let msg = detect_language(&dir).unwrap_err().to_string();
        assert!(msg.contains("settings.gradle"), "{msg}");
        assert!(msg.contains("settings.gradle.kts"), "{msg}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
