//! Checks that the TypeScript declarations of the wire types, committed under
//! `app/ui/src/generated/`, match the Rust model (ts-rs, feature `ts`).
//!
//! Regenerate with:
//! `UPDATE_TS_TYPES=1 cargo test -p structure-ir generated_ts_types`

use crate::model::{CellSymbols, Diff, History, Impact, Matrix, Progress};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use ts_rs::{Config, TS};

/// Committed declarations, relative to this crate.
const GENERATED: &str = "../../app/ui/src/generated";

/// Exports the types returned by the Tauri commands, and everything they use.
fn export(dir: &Path) -> Result<(), ts_rs::ExportError> {
    // JSON numbers, not bigint: the values stay far below 2^53.
    let cfg = Config::new().with_large_int("number").with_out_dir(dir);
    Matrix::export_all(&cfg)?;
    Impact::export_all(&cfg)?;
    History::export_all(&cfg)?;
    Diff::export_all(&cfg)?;
    CellSymbols::export_all(&cfg)?;
    Progress::export_all(&cfg)?;
    Ok(())
}

/// `.ts` files of `dir` by name (an absent directory reads as empty).
fn read_dir(dir: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    if !dir.exists() {
        return files;
    }
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "ts") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            files.insert(name, fs::read_to_string(&path).unwrap());
        }
    }
    files
}

#[test]
fn generated_ts_types() {
    let committed_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GENERATED);
    if std::env::var_os("UPDATE_TS_TYPES").is_some() {
        for name in read_dir(&committed_dir).keys() {
            fs::remove_file(committed_dir.join(name)).unwrap();
        }
        export(&committed_dir).unwrap();
        return;
    }
    let fresh_dir = std::env::temp_dir().join(format!("structure-ts-{}", std::process::id()));
    let _ = fs::remove_dir_all(&fresh_dir);
    export(&fresh_dir).unwrap();
    let fresh = read_dir(&fresh_dir);
    let _ = fs::remove_dir_all(&fresh_dir);
    let committed = read_dir(&committed_dir);
    let stale: BTreeSet<&String> = fresh
        .keys()
        .chain(committed.keys())
        .filter(|name| fresh.get(*name) != committed.get(*name))
        .collect();
    assert!(
        stale.is_empty(),
        "app/ui/src/generated is out of date with crates/ir (differs: {stale:?}); regenerate with:\n  \
         UPDATE_TS_TYPES=1 cargo test -p structure-ir generated_ts_types"
    );
}
