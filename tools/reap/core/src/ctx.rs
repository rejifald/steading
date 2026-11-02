//! The context every operation runs in: where things live, whether this is a
//! dry run, and a log of what was (or would be) done.
//!
//! Reads go straight to git. Every command that changes a clone, its refs or
//! its objects goes through [`Ctx::write`], which in a dry run prints the
//! command instead of running it (rule 14).

use std::cell::RefCell;
use std::path::Path;
use std::process::Output;

use anyhow::{Result, bail};

use crate::git;
use crate::paths::Paths;

pub struct Ctx {
    pub paths: Paths,
    pub dry_run: bool,
    log: RefCell<Vec<String>>,
}

impl Ctx {
    #[must_use]
    pub fn new(paths: Paths, dry_run: bool) -> Self {
        Self {
            paths,
            dry_run,
            log: RefCell::new(Vec::new()),
        }
    }

    pub fn note(&self, line: impl Into<String>) {
        self.log.borrow_mut().push(line.into());
    }

    #[must_use]
    pub fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut self.log.borrow_mut())
    }

    /// A mutating git command: logged, and only printed in a dry run. Returns
    /// the raw output (a dry run returns `None`) so callers can read a refusal.
    ///
    /// # Errors
    ///
    /// Fails when git cannot be started.
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
    /// Fails when git cannot be started or exits non-zero.
    pub fn write(&self, dir: &Path, args: &[&str]) -> Result<()> {
        if let Some(output) = self.write_output(dir, args)?
            && !output.status.success()
        {
            bail!(
                "git {} failed in {}: {}",
                args.join(" "),
                dir.display(),
                git::first_error_line(&output)
            );
        }
        Ok(())
    }
}
