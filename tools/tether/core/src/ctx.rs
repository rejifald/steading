//! The context every operation runs in: where things live, whether this is a
//! dry run, and a log of what was (or would be) done.
//!
//! Reads go straight to git. Every command that changes a clone, its refs or
//! its objects goes through [`Ctx::write`], which in a dry run prints the
//! command instead of running it (rule 13).

use std::cell::RefCell;
use std::path::Path;
use std::process::Output;
use std::time::Duration;

use anyhow::{Result, bail};

use crate::config::Config;
use crate::git;
use crate::paths::Paths;

/// How long a repo's `gate` may run before the sync skips it (ADR 0004 rule 2).
pub const GATE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Ctx {
    pub paths: Paths,
    pub dry_run: bool,
    /// [`GATE_TIMEOUT`]; tests shorten it.
    pub gate_timeout: Duration,
    log: RefCell<Vec<String>>,
}

impl Ctx {
    #[must_use]
    pub fn new(paths: Paths, dry_run: bool) -> Self {
        Self {
            paths,
            dry_run,
            gate_timeout: GATE_TIMEOUT,
            log: RefCell::new(Vec::new()),
        }
    }

    /// # Errors
    ///
    /// Fails when the config cannot be loaded.
    pub fn config(&self) -> Result<Config> {
        Config::load(&self.paths)
    }

    pub fn note(&self, line: impl Into<String>) {
        self.log.borrow_mut().push(line.into());
    }

    pub fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut self.log.borrow_mut())
    }

    /// # Errors
    ///
    /// Fails when git cannot be run or exits non-zero.
    pub fn read(&self, dir: &Path, args: &[&str]) -> Result<String> {
        git::run(&self.paths.git, dir, args)
    }

    pub fn read_ok(&self, dir: &Path, args: &[&str]) -> bool {
        git::ok(&self.paths.git, dir, args)
    }

    pub fn resolve(&self, dir: &Path, refname: &str) -> Option<String> {
        git::resolve(&self.paths.git, dir, refname)
    }

    /// A mutating git command: logged, and only printed in a dry run. Returns
    /// the raw output (a dry run returns `None`) so callers can read a refusal.
    ///
    /// # Errors
    ///
    /// Fails when git cannot be run.
    pub fn write_output(&self, dir: &Path, args: &[&str]) -> Result<Option<Output>> {
        let shown = format!("git -C {} {}", self.paths.tilde(dir), args.join(" "));
        if self.dry_run {
            self.note(format!("would run: {shown}"));
            return Ok(None);
        }
        self.note(format!("run: {shown}"));
        git::output(&self.paths.git, dir, args).map(Some)
    }

    /// Like [`Ctx::write_output`] but a non-zero exit is an error.
    ///
    /// # Errors
    ///
    /// Fails when git cannot be run or exits non-zero; a dry run never fails.
    pub fn write(&self, dir: &Path, args: &[&str]) -> Result<()> {
        if let Some(out) = self.write_output(dir, args)?
            && !out.status.success()
        {
            bail!(
                "git {} failed in {}: {}",
                args.join(" "),
                dir.display(),
                git::first_error_line(&out)
            );
        }
        Ok(())
    }
}
