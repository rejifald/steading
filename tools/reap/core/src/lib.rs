//! `reap-core`: finds the linked worktrees of the clones an operator lists and decides which
//! ones have merged and can go. The spec is ADR 0001; rule numbers in this crate refer to it.
//! No UI dependencies; the tray app and the CLI both call into this crate.

pub mod add;
pub mod classify;
pub mod cli;
pub mod config;
pub mod ctx;
pub mod disk_usage;
pub mod fetch;
pub mod git;
pub mod github;
pub mod in_use;
pub mod lock;
pub mod paths;
pub mod report;
pub mod salvage;
pub mod state_file;
pub mod sweep;
pub mod worktree_list;

pub use paths::Paths;
