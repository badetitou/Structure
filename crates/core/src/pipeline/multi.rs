//! Several roots, or several languages of one root, analysed into one graph
//! and matrix.
//!
//! The analysis unit is a `(root, language)` pair ([`Unit`]): a root holding
//! one language is one unit named after its directory, a root holding several
//! is one unit per language named `<dir> (<language>)` (e.g. `app (java)`,
//! `app (typescript)`), each with its own indexer and SCIP index file
//! (`index.<language>.scip`), its sources, and its language's default rules
//! plus the root's project rules. Each unit runs the single-unit pipeline;
//! with at least two units the results are merged: symbol ids become
//! `<project>::<id>`, files `<project>/<file>` and domains `<project>/<domain>`,
//! and the linkers run on the merged graph.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::bail;

use super::{Analysis, Options, analyze_unit, project_name};
use crate::ir::{Graph, Language};
use crate::matrix;
use crate::model::{Matrix, Progress, Project};
use structure_classify::merge::{MergedClassification, append_graph, merge_layers};

/// Root dir names, suffixed `-2`, `-3`... when they clash.
#[cfg(test)]
fn unique_names(roots: &[PathBuf]) -> Vec<String> {
    dedup_names(roots.iter().map(|r| project_name(r)))
}

/// `bases`, suffixed `-2`, `-3`... when they clash.
fn dedup_names(bases: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    bases
        .into_iter()
        .map(|base| {
            let mut name = base.clone();
            let mut n = 1;
            while !used.insert(name.clone()) {
                n += 1;
                name = format!("{base}-{n}");
            }
            name
        })
        .collect()
}

const SKIPPED_DIRS: &[&str] = &["node_modules", "target", "build", "dist"];

fn detectable(dir: &Path) -> bool {
    crate::lang::all().iter().any(|f| f.detect(dir))
}

/// Visible, non-build sub-directories of `dir`, sorted by path.
fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut out: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            !n.starts_with('.') && !SKIPPED_DIRS.contains(&n.as_str())
        })
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

/// A detectable root is kept; an undetectable one is replaced by its
/// detectable children, else grandchildren. Nothing found: the usual
/// "cannot detect language" error, with a hint.
pub fn expand_roots(roots: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = vec![];
    for root in roots {
        if detectable(root) {
            out.push(root.clone());
            continue;
        }
        let kids = subdirs(root);
        let mut found: Vec<PathBuf> = kids.iter().filter(|p| detectable(p)).cloned().collect();
        if found.is_empty() {
            found = kids
                .iter()
                .flat_map(|d| subdirs(d))
                .filter(|p| detectable(p))
                .collect();
        }
        if found.is_empty() {
            let err = super::detect_language(root)
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default();
            bail!(
                "{err} (also looked for a project in its sub-directories, depth 1 and 2: none found)"
            );
        }
        out.extend(found);
    }
    Ok(out)
}

/// One analysis unit: a root and one of its languages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Unit {
    pub root: PathBuf,
    pub language: Language,
    /// Project name in the matrix: the root's directory name (deduplicated
    /// across roots), followed by ` (<language>)` when the root holds several
    /// languages.
    pub name: String,
    /// SCIP index file name in `root` ([`crate::lang::index_file`]).
    pub index: String,
}

/// The analysis units of detectable `roots`, in root order then in language
/// priority order ([`super::detect_languages`]).
pub(super) fn units(roots: &[PathBuf]) -> anyhow::Result<Vec<Unit>> {
    let mut raw: Vec<(PathBuf, Language, String, String)> = vec![];
    for root in roots {
        let langs = super::detect_languages(root)?;
        let shared = langs.len() > 1;
        let base = project_name(root);
        for lang in langs {
            let name = if shared {
                format!("{base} ({})", crate::lang::language_id(lang))
            } else {
                base.clone()
            };
            raw.push((
                root.clone(),
                lang,
                name,
                crate::lang::index_file(lang, shared),
            ));
        }
    }
    let names = dedup_names(raw.iter().map(|(_, _, n, _)| n.clone()));
    Ok(raw
        .into_iter()
        .zip(names)
        .map(|((root, language, _, index), name)| Unit {
            root,
            language,
            name,
            index,
        })
        .collect())
}

/// One unit behaves exactly as the single-root pipeline; several are
/// analysed one after the other and merged.
pub(super) fn analyze_units(
    units: &[Unit],
    opts: &Options,
    progress: &dyn Fn(Progress),
    from_cache: bool,
) -> anyhow::Result<Analysis> {
    match units {
        [] => bail!("no project root given"),
        [one] => analyze_unit(one, opts, progress, from_cache),
        _ => {
            let total = units.len();
            let mut parts = Vec::with_capacity(total);
            for (i, unit) in units.iter().enumerate() {
                let name = &unit.name;
                let scaled = |p: Progress| {
                    let pct = ((i * 100 + usize::from(p.pct)) / total) as u8;
                    progress(Progress {
                        step: format!("{name}: {}", p.step),
                        pct,
                        detail: p.detail,
                    })
                };
                parts.push(analyze_unit(unit, opts, &scaled, from_cache)?);
            }
            Ok(merge(units, parts))
        }
    }
}

/// One root holding one language behaves exactly as [`super::analyze`].
/// Undetectable roots are first expanded to their sub-projects
/// ([`expand_roots`]); a root holding several languages gives one unit per
/// language ([`Unit`]).
pub fn analyze_many(
    roots: &[PathBuf],
    opts: &Options,
    progress: &dyn Fn(Progress),
) -> anyhow::Result<Analysis> {
    if roots.is_empty() {
        bail!("no project root given");
    }
    let expanded = expand_roots(roots)?;
    analyze_units(&units(&expanded)?, opts, progress, false)
}

fn merge(units: &[Unit], parts: Vec<Analysis>) -> Analysis {
    let roots: Vec<PathBuf> = units.iter().map(|u| u.root.clone()).collect();
    let names: Vec<String> = units.iter().map(|u| u.name.clone()).collect();
    let mut graph = Graph {
        root: parts[0].graph.root.clone(),
        language: parts[0].graph.language,
        symbols: vec![],
        edges: vec![],
    };
    let mut merged = MergedClassification::default();
    let mut projects: Vec<Project> = vec![];
    let mut warnings: Vec<String> = vec![];
    let mut metrics = crate::metrics::Metrics::default();
    // The first root's link stats only (not summed, not a later root's).
    let link_stats = parts[0].link_stats;
    let mut elapsed = 0;
    let layers = merge_layers(parts.iter().map(|a| &a.layers));

    for ((root, name), a) in roots.iter().zip(&names).zip(parts) {
        projects.push(Project {
            name: name.clone(),
            root: root.display().to_string(),
            language: a.graph.language,
        });
        // Remote edges are linked again on the merged graph below.
        append_graph(&mut graph, name, std::sync::Arc::unwrap_or_clone(a.graph));
        let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
        for c in &a.matrix.cells {
            *sizes.entry(c.domain.clone()).or_default() += c.size;
        }
        merged.add(name, a.classification, sizes);
        warnings.extend(a.warnings.into_iter().map(|w| format!("{name}: {w}")));
        elapsed += a.matrix.stats.elapsed_ms;
        metrics.phases.extend(a.metrics.phases);
        metrics.scip_documents += a.metrics.scip_documents;
        metrics.scip_occurrences += a.metrics.scip_occurrences;
    }
    let remote_links = crate::linker::run(&mut graph, true);
    metrics.record_edges(&graph);

    let classification = merged.finish();
    let mut matrix: Matrix = matrix::build(&graph, &classification, &layers, &names[0], elapsed);
    matrix.projects = projects;
    matrix.warnings = warnings.clone();
    Analysis {
        graph: std::sync::Arc::new(graph),
        roots,
        classification,
        matrix,
        link_stats,
        remote_links,
        metrics,
        warnings,
        layers,
        impact_index: Default::default(),
        lenses: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, rel: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "{}").unwrap();
    }

    fn names(v: &[PathBuf]) -> Vec<String> {
        v.iter().map(|p| project_name(p)).collect()
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("structure-expand-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn undetectable_root_becomes_its_children() {
        let d = tmp("kids");
        touch(&d, "backend/pom.xml");
        touch(&d, "frontend/package.json");
        touch(&d, "database/x.sql");
        let out = expand_roots(std::slice::from_ref(&d)).unwrap();
        assert_eq!(names(&out), ["backend", "frontend"]);
    }

    #[test]
    fn depth_two_fallback() {
        let d = tmp("deep");
        touch(&d, "apps/web/package.json");
        touch(&d, "apps/api/pom.xml");
        let out = expand_roots(std::slice::from_ref(&d)).unwrap();
        assert_eq!(names(&out), ["api", "web"]);
    }

    #[test]
    fn detectable_root_is_kept() {
        let d = tmp("maven");
        touch(&d, "pom.xml");
        touch(&d, "module-a/pom.xml");
        touch(&d, "frontend/package.json");
        let out = expand_roots(std::slice::from_ref(&d)).unwrap();
        assert_eq!(out, vec![d]);
    }

    #[test]
    fn skipped_dirs_are_ignored() {
        let d = tmp("skip");
        touch(&d, "node_modules/pkg/package.json");
        touch(&d, ".hidden/package.json");
        touch(&d, "target/pom.xml");
        touch(&d, "app/package.json");
        let out = expand_roots(std::slice::from_ref(&d)).unwrap();
        assert_eq!(names(&out), ["app"]);
    }

    #[test]
    fn nothing_found_is_an_error() {
        let d = tmp("none");
        touch(&d, "docs/readme.md");
        let err = expand_roots(std::slice::from_ref(&d)).unwrap_err();
        assert!(err.to_string().contains("cannot detect language"), "{err}");
    }

    #[test]
    fn mixed_list_keeps_detectable_and_expands_other() {
        let d = tmp("mixed");
        touch(&d, "solo/pom.xml");
        touch(&d, "multi/a/pom.xml");
        touch(&d, "multi/b/package.json");
        let out = expand_roots(&[d.join("solo"), d.join("multi")]).unwrap();
        assert_eq!(names(&out), ["solo", "a", "b"]);
    }

    #[test]
    fn units_split_multi_language_roots_only() {
        let d = tmp("units");
        touch(&d, "both/pom.xml");
        touch(&d, "both/package.json");
        touch(&d, "both/src/A.java");
        touch(&d, "both/web/a.ts");
        touch(&d, "java/pom.xml");
        touch(&d, "java/package.json");
        touch(&d, "java/src/A.java");
        // A directory literally named like a unit gets a suffix.
        touch(&d, "both (java)/tsconfig.json");
        let roots = [d.join("both"), d.join("java"), d.join("both (java)")];
        let got: Vec<(String, Language, String)> = units(&roots)
            .unwrap()
            .into_iter()
            .map(|u| (u.name, u.language, u.index))
            .collect();
        let unit = |n: &str, l, i: &str| (n.to_string(), l, i.to_string());
        assert_eq!(
            got,
            [
                unit("both (java)", Language::Java, "index.java.scip"),
                unit(
                    "both (typescript)",
                    Language::TypeScript,
                    "index.typescript.scip"
                ),
                unit("java", Language::Java, "index.scip"),
                unit("both (java)-2", Language::TypeScript, "index.scip"),
            ]
        );
    }

    #[test]
    fn names_are_deduplicated() {
        let roots: Vec<PathBuf> = ["/a/app", "/b/app", "/c/web", "/d/app"]
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(unique_names(&roots), ["app", "app-2", "web", "app-3"]);
    }

    #[test]
    fn literal_names_cannot_collide_with_suffixed_ones() {
        let roots: Vec<PathBuf> = ["/a/app", "/b/app", "/c/app-2"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let names = unique_names(&roots);
        let set: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(set.len(), 3, "{names:?}");
    }
}
