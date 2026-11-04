//! The walk both tools share (ADR 0004 rules 4 to 7), and the file a scan's result is kept in
//! (rule 10).
//!
//! The walk finds *main clones*: a folder whose `.git` is a directory. It runs no git, follows no
//! symlink and takes its roots as arguments, so tests build a folder tree in a temp dir and
//! never walk the real home directory (rule 13). What a tool does with a found clone (drop the
//! listed and skipped ones, read the facts it wants) is the tool's.

mod store;
mod walk;

pub use store::{Discovery, RootOutcome};
pub use walk::{Bounds, DEFAULT_ROOT_NAMES, RootScan, default_roots, scan_root};
