use std::path::Path;
use std::process::Command;

use crate::ts;
use structure_ir::ir::{Graph, Language};

use super::ts_args::{node_heap_mb, node_options, ts_index_args};
use super::{LanguageFrontend, find_on_path};

pub const SCIP_TS_VERSION: &str = "0.6.1";

pub struct TypeScriptFrontend;

impl LanguageFrontend for TypeScriptFrontend {
    fn language(&self) -> Language {
        Language::TypeScript
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("tsconfig.json").is_file() || root.join("package.json").is_file()
    }

    fn markers(&self) -> &'static str {
        "tsconfig.json/package.json (TypeScript)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx"]
    }

    fn indexer_command(&self, root: &Path) -> Option<Command> {
        find_on_path("npx").map(|p| {
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
        })
    }

    fn indexer_hint(&self, _root: &Path) -> Option<String> {
        None
    }

    fn parse_project(&self, root: &Path) -> anyhow::Result<Graph> {
        ts::parse_project(root)
    }

    fn default_rules(&self) -> (&'static str, &'static str) {
        (
            include_str!("../../../../rules/typescript.toml"),
            "rules/typescript.toml",
        )
    }

    fn module_separator(&self) -> char {
        '/'
    }
}
