//! Shared, bounded filesystem search primitives.
//!
//! The traversal is built on the `ignore` crate so `.gitignore`, nested
//! `.gitignore`, `.ignore` and Git exclude rules are applied before a
//! directory is entered. Filters and execution control stay in Runtime so the
//! WorkspaceGuard remains the only filesystem boundary authority.

pub(crate) mod control;
pub(crate) mod filters;
pub(crate) mod walker;
