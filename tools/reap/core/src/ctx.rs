//! The context every operation runs in: where things live, whether this is a
//! dry run, and a log of what was (or would be) done.
//!
//! Reads go straight to git. Every command that changes a clone, its refs or
//! its objects goes through [`Ctx::write`], which in a dry run prints the
//! command instead of running it (rule 14).

use std::cell::RefCell;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Output;

use anyhow::{Context, Result, bail};

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
        self.write_output_with(dir, args, &[])
    }

    /// Like [`Ctx::write_output`], with `env` set for git (a `GIT_INDEX_FILE`, say).
    ///
    /// # Errors
    ///
    /// Fails when git cannot be started.
    pub fn write_output_with(
        &self,
        dir: &Path,
        args: &[&str],
        env: &[(&str, &OsStr)],
    ) -> Result<Option<Output>> {
        let mut words: Vec<String> = env
            .iter()
            .map(|(name, value)| format!("{name}={}", self.paths.tilde(Path::new(value))))
            .collect();
        words.push(format!("git -C {}", self.paths.tilde(dir)));
        words.extend(args.iter().map(|arg| (*arg).to_string()));
        let shown = words.join(" ");
        if self.dry_run {
            self.note(format!("would run: {shown}"));
            return Ok(None);
        }
        self.note(format!("run: {shown}"));
        git::command(&self.paths.git, dir, args)
            .envs(env.iter().copied())
            .output()
            .with_context(|| format!("running git {}", args.join(" ")))
            .map(Some)
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
