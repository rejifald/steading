//! The CLI (rule 14): `git reap status|sweep|salvage|salvaged|ps|stop|add|remove|discover|skip|unskip|keep|unkeep|pause|resume`. It
//! reads git and the config directly; there is no IPC with the tray app.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};

use crate::add;
use crate::config::Config;
use crate::ctx::Ctx;
use crate::discovery;
use crate::paths::{Paths, absolute};
use crate::pressure::Pressure;
use crate::report;
use crate::salvage;
use crate::servers::{self, StopState};
use crate::sweep::{self, Outcome, Swept};

/// How long an explicit `sweep` waits for another run to release a clone.
const EXPLICIT_WAIT: Duration = Duration::from_secs(30);

#[derive(Parser)]
#[command(
    name = "git-reap",
    bin_name = "git reap",
    version,
    about = "Removes the git worktrees whose work has merged (ADR 0001)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Every linked worktree of every listed repo, with its pull request and reap state.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Run a pass now over every listed repo (Pause does not apply), or one by name or path:
    /// fetch what moved, start grace clocks, remove what is due.
    Sweep {
        repo: Option<String>,
        /// Print every command that would change a repo, prefixed `would run:`, and run none.
        #[arg(long)]
        dry_run: bool,
        /// Background mode: only repos whose last pass is older than the interval, skipping
        /// any another run holds; does nothing while paused.
        #[arg(long, conflicts_with = "repo")]
        due: bool,
        #[arg(long)]
        json: bool,
    },
    /// Save an abandoned worktree's whole state to a ref, then remove it, keeping its branch
    /// (ADR 0006). Only a stale worktree with something local that no guard holds.
    Salvage {
        worktree: String,
        /// Print every command that would change a repo, prefixed `would run:`, and run none.
        #[arg(long)]
        dry_run: bool,
    },
    /// The saves of removed worktrees, abandoned ones apart, and how to restore each (ADRs
    /// dirty-merged-worktrees-are-salvaged and 0006).
    Salvaged {
        #[arg(long, conflicts_with = "delete")]
        json: bool,
        /// Delete one save, by its full ref (`refs/reap/salvage/…`).
        #[arg(long, value_name = "REF")]
        delete: Option<String>,
        /// The repo, by name or path, whose save `--delete` deletes, when several have the ref.
        #[arg(long, requires = "delete")]
        repo: Option<String>,
    },
    /// The dev servers running in listed clones' linked worktrees: every process group that
    /// listens on a TCP port with a process inside one (ADR 0005).
    Ps {
        #[arg(long)]
        json: bool,
    },
    /// Stop one server: SIGTERM to its process group, up to 10 s for it to exit, then SIGHUP to
    /// its idle launching shell. Never SIGKILL; only your own processes, only servers inside a
    /// listed clone's linked worktree (ADR 0005).
    Stop {
        pid: u32,
        /// The `started_at` that `ps --json` listed; refuses if the pid is now another process.
        #[arg(long, value_name = "RFC3339")]
        started_at: Option<String>,
        /// Check everything and print the signals as `would run:`, sending none.
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// List a main clone in repos.toml (rule 1).
    Add {
        path: PathBuf,
        /// The remote to judge merges against (default origin).
        #[arg(long)]
        remote: Option<String>,
        /// The base branch on that remote (default the remote's default branch).
        #[arg(long)]
        base: Option<String>,
    },
    /// Unlist a repo by name or path; its clone and worktrees are left as they are.
    Remove { repo: String },
    /// Look in the scan folders for clones with worktrees that are not listed and print them
    /// (repo ADR 0004). Lists nothing: that is `add`.
    Discover {
        #[arg(long)]
        json: bool,
    },
    /// Stop suggesting a clone (adds it to `skip` in repos.toml).
    Skip { path: String },
    /// Suggest a skipped clone again.
    Unskip { path: String },
    /// Hold a worktree until it gets new commits (rule 10).
    Keep { worktree: String },
    /// End a Keep by hand.
    Unkeep { worktree: String },
    /// Pause background sweeping.
    Pause,
    /// Resume background sweeping.
    Resume,
}

/// A mistake in the command line that clap cannot see (an unknown repo name): exits 2 like
/// any other usage error.
#[derive(Debug)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for UsageError {}

/// Runs the CLI and returns the process exit code.
pub fn run<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() { 2 } else { 0 };
        }
    };
    match dispatch(cli.command) {
        Ok(()) => 0,
        Err(e) if e.downcast_ref::<UsageError>().is_some() => {
            eprintln!("git reap: {e}");
            2
        }
        Err(e) => {
            eprintln!("git reap: {e:#}");
            1
        }
    }
}

fn dispatch(command: Command) -> Result<()> {
    let paths = Paths::from_env()?;
    match command {
        Command::Status { json } => {
            let report = report::report(&paths, Utc::now())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report::render(&report));
            }
        }
        Command::Sweep {
            repo,
            dry_run,
            due,
            json,
        } => sweep_command(paths, repo.as_deref(), dry_run, due, json)?,
        Command::Salvage { worktree, dry_run } => salvage_command(paths, &worktree, dry_run)?,
        Command::Salvaged {
            delete: Some(refname),
            repo,
            ..
        } => {
            let ctx = Ctx::new(paths, false);
            let repo = salvage::delete(&ctx, repo.as_deref(), &refname)?;
            println!("deleted {refname} in {repo}");
        }
        Command::Salvaged { json, .. } => {
            let salvaged = salvage::salvaged(&paths)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&salvaged)?);
            } else {
                print!("{}", salvage::render(&salvaged));
            }
        }
        Command::Ps { json } => ps_command(&paths, json)?,
        Command::Stop {
            pid,
            started_at,
            dry_run,
            json,
        } => stop_command(&paths, pid, started_at.as_deref(), dry_run, json)?,
        Command::Add { path, remote, base } => {
            let listed = add::add(&paths, &path, remote, base)?;
            println!("listed {listed}");
        }
        Command::Remove { repo } => match add::remove(&paths, &repo)? {
            Some(path) => println!("unlisted {path}; its clone and worktrees are untouched"),
            None => return Err(UsageError(format!("{repo} is not listed in repos.toml")).into()),
        },
        Command::Discover { json } => {
            discovery::scan(&paths, &discovery::Bounds::default(), &mut |_| {})?;
            let view = discovery::view(&paths, None)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&view)?);
            } else {
                print!("{}", discovery::render(&view));
            }
        }
        Command::Skip { path } => {
            let path = absolute_path(&paths, &path)?;
            discovery::skip(&paths, &path)?;
            println!("skipping {}", paths.tilde(&paths.expand(&path)));
        }
        Command::Unskip { path } => {
            let path = absolute_path(&paths, &path)?;
            let config = Config::load(&paths)?;
            let target = absolute(&paths.expand(&path));
            if !config
                .skipped(&paths)?
                .iter()
                .any(|skipped| absolute(skipped) == target)
            {
                return Err(UsageError(format!("{path} is not skipped")).into());
            }
            discovery::unskip(&paths, &path)?;
            println!("{} is suggested again", paths.tilde(&paths.expand(&path)));
        }
        Command::Keep { worktree } => {
            let kept = sweep::keep(&paths, &worktree)?;
            println!("kept {kept} until it gets new commits");
        }
        Command::Unkeep { worktree } => match sweep::unkeep(&paths, &worktree)? {
            Some(path) => println!("{path} is no longer kept"),
            None => println!("{worktree} was not kept"),
        },
        Command::Pause => {
            paths.set_paused(true)?;
            println!("background sweeping paused");
        }
        Command::Resume => {
            paths.set_paused(false)?;
            println!("background sweeping resumed");
        }
    }
    Ok(())
}

/// `path` if it is absolute or starts with `~/`; anything else is a usage error.
fn absolute_path(paths: &Paths, path: &str) -> Result<String> {
    if paths.expand(path).is_absolute() {
        Ok(path.to_owned())
    } else {
        Err(UsageError(format!("{path} must be an absolute path or start with ~/")).into())
    }
}

fn sweep_command(
    paths: Paths,
    selector: Option<&str>,
    dry_run: bool,
    due: bool,
    json: bool,
) -> Result<()> {
    let ctx = Ctx::new(paths, dry_run);
    let now = Utc::now();
    // With a repo named, only that repo is swept; otherwise the volumes get their pressure pass.
    let swept = match (due, selector) {
        (true, _) => sweep::sweep_due(&ctx, now)?,
        (false, None) => sweep::sweep_all(&ctx, EXPLICIT_WAIT, now)?,
        (false, Some(_)) => Swept {
            repos: sweep::sweep(&ctx, selector, EXPLICIT_WAIT, now)?,
            volumes: Vec::new(),
        },
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&swept)?);
        return Ok(());
    }
    if dry_run {
        println!("dry run: no command that changes a repo was run");
    }
    if due && ctx.paths.is_paused() {
        println!("paused: nothing swept");
    }
    for outcome in &swept.repos {
        print!("{}", describe(outcome, dry_run));
    }
    for pressure in &swept.volumes {
        print!("{}", describe_pressure(pressure, dry_run));
    }
    Ok(())
}

/// `git reap salvage <worktree>`: the commands it runs (or would), then where the work went.
fn salvage_command(paths: Paths, worktree: &str, dry_run: bool) -> Result<()> {
    let ctx = Ctx::new(paths, dry_run);
    let result = sweep::salvage_now(&ctx, worktree, Utc::now());
    if dry_run {
        println!("dry run: no command that changes a repo was run");
    }
    for line in ctx.take_log() {
        println!("  {line}");
    }
    let removed = result?;
    let verb = if dry_run { "would save" } else { "saved" };
    let refname = removed.salvaged_to.as_deref().unwrap_or("a salvage ref");
    println!("{verb} {} to {refname}", removed.path);
    if let Some(branch) = &removed.branch {
        println!("  its branch {branch} is kept");
    }
    if !dry_run {
        println!("  `git reap salvaged` says how to restore it, or delete the save");
    }
    Ok(())
}

/// `git reap ps`: one line per server and its command line, or the servers as JSON.
fn ps_command(paths: &Paths, json: bool) -> Result<()> {
    let servers = servers::list(paths)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&servers)?);
        return Ok(());
    }
    if servers.is_empty() {
        println!("no servers running in listed worktrees");
    }
    let now = Utc::now();
    for server in &servers {
        println!("{}", servers::describe(server, now));
        println!("  {}", server.command);
    }
    Ok(())
}

/// `git reap stop <pid>`: exits 1 when the server is still running after the wait.
fn stop_command(
    paths: &Paths,
    pid: u32,
    started_at: Option<&str>,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let stopped = servers::stop(paths, pid, started_at, dry_run, servers::STOP_WAIT)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&stopped)?);
    } else {
        print!("{}", describe_stop(&stopped));
    }
    if stopped.state == StopState::StillRunning {
        anyhow::bail!(
            "pid {} is still running; reap does not send SIGKILL",
            stopped.pid
        );
    }
    Ok(())
}

/// What `git reap stop` did, for the terminal.
fn describe_stop(stopped: &servers::Stopped) -> String {
    let mut lines: Vec<String> = stopped.log.iter().map(|line| format!("  {line}")).collect();
    let head = match stopped.state {
        StopState::WouldStop => format!(
            "dry run: would stop process group {} in {}",
            stopped.pgid, stopped.worktree
        ),
        StopState::Stopped => format!(
            "stopped process group {} in {}",
            stopped.pgid, stopped.worktree
        ),
        StopState::StillRunning => {
            let pids: Vec<String> = stopped.still_running.iter().map(u32::to_string).collect();
            format!(
                "process group {} in {} is still running: pid {}",
                stopped.pgid,
                stopped.worktree,
                pids.join(", ")
            )
        }
    };
    lines.insert(0, head);
    if let Some(shell) = stopped.hung_up {
        lines.push(format!("  hung up its idle launching shell, pid {shell}"));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// What the pressure pass did about one volume, for the terminal; nothing for a healthy one. A dry
/// run's `removed` holds what the pass would have removed, so its lines say `would`.
fn describe_pressure(pressure: &Pressure, dry_run: bool) -> String {
    let volume = &pressure.volume;
    if !volume.is_low && pressure.removed.is_empty() && pressure.log.is_empty() {
        return String::new();
    }
    let mut lines = report::describe_volume(volume);
    lines.extend(pressure.log.iter().map(|line| format!("  {line}")));
    for reclaimed in &pressure.removed {
        lines.push(format!(
            "  low disk: {} {} ({})",
            if dry_run { "would remove" } else { "removed" },
            reclaimed.removed.path,
            report::shown_size(reclaimed.bytes)
        ));
    }
    if !pressure.removed.is_empty() {
        lines.push(format!(
            "  {} {} on {}",
            if dry_run { "would free" } else { "freed" },
            report::shown_size(pressure.freed_bytes),
            volume.name
        ));
    }
    for refused in &pressure.refused {
        lines.push(format!(
            "  git refused to remove {}: {}",
            refused.path, refused.message
        ));
    }
    for skipped in &pressure.skipped {
        lines.push(format!("  skipped {}: {}", skipped.path, skipped.message));
    }
    for asked in &pressure.asked {
        lines.push(format!(
            "  low disk: {} is removable; confirm with Remove now",
            asked.path
        ));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// What a pass did to one repo, for the terminal. A dry run's outcome holds what the pass would
/// have done, so its lines say `would`: they must never read as a removal that happened.
fn describe(outcome: &Outcome, dry_run: bool) -> String {
    let mut lines = vec![outcome.repo.clone()];
    if let Some(why) = &outcome.skipped {
        lines.push(format!("  skipped: {why}"));
    }
    if let Some(error) = &outcome.report.error {
        lines.push(format!("  cannot read: {error}"));
    }
    if let Some(note) = &outcome.fetch_note {
        lines.push(format!("  {note}"));
    }
    lines.extend(outcome.log.iter().map(|line| format!("  {line}")));
    for started in &outcome.started {
        let when = started.removes_at.map_or_else(
            || "on confirm".to_string(),
            |time| format!("at {}", local_time(time)),
        );
        let removes = if dry_run { "would remove" } else { "removes" };
        lines.push(format!(
            "  removable {}: {}; {removes} {when}",
            started.path, started.reason
        ));
    }
    for removed in &outcome.removed {
        if dry_run {
            // A dry run never asks git about the branch; it knows only that an unmerged one is kept.
            let branch = match (&removed.branch, &removed.branch_kept) {
                (Some(branch), Some(why)) => format!("; branch {branch} would be kept: {why}"),
                (Some(branch), None) => format!(", then its branch {branch}"),
                (None, _) => String::new(),
            };
            lines.push(format!("  would remove {}{branch}", removed.path));
            if let Some(refname) = &removed.salvaged_to {
                lines.push(format!(
                    "    its uncommitted changes would be saved to {refname} first"
                ));
            }
            continue;
        }
        let branch = match (&removed.branch, &removed.branch_kept) {
            (Some(branch), Some(why)) => format!("; branch {branch} kept: {why}"),
            (Some(branch), None) => format!(" and its branch {branch}"),
            (None, _) => String::new(),
        };
        lines.push(format!("  removed {}{branch}", removed.path));
        if let Some(refname) = &removed.salvaged_to {
            lines.push(format!(
                "    its uncommitted changes are in {refname}; `git reap salvaged` says how to restore them"
            ));
        }
    }
    for refused in &outcome.refused {
        lines.push(format!(
            "  git refused to remove {}: {}",
            refused.path, refused.message
        ));
    }
    if outcome.pruned {
        lines.push("  pruned the entries of missing worktrees".into());
    }
    let deleted = if dry_run { "would delete" } else { "deleted" };
    for refname in &outcome.pruned_salvage {
        lines.push(format!("  {deleted} {refname}, past salvage_max_age"));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// A moment as the owner reads it: local time, to the minute.
fn local_time(time: chrono::DateTime<Utc>) -> String {
    time.with_timezone(&chrono::Local)
        .format("%a %-d %b %H:%M")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("git-reap").chain(args.iter().copied()))
    }

    #[test]
    fn the_documented_forms_parse() {
        for args in [
            &["status"][..],
            &["status", "--json"],
            &["add", "~/Development/app"],
            &["add", "/a", "--remote", "up", "--base", "develop"],
            &["remove", "app"],
            &["discover"],
            &["discover", "--json"],
            &["skip", "~/Development/scratch"],
            &["unskip", "~/Development/scratch"],
            &["sweep"],
            &["sweep", "app", "--dry-run", "--json"],
            &["sweep", "--due"],
            &["salvaged"],
            &["salvaged", "--json"],
            &[
                "salvaged",
                "--delete",
                "refs/reap/salvage/feat/20261105-120000",
            ],
            &[
                "salvaged",
                "--delete",
                "refs/reap/salvage/x/1",
                "--repo",
                "app",
            ],
            &["salvage", "~/Development/app-feat"],
            &["salvage", "~/Development/app-feat", "--dry-run"],
            &["keep", "~/Development/app-feat"],
            &["unkeep", "~/Development/app-feat"],
            &["pause"],
            &["resume"],
            &["ps"],
            &["ps", "--json"],
            &["stop", "4121"],
            &[
                "stop",
                "4121",
                "--started-at",
                "2026-11-05T09:01:02Z",
                "--dry-run",
            ],
            &["stop", "4121", "--json"],
        ] {
            assert!(parse(args).is_ok(), "{args:?}");
        }
    }

    /// A pass that removed a salvaged worktree, started a clock and deleted an old save.
    fn acted() -> Outcome {
        Outcome {
            repo: "~/Development/app".into(),
            skipped: None,
            fetch_note: None,
            started: vec![sweep::Started {
                path: "~/Development/app-next".into(),
                reason: "merged into origin/main".into(),
                removes_at: None,
            }],
            removed: vec![sweep::Removed {
                path: "~/Development/app-feat".into(),
                branch: Some("feat".into()),
                branch_kept: None,
                salvaged_to: Some("refs/reap/salvage/feat".into()),
            }],
            refused: Vec::new(),
            pruned: false,
            pruned_salvage: vec!["refs/reap/salvage/old".into()],
            log: Vec::new(),
            report: report::RepoReport {
                name: "app".into(),
                path: "~/Development/app".into(),
                base: "origin/main".into(),
                policy: crate::config::Policy::Auto,
                gh_note: None,
                error: None,
                worktrees: Vec::new(),
                salvaged: Vec::new(),
                servers: Vec::new(),
            },
        }
    }

    #[test]
    fn a_dry_run_says_what_it_would_do_never_what_it_did() {
        let text = describe(&acted(), true);

        for done in ["removed", "removes", "deleted", " are in "] {
            assert!(!text.contains(done), "{done:?} in a dry run:\n{text}");
        }
        assert!(text.contains("would remove ~/Development/app-feat, then its branch feat"));
        assert!(text.contains("would be saved to refs/reap/salvage/feat"));
        assert!(text.contains("would remove on confirm"));
        assert!(text.contains("would delete refs/reap/salvage/old"));
    }

    #[test]
    fn a_dry_run_forecasts_an_unmerged_branch_as_kept() {
        let mut outcome = acted();
        outcome.removed[0].branch_kept = Some("not merged".into());
        let text = describe(&outcome, true);

        assert!(text.contains(
            "would remove ~/Development/app-feat; branch feat would be kept: not merged"
        ));
        assert!(!text.contains("then its branch"));
    }

    #[test]
    fn a_real_pass_says_what_it_did() {
        let text = describe(&acted(), false);

        assert!(text.contains("removed ~/Development/app-feat and its branch feat"));
        assert!(text.contains("uncommitted changes are in refs/reap/salvage/feat"));
        assert!(text.contains("removes on confirm"));
        assert!(text.contains("deleted refs/reap/salvage/old"));
        assert!(!text.contains("would"));
    }

    #[test]
    fn a_dry_run_pressure_pass_says_would() {
        let pressure = Pressure {
            volume: crate::pressure::VolumeStatus {
                name: "Backup".into(),
                mount: "/Volumes/Backup".into(),
                total_bytes: 1000,
                available_bytes: 10,
                threshold_bytes: Some(100),
                is_low: true,
                removable: 1,
                holders: Vec::new(),
            },
            removed: vec![crate::pressure::Reclaimed {
                repo: "~/Development/app".into(),
                removed: sweep::Removed {
                    path: "~/Development/app-feat".into(),
                    branch: None,
                    branch_kept: None,
                    salvaged_to: None,
                },
                bytes: 500,
            }],
            freed_bytes: 400,
            refused: Vec::new(),
            skipped: Vec::new(),
            asked: Vec::new(),
            announce_asked: false,
            announce_stuck: false,
            log: Vec::new(),
        };

        let dry = describe_pressure(&pressure, true);
        assert!(dry.contains("would remove ~/Development/app-feat"));
        assert!(dry.contains("would free"));
        assert!(!dry.contains("removed"));
        assert!(!dry.contains("freed"));

        let real = describe_pressure(&pressure, false);
        assert!(real.contains("low disk: removed ~/Development/app-feat"));
        assert!(real.contains("freed"));
    }

    #[test]
    fn usage_errors_exit_2() {
        assert_eq!(run(["git-reap", "add"]), 2);
        assert_eq!(run(["git-reap", "frobnicate"]), 2);
        assert_eq!(run(["git-reap", "status", "--yaml"]), 2);
        assert_eq!(run(["git-reap", "sweep", "app", "--due"]), 2);
        assert_eq!(run(["git-reap", "keep"]), 2);
        assert_eq!(run(["git-reap", "salvage"]), 2);
        assert_eq!(run(["git-reap", "salvaged", "--repo", "app"]), 2);
        assert_eq!(run(["git-reap", "skip"]), 2);
        assert_eq!(run(["git-reap", "unskip"]), 2);
        assert_eq!(run(["git-reap", "discover", "--yaml"]), 2);
        assert_eq!(run(["git-reap", "stop"]), 2);
        assert_eq!(run(["git-reap", "stop", "node"]), 2);
        assert_eq!(run(["git-reap", "ps", "--yaml"]), 2);
    }
}
