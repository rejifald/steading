//! `git reap add` / `remove` and the popover's *Add repository…* (rule 1): edit `repos.toml`,
//! and nothing else. `remove` also writes the path to `skip` (repo ADR 0004 rule 3), so a clone
//! just unlisted is not suggested again.

use std::path::Path;

use anyhow::{Result, bail};

use crate::config::{Config, RepoEntry};
use crate::git;
use crate::paths::{Paths, absolute, same_path};

/// Lists the main clone at `path`; returns it as written (`~/…`).
///
/// # Errors
///
/// Refuses a path that is not a clone's top level, is a linked worktree, or is already listed.
pub fn add(
    paths: &Paths,
    path: &Path,
    remote: Option<String>,
    base: Option<String>,
) -> Result<String> {
    let path = absolute(path);
    if !path.is_dir() {
        bail!("{} is not a folder", paths.tilde(&path));
    }
    let top_level = git::path_of(&paths.git, &path, &["rev-parse", "--show-toplevel"])
        .map_err(|_| anyhow::anyhow!("{} is not a git clone", paths.tilde(&path)))?;
    if !same_path(&top_level, &path) {
        bail!(
            "{} is inside a clone; list its top level, {}",
            paths.tilde(&path),
            paths.tilde(&absolute(&top_level))
        );
    }
    let git_dir = git::path_of(&paths.git, &path, &["rev-parse", "--absolute-git-dir"])?;
    let common_dir = git::path_of(&paths.git, &path, &["rev-parse", "--git-common-dir"])?;
    if !same_path(&git_dir, &common_dir) {
        bail!(
            "{} is a linked worktree; list its main clone instead",
            paths.tilde(&path)
        );
    }
    let mut config = Config::load(paths)?;
    let listed = config.repos(paths)?;
    if listed.iter().any(|repo| same_path(&repo.path, &path)) {
        bail!("{} is already listed", paths.tilde(&path));
    }
    let written = paths.tilde(&path);
    config.repo.push(RepoEntry {
        path: written.clone(),
        remote,
        base,
        ..RepoEntry::default()
    });
    config.save(paths)?;
    Ok(written)
}

/// Unlists a repo by name or path and adds it to `skip`, in one save; returns it as it was
/// written, or `None` when not listed. Its clone, worktrees and state are left as they are.
///
/// # Errors
///
/// Fails when `repos.toml` cannot be read or written.
pub fn remove(paths: &Paths, selector: &str) -> Result<Option<String>> {
    let mut config = Config::load(paths)?;
    let repos = config.repos(paths)?;
    let Some(at) = repos.iter().position(|repo| repo.matches(selector, paths)) else {
        return Ok(None);
    };
    let removed = config.repo.remove(at);
    config.add_skip(paths, &removed.path);
    config.save(paths)?;
    Ok(Some(removed.path))
}
