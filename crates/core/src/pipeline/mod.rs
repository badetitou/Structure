//! End-to-end pipeline: detect language, index (SCIP), parse, link,
//! classify, build the matrix, compute impact; reclassify an analysis.

mod history;
mod indexer;
mod multi;
mod process;
mod progress;
mod reclassify;
pub mod scip_cache;
mod snapshot;
#[cfg(test)]
pub(crate) mod test_support;

pub use crate::lang::{DEFAULT_NODE_HEAP_MB, discover_tsconfigs, node_options, ts_index_args};
pub use crate::lang::{SCIP_JAVA_VERSION, SCIP_TS_VERSION};
pub use history::{history, impact};
pub use indexer::{ensure_index, ensure_index_with_progress};
pub use multi::{analyze_many, expand_roots};
pub use reclassify::{reclassify, rules_stamp, rules_stamp_of};
pub use snapshot::{MAX_SNAPSHOTS, analyze_snapshot, impact_at, impact_in_snapshot, snapshot};
pub use structure_git::{resolve_base, resolve_commit, worktree_fingerprint};

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use anyhow::{Context, bail};

use crate::classify::{RuleSet, classify};
use crate::impact::ImpactIndex;
use crate::ir::{Graph, Language};
use crate::layers::Layers;
use crate::link::{LinkStats, link};
use crate::matrix;
use crate::metrics::Metrics;
use crate::model::{Classification, Matrix, Progress};
use crate::scip;

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
    /// Snapshots: always run the indexer, never build the index from SCIP
    /// documents cached by earlier analyses (see [`scip_cache`]).
    pub exact: bool,
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
            exact: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Analysis {
    /// Shared with the analyses [`reclassify`] makes from this one.
    pub graph: Arc<Graph>,
    /// The analysed roots, in matrix order: the project itself, or each root
    /// of a multi-root analysis after [`expand_roots`] (named as in
    /// `matrix.projects`).
    pub roots: Vec<PathBuf>,
    pub classification: Classification,
    pub matrix: Matrix,
    pub link_stats: Option<LinkStats>,
    /// Cross-project linker runs worth reporting, in registry order (see
    /// [`crate::linker::run`]): HTTP only with several roots, GWT RPC when the
    /// graph has RPC calls.
    pub remote_links: Vec<crate::linker::LinkerRun>,
    pub metrics: Metrics,
    pub warnings: Vec<String>,
    /// Declared layers of the rules used (merged across roots, see
    /// [`analyze_many`]); the matrix only lists the ones it places symbols in.
    pub layers: Layers,
    /// Built from `graph` on the first impact (see [`Analysis::impact_index`]);
    /// shared with every analysis holding the same `graph` ([`reclassify`]).
    impact_index: Arc<OnceLock<ImpactIndex>>,
}

impl Analysis {
    /// Stats of the HTTP linker, if it ran (several roots).
    pub fn http_stats(&self) -> Option<&crate::http_link::HttpLinkStats> {
        crate::linker::http_stats(&self.remote_links)
    }

    /// Stats of the GWT RPC linker, if the graph had RPC calls.
    pub fn rpc_stats(&self) -> Option<&crate::rpc_link::RpcLinkStats> {
        crate::linker::rpc_stats(&self.remote_links)
    }

    /// Index shared by every impact computed on this analysis, built on first
    /// use. `graph` must not change once it is built.
    pub fn impact_index(&self) -> &ImpactIndex {
        self.impact_index
            .get_or_init(|| ImpactIndex::new(&self.graph))
    }
}

pub fn detect_language(root: &Path) -> anyhow::Result<Language> {
    if let Some(f) = crate::lang::all().iter().find(|f| f.detect(root)) {
        return Ok(f.language());
    }
    let markers: Vec<&str> = crate::lang::all().iter().map(|f| f.markers()).collect();
    bail!(
        "cannot detect language in {}: expected {}",
        root.display(),
        markers.join(" or ")
    )
}

/// `<git-common-dir>/structure` of the repository containing `root`, shared
/// by all its worktrees (snapshots, SCIP document cache).
pub(super) fn structure_dir(root: &Path) -> anyhow::Result<std::path::PathBuf> {
    let common = crate::impact::git(root, &["rev-parse", "--git-common-dir"])?;
    Ok(root.join(common.trim()).join("structure"))
}

/// Cache key of the indexer whose documents a language reuses.
pub(super) use crate::lang::scip_cache_tag;

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
    analyze_in(root, opts, progress, false)
}

/// [`analyze`]; with `from_cache` (a clean checkout without its own
/// `index.scip`), the SCIP index is first composed from cached documents.
pub(super) fn analyze_in(
    root: &Path,
    opts: &Options,
    progress: &dyn Fn(Progress),
    from_cache: bool,
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
    let exts = crate::lang::pure(lang).extensions();
    let tag = scip_cache_tag(lang);
    let reuse = opts.use_scip && !opts.reindex && from_cache && !root.join("index.scip").is_file();
    let composed = if reuse {
        // The cache is only a shortcut: on error, index as usual.
        scip_cache::compose(root, exts, &tag).unwrap_or_else(|e| {
            warnings.push(format!("cannot reuse cached SCIP documents: {e:#}"));
            None
        })
    } else {
        None
    };
    let index_path = if composed.is_some() || !opts.use_scip {
        None
    } else {
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
    };
    if let Some(p) = &index_path
        && let Err(e) = scip_cache::store(root, p, exts, &tag)
    {
        warnings.push(format!("cannot cache SCIP documents: {e:#}"));
    }

    let note = match (
        opts.use_scip,
        &composed,
        index_path.is_some(),
        ran_indexer.get(),
    ) {
        (false, ..) => "skipped (--no-scip)".to_string(),
        (true, Some((_, c)), ..) => format!("cached documents for {}/{} files", c.covered, c.total),
        (true, None, false, _) => "no index (tree-sitter only)".into(),
        (true, None, true, true) => "ran indexer".into(),
        (true, None, true, false) => "reused index.scip".into(),
    };
    metrics.phase("indexer", t, Some(&note));

    step("parsing", 35);
    let t = Instant::now();
    let parsed = crate::lang::frontend(lang)
        .parse_project(root, &mut warnings)
        .with_context(|| format!("parsing {}", root.display()))?;
    metrics.parse_stats = parsed.stats;
    let graph = parsed.graph;
    metrics.phase("parse", t, None);

    step("linking", 60);
    let t = Instant::now();
    let loaded = match composed {
        Some((idx, coverage)) => {
            warnings.push(coverage.warning());
            Some(Ok(idx))
        }
        None => index_path.map(|p| {
            scip::load(&p).map_err(|e| {
                anyhow::anyhow!("cannot read {}: {e:#}; using tree-sitter only", p.display())
            })
        }),
    };
    metrics.phase("scip load", t, None);
    let t = Instant::now();
    let (mut graph, link_stats) = match loaded {
        Some(Ok(idx)) => {
            metrics.record_scip(&idx);
            let (g, st) = link(graph, &idx);
            (g, Some(st))
        }
        Some(Err(e)) => {
            warnings.push(e.to_string());
            (graph, None)
        }
        None => (graph, None),
    };
    let remote_links = crate::linker::run(&mut graph, false);
    metrics.phase("link", t, None);
    metrics.record_edges(&graph);

    step("classifying", 80);
    let t = Instant::now();
    let rules = rules_for(root, lang)?;
    let classification = classify(&graph, &rules, crate::lang::pure(lang).module_separator());
    metrics.phase("classify", t, None);

    step("building matrix", 95);
    let layers = rules.layers();
    let t = Instant::now();
    let elapsed = started.elapsed().as_millis() as u64;
    let mut matrix = matrix::build(
        &graph,
        &classification,
        &layers,
        &project_name(root),
        elapsed,
    );
    matrix.warnings = warnings.clone();
    metrics.phase("matrix", t, None);
    step("done", 100);

    Ok(Analysis {
        graph: Arc::new(graph),
        roots: vec![root.to_path_buf()],
        classification,
        matrix,
        link_stats,
        remote_links,
        metrics,
        warnings,
        layers,
        impact_index: Default::default(),
    })
}

/// Path of a project's own rules, relative to its root.
pub const PROJECT_RULES: &str = ".structure/rules.toml";

/// The language's default rules, with the project's [`PROJECT_RULES`] merged
/// on top when present ([`RuleSet::merge`]: its rules first, its layers
/// replacing or added by id). An unreadable, unparsable or invalid project
/// file is an error naming its path.
fn rules_for(root: &Path, lang: Language) -> anyhow::Result<RuleSet> {
    let rules = {
        let (toml, name) = crate::lang::pure(lang).default_rules();
        RuleSet::from_embedded(toml, name)?
    };
    let custom = root.join(PROJECT_RULES);
    if !custom.is_file() {
        return Ok(rules);
    }
    let text = std::fs::read_to_string(&custom)
        .with_context(|| format!("reading {}", custom.display()))?;
    let user = RuleSet::parse(&text).with_context(|| format!("invalid {}", custom.display()))?;
    let rules = rules.merge(user);
    rules
        .validate()
        .with_context(|| format!("invalid {}", custom.display()))?;
    Ok(rules)
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
