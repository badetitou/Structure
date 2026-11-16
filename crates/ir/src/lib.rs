//! Shared contracts: the language-agnostic graph IR, the output model and
//! the highlight model.

pub mod highlight;
pub mod ir;
pub mod model;

#[cfg(all(test, feature = "ts"))]
mod ts;
