//! A pass that acts (rules 7, 8, 10 to 12): under the clone's locks, fetch what moved, look at
//! every worktree, run the grace clocks, and remove what is due with git's own guards.
//!
//! Removal is `git worktree remove` without `--force`, then `git branch -d`; `git branch -D`
//! only when `gh` reports a merged pull request that contains the branch tip (rule 11, ADR
//! 0002). A merged worktree with changes is removed with `--force` only once they are saved to a
//! ref and checked (ADR dirty-merged-worktrees-are-salvaged). Nothing is pushed, no remote branch
//! is deleted, and a main clone's checkout is never changed.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::classify::{Dirt, Merge, Salvage, State};
use crate::config::{Config, Policy, Repo};
use crate::ctx::Ctx;
use crate::fetch;
use crate::git;
use crate::in_use::WorkingDirectories;
use crate::lock;
use crate::paths::{Paths, absolute, same_path};
use crate::report::{self, Inspected, RepoReport};
use crate::salvage::{self, Source};
use crate::state_file::{self, Clock, Refusal, RepoMemory};
use crate::worktree_list;

/// One worktree a pass removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Removed {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Why git kept the branch (`git branch -d` refused): *merged, branch kept*.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_kept: Option<String>,
    /// The salvage ref its uncommitted changes were saved to before it was removed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salvaged_to: Option<String>,
}

/// A worktree whose grace clock this pass started: the notification of rule 10.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Started {
    pub path: String,
    pub reason: String,
    /// When it will be removed; `None` under `policy = "ask"` or `"off"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removes_at: Option<DateTime<Utc>>,
}

/// A removal git refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refused {
    pub path: String,
    pub message: String,
}

/// What a pass did to one repo.
#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub repo: String,
    /// Why the pass left the repo alone (another writer holds it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_note: Option<String>,
    pub started: Vec<Started>,
    pub removed: Vec<Removed>,
    pub refused: Vec<Refused>,
    pub pruned: bool,
    /// The salvage refs deleted for their age.
    pub pruned_salvage: Vec<String>,
    /// Every command run, or, in a dry run, every command that would have run.
    pub log: Vec<String>,
    /// The repo as it is after the pass.
    pub report: RepoReport,
}

/// What [`plan`] decides for one repo.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Plan {
    /// The clocks to keep, by worktree path.
    pub clocks: BTreeMap<String, Clock>,
    /// Paths whose clock starts now.
    pub started: Vec<String>,
    /// Paths to remove now.
    pub removals: Vec<String>,
    pub should_prune: bool,
}

/// Runs the grace clocks of rule 10 and picks what is due, without any I/O.
pub(crate) fn plan(
    worktrees: &[Inspected],
    memory: &RepoMemory,
    policy: Policy,
    grace: Duration,
    now: DateTime<Utc>,
) -> Plan {
    let mut plan = Plan::default();
    let grace = chrono::Duration::from_std(grace).unwrap_or(chrono::Duration::MAX);
    let mut missing_due = Vec::new();
    // `git worktree prune` is repo-wide, so a kept entry whose folder is gone would be pruned
    // along with the others; it holds the prune back instead. Locked entries are skipped by git.
    let has_kept_missing = worktrees.iter().any(|worktree| {
        worktree.state == State::Kept
            && worktree.facts.is_missing
            && worktree.facts.lock_reason.is_none()
    });
    for worktree in worktrees {
        if !matches!(worktree.state, State::Removable | State::Missing) {
            continue;
        }
        let head = worktree.head.clone().unwrap_or_default();
        let clock = if let Some(clock) = memory.clock(&worktree.key, &head) {
            clock.clone()
        } else {
            plan.started.push(worktree.key.clone());
            Clock { head, since: now }
        };
        let is_due = now >= clock.since + grace;
        plan.clocks.insert(worktree.key.clone(), clock);
        match worktree.state {
            State::Removable if is_due && policy == Policy::Auto => {
                plan.removals.push(worktree.key.clone());
            }
            State::Missing => missing_due.push(is_due),
            _ => {}
        }
    }
    // Rule 12: prune once every missing entry has had its full grace and none is kept.
    plan.should_prune = policy == Policy::Auto
        && !has_kept_missing
        && !missing_due.is_empty()
        && missing_due.iter().all(|is_due| *is_due);
    plan
}

/// Sweeps every listed repo, or those `selector` names (a name or a path).
///
/// # Errors
///
/// Fails when `repos.toml` cannot be read, or `selector` names no listed repo.
pub fn sweep(
    ctx: &Ctx,
    selector: Option<&str>,
    wait: Duration,
    now: DateTime<Utc>,
) -> Result<Vec<Outcome>> {
    let repos = Config::load(&ctx.paths)?.repos(&ctx.paths)?;
    let chosen: Vec<&Repo> = repos
        .iter()
        .filter(|repo| selector.is_none_or(|selector| repo.matches(selector, &ctx.paths)))
        .collect();
    if let (Some(selector), true) = (selector, chosen.is_empty()) {
        bail!(crate::cli::UsageError(format!(
            "{selector} is not listed in repos.toml"
        )));
    }
    Ok(chosen
        .into_iter()
        .map(|repo| sweep_repo(ctx, repo, wait, now))
        .collect())
}

/// The tray's tick and `git reap sweep --due`: nothing while paused, else every repo whose last
/// pass is older than the interval, skipping any another run holds.
///
/// # Errors
///
/// Fails when `repos.toml` cannot be read.
pub fn sweep_due(ctx: &Ctx, now: DateTime<Utc>) -> Result<Vec<Outcome>> {
    if ctx.paths.is_paused() {
        return Ok(Vec::new());
    }
    let config = Config::load(&ctx.paths)?;
    let interval = chrono::Duration::from_std(config.interval()?).unwrap_or(chrono::Duration::MAX);
    let memories = state_file::load(&ctx.paths);
    Ok(config
        .repos(&ctx.paths)?
        .iter()
        .filter(|repo| {
            let last_pass = memories
                .get(&ctx.paths.tilde(&repo.path))
                .and_then(|memory| memory.last_pass);
            last_pass.is_none_or(|last| now >= last + interval)
        })
        .map(|repo| sweep_repo(ctx, repo, Duration::ZERO, now))
        .collect())
}

/// One pass over one repo. A repo that cannot be read or is busy comes back with its reason; it
/// never fails the others.
pub fn sweep_repo(ctx: &Ctx, repo: &Repo, wait: Duration, now: DateTime<Utc>) -> Outcome {
    let key = ctx.paths.tilde(&repo.path);
    let mut outcome = Outcome {
        repo: key.clone(),
        skipped: None,
        fetch_note: None,
        started: Vec::new(),
        removed: Vec::new(),
        refused: Vec::new(),
        pruned: false,
        pruned_salvage: Vec::new(),
        log: Vec::new(),
        report: report::unreadable(&ctx.paths, repo, &anyhow::anyhow!("not read yet")),
    };
    if let Err(e) = pass(ctx, repo, wait, now, &mut outcome) {
        outcome.report = report::unreadable(&ctx.paths, repo, &e);
    }
    outcome.log = ctx.take_log();
    outcome
}

fn pass(
    ctx: &Ctx,
    repo: &Repo,
    wait: Duration,
    now: DateTime<Utc>,
    outcome: &mut Outcome,
) -> Result<()> {
    let paths = &ctx.paths;
    let common_dir = git::path_of(&paths.git, &repo.path, &["rev-parse", "--git-common-dir"])
        .with_context(|| format!("{} is not a git clone", paths.tilde(&repo.path)))?;
    let lock = match lock::acquire(&common_dir, wait)? {
        Ok(lock) => lock,
        Err(why) => {
            outcome.skipped = Some(why);
            outcome.report = read_only(paths, repo, now)?;
            report::measure(paths, &mut outcome.report);
            return Ok(());
        }
    };
    let result = locked_pass(ctx, repo, now, outcome);
    // Sizes are display-only: walk the folders after the lock is released.
    drop(lock);
    result?;
    report::measure(paths, &mut outcome.report);
    Ok(())
}

/// The part of a pass that runs under the clone's lock.
fn locked_pass(ctx: &Ctx, repo: &Repo, now: DateTime<Utc>, outcome: &mut Outcome) -> Result<()> {
    let paths = &ctx.paths;
    let prepared = report::prepare(paths, repo)?;
    outcome.fetch_note = fetch::fetch_moved(ctx, repo, &prepared);
    let directories = WorkingDirectories::read().map_err(|e| format!("{e:#}"));
    let key = paths.tilde(&repo.path);
    let memory = state_file::load(paths).remove(&key).unwrap_or_default();
    let inspection = report::inspect(paths, repo, &prepared, &directories, &memory, now)?;
    let plan = plan(&inspection.worktrees, &memory, repo.policy, repo.grace, now);
    let grace = chrono::Duration::from_std(repo.grace).unwrap_or(chrono::Duration::MAX);
    outcome.started = plan
        .started
        .iter()
        .filter_map(|path| {
            inspection
                .report
                .worktrees
                .iter()
                .find(|row| &row.path == path)
        })
        .map(|row| Started {
            path: row.path.clone(),
            reason: row.reason.clone(),
            removes_at: (repo.policy == Policy::Auto).then(|| now + grace),
        })
        .collect();
    // The lsof of the inspection is seconds old; a process that started since keeps its worktree.
    let fresh = if plan.removals.is_empty() {
        None
    } else {
        Some(WorkingDirectories::read().context("checking for processes before removing")?)
    };
    let mut refusals = BTreeMap::new();
    for path in &plan.removals {
        let Some(worktree) = inspection
            .worktrees
            .iter()
            .find(|worktree| &worktree.key == path)
        else {
            continue;
        };
        if fresh
            .as_ref()
            .is_some_and(|fresh| fresh.is_inside(&paths.expand(path)))
        {
            continue;
        }
        match remove(ctx, repo, worktree, now)? {
            Ok(removed) => outcome.removed.push(removed),
            Err(message) => {
                refusals.insert(path.clone(), message);
            }
        }
    }
    if plan.should_prune {
        ctx.write(&repo.path, &["worktree", "prune"])?;
        outcome.pruned = !ctx.dry_run;
    }
    outcome.refused = refusals
        .iter()
        .map(|(path, message)| Refused {
            path: path.clone(),
            message: message.clone(),
        })
        .collect();
    if !ctx.dry_run {
        remember(
            paths,
            &key,
            &inspection.worktrees,
            plan.clocks,
            &refusals,
            outcome,
            now,
        )?;
    }
    // After the state is saved: a failed deletion must not lose this pass's clocks and refusals.
    outcome.pruned_salvage = salvage::prune(ctx, repo, now)?;
    let has_changed = !outcome.removed.is_empty()
        || !refusals.is_empty()
        || outcome.pruned
        || !outcome.pruned_salvage.is_empty()
        || !outcome.started.is_empty();
    outcome.report = if has_changed && !ctx.dry_run {
        // Re-read after acting, so the report shows the clocks and removals of this pass.
        let memory = state_file::load(paths).remove(&key).unwrap_or_default();
        report::inspect(paths, repo, &prepared, &directories, &memory, now)?.report
    } else {
        inspection.report
    };
    Ok(())
}

/// The repo's report without acting, for a busy repo.
fn read_only(paths: &Paths, repo: &Repo, now: DateTime<Utc>) -> Result<RepoReport> {
    let prepared = report::prepare(paths, repo)?;
    let directories = WorkingDirectories::read().map_err(|e| format!("{e:#}"));
    let memory = state_file::load(paths)
        .remove(&paths.tilde(&repo.path))
        .unwrap_or_default();
    Ok(report::inspect(paths, repo, &prepared, &directories, &memory, now)?.report)
}

/// Saves this pass's clocks and refusals, ends Keeps whose HEAD moved, and records the pass.
fn remember(
    paths: &Paths,
    key: &str,
    worktrees: &[Inspected],
    clocks: BTreeMap<String, Clock>,
    refusals: &BTreeMap<String, String>,
    outcome: &Outcome,
    now: DateTime<Utc>,
) -> Result<()> {
    let removed: Vec<&str> = outcome
        .removed
        .iter()
        .map(|removed| removed.path.as_str())
        .collect();
    let pruned_missing = |path: &str| {
        outcome.pruned
            && worktrees
                .iter()
                .any(|worktree| worktree.key == path && worktree.state == State::Missing)
    };
    let head_of = |path: &str| {
        worktrees
            .iter()
            .find(|worktree| worktree.key == path)
            .and_then(|worktree| worktree.head.clone())
    };
    state_file::update(paths, key, |memory| {
        memory.clocks = clocks;
        memory
            .clocks
            .retain(|path, _| !removed.contains(&path.as_str()) && !pruned_missing(path));
        memory
            .keeps
            .retain(|path, head| head_of(path).as_deref() == Some(head.as_str()));
        memory.refusals.retain(|path, refusal| {
            let is_still_refused = worktrees
                .iter()
                .any(|worktree| &worktree.key == path && worktree.state == State::Refused);
            is_still_refused && head_of(path).as_deref() == Some(refusal.head.as_str())
        });
        for (path, message) in refusals {
            if let Some(head) = head_of(path) {
                memory.refusals.insert(
                    path.clone(),
                    Refusal {
                        head,
                        message: message.clone(),
                    },
                );
            }
        }
        memory.last_pass = Some(now);
    })
}

/// Removes one worktree and then its branch (rule 11), saving its changes to a ref first when
/// it has any (ADR dirty-merged-worktrees-are-salvaged). `Ok(Err(message))` is git's refusal,
/// or why the save failed.
fn remove(
    ctx: &Ctx,
    repo: &Repo,
    worktree: &Inspected,
    now: DateTime<Utc>,
) -> Result<Result<Removed, String>> {
    let path = ctx.paths.expand(&worktree.key);
    let path_text = path.to_string_lossy();
    let has_changes = worktree.facts.dirt == Some(Dirt::Changes);
    let salvaged_to = match (&worktree.facts.salvage, &worktree.head) {
        (Some(Salvage::Ready { .. }), Some(head)) if has_changes => {
            let source = Source {
                key: &worktree.key,
                branch: worktree.branch.as_deref(),
                head,
            };
            match salvage::save(ctx, repo, &source, now)? {
                Ok(refname) => Some(refname),
                Err(why) => return Ok(Err(format!("saving its changes failed: {why}"))),
            }
        }
        // Only a save makes `--force` safe; a dirty worktree without one is git's to refuse.
        _ => None,
    };
    let args: &[&str] = if salvaged_to.is_some() {
        &["worktree", "remove", "--force", &path_text]
    } else {
        &["worktree", "remove", &path_text]
    };
    let Some(output) = ctx.write_output(&repo.path, args)? else {
        return Ok(Ok(Removed {
            path: worktree.key.clone(),
            branch: worktree.branch.clone(),
            branch_kept: None,
            salvaged_to,
        }));
    };
    if !output.status.success() {
        return Ok(Err(git::first_error_line(&output)));
    }
    let branch_kept = match &worktree.branch {
        // A pushed stale worktree is not merged: its branch is where the work lives (ADR 0003).
        Some(_) if worktree.facts.merge.is_none() => Some("not merged".to_string()),
        Some(branch) => delete_branch(ctx, repo, worktree, branch)?,
        None => None,
    };
    Ok(Ok(Removed {
        path: worktree.key.clone(),
        branch: worktree.branch.clone(),
        branch_kept,
        salvaged_to,
    }))
}

/// `git branch -D` when the branch tip is HEAD and the merged pull request contains it (rule
/// 3(c), ADR 0002), else `-d`;
/// `Some(why)` when git kept the branch.
fn delete_branch(
    ctx: &Ctx,
    repo: &Repo,
    worktree: &Inspected,
    branch: &str,
) -> Result<Option<String>> {
    let tip = git::resolve(&ctx.paths.git, &repo.path, &format!("refs/heads/{branch}"));
    let is_merged_at_tip = matches!(worktree.facts.merge, Some(Merge::PullRequest(_)))
        && tip.is_some()
        && tip == worktree.head;
    let flag = if is_merged_at_tip { "-D" } else { "-d" };
    let output = ctx.write_output(&repo.path, &["branch", flag, branch])?;
    Ok(output
        .filter(|output| !output.status.success())
        .map(|output| git::first_error_line(&output)))
}

/// Finds the listed repo a linked worktree belongs to.
fn locate(paths: &Paths, selector: &str) -> Result<(Repo, PathBuf)> {
    let wanted = absolute(&paths.expand(selector));
    for repo in Config::load(paths)?.repos(paths)? {
        let Ok(output) = git::run(
            &paths.git,
            &repo.path,
            &["worktree", "list", "--porcelain", "-z"],
        ) else {
            continue;
        };
        let worktrees = worktree_list::parse(&output)?;
        if worktrees
            .iter()
            .skip(1)
            .any(|worktree| same_path(&worktree.path, &wanted))
        {
            return Ok((repo, wanted));
        }
    }
    bail!(crate::cli::UsageError(format!(
        "{} is not a linked worktree of a listed repo",
        paths.tilde(&wanted)
    )))
}

/// *Keep* (rule 10): holds a worktree until its HEAD moves. Returns its path as shown.
///
/// # Errors
///
/// Fails when the worktree is not a linked worktree of a listed repo, or has no HEAD.
pub fn keep(paths: &Paths, selector: &str) -> Result<String> {
    let (repo, path) = locate(paths, selector)?;
    let head = git::run(&paths.git, &path, &["rev-parse", "HEAD"]).context("reading its HEAD")?;
    let key = paths.tilde(&path);
    state_file::update(paths, &paths.tilde(&repo.path), |memory| {
        memory.keeps.insert(key.clone(), head);
        memory.clocks.remove(&key);
    })?;
    Ok(key)
}

/// *Unkeep*: ends a Keep by hand. Returns its path as shown, or `None` when it was not kept.
///
/// # Errors
///
/// Fails when the worktree is not a linked worktree of a listed repo.
pub fn unkeep(paths: &Paths, selector: &str) -> Result<Option<String>> {
    let (repo, path) = locate(paths, selector)?;
    let key = paths.tilde(&path);
    let was_kept = state_file::update(paths, &paths.tilde(&repo.path), |memory| {
        memory.keeps.remove(&key).is_some()
    })?;
    Ok(was_kept.then_some(key))
}

/// *Remove now* (rule 15): removes one removable or refused worktree at once, with the same
/// guards and no grace period, whatever the policy.
///
/// # Errors
///
/// Fails when the worktree is not a linked worktree of a listed repo, its repo is busy, or it
/// is not removable.
pub fn remove_now(ctx: &Ctx, selector: &str, now: DateTime<Utc>) -> Result<Removed> {
    let paths = &ctx.paths;
    let (repo, path) = locate(paths, selector)?;
    let key = paths.tilde(&path);
    let common_dir = git::path_of(&paths.git, &repo.path, &["rev-parse", "--git-common-dir"])?;
    let _lock = lock::acquire(&common_dir, Duration::from_secs(10))?
        .map_err(|why| anyhow::anyhow!("{} is busy: {why}", paths.tilde(&repo.path)))?;
    let prepared = report::prepare(paths, &repo)?;
    let directories = WorkingDirectories::read().map_err(|e| format!("{e:#}"));
    let mut memory = state_file::load(paths)
        .remove(&paths.tilde(&repo.path))
        .unwrap_or_default();
    // A refusal is retried on request: drop it so the worktree is judged afresh.
    memory.refusals.remove(&key);
    let inspection = report::inspect(paths, &repo, &prepared, &directories, &memory, now)?;
    let (worktree, row) = inspection
        .worktrees
        .iter()
        .zip(&inspection.report.worktrees)
        .find(|(worktree, _)| worktree.key == key)
        .context("the worktree disappeared while reap looked at it")?;
    if worktree.state != State::Removable {
        bail!("{key} is not removable: {}", row.reason);
    }
    let removed = remove(ctx, &repo, worktree, now)?;
    if !ctx.dry_run {
        state_file::update(paths, &paths.tilde(&repo.path), |memory| {
            memory.clocks.remove(&key);
            match &removed {
                Ok(_) => {
                    memory.refusals.remove(&key);
                }
                Err(message) => {
                    memory.refusals.insert(
                        key.clone(),
                        Refusal {
                            head: worktree.head.clone().unwrap_or_default(),
                            message: message.clone(),
                        },
                    );
                }
            }
        })?;
    }
    removed.map_err(|message| anyhow::anyhow!("git refused to remove {key}: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::Facts;

    const DAY: Duration = Duration::from_secs(24 * 3600);

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-29T12:00:00Z")
            .unwrap()
            .into()
    }

    fn worktree(key: &str, head: &str, state: State) -> Inspected {
        Inspected {
            key: key.into(),
            branch: Some(key.into()),
            head: Some(head.into()),
            facts: Facts::default(),
            state,
        }
    }

    fn clock(head: &str, hours_ago: i64) -> Clock {
        Clock {
            head: head.into(),
            since: now() - chrono::Duration::hours(hours_ago),
        }
    }

    #[test]
    fn a_newly_removable_worktree_starts_its_clock_and_waits() {
        let plan = plan(
            &[worktree("~/w", "a", State::Removable)],
            &RepoMemory::default(),
            Policy::Auto,
            DAY,
            now(),
        );

        assert_eq!(plan.started, ["~/w"]);
        assert_eq!(plan.clocks["~/w"].since, now());
        assert_eq!(plan.removals, Vec::<String>::new());
    }

    #[test]
    fn a_worktree_is_removed_once_its_grace_has_passed() {
        let mut memory = RepoMemory::default();
        memory.clocks.insert("~/old".into(), clock("a", 25));
        memory.clocks.insert("~/young".into(), clock("b", 23));
        let worktrees = [
            worktree("~/old", "a", State::Removable),
            worktree("~/young", "b", State::Removable),
        ];

        let plan = plan(&worktrees, &memory, Policy::Auto, DAY, now());

        assert_eq!(plan.removals, ["~/old"]);
        assert_eq!(plan.started, Vec::<String>::new());
    }

    #[test]
    fn new_commits_restart_the_clock() {
        let mut memory = RepoMemory::default();
        memory.clocks.insert("~/w".into(), clock("old-head", 48));

        let plan = plan(
            &[worktree("~/w", "new-head", State::Removable)],
            &memory,
            Policy::Auto,
            DAY,
            now(),
        );

        assert_eq!(plan.removals, Vec::<String>::new());
        assert_eq!(plan.clocks["~/w"].head, "new-head");
    }

    #[test]
    fn a_worktree_that_stops_being_removable_loses_its_clock() {
        let mut memory = RepoMemory::default();
        memory.clocks.insert("~/w".into(), clock("a", 48));

        let plan = plan(
            &[worktree("~/w", "a", State::Dirty)],
            &memory,
            Policy::Auto,
            DAY,
            now(),
        );

        assert!(plan.clocks.is_empty() && plan.removals.is_empty());
    }

    #[test]
    fn ask_and_off_run_the_clock_but_never_remove() {
        let mut memory = RepoMemory::default();
        memory.clocks.insert("~/w".into(), clock("a", 48));
        let worktrees = [
            worktree("~/w", "a", State::Removable),
            worktree("~/gone", "g", State::Missing),
        ];
        let mut memory_with_missing = memory.clone();
        memory_with_missing
            .clocks
            .insert("~/gone".into(), clock("g", 48));

        for policy in [Policy::Ask, Policy::Off] {
            let plan = plan(&worktrees, &memory_with_missing, policy, DAY, now());
            assert!(plan.removals.is_empty() && !plan.should_prune, "{policy:?}");
        }
    }

    #[test]
    fn missing_entries_are_pruned_only_when_all_have_had_their_grace() {
        let mut memory = RepoMemory::default();
        memory.clocks.insert("~/gone-long".into(), clock("a", 48));
        let worktrees = [
            worktree("~/gone-long", "a", State::Missing),
            worktree("~/gone-now", "b", State::Missing),
        ];

        let waiting = plan(&worktrees, &memory, Policy::Auto, DAY, now());
        memory.clocks.insert("~/gone-now".into(), clock("b", 25));
        let due = plan(&worktrees, &memory, Policy::Auto, DAY, now());

        assert!(!waiting.should_prune);
        assert!(due.should_prune);
    }

    #[test]
    fn a_kept_missing_entry_holds_the_prune_back() {
        let mut memory = RepoMemory::default();
        memory.clocks.insert("~/gone".into(), clock("a", 48));
        let mut kept = worktree("~/kept", "b", State::Kept);
        kept.facts.is_missing = true;
        let worktrees = [worktree("~/gone", "a", State::Missing), kept];

        let plan = plan(&worktrees, &memory, Policy::Auto, DAY, now());

        assert!(!plan.should_prune);
    }
}
