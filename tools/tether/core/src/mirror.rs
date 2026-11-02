//! `mirror` mode (rules 7–9): a shallow, read-only copy forced to the branch
//! tip on every sync. Tracked edits are saved as a patch before they are
//! discarded; untracked and ignored files are discarded without one.

use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::Local;

use crate::config::Repo;
use crate::ctx::Ctx;
use crate::git;
use crate::inspect::{Health, Reason};
use crate::sync::{Outcome, Poll, short};

/// A mirror whose path is missing or an empty directory is cloned, not synced.
pub fn needs_fresh_clone(path: &Path) -> bool {
    match std::fs::read_dir(path) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => !path.exists(),
    }
}

/// Rule 8, first half: `git clone --depth 1 --single-branch` into a new or
/// empty path, from the entry's `url`.
pub fn clone_fresh(ctx: &Ctx, repo: &Repo, out: &mut Outcome) -> Result<(), Reason> {
    let Some(url) = &repo.url else {
        return Err(Reason::NotAClone {
            detail: format!(
                "{} is missing or empty and the entry has no url to clone from",
                ctx.paths.tilde(&repo.path)
            ),
        });
    };
    let parent = repo.path.parent().unwrap_or(Path::new("/"));
    if !ctx.dry_run {
        std::fs::create_dir_all(parent).map_err(|e| Reason::NotAClone {
            detail: format!("creating {}: {e}", parent.display()),
        })?;
    }
    let target = repo.path.to_string_lossy();
    let args = [
        "clone",
        "--quiet",
        "--depth",
        "1",
        "--single-branch",
        "--no-tags",
        "--branch",
        &repo.branch,
        "--origin",
        &repo.remote,
        url,
        &target,
    ];
    match ctx.write_output(parent, &args) {
        Ok(Some(o)) if !o.status.success() => Err(Reason::FetchFailed {
            error: git::first_error_line(&o),
        }),
        Err(e) => Err(Reason::FetchFailed {
            error: format!("{e:#}"),
        }),
        Ok(Some(_)) => {
            out.fetched = true;
            let tip = ctx.resolve(&repo.path, "HEAD").unwrap_or_default();
            out.moved = Some((String::new(), tip));
            Ok(())
        }
        Ok(None) => Ok(()),
    }
}

/// Makes the clone exactly the remote tip: `reset --hard` (or a forced
/// checkout when HEAD is elsewhere) and `clean -ffdx`, then, when the tip
/// moved, drops the reflog and other refs and runs `git gc` (rule 9).
pub fn apply(ctx: &Ctx, repo: &Repo, poll: &Poll, out: &mut Outcome) -> Result<(), Reason> {
    let dir = &repo.path;
    let failed = |e: anyhow::Error| Reason::NotAClone {
        detail: format!("{e:#}"),
    };
    let head = ctx.resolve(dir, "HEAD");
    let on_branch =
        git::current_branch(&ctx.paths.git, dir).as_deref() == Some(repo.branch.as_str());
    let tracked = if head.is_some() {
        ctx.read(dir, &["diff", "--binary", "HEAD"])
            .map_err(failed)?
    } else {
        String::new()
    };
    let leftovers = ctx
        .read(
            dir,
            &[
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--ignored",
            ],
        )
        .map_err(failed)?;
    let untracked = leftovers.lines().filter(|l| l.starts_with("??")).count();
    let exact = on_branch
        && head.as_deref() == Some(poll.tip.as_str())
        && tracked.is_empty()
        && leftovers.trim().is_empty();
    if exact {
        return Ok(());
    }
    if ctx.dry_run && head.as_deref() != Some(poll.tip.as_str()) {
        out.health = Health::Behind;
    }
    if !poll.available {
        ctx.note(format!(
            "would reset {} to {} and clean it",
            repo.branch,
            short(&poll.tip)
        ));
        return Ok(());
    }
    if !tracked.is_empty() {
        out.backup = save_patch(ctx, repo, &tracked).map_err(failed)?;
    }
    let tracking = repo.tracking_ref();
    if on_branch {
        ctx.write(dir, &["reset", "--hard", "--quiet", &tracking])
            .map_err(failed)?;
    } else {
        ctx.write(
            dir,
            &[
                "checkout",
                "--quiet",
                "--force",
                "-B",
                &repo.branch,
                &tracking,
            ],
        )
        .map_err(failed)?;
    }
    ctx.write(dir, &["clean", "-ffdx", "--quiet"])
        .map_err(failed)?;
    out.discarded = !ctx.dry_run && (!tracked.is_empty() || untracked > 0);
    if head.as_deref() != Some(poll.tip.as_str()) {
        prune(ctx, repo).map_err(failed)?;
        if !ctx.dry_run {
            out.moved = Some((head.unwrap_or_default(), poll.tip.clone()));
        }
    }
    Ok(())
}

/// Rule 9: `backups/<repo>/<timestamp>.diff`, never overwriting an older one.
fn save_patch(ctx: &Ctx, repo: &Repo, patch: &str) -> anyhow::Result<Option<PathBuf>> {
    let dir = ctx.paths.backups(&repo.name());
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let mut file = dir.join(format!("{stamp}.diff"));
    let mut n = 1;
    while file.exists() {
        n += 1;
        file = dir.join(format!("{stamp}-{n}.diff"));
    }
    if ctx.dry_run {
        ctx.note(format!(
            "would save tracked changes to {}",
            ctx.paths.tilde(&file)
        ));
        return Ok(None);
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(&file, patch).with_context(|| format!("writing {}", file.display()))?;
    ctx.note(format!(
        "saved tracked changes to {}",
        ctx.paths.tilde(&file)
    ));
    Ok(Some(file))
}

/// Keeps the clone at one commit's size: only the branch and its tracking ref
/// stay; tags and other remote-tracking refs go (they are re-fetchable), then
/// the reflog is expired and unreachable objects pruned. Other local branches
/// are left alone: adoption refuses a clone that has any.
fn prune(ctx: &Ctx, repo: &Repo) -> anyhow::Result<()> {
    let dir = &repo.path;
    let keep = repo.tracking_ref();
    let refs = ctx.read(
        dir,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/remotes/",
            "refs/tags/",
        ],
    )?;
    for r in refs
        .lines()
        .filter(|r| !r.is_empty() && *r != keep && !r.ends_with("/HEAD"))
    {
        ctx.write(dir, &["update-ref", "-d", r])?;
    }
    ctx.write(dir, &["reflog", "expire", "--expire=now", "--all"])?;
    ctx.write(dir, &["gc", "--quiet", "--prune=now"])
}
