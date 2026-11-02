//! `follow` mode (rules 5, 6): fast-forward the branch or skip with a reason.
//! Nothing here stashes, resets, rebases or switches branches; the only write
//! is `git merge --ff-only`, and `--ff-only` itself refuses to overwrite an
//! untracked file.

use crate::config::Repo;
use crate::ctx::Ctx;
use crate::git;
use crate::inspect::{self, Health, Reason};
use crate::sync::{Outcome, Poll};

/// What a `follow` sync would do against remote tip `tip`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Level with the remote, or only ahead of it (rule 6: that is in sync).
    UpToDate,
    FastForward {
        from: String,
        to: String,
    },
    /// A dry run that did not fetch: the tip's objects are not here yet.
    AfterFetch,
}

/// Decides without writing. `available` is false when `tip` was not fetched.
///
/// # Errors
///
/// Fails with the [`Reason`] the repo cannot be fast-forwarded: detached HEAD, another branch, uncommitted tracked changes, a diverged history, or an unreadable clone.
pub fn evaluate(ctx: &Ctx, repo: &Repo, tip: &str, available: bool) -> Result<Plan, Reason> {
    let dir = &repo.path;
    match git::current_branch(&ctx.paths.git, dir) {
        None => return Err(Reason::Detached),
        Some(b) if b != repo.branch => return Err(Reason::OtherBranch { branch: b }),
        Some(_) => {}
    }
    let local = ctx
        .resolve(dir, &format!("refs/heads/{}", repo.branch))
        .ok_or(Reason::Detached)?;
    if local == tip {
        return Ok(Plan::UpToDate);
    }
    if !available {
        return Ok(Plan::AfterFetch);
    }
    if git::is_ancestor(&ctx.paths.git, dir, tip, &local) {
        return Ok(Plan::UpToDate);
    }
    // Something to pull: only now do local edits stand in the way. An edited
    // clone that is already current is ordinary work, not a skip.
    let changed = inspect::tracked_changes(ctx, dir).map_err(|e| Reason::NotAClone {
        detail: format!("{e:#}"),
    })?;
    if changed > 0 {
        return Err(Reason::TrackedChanges { count: changed });
    }
    if git::is_ancestor(&ctx.paths.git, dir, &local, tip) {
        return Ok(Plan::FastForward {
            from: local,
            to: tip.to_string(),
        });
    }
    let (ahead, behind) = inspect::ahead_behind(ctx, dir, &local, tip).unwrap_or((0, 0));
    Err(Reason::Diverged { ahead, behind })
}

/// # Errors
///
/// Fails with the [`Reason`] the repo was skipped, or the fast-forward was refused.
pub fn apply(ctx: &Ctx, repo: &Repo, poll: &Poll, out: &mut Outcome) -> Result<(), Reason> {
    match evaluate(ctx, repo, &poll.tip, poll.available)? {
        Plan::UpToDate => Ok(()),
        Plan::AfterFetch => {
            ctx.note(format!(
                "would fast-forward {} to {} if it descends from the local branch",
                repo.branch,
                crate::sync::short(&poll.tip)
            ));
            out.health = Health::Behind;
            Ok(())
        }
        Plan::FastForward { from, to } => {
            let tracking = repo.tracking_ref();
            match ctx.write_output(&repo.path, &["merge", "--ff-only", "--quiet", &tracking]) {
                Ok(Some(o)) if !o.status.success() => Err(Reason::FastForwardRefused {
                    error: git::first_error_line(&o),
                }),
                Err(e) => Err(Reason::FastForwardRefused {
                    error: format!("{e:#}"),
                }),
                Ok(Some(_)) => {
                    out.moved = Some((from, to));
                    Ok(())
                }
                // A dry run: the merge was only printed.
                Ok(None) => {
                    out.health = Health::Behind;
                    Ok(())
                }
            }
        }
    }
}
