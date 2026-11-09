//! Git helpers: ref resolution, worktree fingerprint, diff impact analysis and display diffs.

pub mod diff;
pub mod impact;
mod refs;

pub use refs::{resolve_base, resolve_commit, worktree_fingerprint};
