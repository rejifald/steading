//! One sync of one repo: the repo's `gate` (background syncs only), lock, skip
//! checks, poll, the mode's own step (`follow` or `mirror`), and the repo's
//! `after` once the lock is released (ADR 0004). Also the `--due` tick that
//! runs every repo whose interval has passed, unless background sync is paused.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use chrono::Local;
use serde::Serialize;

use crate::config::{Mode, Repo};
use crate::ctx::Ctx;
use crate::inspect::{self, Health, Reason};
use crate::record::{self, AfterFailed, Record};
use crate::{follow, git, lock, mirror, shell};

/// What one sync did.
#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub path: String,
    pub health: Health,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    /// `(from, to)` when the branch tip moved (`from` is empty for a fresh clone).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moved: Option<(String, String)>,
    /// Whether the remote was fetched (rule 3: only when its tip changed).
    pub fetched: bool,
    /// A `mirror` sync that threw away tracked edits or untracked files.
    pub discarded: bool,
    /// The patch saved before discarding tracked changes (rule 9).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<PathBuf>,
    pub log: Vec<String>,
}

impl Outcome {
    pub fn new(ctx: &Ctx, repo: &Repo) -> Self {
        Self {
            path: ctx.paths.tilde(&repo.path),
            health: Health::InSync,
            reason: None,
            moved: None,
            fetched: false,
            discarded: false,
            backup: None,
            log: Vec::new(),
        }
    }

    pub fn skip(mut self, reason: Reason) -> Self {
        self.health = reason.health();
        self.reason = Some(reason);
        self
    }

    pub fn describe(&self) -> String {
        match (&self.reason, &self.moved) {
            (Some(r), _) => format!("{}: {}", self.health.label(), r),
            (None, Some((from, to))) if from.is_empty() => format!("cloned at {}", short(to)),
            (None, Some((from, to))) => format!("updated {}..{}", short(from), short(to)),
            (None, None) => self.health.label().into(),
        }
    }
}

pub fn short(sha: &str) -> &str {
    &sha[..sha.len().min(9)]
}

/// The remote side after a poll.
#[derive(Debug, Clone)]
pub struct Poll {
    /// The branch tip on the remote.
    pub tip: String,
    pub fetched: bool,
    /// Whether the tip's objects are in the clone. False only in a dry run that
    /// skipped a needed fetch: the step can then only describe its plan.
    pub available: bool,
}

/// Rule 3: `ls-remote`, and fetch only when the remote tip differs from the
/// remote-tracking ref. `depth` is `Some(1)` for a mirror.
pub fn poll(ctx: &Ctx, repo: &Repo, depth: Option<u32>) -> Result<Poll, Reason> {
    let dir = &repo.path;
    let tip = match git::ls_remote(&ctx.paths.git, dir, &repo.remote, &repo.branch) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return Err(Reason::NoRemoteBranch {
                remote: repo.remote.clone(),
                branch: repo.branch.clone(),
            });
        }
        Err(e) => {
            return Err(Reason::FetchFailed {
                error: format!("{e:#}"),
            });
        }
    };
    let tracking = repo.tracking_ref();
    if ctx.resolve(dir, &tracking).as_deref() == Some(tip.as_str()) {
        return Ok(Poll {
            tip,
            fetched: false,
            available: true,
        });
    }
    let refspec = format!("+refs/heads/{}:{tracking}", repo.branch);
    let depth_arg = depth.map(|d| format!("--depth={d}"));
    let mut args = vec!["fetch", "--quiet", "--no-tags"];
    if let Some(d) = &depth_arg {
        args.push(d);
    }
    args.extend([repo.remote.as_str(), refspec.as_str()]);
    match ctx.write_output(dir, &args) {
        Ok(Some(out)) if !out.status.success() => Err(Reason::FetchFailed {
            error: git::first_error_line(&out),
        }),
        Err(e) => Err(Reason::FetchFailed {
            error: format!("{e:#}"),
        }),
        Ok(Some(_)) => Ok(Poll {
            tip,
            fetched: true,
            available: true,
        }),
        Ok(None) => {
            let available = ctx.read_ok(dir, &["cat-file", "-e", &format!("{tip}^{{commit}}")]);
            Ok(Poll {
                tip,
                fetched: false,
                available,
            })
        }
    }
}

/// What started a sync. It decides whether the repo's `gate` is asked and how
/// long to wait for another run's lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The tray's timers and `sync --due`: asks the gate, never waits for the
    /// lock (another run holding it skips the repo this round).
    Background,
    /// *Sync now*, *Sync all* and `git tether sync [<repo>]`: does not ask the
    /// gate, waits up to `wait` for another run, and runs a failed `after` again.
    Explicit { wait: Duration },
}

impl Trigger {
    fn wait(self) -> Duration {
        match self {
            Trigger::Background => Duration::ZERO,
            Trigger::Explicit { wait } => wait,
        }
    }
}

/// Syncs one repo and records the result.
pub fn sync_repo(ctx: &Ctx, repo: &Repo, trigger: Trigger) -> Result<Outcome> {
    let mut out = Outcome::new(ctx, repo);
    // Held until the record is saved: `after_failed` is read here and written
    // back below, with `after` (and tether.lock released) in between.
    let Some(_after_lock) = lock_after(ctx, repo, &out.path, trigger)? else {
        // Another run owns this repo's record; writing ours would undo theirs.
        out.log = ctx.take_log();
        return Ok(out.skip(Reason::Busy));
    };
    let previous = record::load(&ctx.paths).remove(&out.path);
    let mut after_failed = previous.as_ref().and_then(|r| r.after_failed.clone());
    let result = ask_gate(ctx, repo, trigger)
        .and_then(|()| run_steps(ctx, repo, trigger.wait(), &mut out))
        // run_steps has dropped the lock by now, so `after` can take it itself.
        .and_then(|()| run_after(ctx, repo, trigger, &mut out, &mut after_failed));
    out.log = ctx.take_log();
    let out = match result {
        Ok(()) => out,
        Err(reason) => out.skip(reason),
    };
    if !ctx.dry_run {
        let tip = if matches!(out.reason, Some(Reason::Gated { .. })) {
            // A gate that says no leaves the clone unread, git included.
            previous.and_then(|r| r.tip)
        } else {
            ctx.resolve(&repo.path, &format!("refs/heads/{}", repo.branch))
        };
        record::save(
            &ctx.paths,
            &out.path,
            Record {
                last_sync: record::now(),
                health: out.health,
                reason: out.reason.clone(),
                tip,
                after_failed,
            },
        )?;
    }
    Ok(out)
}

/// The per-repo `after` lock ([`crate::paths::Paths::after_lock`]): `Some(None)`
/// when the repo has no `after` or this is a dry run, `None` when another run
/// still holds it after the trigger's wait.
fn lock_after(
    ctx: &Ctx,
    repo: &Repo,
    key: &str,
    trigger: Trigger,
) -> Result<Option<Option<lock::Lock>>> {
    if repo.after.is_none() || ctx.dry_run {
        return Ok(Some(None));
    }
    Ok(lock::acquire_file(&ctx.paths.after_lock(key), trigger.wait())?.map(Some))
}

/// ADR 0004 rule 2: a background sync goes ahead only when the gate exits 0
/// within [`Ctx::gate_timeout`]. Runs before the lock and any git command.
fn ask_gate(ctx: &Ctx, repo: &Repo, trigger: Trigger) -> Result<(), Reason> {
    let (Trigger::Background, Some(gate)) = (trigger, &repo.gate) else {
        return Ok(());
    };
    if ctx.dry_run {
        ctx.note(format!("would run gate: {gate}"));
        return Ok(());
    }
    ctx.note(format!("run gate: {gate}"));
    match run_command(ctx, repo, gate, &[], Some(ctx.gate_timeout)) {
        Ok(ran) if ran.is_success() => Ok(()),
        Ok(ran) => Err(Reason::Gated {
            output: ran.summary(),
        }),
        Err(e) => Err(Reason::Gated {
            output: format!("{e:#}"),
        }),
    }
}

/// ADR 0004 rule 3: `after` runs for a sync that moved the tip, from the last
/// tip it succeeded at. A failure stays in `failed` (and the repo *needs you*)
/// until a later move runs it successfully, or an explicit sync runs it again.
fn run_after(
    ctx: &Ctx,
    repo: &Repo,
    trigger: Trigger,
    out: &mut Outcome,
    failed: &mut Option<AfterFailed>,
) -> Result<(), Reason> {
    let Some(after) = &repo.after else {
        // An `after` taken out of repos.toml leaves nothing to fix.
        *failed = None;
        return Ok(());
    };
    let is_retry = matches!(trigger, Trigger::Explicit { .. });
    if ctx.dry_run {
        if out.health == Health::Behind || (is_retry && failed.is_some()) {
            ctx.note(format!("would run after: {after}"));
        }
        return Ok(());
    }
    let (from, to) = match (&out.moved, failed.as_ref()) {
        (Some((from, to)), pending) => (pending.map_or(from, |p| &p.from).clone(), to.clone()),
        (None, Some(p)) if is_retry => (p.from.clone(), p.to.clone()),
        (None, Some(p)) => {
            return Err(Reason::AfterFailed {
                output: p.output.clone(),
            });
        }
        (None, None) => return Ok(()),
    };
    ctx.note(format!("run after: {after}"));
    let env = [("TETHER_FROM", from.as_str()), ("TETHER_TO", to.as_str())];
    let output = match run_command(ctx, repo, after, &env, None) {
        Ok(ran) if ran.is_success() => {
            *failed = None;
            return Ok(());
        }
        Ok(ran) => ran.summary(),
        Err(e) => format!("{e:#}"),
    };
    *failed = Some(AfterFailed {
        from,
        to,
        output: output.clone(),
    });
    Err(Reason::AfterFailed { output })
}

/// A repo's `gate` or `after`, in the clone (in `$HOME` while a mirror is not
/// cloned yet) with `TETHER_REPO`, `TETHER_BRANCH` and `extra` set.
fn run_command(
    ctx: &Ctx,
    repo: &Repo,
    script: &str,
    extra: &[(&str, &str)],
    timeout: Option<Duration>,
) -> Result<shell::Ran> {
    let dir = if repo.path.is_dir() {
        &repo.path
    } else {
        &ctx.paths.home
    };
    let listed = ctx.paths.tilde(&repo.path);
    let mut env = vec![
        ("TETHER_REPO", listed.as_str()),
        ("TETHER_BRANCH", repo.branch.as_str()),
    ];
    env.extend_from_slice(extra);
    shell::run(script, dir, &env, timeout)
}

fn run_steps(ctx: &Ctx, repo: &Repo, wait: Duration, out: &mut Outcome) -> Result<(), Reason> {
    if repo.mode == Mode::Mirror && mirror::needs_fresh_clone(&repo.path) {
        return mirror::clone_fresh(ctx, repo, out);
    }
    let git_dir = inspect::git_dir(ctx, &repo.path)?;
    // A dry run changes nothing, so it needs no lock (and creates no lock file).
    let _lock = if ctx.dry_run {
        None
    } else {
        match lock::acquire(&git_dir, wait) {
            Ok(Some(l)) => Some(l),
            Ok(None) => return Err(Reason::Busy),
            Err(e) => {
                return Err(Reason::NotAClone {
                    detail: format!("{e:#}"),
                });
            }
        }
    };
    if let Some(r) = inspect::busy_reason(&git_dir) {
        return Err(r);
    }
    match repo.mode {
        Mode::Follow => {
            let p = poll(ctx, repo, None)?;
            out.fetched = p.fetched;
            follow::apply(ctx, repo, &p, out)
        }
        Mode::Mirror => {
            let p = poll(ctx, repo, Some(1))?;
            out.fetched = p.fetched;
            mirror::apply(ctx, repo, &p, out)
        }
    }
}

/// Whether `repo` is due: never synced, or its last sync is `interval` old.
pub fn is_due(ctx: &Ctx, repo: &Repo, records: &record::Records) -> bool {
    let Some(last) = records
        .get(&ctx.paths.tilde(&repo.path))
        .and_then(Record::last_sync_time)
    else {
        return true;
    };
    let age = Local::now()
        .signed_duration_since(last)
        .to_std()
        .unwrap_or(Duration::ZERO);
    age >= repo.interval
}

/// The background tick (`git tether sync --due`): every due repo, never waiting
/// for a lock. Pause stops it entirely.
pub fn tick(ctx: &Ctx) -> Result<Vec<Outcome>> {
    if ctx.paths.is_paused() {
        return Ok(Vec::new());
    }
    let repos = ctx.config()?.repos(&ctx.paths)?;
    let records = record::load(&ctx.paths);
    let mut outs = Vec::new();
    for repo in repos.iter().filter(|r| is_due(ctx, r, &records)) {
        outs.push(sync_repo(ctx, repo, Trigger::Background)?);
    }
    Ok(outs)
}
