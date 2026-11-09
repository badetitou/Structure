//! Structure core library: the analysis pipeline over the focused crates.
//! The per-concern modules are re-exported here so consumers see one API.

pub mod lang;
pub mod pipeline;

pub use structure_classify::{classify, matrix};
pub use structure_git::{diff, impact};
pub use structure_ir::{ir, model};
pub use structure_lang_java as java;
pub use structure_lang_ts as ts;
pub use structure_scip::{link, metrics, scip};
