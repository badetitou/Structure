//! Pull-request analysis fully in memory, for WebAssembly (no filesystem, git
//! or SCIP). The pure core is [`analyze_pr`]; [`wasm`] is the thin JS glue.

use anyhow::{Context, bail};
use structure_classify::classify::{RuleSet, classify};
use structure_classify::matrix;
use structure_git::changes_from_diff;
use structure_git::impact;
use structure_ir::ir::Graph;
use structure_ir::model::{CellSymbols, Classification, Impact};

mod wasm;

const JAVA_RULES: &str = include_str!("../../../rules/java.toml");
const TS_RULES: &str = include_str!("../../../rules/typescript.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Java,
    TypeScript,
}

/// Language of a set of paths: the one with more files (Java wins a tie).
pub fn detect_language<'a>(paths: impl IntoIterator<Item = &'a str>) -> anyhow::Result<Lang> {
    let (mut java, mut ts) = (0usize, 0usize);
    for p in paths {
        if p.ends_with(".java") {
            java += 1;
        } else if p.ends_with(".ts") || p.ends_with(".tsx") {
            ts += 1;
        }
    }
    match (java, ts) {
        (0, 0) => bail!("no supported sources: expected .java, .ts or .tsx files"),
        (j, t) if j >= t => Ok(Lang::Java),
        _ => Ok(Lang::TypeScript),
    }
}

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

/// Analyse the sources of a PR head (`(path, content)`) against its unified
/// `diff`, with the default rules of the detected language.
pub fn analyze_pr(
    files: &[(String, String)],
    diff: &str,
    project: &str,
    base: &str,
    head: &str,
) -> anyhow::Result<Analysis> {
    let lang = detect_language(files.iter().map(|(p, _)| p.as_str()))?;
    let mut warnings = Vec::new();
    let (graph, rules, sep) = match lang {
        Lang::Java => (
            structure_lang_java::parse_sources(project, files, &mut warnings)?,
            JAVA_RULES,
            '.',
        ),
        Lang::TypeScript => (
            structure_lang_ts::parse_sources(project, files)?,
            TS_RULES,
            '/',
        ),
    };
    let rules = RuleSet::from_embedded(rules, "default rules").context("loading default rules")?;
    let classification = classify(&graph, &rules, sep);
    let mut matrix = matrix::build(&graph, &classification, project, 0);
    matrix.warnings = warnings.clone();
    let changes = changes_from_diff(diff);
    let mut impact = impact::compute(&graph, &classification, &changes, base, Some(head));
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
    fn mixed_picks_majority() {
        let p = ["a.ts", "b.tsx", "C.java"];
        assert_eq!(detect_language(p).unwrap(), Lang::TypeScript);
        let p = ["a.ts", "B.java", "C.java"];
        assert_eq!(detect_language(p).unwrap(), Lang::Java);
    }

    #[test]
    fn no_supported_sources() {
        let err = analyze_pr(&[f("README.md", "x")], "", "d", "b", "h")
            .err()
            .expect("error");
        assert!(err.to_string().contains("no supported sources"));
    }
}
