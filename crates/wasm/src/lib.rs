//! Pull-request analysis fully in memory, for WebAssembly (no filesystem, git
//! or SCIP). The pure core is [`analyze_pr`]; [`wasm`] is the thin JS glue.

use std::collections::BTreeMap;

use anyhow::{Context, bail};
use structure_classify::classify::{RuleSet, classify};
use structure_classify::layers::Layers;
use structure_classify::matrix;
use structure_classify::merge::{MergedClassification, append_graph, merge_layers};
use structure_git::changes_from_diff;
use structure_git::impact::{self, FileChange};
use structure_ir::ir::{Graph, Language};
use structure_ir::model::{CellSymbols, Classification, Impact, Matrix, Project};
use structure_lang::Frontend;

mod wasm;

/// Language of a set of paths (see [`structure_lang::detect_by_paths`]).
pub use structure_lang::detect_by_paths as detect_language;

/// Result of [`analyze_pr`].
pub struct Analysis {
    pub graph: Graph,
    pub classification: Classification,
    /// Impact of the diff; `matrix` is always `Some`.
    pub impact: Impact,
}

impl Analysis {
    /// Top-level symbols of `cells`, `limit` from `offset`.
    pub fn cell_symbols(&self, cells: &[String], offset: usize, limit: usize) -> CellSymbols {
        matrix::cell_symbols(&self.graph, &self.classification, cells, offset, limit)
    }
}

/// The languages of a set of paths, in registry order: those with at least
/// one source (a path counting for the first frontend of
/// [`structure_lang::all`] that accepts it, as in [`detect_language`]).
/// Fails when no path is a source.
pub fn detect_languages<'a>(
    paths: impl IntoIterator<Item = &'a str>,
) -> anyhow::Result<Vec<Language>> {
    let frontends = structure_lang::all();
    let mut seen = vec![false; frontends.len()];
    for p in paths {
        if let Some(i) = frontends.iter().position(|f| f.is_source(p)) {
            seen[i] = true;
        }
    }
    let langs: Vec<Language> = frontends
        .iter()
        .zip(seen)
        .filter(|(_, s)| *s)
        .map(|(f, _)| f.language())
        .collect();
    if langs.is_empty() {
        // The registry's own "no supported sources: expected ..." error.
        detect_language(std::iter::empty())?;
    }
    Ok(langs)
}

/// One language of the PR analysed alone.
struct Part {
    graph: Graph,
    classification: Classification,
    layers: Layers,
    matrix: Matrix,
    warnings: Vec<String>,
}

fn analyze_language(
    lang: &dyn Frontend,
    project: &str,
    files: &[(String, String)],
) -> anyhow::Result<Part> {
    let mut warnings = Vec::new();
    let graph = lang.parse_sources(project, files, &mut warnings)?.graph;
    let (toml, name) = lang.default_rules();
    let rules = RuleSet::from_embedded(toml, name).context("loading default rules")?;
    let classification = classify(&graph, &rules, lang.module_separator());
    let layers = rules.layers();
    let mut matrix = matrix::build(&graph, &classification, &layers, project, 0);
    matrix.warnings = warnings.clone();
    Ok(Part {
        graph,
        classification,
        layers,
        matrix,
        warnings,
    })
}

/// Serialized id of `lang` (`java`, `typescript`, `pharo`).
fn language_id(lang: Language) -> anyhow::Result<String> {
    match serde_json::to_value(lang)? {
        serde_json::Value::String(s) => Ok(s),
        v => bail!("unexpected language id {v}"),
    }
}

/// Analyse the sources of a PR head (`(path, content)`) against its unified
/// `diff`, with the default rules of each language found
/// ([`detect_languages`]). One language is analysed as one project named
/// `project`. Several are analysed as the desktop analyses a root holding
/// several languages: one unit per language, named `<repo> (<language>)`
/// (`repo` being the last `/` segment of `project`), merged into one graph
/// and matrix (ids `<unit>::<id>`, files `<unit>/<file>`, domains
/// `<unit>/<domain>`, one entry per unit in `matrix.projects`); the diff then
/// applies to every unit and `changed_files` keeps the diff's paths. No
/// cross-language (HTTP, RPC) links are made.
pub fn analyze_pr(
    files: &[(String, String)],
    diff: &str,
    project: &str,
    base: &str,
    head: &str,
) -> anyhow::Result<Analysis> {
    let langs = detect_languages(files.iter().map(|(p, _)| p.as_str()))?;
    let changes = changes_from_diff(diff);
    if let [lang] = langs.as_slice() {
        let part = analyze_language(structure_lang::frontend(*lang), project, files)?;
        let mut impact = impact::compute(
            &part.graph,
            &part.classification,
            &changes,
            base,
            Some(head),
        );
        impact.warnings = part.warnings;
        impact.matrix = Some(part.matrix);
        return Ok(Analysis {
            graph: part.graph,
            classification: part.classification,
            impact,
        });
    }

    let repo = project.rsplit('/').next().unwrap_or(project);
    let mut names = Vec::with_capacity(langs.len());
    let mut parts = Vec::with_capacity(langs.len());
    for &lang in &langs {
        let name = format!("{repo} ({})", language_id(lang)?);
        let part = analyze_language(structure_lang::frontend(lang), project, files)
            .with_context(|| format!("project {name}"))?;
        names.push(name);
        parts.push(part);
    }

    let Some(first) = parts.first() else {
        bail!("no supported sources");
    };
    let mut graph = Graph {
        root: first.graph.root.clone(),
        language: first.graph.language,
        symbols: vec![],
        edges: vec![],
    };
    let layers = merge_layers(parts.iter().map(|p| &p.layers));
    let mut merged = MergedClassification::default();
    let mut projects = Vec::with_capacity(parts.len());
    let mut warnings = Vec::new();
    for (name, part) in names.iter().zip(parts) {
        projects.push(Project {
            name: name.clone(),
            root: project.to_string(),
            language: part.graph.language,
        });
        append_graph(&mut graph, name, part.graph);
        let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
        for c in &part.matrix.cells {
            *sizes.entry(c.domain.clone()).or_default() += c.size;
        }
        merged.add(name, part.classification, sizes);
        warnings.extend(part.warnings.into_iter().map(|w| format!("{name}: {w}")));
    }
    let classification = merged.finish();
    let mut matrix = matrix::build(&graph, &classification, &layers, project, 0);
    matrix.projects = projects;
    matrix.warnings = warnings.clone();

    let unit_changes: Vec<FileChange> = names
        .iter()
        .flat_map(|name| {
            changes.iter().map(move |c| FileChange {
                file: format!("{name}/{}", c.file),
                lines: c.lines.clone(),
            })
        })
        .collect();
    let mut impact = impact::compute(&graph, &classification, &unit_changes, base, Some(head));
    let mut changed: Vec<String> = changes.into_iter().map(|c| c.file).collect();
    changed.sort();
    changed.dedup();
    impact.changed_files = changed;
    impact.warnings = warnings;
    impact.matrix = Some(matrix);
    Ok(Analysis {
        graph,
        classification,
        impact,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use structure_ir::ir::Language;

    fn f(p: &str, c: &str) -> (String, String) {
        (p.to_string(), c.to_string())
    }

    fn diff_for(path: &str) -> String {
        format!(
            "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1,3 +1,3 @@\n a\n-b\n+c\n d\n"
        )
    }

    fn cell_of(a: &Analysis, id: &str) -> String {
        matrix::cell_of(&a.graph, &a.classification, id).expect("cell")
    }

    #[test]
    fn java_changed_and_impacted() {
        let files = vec![
            f(
                "src/main/java/p/Svc.java",
                "package p;\n@Service\npublic class Svc {\n    public int run() {\n        return 1;\n    }\n}\n",
            ),
            f(
                "src/main/java/p/Ctl.java",
                "package p;\n@RestController\npublic class Ctl {\n    private Svc svc;\n    public int get() {\n        return svc.run();\n    }\n}\n",
            ),
        ];
        let a = analyze_pr(
            &files,
            &diff_for("src/main/java/p/Svc.java"),
            "demo",
            "b",
            "h",
        )
        .unwrap();
        let svc = cell_of(&a, "p.Svc");
        let ctl = cell_of(&a, "p.Ctl");
        assert_ne!(svc, ctl);
        assert!(a.impact.changed_cells.contains(&svc));
        assert!(a.impact.impacted_cells.iter().any(|c| c.id == ctl));
        let m = a.impact.matrix.as_ref().expect("matrix");
        assert!(!m.cells.is_empty());
        assert_eq!(a.cell_symbols(&[svc], 0, 10).total, 1);
    }

    #[test]
    fn typescript_sample() {
        let files = vec![
            f(
                "src/a.service.ts",
                "export class AService {\n  run(): number {\n    return 1;\n  }\n}\n",
            ),
            f(
                "src/b.component.ts",
                "import { AService } from './a.service';\n@Component({})\nexport class BComponent {\n  constructor(private s: AService) {}\n  go() { return this.s.run(); }\n}\n",
            ),
        ];
        let a = analyze_pr(&files, &diff_for("src/a.service.ts"), "demo", "b", "h").unwrap();
        assert!(!a.impact.changed_cells.is_empty());
        assert!(a.impact.matrix.is_some());
        assert!(!a.impact.impacted_cells.is_empty());
    }

    #[test]
    fn pharo_changed_and_impacted() {
        let files = vec![
            f(
                "src/BaselineOfDemo/BaselineOfDemo.class.st",
                "Class { #name : 'BaselineOfDemo', #superclass : 'BaselineOf', #category : 'BaselineOfDemo' }\n",
            ),
            f(
                "src/Demo-Model/package.st",
                "Package { #name : 'Demo-Model' }\n",
            ),
            f(
                "src/Demo-Model/DmItem.class.st",
                "Class {\n\t#name : 'DmItem',\n\t#superclass : 'Object',\n\t#category : 'Demo-Model'\n}\n\n{ #category : 'accessing' }\nDmItem >> price [\n\t^ 1\n]\n",
            ),
            f(
                "src/Demo-UI/DmPresenter.class.st",
                "Class {\n\t#name : 'DmPresenter',\n\t#superclass : 'SpPresenter',\n\t#category : 'Demo-UI'\n}\n\n{ #category : 'rendering' }\nDmPresenter >> total [\n\t^ DmItem new price\n]\n",
            ),
        ];
        let a = analyze_pr(
            &files,
            &diff_for("src/Demo-Model/DmItem.class.st"),
            "demo",
            "b",
            "h",
        )
        .unwrap();
        assert_eq!(a.graph.language, structure_ir::ir::Language::Pharo);
        let item = cell_of(&a, "DmItem");
        assert!(a.impact.changed_cells.contains(&item));
        assert!(
            a.impact
                .impacted_cells
                .iter()
                .any(|c| c.id == cell_of(&a, "DmPresenter"))
        );
    }

    #[test]
    fn mixed_picks_majority() {
        let p = [
            "src/P/A.class.st",
            "src/P/B.class.st",
            "src/P/package.st",
            "x.ts",
        ];
        assert_eq!(detect_language(p).unwrap(), Language::Pharo);
        let p = ["a.ts", "b.tsx", "C.java"];
        assert_eq!(detect_language(p).unwrap(), Language::TypeScript);
        let p = ["a.ts", "B.java", "C.java"];
        assert_eq!(detect_language(p).unwrap(), Language::Java);
    }

    #[test]
    fn languages_with_sources_in_registry_order() {
        let p = ["x.ts", "src/P/A.class.st", "B.java", "README.md"];
        assert_eq!(
            detect_languages(p).unwrap(),
            [Language::Java, Language::TypeScript, Language::Pharo]
        );
        assert_eq!(
            detect_languages(["a.ts", "src/P/package.st"]).unwrap(),
            [Language::TypeScript]
        );
        let err = detect_languages(["README.md"]).unwrap_err();
        assert!(err.to_string().contains("no supported sources"), "{err}");
    }

    #[test]
    fn mixed_languages_are_split_and_merged() {
        let files = vec![
            f(
                "back/src/main/java/p/Svc.java",
                "package p;\n@Service\npublic class Svc {\n    public int run() {\n        return 1;\n    }\n}\n",
            ),
            f(
                "back/src/main/java/p/Ctl.java",
                "package p;\n@RestController\npublic class Ctl {\n    private Svc svc;\n    public int get() {\n        return svc.run();\n    }\n}\n",
            ),
            f(
                "front/src/a.service.ts",
                "export class AService {\n  run(): number {\n    return 1;\n  }\n}\n",
            ),
            f(
                "front/src/b.component.ts",
                "import { AService } from './a.service';\n@Component({})\nexport class BComponent {\n  constructor(private s: AService) {}\n  go() { return this.s.run(); }\n}\n",
            ),
        ];
        // The PR only touches the minority language.
        let a = analyze_pr(
            &files,
            &diff_for("front/src/a.service.ts"),
            "owner/demo",
            "b",
            "h",
        )
        .unwrap();
        let m = a.impact.matrix.as_ref().expect("matrix");
        assert_eq!(m.project, "owner/demo");
        let projects: Vec<(&str, Language)> = m
            .projects
            .iter()
            .map(|p| (p.name.as_str(), p.language))
            .collect();
        assert_eq!(
            projects,
            [
                ("demo (java)", Language::Java),
                ("demo (typescript)", Language::TypeScript)
            ]
        );
        assert!(a.graph.symbols.iter().any(|s| s.id == "demo (java)::p.Svc"));
        assert!(
            a.graph
                .symbols
                .iter()
                .any(|s| s.file == "demo (typescript)/front/src/a.service.ts")
        );
        assert_eq!(a.impact.changed_files, ["front/src/a.service.ts"]);
        let svc_ts = a
            .graph
            .symbols
            .iter()
            .find(|s| s.file.ends_with("a.service.ts") && s.container.is_none())
            .expect("AService");
        let changed = cell_of(&a, &svc_ts.id);
        assert!(changed.contains("demo (typescript)/"), "{changed}");
        assert_eq!(a.impact.changed_cells, [changed]);
        assert!(!a.impact.impacted_cells.is_empty());
        let java_cell = cell_of(&a, "demo (java)::p.Svc");
        assert!(!a.impact.impacted_cells.iter().any(|c| c.id == java_cell));
        assert!(m.cells.iter().any(|c| c.id == java_cell));
    }

    #[test]
    fn no_supported_sources() {
        let err = analyze_pr(&[f("README.md", "x")], "", "d", "b", "h")
            .err()
            .expect("error");
        assert!(err.to_string().contains("no supported sources"));
    }
}
