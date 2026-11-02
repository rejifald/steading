//! The decision of rules 3 to 6 and 13, without any I/O: given what was found out about a
//! worktree, which reap state it is in and the one line that says why.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;

/// A worktree's reap state, as `git reap status` and the popover spell it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Not merged: work in progress.
    Open,
    /// Merged, clean, not in use and not locked (rule 6).
    Removable,
    /// Merged, but with changes or an operation under way (rule 4).
    Dirty,
    /// Merged, but a process runs inside it (rule 5).
    InUse,
    /// Merged, but `git worktree lock`ed (rule 5).
    Locked,
    /// Not merged and abandoned (rule 13); flagged, never removed.
    Stale,
    /// Its folder is gone; git can prune the entry (rule 12).
    Missing,
    /// Merged, but held by *Keep* until its HEAD moves (rule 10).
    Kept,
    /// Merged, but git refused to remove it at this HEAD (rule 11).
    Refused,
}

impl State {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Removable => "removable",
            Self::Dirty => "dirty",
            Self::InUse => "in use",
            Self::Locked => "locked",
            Self::Stale => "stale",
            Self::Missing => "missing",
            Self::Kept => "kept",
            Self::Refused => "refused",
        }
    }
}

/// Which clause of rule 3 makes a worktree merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merge {
    /// (a) HEAD is an ancestor of the base ref, named as shown (`origin/main`).
    IntoBase(String),
    /// (b) HEAD is an ancestor of its pull request's target, named as shown.
    IntoTarget { target: String, pull_request: u64 },
    /// (c) `gh` reports the pull request merged at HEAD or at a head that contains it
    /// (ADR 0002).
    PullRequest(u64),
}

impl Merge {
    fn describe(&self) -> String {
        match self {
            Self::IntoBase(base) => format!("merged into {base}"),
            Self::IntoTarget {
                target,
                pull_request,
            } => format!("merged into {target}, the target of pull request #{pull_request}"),
            Self::PullRequest(number) => format!("pull request #{number} merged"),
        }
    }
}

/// Uncommitted work that keeps a merged worktree (rule 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dirt {
    Changes,
    /// A merge, rebase, cherry-pick, revert or bisect, by name.
    Operation(&'static str),
}

/// What a pass found out about one linked worktree.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub is_missing: bool,
    pub lock_reason: Option<String>,
    pub merge: Option<Merge>,
    /// Checked only for a merged or pushed worktree; `None` when clean.
    pub dirt: Option<Dirt>,
    pub in_use: InUse,
    /// The number of its pull request when that closed without merging.
    pub closed_pull_request: Option<u64>,
    /// The number of its open pull request.
    pub open_pull_request: Option<u64>,
    /// Its last commit or checkout.
    pub last_activity: Option<DateTime<Utc>>,
    /// Kept at its current HEAD.
    pub is_kept: bool,
    /// What git said when it last refused to remove it at its current HEAD.
    pub refusal: Option<String>,
    /// The remote, as shown, when the worktree is on a branch and every commit on it is in that
    /// remote's tracking refs (ADR 0003).
    pub pushed_to: Option<String>,
}

/// Whether a process runs inside a worktree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum InUse {
    #[default]
    No,
    Yes,
    /// `lsof` failed; a guard that cannot be checked does not pass.
    Unknown(String),
}

/// A missing entry is pruned only when nothing protects it (rule 12): a kept or locked one keeps
/// its state, so it gets no grace clock and is never counted as waiting for a prune.
fn missing(facts: &Facts) -> (State, String) {
    if facts.is_kept {
        return (State::Kept, "folder is gone, but kept".into());
    }
    if let Some(lock_reason) = &facts.lock_reason {
        let detail = if lock_reason.is_empty() {
            "locked".to_string()
        } else {
            format!("locked: {lock_reason}")
        };
        return (State::Locked, format!("folder is gone, but {detail}"));
    }
    (State::Missing, "folder is gone".into())
}

/// The state of a worktree and the line that says why. `base` names the base ref as shown.
#[must_use]
pub fn classify(
    facts: &Facts,
    base: &str,
    stale_after: Duration,
    now: DateTime<Utc>,
) -> (State, String) {
    if facts.is_missing {
        return missing(facts);
    }
    let Some(merge) = &facts.merge else {
        let (state, reason) = unmerged(facts, base, stale_after, now);
        return match &facts.pushed_to {
            Some(remote) if state == State::Stale => pushed_stale(facts, reason, remote),
            _ => (state, reason),
        };
    };
    guarded(facts, merge.describe())
}

/// ADR 0003: a stale worktree whose commits are all on the remote is removed like a merged one,
/// with its branch kept. Any guard that holds it leaves it stale, so it still needs the
/// operator.
fn pushed_stale(facts: &Facts, stale: String, remote: &str) -> (State, String) {
    let ground = format!("{stale}; every commit is on {remote}, so its branch is kept");
    match guarded(facts, ground) {
        found @ (State::Removable | State::Refused | State::Kept, _) => found,
        _ => (State::Stale, stale),
    }
}

/// Rules 4 to 6 and 10 to 11 over a worktree with grounds for removal, `ground` saying which.
fn guarded(facts: &Facts, ground: String) -> (State, String) {
    if facts.is_kept {
        return (
            State::Kept,
            format!("{ground}, but kept until it gets new commits"),
        );
    }
    if let Some(lock_reason) = &facts.lock_reason {
        let detail = if lock_reason.is_empty() {
            "locked".to_string()
        } else {
            format!("locked: {lock_reason}")
        };
        return (State::Locked, format!("{ground}, but {detail}"));
    }
    match &facts.in_use {
        InUse::Yes => {
            return (
                State::InUse,
                format!("{ground}, but a process is running inside it"),
            );
        }
        InUse::Unknown(why) => {
            return (
                State::InUse,
                format!("{ground}, but processes inside it could not be checked: {why}"),
            );
        }
        InUse::No => {}
    }
    match facts.dirt {
        Some(Dirt::Changes) => (
            State::Dirty,
            format!("{ground}, but has uncommitted changes"),
        ),
        Some(Dirt::Operation(operation)) => (
            State::Dirty,
            format!("{ground}, but a {operation} is in progress"),
        ),
        None => match &facts.refusal {
            Some(message) => (
                State::Refused,
                format!("{ground}, but git refused to remove it: {message}"),
            ),
            None => (State::Removable, ground),
        },
    }
}

fn unmerged(
    facts: &Facts,
    base: &str,
    stale_after: Duration,
    now: DateTime<Utc>,
) -> (State, String) {
    if let Some(number) = facts.closed_pull_request {
        return (
            State::Stale,
            format!("pull request #{number} closed without merging"),
        );
    }
    let idle = facts
        .last_activity
        .and_then(|last| now.signed_duration_since(last).to_std().ok());
    if let Some(idle) = idle.filter(|idle| *idle >= stale_after) {
        let days = idle.as_secs() / (24 * 3600);
        return (
            State::Stale,
            format!("no commit or checkout for {days} days"),
        );
    }
    match facts.open_pull_request {
        Some(number) => (State::Open, format!("pull request #{number} is open")),
        None => (State::Open, format!("not merged into {base}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "origin/main";
    const TWO_WEEKS: Duration = Duration::from_secs(14 * 24 * 3600);

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-29T12:00:00Z")
            .unwrap()
            .into()
    }

    fn merged() -> Facts {
        Facts {
            merge: Some(Merge::IntoBase(BASE.into())),
            ..Facts::default()
        }
    }

    fn state_of(facts: &Facts) -> (State, String) {
        classify(facts, BASE, TWO_WEEKS, now())
    }

    #[test]
    fn a_clean_free_merged_worktree_is_removable() {
        assert_eq!(
            state_of(&merged()),
            (State::Removable, "merged into origin/main".into())
        );
    }

    #[test]
    fn every_guard_keeps_a_merged_worktree() {
        let locked = Facts {
            lock_reason: Some("on usb".into()),
            ..merged()
        };
        let in_use = Facts {
            in_use: InUse::Yes,
            ..merged()
        };
        let unchecked = Facts {
            in_use: InUse::Unknown("lsof failed".into()),
            ..merged()
        };
        let dirty = Facts {
            dirt: Some(Dirt::Changes),
            ..merged()
        };
        let rebasing = Facts {
            dirt: Some(Dirt::Operation("rebase")),
            ..merged()
        };

        assert_eq!(state_of(&locked).0, State::Locked);
        assert_eq!(state_of(&in_use).0, State::InUse);
        assert_eq!(state_of(&unchecked).0, State::InUse);
        assert_eq!(state_of(&dirty).0, State::Dirty);
        assert_eq!(
            state_of(&rebasing).1,
            "merged into origin/main, but a rebase is in progress"
        );
    }

    #[test]
    fn a_keep_holds_a_merged_worktree_and_a_refusal_is_shown() {
        let kept = Facts {
            is_kept: true,
            dirt: Some(Dirt::Changes),
            ..merged()
        };
        let refused = Facts {
            refusal: Some("contains modified or untracked files".into()),
            ..merged()
        };
        let kept_but_open = Facts {
            is_kept: true,
            ..Facts::default()
        };

        assert_eq!(state_of(&kept).0, State::Kept);
        assert_eq!(state_of(&refused).0, State::Refused);
        assert_eq!(state_of(&kept_but_open).0, State::Open);
    }

    #[test]
    fn a_missing_folder_that_is_locked_or_kept_is_not_pruned() {
        let locked = Facts {
            is_missing: true,
            lock_reason: Some(String::new()),
            ..merged()
        };
        let kept = Facts {
            is_missing: true,
            is_kept: true,
            ..merged()
        };
        let plain = Facts {
            is_missing: true,
            ..merged()
        };

        assert_eq!(state_of(&locked).0, State::Locked);
        assert_eq!(state_of(&kept).0, State::Kept);
        assert_eq!(state_of(&plain).0, State::Missing);
    }

    #[test]
    fn each_merge_clause_says_how_it_merged() {
        let squash = Facts {
            merge: Some(Merge::PullRequest(12)),
            ..Facts::default()
        };
        let cascade = Facts {
            merge: Some(Merge::IntoTarget {
                target: "origin/feat/base".into(),
                pull_request: 13,
            }),
            ..Facts::default()
        };

        assert_eq!(state_of(&squash).1, "pull request #12 merged");
        assert_eq!(
            state_of(&cascade).1,
            "merged into origin/feat/base, the target of pull request #13"
        );
    }

    #[test]
    fn unmerged_work_is_open_until_it_looks_abandoned() {
        let recent = Facts {
            last_activity: Some(now() - chrono::Duration::days(13)),
            open_pull_request: Some(7),
            ..Facts::default()
        };
        let idle = Facts {
            last_activity: Some(now() - chrono::Duration::days(21)),
            ..Facts::default()
        };
        let closed = Facts {
            closed_pull_request: Some(9),
            ..Facts::default()
        };

        assert_eq!(
            state_of(&recent),
            (State::Open, "pull request #7 is open".into())
        );
        assert_eq!(
            state_of(&idle),
            (State::Stale, "no commit or checkout for 21 days".into())
        );
        assert_eq!(
            state_of(&closed),
            (
                State::Stale,
                "pull request #9 closed without merging".into()
            )
        );
        assert_eq!(
            state_of(&Facts::default()),
            (State::Open, "not merged into origin/main".into())
        );
    }

    #[test]
    fn a_stale_worktree_whose_commits_are_all_pushed_is_removable() {
        let pushed = Facts {
            last_activity: Some(now() - chrono::Duration::days(21)),
            pushed_to: Some("origin".into()),
            ..Facts::default()
        };
        let dirty = Facts {
            dirt: Some(Dirt::Changes),
            ..pushed.clone()
        };
        let busy = Facts {
            in_use: InUse::Yes,
            ..pushed.clone()
        };
        let recent = Facts {
            last_activity: Some(now() - chrono::Duration::days(3)),
            ..pushed.clone()
        };

        assert_eq!(
            state_of(&pushed),
            (
                State::Removable,
                "no commit or checkout for 21 days; every commit is on origin, so its branch is kept"
                    .into()
            )
        );
        // A guard that holds it leaves it stale: it still needs the operator.
        assert_eq!(
            state_of(&dirty),
            (State::Stale, "no commit or checkout for 21 days".into())
        );
        assert_eq!(state_of(&busy).0, State::Stale);
        assert_eq!(state_of(&recent).0, State::Open);
    }

    #[test]
    fn a_merged_worktree_is_never_stale() {
        let facts = Facts {
            last_activity: Some(now() - chrono::Duration::days(90)),
            closed_pull_request: Some(9),
            ..merged()
        };

        assert_eq!(state_of(&facts).0, State::Removable);
    }
}
