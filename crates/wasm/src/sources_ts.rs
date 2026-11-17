//! Checks that the source extensions the browser extension keeps from the PR
//! archive, committed as `app/ui/src/extension/sources.generated.ts`, are
//! those of the language registry ([`structure_lang::all`]). The extension
//! needs them before the wasm module is loaded (the archive is read while it
//! loads), hence a committed constant rather than a wasm export.
//!
//! Regenerate with:
//! `UPDATE_SOURCE_EXTENSIONS=1 cargo test -p structure-wasm generated_source_extensions`

use std::fs;
use std::path::PathBuf;

/// Committed constant, relative to this crate.
const GENERATED: &str = "../../app/ui/src/extension/sources.generated.ts";

/// The TypeScript module: every extension of the registry, in order, with
/// its dot, and the same list as text for messages.
fn render() -> String {
    let exts: Vec<String> = structure_lang::all()
        .iter()
        .flat_map(|f| f.extensions().iter().map(|e| format!(".{e}")))
        .collect();
    let list = exts
        .iter()
        .map(|e| format!("\"{e}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let text = match exts.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => String::new(),
    };
    format!(
        "// Generated from the language registry (crates/lang) by the structure-wasm test\n\
         // `generated_source_extensions`. Do not edit; regenerate with:\n\
         // UPDATE_SOURCE_EXTENSIONS=1 cargo test -p structure-wasm generated_source_extensions\n\
         \n\
         /** Source file extensions of the supported languages, in registry order. */\n\
         export const SOURCE_EXTENSIONS: readonly string[] = [{list}];\n\
         \n\
         /** The same list for messages. */\n\
         export const SOURCE_EXTENSIONS_TEXT = \"{text}\";\n"
    )
}

#[test]
fn generated_source_extensions() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GENERATED);
    let fresh = render();
    if std::env::var_os("UPDATE_SOURCE_EXTENSIONS").is_some() {
        fs::write(&path, &fresh).unwrap();
        return;
    }
    let committed = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == fresh,
        "app/ui/src/extension/sources.generated.ts is out of date with the language registry; \
         regenerate with:\n  \
         UPDATE_SOURCE_EXTENSIONS=1 cargo test -p structure-wasm generated_source_extensions"
    );
}
