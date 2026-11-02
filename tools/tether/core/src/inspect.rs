//! What a clone looks like right now, read without changing anything, and
//! the reasons a sync can skip it (rule 6: skip reasons are states, not errors).

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::absolute;
use crate::ctx::Ctx;

/// What the tray icon and `status` show for one repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Health {
    InSync,
    /// Short of the remote tip, and the next sync moves it there without the
    /// owner. A state of its own so a stale clone never reads as *in sync*;
    /// the tray icon still draws it as in sync (rule 11 has three icons).
    Behind,
    /// Skipped this round for a passing reason (another run holds the lock,
    /// git holds `index.lock`, the repo's gate said no); the next round
    /// retries without the owner.
    Skipped,
    NeedsYou,
}

impl Health {
    pub fn label(self) -> &'static str {
        match self {
            Health::InSync => "in sync",
            Health::Behind => "behind",
            Health::Skipped => "skipped",
            Health::NeedsYou => "needs you",
        }
    }
}

/// Why a sync left a repo as it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Reason {
    /// The path does not exist (a `mirror` with a `url` clones it instead).
    Missing,
    /// Not a clone's top level, or a linked worktree (rule 2).
    NotAClone {
        detail: String,
    },
    OtherBranch {
        branch: String,
    },
    Detached,
    TrackedChanges {
        count: usize,
    },
    Diverged {
        ahead: usize,
        behind: usize,
    },
    /// A merge, rebase, cherry-pick, revert or bisect is under way (rule 4).
    OperationInProgress {
        operation: String,
    },
    FetchFailed {
        error: String,
    },
    NoRemoteBranch {
        remote: String,
        branch: String,
    },
    /// `merge --ff-only` itself refused, e.g. an untracked file in the way.
    FastForwardRefused {
        error: String,
    },
    IndexLocked,
    Busy,
    /// The repo's `gate` exited non-zero or timed out (ADR 0004 rule 2).
    Gated {
        output: String,
    },
    /// The repo's `after` failed for the last move (ADR 0004 rule 3).
    AfterFailed {
        output: String,
    },
}

impl Reason {
    pub fn health(&self) -> Health {
        match self {
            Reason::IndexLocked | Reason::Busy | Reason::Gated { .. } => Health::Skipped,
            _ => Health::NeedsYou,
        }
    }

    /// What the owner does to clear a *needs you* reason, for the repo listed
    /// on `branch` of `remote`. `None` for a skip that clears by itself.
    pub fn hint(&self, remote: &str, branch: &str) -> Option<String> {
        let text = match self {
            Reason::Missing => "Clone the repo there, or fix its path in repos.toml.".into(),
            Reason::NotAClone { .. } => {
                "Point repos.toml at the top folder of a main clone.".into()
            }
            Reason::OtherBranch { .. } => format!("Switch back to {branch} and tether resumes."),
            Reason::Detached => format!("Check out {branch} and tether resumes."),
            Reason::TrackedChanges { .. } => {
                "Commit or stash them; tether never touches your work.".into()
            }
            Reason::Diverged { .. } => {
                format!("Rebase onto {remote}/{branch} or push; tether only fast-forwards.")
            }
            Reason::OperationInProgress { operation } => {
                format!("Finish or abort the {operation}.")
            }
            Reason::FetchFailed { .. } => {
                format!("Check the network and your access to {remote}, then sync again.")
            }
            Reason::NoRemoteBranch { .. } => {
                format!("Push {branch} to {remote}, or list a branch that exists there.")
            }
            Reason::FastForwardRefused { .. } => {
                "Move aside the files git names, then sync again.".into()
            }
            Reason::AfterFailed { .. } => {
                "Fix what the after command reports, then Sync now to run it again.".into()
            }
            Reason::IndexLocked | Reason::Busy | Reason::Gated { .. } => return None,
        };
        Some(text)
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::Missing => write!(f, "path does not exist"),
            Reason::NotAClone { detail } => write!(f, "{detail}"),
            Reason::OtherBranch { branch } => write!(f, "on another branch ({branch})"),
            Reason::Detached => write!(f, "HEAD is detached"),
            Reason::TrackedChanges { count } => write!(f, "tracked changes ({count} file(s))"),
            Reason::Diverged { ahead, behind } => write!(
                f,
                "diverged from the remote ({ahead} ahead, {behind} behind)"
            ),
            Reason::OperationInProgress { operation } => write!(f, "{operation} in progress"),
            Reason::FetchFailed { error } => write!(f, "fetch failed: {error}"),
            Reason::NoRemoteBranch { remote, branch } => {
                write!(f, "{remote} has no branch {branch}")
            }
            Reason::FastForwardRefused { error } => write!(f, "fast-forward refused: {error}"),
            Reason::IndexLocked => {
                write!(f, "index.lock exists (git is running); retrying next round")
            }
            Reason::Busy => write!(f, "another tether run is syncing it; retrying next round"),
            Reason::Gated { output } => write!(f, "gate: {output}; asking again next round"),
            Reason::AfterFailed { output } => write!(f, "after failed: {output}"),
        }
    }
}

/// A clone's git directory, checked to be a main clone at exactly `path`.
pub fn git_dir(ctx: &Ctx, path: &Path) -> Result<PathBuf, Reason> {
    if !path.exists() {
        return Err(Reason::Missing);
    }
    let not = |detail: String| Reason::NotAClone { detail };
    let top = ctx
        .read(path, &["rev-parse", "--show-toplevel"])
        .map_err(|_| not(format!("{} is not a git clone", ctx.paths.tilde(path))))?;
    if absolute(Path::new(&top)) != absolute(path) {
        return Err(not(format!(
            "{} is inside {top}, not a clone's top level",
            ctx.paths.tilde(path)
        )));
    }
    let dir = ctx
        .read(path, &["rev-parse", "--absolute-git-dir"])
        .map_err(|e| not(format!("{e:#}")))?;
    let common = ctx
        .read(
            path,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )
        .map_err(|e| not(format!("{e:#}")))?;
    if absolute(Path::new(&dir)) != absolute(Path::new(&common)) {
        return Err(not(format!(
            "{} is a linked worktree of {common}, not a main clone",
            ctx.paths.tilde(path)
        )));
    }
    Ok(PathBuf::from(dir))
}

/// The operation under way in `git_dir`, if any. These are the marker files
/// git itself checks (`git status` reads the same ones).
pub fn operation_in_progress(git_dir: &Path) -> Option<&'static str> {
    [
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("MERGE_HEAD", "merge"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
        ("BISECT_LOG", "bisect"),
    ]
    .into_iter()
    .find(|(marker, _)| git_dir.join(marker).exists())
    .map(|(_, op)| op)
}

pub fn index_locked(git_dir: &Path) -> bool {
    git_dir.join("index.lock").exists()
}

/// Rule 4 for both modes: skip while git or an operation holds the clone.
pub fn busy_reason(git_dir: &Path) -> Option<Reason> {
    if index_locked(git_dir) {
        return Some(Reason::IndexLocked);
    }
    operation_in_progress(git_dir).map(|op| Reason::OperationInProgress {
        operation: op.into(),
    })
}

/// Tracked files that differ from HEAD, staged or not. Untracked files are
/// not counted: `follow` allows them (rule 5).
pub fn tracked_changes(ctx: &Ctx, path: &Path) -> anyhow::Result<usize> {
    let out = ctx.read(path, &["status", "--porcelain=v1", "--untracked-files=no"])?;
    Ok(out.lines().filter(|l| !l.is_empty()).count())
}

/// `(ahead, behind)` of `local` against `remote`.
pub fn ahead_behind(
    ctx: &Ctx,
    path: &Path,
    local: &str,
    remote: &str,
) -> anyhow::Result<(usize, usize)> {
    let out = ctx.read(
        path,
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("{local}...{remote}"),
        ],
    )?;
    let mut it = out
        .split_whitespace()
        .map(|n| n.parse::<usize>().unwrap_or(0));
    Ok((it.next().unwrap_or(0), it.next().unwrap_or(0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_needs_you_reason_says_what_to_do() {
        let reasons = [
            Reason::Missing,
            Reason::NotAClone { detail: "x".into() },
            Reason::OtherBranch {
                branch: "feature".into(),
            },
            Reason::Detached,
            Reason::TrackedChanges { count: 2 },
            Reason::Diverged {
                ahead: 1,
                behind: 3,
            },
            Reason::OperationInProgress {
                operation: "rebase".into(),
            },
            Reason::FetchFailed { error: "x".into() },
            Reason::NoRemoteBranch {
                remote: "origin".into(),
                branch: "main".into(),
            },
            Reason::FastForwardRefused { error: "x".into() },
            Reason::AfterFailed { output: "x".into() },
        ];
        for r in reasons {
            assert_eq!(r.health(), Health::NeedsYou);
            assert!(r.hint("origin", "main").is_some(), "{r:?} has no hint");
        }
    }

    #[test]
    fn a_skip_that_clears_by_itself_has_no_hint() {
        assert_eq!(Reason::Busy.hint("origin", "main"), None);
        assert_eq!(Reason::IndexLocked.hint("origin", "main"), None);
        let gated = Reason::Gated { output: "x".into() };
        assert_eq!(gated.health(), Health::Skipped);
        assert_eq!(gated.hint("origin", "main"), None);
    }

    #[test]
    fn a_hint_names_the_listed_branch_not_the_checked_out_one() {
        let r = Reason::OtherBranch {
            branch: "feature".into(),
        };
        assert_eq!(
            r.hint("origin", "release").as_deref(),
            Some("Switch back to release and tether resumes.")
        );
    }
}
