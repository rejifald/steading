//! tether core (ADR 0001): keeps the clones an operator lists on one branch
//! each, by fast-forward (`follow`) or by forcing a shallow copy (`mirror`).
//! No UI dependencies; the tray app and the CLI both call into this crate.

pub mod add;
pub mod cli;
pub mod config;
pub mod ctx;
pub mod follow;
pub mod git;
pub mod inspect;
pub mod lock;
pub mod mirror;
pub mod paths;
pub mod record;
pub mod report;
pub mod shell;
pub mod sync;

pub use ctx::Ctx;
pub use paths::Paths;
