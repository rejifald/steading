//! The CLI (rule 10): `git tether status|sync|add|remove|pause|resume`. It reads
//! git and the config directly; there is no IPC with the tray app.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use crate::add::{self, AddOpts};
use crate::config::{self, Mode};
use crate::ctx::Ctx;
use crate::paths::Paths;
use crate::report;
use crate::sync::{self, Outcome, Trigger};

/// How long an explicit `sync` waits for a background run to finish.
const EXPLICIT_WAIT: Duration = Duration::from_secs(30);

#[derive(Parser)]
#[command(
    name = "git-tether",
    bin_name = "git tether",
    version,
    about = "Keeps the repos you list on their branch tip (ADR 0001)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    Follow,
    Mirror,
}

impl From<ModeArg> for Mode {
    fn from(m: ModeArg) -> Self {
        match m {
            ModeArg::Follow => Mode::Follow,
            ModeArg::Mirror => Mode::Mirror,
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// Each listed repo with its branch, mode, last sync and skip reason.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Sync every listed repo now (Pause does not apply), or one by name or path.
    Sync {
        repo: Option<String>,
        /// Print the git commands a sync would run; run only read-only ones.
        #[arg(long)]
        dry_run: bool,
        /// Background mode: only repos whose interval has passed, skipping
        /// any another run holds; does nothing while paused.
        #[arg(long, conflicts_with = "repo")]
        due: bool,
        #[arg(long)]
        json: bool,
    },
    /// List a clone in repos.toml (rule 1).
    Add {
        path: PathBuf,
        #[arg(long)]
        branch: String,
        #[arg(long, value_enum, default_value = "follow")]
        mode: ModeArg,
        #[arg(long)]
        remote: Option<String>,
        /// <n>s, <n>m or <n>h (default 5m).
        #[arg(long, value_parser = parse_interval_arg)]
        interval: Option<String>,
        /// Clone URL for a mirror at a new or empty path.
        #[arg(long)]
        url: Option<String>,
        /// Shell command a background sync asks first; exit 0 means go (ADR 0004).
        #[arg(long)]
        gate: Option<String>,
        /// Shell command run after a sync moved the tip (ADR 0004).
        #[arg(long)]
        after: Option<String>,
    },
    /// Change a listed repo's settings.
    Set {
        repo: String,
        /// <n>s, <n>m or <n>h.
        #[arg(long, value_parser = parse_interval_arg)]
        interval: String,
    },
    /// Unlist a repo; its clone is left as it is.
    Remove { path: String },
    /// Pause background sync (the tray's timers and `sync --due`).
    Pause,
    /// Resume background sync.
    Resume,
}

fn parse_interval_arg(s: &str) -> Result<String, String> {
    config::parse_interval(s)
        .map(|_| s.to_string())
        .map_err(|e| format!("{e:#}"))
}

/// A mistake in the command line that clap cannot see (an unknown repo name):
/// exits 2 like any other usage error.
#[derive(Debug)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
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
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() { 2 } else { 0 };
        }
    };
    match dispatch(cli.cmd) {
        Ok(code) => code,
        Err(e) if e.downcast_ref::<UsageError>().is_some() => {
            eprintln!("git tether: {e}");
            2
        }
        Err(e) => {
            eprintln!("git tether: {e:#}");
            1
        }
    }
}

fn dispatch(cmd: Cmd) -> Result<i32> {
    let paths = Paths::from_env()?;
    match cmd {
        Cmd::Status { json } => {
            let r = report::report(&Ctx::new(paths, true))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                print!("{}", report::render(&r));
            }
            Ok(0)
        }
        Cmd::Sync {
            repo,
            dry_run,
            due,
            json,
        } => sync_cmd(paths, repo, dry_run, due, json),
        Cmd::Add {
            path,
            branch,
            mode,
            remote,
            interval,
            url,
            gate,
            after,
        } => {
            let ctx = Ctx::new(paths, false);
            let opts = AddOpts {
                path,
                branch,
                mode: mode.into(),
                remote,
                interval,
                url,
                gate,
                after,
            };
            let res = add::add(&ctx, &opts);
            print_log(&ctx.take_log());
            res.map(|_| 0)
        }
        Cmd::Set { repo, interval } => {
            match add::set_interval(&Ctx::new(paths, false), &repo, &interval)? {
                Some(p) => {
                    println!("{p} now syncs every {interval}");
                    Ok(0)
                }
                None => Err(UsageError(format!("{repo} is not listed in repos.toml")).into()),
            }
        }
        Cmd::Remove { path } => match add::remove(&Ctx::new(paths, false), &path)? {
            Some(p) => {
                println!("unlisted {p}; the clone is untouched");
                Ok(0)
            }
            None => Err(UsageError(format!("{path} is not listed in repos.toml")).into()),
        },
        Cmd::Pause => {
            paths.set_paused(true)?;
            println!("background sync paused; `git tether sync` still runs on request");
            Ok(0)
        }
        Cmd::Resume => {
            paths.set_paused(false)?;
            println!("background sync resumed");
            Ok(0)
        }
    }
}

fn sync_cmd(
    paths: Paths,
    selector: Option<String>,
    dry_run: bool,
    due: bool,
    json: bool,
) -> Result<i32> {
    let ctx = Ctx::new(paths, dry_run);
    let outs: Vec<Outcome> = if due {
        sync::tick(&ctx)?
    } else {
        let repos = ctx.config()?.repos(&ctx.paths)?;
        let chosen: Vec<_> = match &selector {
            Some(s) => repos
                .into_iter()
                .filter(|r| r.matches(s, &ctx.paths))
                .collect(),
            None => repos,
        };
        if let (Some(s), true) = (&selector, chosen.is_empty()) {
            return Err(UsageError(format!("{s} is not listed in repos.toml")).into());
        }
        chosen
            .iter()
            .map(|r| {
                sync::sync_repo(
                    &ctx,
                    r,
                    Trigger::Explicit {
                        wait: EXPLICIT_WAIT,
                    },
                )
            })
            .collect::<Result<_>>()?
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&outs)?);
        return Ok(0);
    }
    if dry_run {
        println!("dry run: no mutating git command was run");
    }
    if due && ctx.paths.is_paused() {
        println!("paused: nothing synced");
    }
    for o in &outs {
        println!("{}: {}", o.path, o.describe());
        print_log(&o.log);
        if let Some(b) = &o.backup {
            println!("  tracked changes saved to {}", ctx.paths.tilde(b));
        }
    }
    Ok(0)
}

fn print_log(log: &[String]) {
    for l in log {
        println!("  {l}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("tether").chain(args.iter().copied()))
    }

    #[test]
    fn usage_errors_exit_2() {
        assert_eq!(run(["tether", "add", "/x"]), 2, "add needs --branch");
        assert_eq!(
            run(["tether", "add", "/x", "--branch", "main", "--mode", "rsync"]),
            2
        );
        assert_eq!(
            run([
                "tether",
                "add",
                "/x",
                "--branch",
                "main",
                "--interval",
                "1s"
            ]),
            2
        );
        assert_eq!(run(["tether", "set", "notes"]), 2, "set needs --interval");
        assert_eq!(run(["tether", "set", "notes", "--interval", "10s"]), 2);
        assert_eq!(run(["tether", "frobnicate"]), 2);
        assert_eq!(run(["tether", "sync", "notes", "--due"]), 2);
    }

    #[test]
    fn the_documented_forms_parse() {
        for ok in [
            &["status", "--json"][..],
            &["sync"],
            &["sync", "notes", "--dry-run"],
            &["sync", "--due"],
            &["add", "~/Development/notes", "--branch", "main"],
            &[
                "add",
                "/m",
                "--branch",
                "main",
                "--mode",
                "mirror",
                "--remote",
                "up",
                "--interval",
                "10m",
                "--url",
                "https://x/y.git",
                "--gate",
                "tool idle",
                "--after",
                "tool reconcile",
            ],
            &["set", "notes", "--interval", "15m"],
            &["remove", "/m"],
            &["pause"],
            &["resume"],
        ] {
            assert!(parse(ok).is_ok(), "{ok:?}");
        }
    }
}
