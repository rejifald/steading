//! The CLI (rule 14): `git reap status|sweep|salvaged|add|remove|keep|unkeep|pause|resume`. It
//! reads git and the config directly; there is no IPC with the tray app.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};

use crate::add;
use crate::ctx::Ctx;
use crate::paths::Paths;
use crate::report;
use crate::salvage;
use crate::sweep::{self, Outcome};

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
    /// The uncommitted changes saved from removed worktrees, and how to restore each (ADR
    /// dirty-merged-worktrees-are-salvaged).
    Salvaged {
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
        Command::Salvaged { json } => {
            let salvaged = salvage::salvaged(&paths)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&salvaged)?);
            } else {
                print!("{}", salvage::render(&salvaged));
            }
        }
        Command::Add { path, remote, base } => {
            let listed = add::add(&paths, &path, remote, base)?;
            println!("listed {listed}");
        }
        Command::Remove { repo } => match add::remove(&paths, &repo)? {
            Some(path) => println!("unlisted {path}; its clone and worktrees are untouched"),
            None => return Err(UsageError(format!("{repo} is not listed in repos.toml")).into()),
        },
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

fn sweep_command(
    paths: Paths,
    selector: Option<&str>,
    dry_run: bool,
    due: bool,
    json: bool,
) -> Result<()> {
    let ctx = Ctx::new(paths, dry_run);
    let now = Utc::now();
    let outcomes = if due {
        sweep::sweep_due(&ctx, now)?
    } else {
        sweep::sweep(&ctx, selector, EXPLICIT_WAIT, now)?
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&outcomes)?);
        return Ok(());
    }
    if dry_run {
        println!("dry run: no command that changes a repo was run");
    }
    if due && ctx.paths.is_paused() {
        println!("paused: nothing swept");
    }
    for outcome in &outcomes {
        print!("{}", describe(outcome, dry_run));
    }
    Ok(())
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
            &["sweep"],
            &["sweep", "app", "--dry-run", "--json"],
            &["sweep", "--due"],
            &["salvaged"],
            &["salvaged", "--json"],
            &["keep", "~/Development/app-feat"],
            &["unkeep", "~/Development/app-feat"],
            &["pause"],
            &["resume"],
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
    fn usage_errors_exit_2() {
        assert_eq!(run(["git-reap", "add"]), 2);
        assert_eq!(run(["git-reap", "frobnicate"]), 2);
        assert_eq!(run(["git-reap", "status", "--yaml"]), 2);
        assert_eq!(run(["git-reap", "sweep", "app", "--due"]), 2);
        assert_eq!(run(["git-reap", "keep"]), 2);
    }
}
