use std::path::Path;
use std::process::Command;

use crate::ts;
use structure_ir::ir::Language;

use super::ts_args::{node_heap_mb, node_options, ts_index_args};
use super::{Frontend, Indexer, LanguageFrontend, ParseOutput, find_on_path};

pub const SCIP_TS_VERSION: &str = "0.6.1";

pub struct TypeScriptFrontend;

impl LanguageFrontend for TypeScriptFrontend {
    fn pure(&self) -> &'static dyn Frontend {
        super::pure(Language::TypeScript)
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("tsconfig.json").is_file() || root.join("package.json").is_file()
    }

    fn markers(&self) -> &'static str {
        "tsconfig.json/package.json (TypeScript)"
    }

    fn indexer(&self) -> Option<&dyn Indexer> {
        Some(&ScipTypeScript)
    }

    fn parse_project(
        &self,
        root: &Path,
        _warnings: &mut Vec<String>,
    ) -> anyhow::Result<ParseOutput> {
        ts::parse_project(root).map(ParseOutput::new)
    }
}

/// `@sourcegraph/scip-typescript`, through `npx`.
pub struct ScipTypeScript;

impl Indexer for ScipTypeScript {
    fn cache_tag(&self) -> String {
        format!("scip-typescript-{SCIP_TS_VERSION}")
    }

    fn command(&self, root: &Path, output: &str) -> Option<Command> {
        find_on_path("npx").map(|p| {
            let mut c = Command::new(p);
            c.args(["-y"])
                .arg(format!("@sourcegraph/scip-typescript@{SCIP_TS_VERSION}"))
                .args(["index", "--output", output])
                .args(ts_index_args(root));
            let existing = std::env::var("NODE_OPTIONS").ok();
            c.env(
                "NODE_OPTIONS",
                node_options(existing.as_deref(), node_heap_mb()),
            );
            c
        })
    }
}
