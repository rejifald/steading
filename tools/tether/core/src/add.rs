//! `tether add`, `tether set` and `tether remove`: the only writers of `repos.toml` besides
//! the operator. `add` refuses concord's clone without a gate (ADR 0004 rule 6)
//! and what rule 8 says a mirror may not adopt.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::config::{self, DEFAULT_REMOTE, Mode, Repo, RepoEntry, absolute, same_path};
use crate::ctx::Ctx;
use crate::inspect;
use crate::{git, mirror, record, sync};

#[derive(Debug, Clone)]
pub struct AddOpts {
    pub path: PathBuf,
    pub branch: String,
    pub mode: Mode,
    pub remote: Option<String>,
    pub interval: Option<String>,
    pub url: Option<String>,
    pub gate: Option<String>,
    pub after: Option<String>,
}

/// Lists a repo. A new or empty `mirror` path is cloned here; anything else
/// must already be a main clone whose remote has the branch.
///
/// # Errors
///
/// Fails when the config cannot be read or saved, the path is already listed, a `--gate`, `--after` or `--interval` is invalid, concord's clone is listed without a gate, a mirror clone fails, the clone is not a main clone whose remote has the branch, or an existing clone cannot become a mirror.
pub fn add(ctx: &Ctx, opts: &AddOpts) -> Result<Repo> {
    let path = absolute(&ctx.paths.expand(&opts.path.to_string_lossy()));
    let shown = ctx.paths.tilde(&path);
    let mut cfg = ctx.config()?;
    let listed = cfg.repos(&ctx.paths)?;
    if listed.iter().any(|r| same_path(&r.path, &path)) {
        bail!("{shown} is already listed; `git tether remove` it first to change it");
    }
    let gate = config::parse_command("gate", opts.gate.as_deref())?;
    let after = config::parse_command("after", opts.after.as_deref())?;
    if gate.is_none() && ctx.paths.concord_installed() && same_path(&path, &ctx.paths.concord_repo)
    {
        bail!(
            "{shown} is concord's clone and concord is installed: list it with a --gate that waits for concord, so it is never pulled under a running session (ADR 0004 rule 6)"
        );
    }
    if let Some(i) = &opts.interval {
        config::parse_interval(i)?;
    }
    let entry = RepoEntry {
        path: shown.clone(),
        branch: opts.branch.clone(),
        mode: opts.mode,
        remote: opts.remote.clone().filter(|r| r != DEFAULT_REMOTE),
        interval: opts.interval.clone(),
        url: opts.url.clone(),
        gate: gate.clone(),
        after: after.clone(),
    };
    let repo = Repo {
        path: path.clone(),
        branch: opts.branch.clone(),
        mode: opts.mode,
        remote: opts.remote.clone().unwrap_or_else(|| DEFAULT_REMOTE.into()),
        interval: opts
            .interval
            .as_deref()
            .map(config::parse_interval)
            .transpose()?
            .unwrap_or(config::DEFAULT_INTERVAL),
        url: opts.url.clone(),
        gate,
        after,
    };
    if repo.mode == Mode::Mirror && mirror::needs_fresh_clone(&path) {
        if repo.url.is_none() {
            bail!("{shown} is missing or empty: pass --url <clone-url> to clone the mirror there");
        }
        let mut out = sync::Outcome::new(ctx, &repo);
        mirror::clone_fresh(ctx, &repo, &mut out)
            .map_err(|r| anyhow::anyhow!("cloning {shown}: {r}"))?;
    } else {
        inspect::git_dir(ctx, &path).map_err(|r| anyhow::anyhow!("{r}"))?;
        match git::ls_remote(&ctx.paths.git, &path, &repo.remote, &repo.branch) {
            Ok(Some(_)) => {}
            Ok(None) => bail!("{} has no branch {}", repo.remote, repo.branch),
            Err(e) => bail!("cannot reach {}: {e:#}", repo.remote),
        }
        if repo.mode == Mode::Mirror {
            let found = adoption_blockers(ctx, &repo)?;
            if !found.is_empty() {
                bail!(
                    "{shown} cannot become a mirror, which discards local work on every sync: {}",
                    found.join("; ")
                );
            }
        }
    }
    cfg.repo.push(entry);
    cfg.save(&ctx.paths)?;
    ctx.note(format!(
        "listed {shown} ({} {})",
        repo.mode.label(),
        repo.branch
    ));
    Ok(repo)
}

/// Rule 8, second half: everything an existing clone holds that a mirror sync
/// would lose. Fetches the remote first so "missing from the remote" is current.
///
/// # Errors
///
/// Fails when a git command used to inspect or fetch the clone fails.
pub fn adoption_blockers(ctx: &Ctx, repo: &Repo) -> Result<Vec<String>> {
    let dir = &repo.path;
    let mut found = Vec::new();
    let worktrees = ctx.read(dir, &["worktree", "list", "--porcelain"])?;
    let linked = worktrees
        .lines()
        .filter(|l| l.starts_with("worktree "))
        .count()
        .saturating_sub(1);
    if linked > 0 {
        found.push(format!("{linked} linked worktree(s)"));
    }
    if ctx.resolve(dir, "refs/stash").is_some() {
        let n = ctx
            .read(dir, &["stash", "list"])
            .map_or(1, |s| s.lines().count());
        found.push(format!(
            "{n} stash entr{}",
            if n == 1 { "y" } else { "ies" }
        ));
    }
    let others: Vec<String> = git::local_branches(&ctx.paths.git, dir)?
        .into_iter()
        .filter(|b| *b != repo.branch)
        .collect();
    if !others.is_empty() {
        found.push(format!(
            "local branch(es) other than {}: {}",
            repo.branch,
            others.join(", ")
        ));
    }
    ctx.write(dir, &["fetch", "--quiet", &repo.remote])?;
    let remotes = format!("--remotes={}", repo.remote);
    let unpushed = ctx.read(
        dir,
        &[
            "rev-list",
            "--count",
            "HEAD",
            "--branches",
            "--not",
            &remotes,
        ],
    )?;
    if unpushed.trim() != "0" {
        found.push(format!(
            "{} commit(s) not on {}",
            unpushed.trim(),
            repo.remote
        ));
    }
    Ok(found)
}

/// Unlists a repo. The clone itself is left exactly as it is.
///
/// # Errors
///
/// Fails when the config cannot be read, validated or saved, or the repo's record cannot be dropped.
pub fn remove(ctx: &Ctx, selector: &str) -> Result<Option<String>> {
    let mut cfg = ctx.config()?;
    let repos = cfg.repos(&ctx.paths)?;
    let Some(i) = repos.iter().position(|r| r.matches(selector, &ctx.paths)) else {
        return Ok(None);
    };
    let removed = cfg.repo.remove(i);
    cfg.save(&ctx.paths)?;
    record::forget(&ctx.paths, &ctx.paths.tilde(&repos[i].path))?;
    Ok(Some(removed.path))
}

/// Sets how often a listed repo is polled. The new interval applies from its
/// next due check; nothing is synced here.
///
/// # Errors
///
/// Fails when `interval` is not valid, or the config cannot be read, validated or saved.
pub fn set_interval(ctx: &Ctx, selector: &str, interval: &str) -> Result<Option<String>> {
    config::parse_interval(interval)?;
    let mut cfg = ctx.config()?;
    let repos = cfg.repos(&ctx.paths)?;
    let Some(i) = repos.iter().position(|r| r.matches(selector, &ctx.paths)) else {
        return Ok(None);
    };
    cfg.repo[i].interval = Some(interval.trim().to_string());
    cfg.save(&ctx.paths)?;
    Ok(Some(cfg.repo[i].path.clone()))
}

/// The branch a clone has checked out, for the tray's *Add repository…* default.
pub fn current_branch(ctx: &Ctx, path: &Path) -> Option<String> {
    git::current_branch(&ctx.paths.git, path)
}
