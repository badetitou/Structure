//! JS glue. `files` is a JSON string `[{"path": "...", "content": "..."}]`.
//!
//! ```js
//! const a = new PrAnalysis(filesJson, diff, project, base, head); // throws on error
//! const impact = a.impact();                     // Impact JSON string, with `matrix`
//! const syms = a.cell_symbols(cellsJson, 0, 50); // CellSymbols JSON string;
//!                                                // cellsJson = '["entry:order", ...]'
//! ```

use crate::{Analysis, analyze_pr};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Deserialize)]
struct FileIn {
    path: String,
    content: String,
}

fn err<E: std::fmt::Display>(e: E) -> JsError {
    JsError::new(&format!("{e:#}"))
}

/// A finished analysis kept in memory.
#[wasm_bindgen]
pub struct PrAnalysis {
    analysis: Analysis,
}

#[wasm_bindgen]
impl PrAnalysis {
    #[wasm_bindgen(constructor)]
    pub fn new(
        files: &str,
        diff: &str,
        project: &str,
        base: &str,
        head: &str,
    ) -> Result<PrAnalysis, JsError> {
        let files: Vec<FileIn> = serde_json::from_str(files).map_err(err)?;
        let files: Vec<(String, String)> = files.into_iter().map(|f| (f.path, f.content)).collect();
        let analysis = analyze_pr(&files, diff, project, base, head).map_err(err)?;
        Ok(PrAnalysis { analysis })
    }

    /// `Impact` JSON (with `matrix`).
    pub fn impact(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.analysis.impact).map_err(err)
    }

    /// `CellSymbols` JSON for the cell ids in the JSON array `cells`.
    pub fn cell_symbols(
        &self,
        cells: &str,
        offset: usize,
        limit: usize,
    ) -> Result<String, JsError> {
        let cells: Vec<String> = serde_json::from_str(cells).map_err(err)?;
        serde_json::to_string(&self.analysis.cell_symbols(&cells, offset, limit)).map_err(err)
    }
}
