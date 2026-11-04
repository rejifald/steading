//! The decision of rules 3 to 6 and 13, of ADR dirty-merged-worktrees-are-salvaged and of ADR
//! abandoned-worktrees-are-saved-then-offered, without any I/O: given what was found out about a
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
    /// Merged, clean or with changes that can be saved, not in use and not locked (rule 6).
    Removable,
    /// Merged, but with changes that cannot be saved yet, or an operation under way (rule 4).
    Dirty,
    /// Merged, but a process runs inside it (rule 5).
    InUse,
    /// Merged, but `git worktree lock`ed (rule 5).
    Locked,
    /// Not merged and abandoned (rule 13), and held by a guard, or with nothing local to save;
    /// flagged, never removed.
    Stale,
    /// Stale with something local in it, and nothing holds it (ADR 0006): offered for *Save and
    /// remove*, never removed by a sweep.
    Abandoned,
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
            Self::Abandoned => "abandoned",
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
    /// (d) a merged pull request associated with HEAD, whatever its branch, contains it (ADR
    /// 0006).
    Commit(u64),
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
            Self::Commit(number) => format!("in #{number}, merged"),
        }
    }
}

/// Uncommitted work that keeps a merged worktree (rule 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dirt {
    Changes,
    /// A merge, rebase, cherry-pick, revert or bisect, by name.
    Operation(&'static str),
    /// Conflicted paths in the index with no operation under way (a `stash pop`, say).
    Unmerged,
}

/// Whether a merged worktree's changes can be saved to a ref before it is removed (ADR
/// dirty-merged-worktrees-are-salvaged).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Salvage {
    /// Nothing holds it; its files, commits or checkouts last changed at `last_change`.
    Ready { last_change: DateTime<Utc> },
    /// A guard holds it; the text says which, to follow "and".
    Held(String),
}

/// What a pass found out about one linked worktree.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub is_missing: bool,
    pub lock_reason: Option<String>,
    pub merge: Option<Merge>,
    /// Checked only for a merged or pushed worktree; `None` when clean.
    pub dirt: Option<Dirt>,
    /// Checked only for a merged or stale worktree with [`Dirt::Changes`].
    pub salvage: Option<Salvage>,
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
    /// For a stale worktree only: what in it exists only here, each as shown (`13 uncommitted
    /// changes`, `22 commits on no remote`, `a detached HEAD`, `pull request #15 open`), ADR
    /// 0006 rule 9.
    pub local: Vec<String>,
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

/// The settings of one repo that the decision depends on.
#[derive(Debug, Clone, Copy)]
pub struct Rules<'a> {
    /// The base ref as shown, such as `origin/main`.
    pub base: &'a str,
    pub stale_after: Duration,
    pub dirty_grace: Duration,
}

/// The state of a worktree and the line that says why.
#[must_use]
pub fn classify(facts: &Facts, rules: &Rules, now: DateTime<Utc>) -> (State, String) {
    if facts.is_missing {
        return missing(facts);
    }
    let Some(merge) = &facts.merge else {
        let (state, reason) = unmerged(facts, rules.base, rules.stale_after, now);
        if state != State::Stale {
            return (state, reason);
        }
        if let (Some(remote), None) = (&facts.pushed_to, &facts.dirt)
            && let Some(found) = pushed_stale(facts, &reason, remote)
        {
            return found;
        }
        return abandoned(facts, &reason);
    };
    guarded(facts, merge.describe(), rules.dirty_grace, now)
}

/// Why an unmerged worktree is stale (rule 13), or `None` when it is not.
#[must_use]
pub fn stale_reason(facts: &Facts, stale_after: Duration, now: DateTime<Utc>) -> Option<String> {
    if facts.merge.is_some() {
        return None;
    }
    let (state, reason) = unmerged(facts, "", stale_after, now);
    (state == State::Stale).then_some(reason)
}

/// The guard of ADR 0004 rule 1 that holds a stale worktree, other than a *Keep*, as it follows
/// "reap won't remove it:" (ADR 0006 rule 5); `None` when none does.
#[must_use]
pub fn guard_of(facts: &Facts) -> Option<String> {
    if let Some(lock_reason) = &facts.lock_reason {
        return Some(if lock_reason.is_empty() {
            "locked".to_string()
        } else {
            format!("locked: {lock_reason}")
        });
    }
    match &facts.in_use {
        InUse::Yes => return Some("a process is running inside it".into()),
        InUse::Unknown(why) => {
            return Some(format!("processes inside it could not be checked: {why}"));
        }
        InUse::No => {}
    }
    match (&facts.dirt, &facts.salvage) {
        (Some(Dirt::Operation(operation)), _) => Some(format!("a {operation} is in progress")),
        (Some(Dirt::Unmerged), _) => Some("it has unmerged paths".into()),
        (Some(Dirt::Changes), Some(Salvage::Held(why))) => Some(why.clone()),
        (Some(Dirt::Changes), None) => Some("its changes were not checked".into()),
        _ => None,
    }
}

/// ADR 0006 rules 4, 5 and 9: a stale worktree with something local is *abandoned*, offered for
/// *Save and remove*, unless a guard holds it; either way its line names what is local and the
/// guard. A kept one is *kept*, and a save that failed at this HEAD is *refused*.
fn abandoned(facts: &Facts, stale: &str) -> (State, String) {
    let ground = std::iter::once(stale)
        .chain(facts.local.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" · ");
    if facts.is_kept {
        return (
            State::Kept,
            format!("{ground}, but kept until it gets new commits"),
        );
    }
    if let Some(guard) = guard_of(facts) {
        return (State::Stale, format!("{ground} · {guard}"));
    }
    if facts.local.is_empty() {
        return (State::Stale, ground);
    }
    match &facts.refusal {
        Some(message) => (
            State::Refused,
            format!("{ground}, but saving and removing it failed: {message}"),
        ),
        None => (State::Abandoned, ground),
    }
}

/// When a merged worktree's changes are saved and it becomes removable: `None` unless they can
/// be saved and are still too recent.
#[must_use]
pub fn saves_at(facts: &Facts, dirty_grace: Duration, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let Some(Salvage::Ready { last_change }) = facts.salvage else {
        return None;
    };
    let at = last_change + chrono::Duration::from_std(dirty_grace).unwrap_or(chrono::Duration::MAX);
    (facts.merge.is_some() && facts.dirt == Some(Dirt::Changes) && now < at).then_some(at)
}

/// ADR 0003: a clean stale worktree whose commits are all on the remote is removed like a merged
/// one, with its branch kept. `None` when a lock or a process holds it: it is then judged as
/// abandoned, which names the guard.
fn pushed_stale(facts: &Facts, stale: &str, remote: &str) -> Option<(State, String)> {
    let ground = format!("{stale}; every commit is on {remote}, so its branch is kept");
    let facts = Facts {
        salvage: None,
        ..facts.clone()
    };
    match guarded(&facts, ground, Duration::ZERO, DateTime::<Utc>::MIN_UTC) {
        found @ (State::Removable | State::Refused | State::Kept, _) => Some(found),
        _ => None,
    }
}

/// Rules 4 to 6 and 10 to 11 over a worktree with grounds for removal, `ground` saying which.
fn guarded(
    facts: &Facts,
    ground: String,
    dirty_grace: Duration,
    now: DateTime<Utc>,
) -> (State, String) {
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
        Some(Dirt::Changes) => salvaged(facts, &ground, dirty_grace, now),
        Some(Dirt::Operation(operation)) => (
            State::Dirty,
            format!("{ground}, but a {operation} is in progress"),
        ),
        Some(Dirt::Unmerged) => (State::Dirty, format!("{ground}, but has unmerged paths")),
        None => unless_refused(facts, ground),
    }
}

/// ADR dirty-merged-worktrees-are-salvaged: changes no guard holds are saved to a ref and the
/// worktree removed, once nothing in it has changed for `dirty_grace`.
fn salvaged(
    facts: &Facts,
    ground: &str,
    dirty_grace: Duration,
    now: DateTime<Utc>,
) -> (State, String) {
    match &facts.salvage {
        None => (
            State::Dirty,
            format!("{ground}, but has uncommitted changes"),
        ),
        Some(Salvage::Held(why)) => (
            State::Dirty,
            format!("{ground}, but has uncommitted changes, and {why}"),
        ),
        Some(Salvage::Ready { last_change }) if saves_at(facts, dirty_grace, now).is_some() => {
            let ago = match now.signed_duration_since(*last_change).num_days() {
                0 => "today".to_string(),
                1 => "1 day ago".to_string(),
                n => format!("{n} days ago"),
            };
            (
                State::Dirty,
                format!(
                    "{ground}, but has uncommitted changes, last changed {ago}; they are saved \
                     to a ref once unchanged for {}",
                    shown_period(dirty_grace)
                ),
            )
        }
        Some(Salvage::Ready { .. }) => unless_refused(
            facts,
            format!("{ground}; its uncommitted changes are saved to a ref first"),
        ),
    }
}

/// "7 days", "1 day", "12h": whole days when the period is some, else hours.
fn shown_period(period: Duration) -> String {
    const DAY: u64 = 24 * 3600;
    match period.as_secs() {
        DAY => "1 day".into(),
        seconds if seconds % DAY == 0 => format!("{} days", seconds / DAY),
        seconds => format!("{}h", seconds / 3600),
    }
}

/// Removable, unless git refused to remove it at this HEAD (rule 11).
fn unless_refused(facts: &Facts, ground: String) -> (State, String) {
    match &facts.refusal {
        Some(message) => (
            State::Refused,
            format!("{ground}, but git refused to remove it: {message}"),
        ),
        None => (State::Removable, ground),
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

    const WEEK: Duration = Duration::from_secs(7 * 24 * 3600);

    fn state_of(facts: &Facts) -> (State, String) {
        let rules = Rules {
            base: BASE,
            stale_after: TWO_WEEKS,
            dirty_grace: WEEK,
        };
        classify(facts, &rules, now())
    }

    fn changed(days_ago: i64) -> Facts {
        Facts {
            dirt: Some(Dirt::Changes),
            salvage: Some(Salvage::Ready {
                last_change: now() - chrono::Duration::days(days_ago),
            }),
            ..merged()
        }
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
    fn changes_are_saved_only_once_unchanged_for_the_dirty_grace() {
        let fresh = changed(0);
        let recent = changed(3);
        let old = changed(8);

        assert_eq!(
            state_of(&fresh),
            (
                State::Dirty,
                "merged into origin/main, but has uncommitted changes, last changed today; they \
                 are saved to a ref once unchanged for 7 days"
                    .into()
            )
        );
        assert_eq!(state_of(&recent).0, State::Dirty);
        assert!(state_of(&recent).1.contains("last changed 3 days ago"));
        assert_eq!(
            saves_at(&recent, WEEK, now()),
            Some(now() + chrono::Duration::days(4))
        );
        assert_eq!(
            state_of(&old),
            (
                State::Removable,
                "merged into origin/main; its uncommitted changes are saved to a ref first".into()
            )
        );
        assert_eq!(saves_at(&old, WEEK, now()), None);
    }

    #[test]
    fn a_salvage_guard_or_a_conflict_holds_changes_whatever_their_age() {
        let nested = Facts {
            salvage: Some(Salvage::Held(
                "an untracked repository at vendor/lib/ would be lost".into(),
            )),
            ..changed(30)
        };
        let unmerged = Facts {
            dirt: Some(Dirt::Unmerged),
            ..changed(30)
        };
        let locked = Facts {
            lock_reason: Some(String::new()),
            ..changed(30)
        };
        let refused = Facts {
            refusal: Some("the save did not match the worktree".into()),
            ..changed(30)
        };

        assert_eq!(
            state_of(&nested).1,
            "merged into origin/main, but has uncommitted changes, and an untracked repository \
             at vendor/lib/ would be lost"
        );
        assert_eq!(
            state_of(&unmerged),
            (
                State::Dirty,
                "merged into origin/main, but has unmerged paths".into()
            )
        );
        assert_eq!(state_of(&locked).0, State::Locked);
        assert_eq!(state_of(&refused).0, State::Refused);
    }

    #[test]
    fn a_pushed_stale_worktree_with_changes_is_abandoned_not_removed() {
        let facts = Facts {
            merge: None,
            last_activity: Some(now() - chrono::Duration::days(21)),
            pushed_to: Some("origin".into()),
            local: vec!["1 uncommitted change".into()],
            ..changed(21)
        };

        assert_eq!(
            state_of(&facts),
            (
                State::Abandoned,
                "no commit or checkout for 21 days · 1 uncommitted change".into()
            )
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
            salvage: Some(Salvage::Ready {
                last_change: now() - chrono::Duration::days(21),
            }),
            local: vec!["2 uncommitted changes".into()],
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
        // Changes make it abandoned, offered and never removed; a guard leaves it stale.
        assert_eq!(state_of(&dirty).0, State::Abandoned);
        assert_eq!(
            state_of(&busy),
            (
                State::Stale,
                "no commit or checkout for 21 days · a process is running inside it".into()
            )
        );
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

    fn stale_with(local: &[&str]) -> Facts {
        Facts {
            last_activity: Some(now() - chrono::Duration::days(119)),
            local: local.iter().map(|part| (*part).to_string()).collect(),
            ..Facts::default()
        }
    }

    #[test]
    fn an_abandoned_row_names_everything_local_in_it() {
        let facts = Facts {
            dirt: Some(Dirt::Changes),
            salvage: Some(Salvage::Ready {
                last_change: now() - chrono::Duration::days(119),
            }),
            ..stale_with(&["13 uncommitted changes", "22 commits on no remote"])
        };
        let detached = stale_with(&["a detached HEAD"]);

        assert_eq!(
            state_of(&facts),
            (
                State::Abandoned,
                "no commit or checkout for 119 days · 13 uncommitted changes · 22 commits on no \
                 remote"
                    .into()
            )
        );
        assert_eq!(
            state_of(&detached),
            (
                State::Abandoned,
                "no commit or checkout for 119 days · a detached HEAD".into()
            )
        );
    }

    #[test]
    fn a_held_stale_row_names_what_is_local_and_the_guard() {
        let merging = Facts {
            dirt: Some(Dirt::Operation("merge")),
            ..stale_with(&["pull request #15 open"])
        };
        let locked = Facts {
            lock_reason: Some("agent".into()),
            ..stale_with(&["3 commits on no remote"])
        };
        let nested = Facts {
            dirt: Some(Dirt::Changes),
            salvage: Some(Salvage::Held(
                "an untracked repository at vendor/lib/ would be lost".into(),
            )),
            ..stale_with(&["1 uncommitted change"])
        };
        let unmerged = Facts {
            dirt: Some(Dirt::Unmerged),
            ..stale_with(&["2 uncommitted changes"])
        };

        assert_eq!(
            state_of(&merging),
            (
                State::Stale,
                "no commit or checkout for 119 days · pull request #15 open · a merge is in \
                 progress"
                    .into()
            )
        );
        assert_eq!(
            guard_of(&merging).as_deref(),
            Some("a merge is in progress")
        );
        assert_eq!(
            state_of(&locked).1,
            "no commit or checkout for 119 days · 3 commits on no remote · locked: agent"
        );
        assert_eq!(state_of(&nested).0, State::Stale);
        assert!(
            state_of(&nested)
                .1
                .ends_with("· an untracked repository at vendor/lib/ would be lost")
        );
        assert_eq!(
            state_of(&unmerged).1,
            "no commit or checkout for 119 days · 2 uncommitted changes · it has unmerged paths"
        );
    }

    #[test]
    fn a_kept_or_refused_abandoned_worktree_says_so() {
        let kept = Facts {
            is_kept: true,
            ..stale_with(&["a detached HEAD"])
        };
        let refused = Facts {
            refusal: Some("the save did not match the worktree".into()),
            ..stale_with(&["a detached HEAD"])
        };

        assert_eq!(state_of(&kept).0, State::Kept);
        assert_eq!(
            state_of(&refused),
            (
                State::Refused,
                "no commit or checkout for 119 days · a detached HEAD, but saving and removing it \
                 failed: the save did not match the worktree"
                    .into()
            )
        );
    }

    #[test]
    fn a_worktree_merged_by_commit_says_which_pull_request() {
        let found = Facts {
            merge: Some(Merge::Commit(212)),
            last_activity: Some(now() - chrono::Duration::days(90)),
            local: vec!["a detached HEAD".into()],
            ..Facts::default()
        };

        assert_eq!(
            state_of(&found),
            (State::Removable, "in #212, merged".into())
        );
        assert_eq!(stale_reason(&found, TWO_WEEKS, now()), None);
    }
}
