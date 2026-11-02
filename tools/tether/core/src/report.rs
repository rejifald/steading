//! `git tether status` and the tray's rows: each listed repo with its last sync
//! and its state. Read-only and offline: it judges against the
//! remote-tracking ref, and keeps a network failure the last sync recorded.

use std::fmt::Write as _;

use serde::Serialize;

use crate::config::{Mode, Repo};
use crate::ctx::Ctx;
use crate::follow;
use crate::inspect::{self, Health, Reason};
use crate::record::{self, Record};

#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub name: String,
    pub path: String,
    pub branch: String,
    pub mode: Mode,
    pub remote: String,
    pub interval_seconds: u64,
    /// The repo's `gate` and `after` commands (ADR 0004), as listed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<String>,
    pub health: Health,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    /// What the owner does to clear `reason` (`Reason::hint`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Commits the next sync will bring in, when the health is *behind*.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<usize>,
}

impl Row {
    pub fn reason_text(&self) -> Option<String> {
        self.reason.as_ref().map(ToString::to_string)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub paused: bool,
    /// The worst row: *needs you* beats *skipped* beats *behind* beats *in sync*.
    pub overall: Health,
    pub needs_you: usize,
    pub repos: Vec<Row>,
}

/// # Errors
///
/// Fails when the config cannot be read or validated.
pub fn report(ctx: &Ctx) -> anyhow::Result<Report> {
    let repos = ctx.config()?.repos(&ctx.paths)?;
    let records = record::load(&ctx.paths);
    let rows: Vec<Row> = repos
        .iter()
        .map(|r| row(ctx, r, records.get(&ctx.paths.tilde(&r.path))))
        .collect();
    let needs_you = rows.iter().filter(|r| r.health == Health::NeedsYou).count();
    let overall = if needs_you > 0 {
        Health::NeedsYou
    } else if rows.iter().any(|r| r.health == Health::Skipped) {
        Health::Skipped
    } else if rows.iter().any(|r| r.health == Health::Behind) {
        Health::Behind
    } else {
        Health::InSync
    };
    Ok(Report {
        paused: ctx.paths.is_paused(),
        overall,
        needs_you,
        repos: rows,
    })
}

fn row(ctx: &Ctx, repo: &Repo, last: Option<&Record>) -> Row {
    let reason = local_reason(ctx, repo)
        .or_else(|| after_failed(repo, last))
        .or_else(|| last.and_then(|l| l.reason.clone()).filter(is_sync_side));
    let behind = match reason {
        Some(_) => None,
        None => commits_to_pull(ctx, repo),
    };
    let health = match (&reason, behind) {
        (Some(r), _) => r.health(),
        (None, Some(_)) => Health::Behind,
        (None, None) => Health::InSync,
    };
    Row {
        name: repo.name(),
        path: ctx.paths.tilde(&repo.path),
        branch: repo.branch.clone(),
        mode: repo.mode,
        remote: repo.remote.clone(),
        interval_seconds: repo.interval.as_secs(),
        gate: repo.gate.clone(),
        after: repo.after.clone(),
        last_sync: last.map(|l| l.last_sync.clone()),
        health,
        hint: reason
            .as_ref()
            .and_then(|r| r.hint(&repo.remote, &repo.branch)),
        reason,
        behind,
    }
}

/// How many commits the local branch lacks from the remote-tracking ref, when
/// it lacks any. A mirror is reset rather than merged, so any difference
/// counts, and a shallow history that cannot be counted is one commit.
fn commits_to_pull(ctx: &Ctx, repo: &Repo) -> Option<usize> {
    let local = ctx.resolve(&repo.path, &format!("refs/heads/{}", repo.branch))?;
    let tip = ctx.resolve(&repo.path, &repo.tracking_ref())?;
    if local == tip {
        return None;
    }
    let count =
        inspect::ahead_behind(ctx, &repo.path, &local, &tip).map_or(0, |(_, behind)| behind);
    match repo.mode {
        Mode::Follow => (count > 0).then_some(count),
        Mode::Mirror => Some(count.max(1)),
    }
}

/// Reasons only a sync can see (it talked to the remote, tried to merge, or
/// asked the gate).
fn is_sync_side(r: &Reason) -> bool {
    matches!(
        r,
        Reason::FetchFailed { .. }
            | Reason::NoRemoteBranch { .. }
            | Reason::FastForwardRefused { .. }
            | Reason::Gated { .. }
    )
}

/// A failed `after` the clone itself cannot show: it stays until a run of it
/// succeeds, whatever the later syncs skipped for.
fn after_failed(repo: &Repo, last: Option<&Record>) -> Option<Reason> {
    repo.after.as_ref()?;
    let failed = last?.after_failed.as_ref()?;
    Some(Reason::AfterFailed {
        output: failed.output.clone(),
    })
}

/// What a sync would skip for, judged from the clone alone.
fn local_reason(ctx: &Ctx, repo: &Repo) -> Option<Reason> {
    if repo.mode == Mode::Mirror && crate::mirror::needs_fresh_clone(&repo.path) {
        // The next sync clones it, or says why it cannot.
        return repo.url.is_none().then_some(Reason::Missing);
    }
    let git_dir = match inspect::git_dir(ctx, &repo.path) {
        Ok(d) => d,
        Err(r) => return Some(r),
    };
    if let Some(r) = inspect::busy_reason(&git_dir) {
        return Some(r);
    }
    if repo.mode == Mode::Mirror {
        return None;
    }
    let tip = ctx.resolve(&repo.path, &repo.tracking_ref())?;
    follow::evaluate(ctx, repo, &tip, true).err()
}

/// Human-readable `status`.
#[must_use]
pub fn render(r: &Report) -> String {
    let mut out = String::new();
    let head = if r.paused {
        format!("git tether: {} (paused)", r.overall.label())
    } else {
        format!("git tether: {}", r.overall.label())
    };
    out.push_str(&head);
    out.push('\n');
    if r.repos.is_empty() {
        out.push_str("  no repos listed; add one with `git tether add <path> --branch <branch>`\n");
    }
    for row in &r.repos {
        let _ = writeln!(
            out,
            "  {:<10} {}  {} ({}, every {})  last sync {}",
            row.health.label(),
            row.path,
            row.branch,
            row.mode.label(),
            human_interval(row.interval_seconds),
            row.last_sync.as_deref().unwrap_or("never"),
        );
        if let Some(why) = row.reason_text() {
            let _ = writeln!(out, "             {why}");
        }
        if let Some(hint) = &row.hint {
            let _ = writeln!(out, "             {hint}");
        }
        if let Some(n) = row.behind {
            let _ = writeln!(out, "             {}", pending_text(n));
        }
    }
    out
}

/// The detail line under a *behind* row, in `status` and the tray.
#[must_use]
pub fn pending_text(commits: usize) -> String {
    format!("{commits} commit(s) to pull on the next sync")
}

#[must_use]
pub fn human_interval(secs: u64) -> String {
    match secs {
        s if s % 3600 == 0 => format!("{}h", s / 3600),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}
