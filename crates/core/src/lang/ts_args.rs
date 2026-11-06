use std::path::{Path, PathBuf};

use crate::ts;

pub const DEFAULT_NODE_HEAP_MB: u64 = 8192;

/// `NODE_OPTIONS` value with a heap limit appended to any existing options.
pub fn node_options(existing: Option<&str>, heap_mb: u64) -> String {
    let heap = format!("--max-old-space-size={heap_mb}");
    match existing.map(str::trim) {
        Some(e) if !e.is_empty() => format!("{e} {heap}"),
        _ => heap,
    }
}

pub(super) fn node_heap_mb() -> u64 {
    std::env::var("STRUCTURE_NODE_HEAP_MB")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(DEFAULT_NODE_HEAP_MB)
}

/// Directories holding a `tsconfig.json` under `root` (relative to `root`,
/// `"."` for the root itself), sorted. Skips the dirs the TS frontend ignores.
/// Dirs holding sample or deliberately broken projects for tests: each would
/// cost a full TypeScript program build, so they are not indexed as projects
/// (their files are still parsed by tree-sitter).
const SAMPLE_PROJECT_DIRS: &[&str] = &["fixtures", "test-workspace", "testWorkspace", "testbed"];

pub fn discover_tsconfigs(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = ignore::WalkBuilder::new(root)
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !(e.depth() > 0
                && e.file_type().is_some_and(|t| t.is_dir())
                && (ts::IGNORED_DIRS.contains(&n.as_ref())
                    || SAMPLE_PROJECT_DIRS.contains(&n.as_ref())))
        })
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_name() == "tsconfig.json" && e.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|e| {
            let dir = e.path().parent()?.strip_prefix(root).ok()?;
            Some(if dir.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                dir.to_path_buf()
            })
        })
        .collect();
    out.sort();
    out
}

/// Positional arguments for scip-typescript: nothing when the root has a
/// tsconfig, every discovered project dir otherwise, `--infer-tsconfig` if none.
pub fn ts_index_args(root: &Path) -> Vec<String> {
    if root.join("tsconfig.json").is_file() {
        return Vec::new();
    }
    let found = discover_tsconfigs(root);
    if found.is_empty() {
        return vec!["--infer-tsconfig".into()];
    }
    found
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsconfig_discovery_and_args() {
        let dir = std::env::temp_dir().join(format!("structure-tsc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in [
            "src",
            "build",
            "node_modules/x",
            "ext/a/dist",
            "ext/a/test/fixtures/p1",
        ] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        assert_eq!(ts_index_args(&dir), vec!["--infer-tsconfig"]);
        for d in [
            "src",
            "build",
            "node_modules/x",
            "ext/a/dist",
            "ext/a",
            "ext/a/test/fixtures/p1",
        ] {
            std::fs::write(dir.join(d).join("tsconfig.json"), "{}").unwrap();
        }
        assert_eq!(ts_index_args(&dir), vec!["build", "ext/a", "src"]);
        std::fs::write(dir.join("tsconfig.json"), "{}").unwrap();
        assert!(ts_index_args(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn node_options_appends() {
        assert_eq!(node_options(None, 8192), "--max-old-space-size=8192");
        assert_eq!(
            node_options(Some("--foo"), 1),
            "--foo --max-old-space-size=1"
        );
    }
}
