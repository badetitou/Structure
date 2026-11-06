//! Git helpers: ref resolution, worktree fingerprint, and diff impact analysis.

pub mod impact;
mod refs;

pub use refs::{resolve_base, resolve_commit, worktree_fingerprint};
