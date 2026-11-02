//! An operator's own command from `repos.toml` (ADR 0004): a repo's `gate` or
//! `after`, run with `sh -c` in the clone, like a git hook. Its output goes to
//! an unlinked file rather than a pipe, so a background job it leaves behind
//! can never hold tether waiting.

use std::fs::File;
use std::io::{Read, Seek};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::git;

/// How often a command with a deadline is checked for having exited.
const POLL: Duration = Duration::from_millis(20);
/// Longer first lines are cut, so a reason stays one line in the popover.
const MAX_LINE: usize = 200;

/// How a command ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    Exited(ExitStatus),
    /// Still running at the deadline; it was killed with its process group.
    TimedOut(Duration),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub end: End,
    /// The first non-empty line of stdout and stderr together.
    pub first_line: Option<String>,
}

impl Ran {
    pub fn is_success(&self) -> bool {
        matches!(&self.end, End::Exited(s) if s.success())
    }

    /// The reason a failed run gives: its first line, else how it ended.
    pub fn summary(&self) -> String {
        if let Some(line) = &self.first_line {
            return line.clone();
        }
        match &self.end {
            End::TimedOut(after) => format!("no exit within {}s", after.as_secs()),
            End::Exited(s) => match (s.code(), s.signal()) {
                (Some(code), _) => format!("exited with status {code}"),
                (None, Some(signal)) => format!("killed by signal {signal}"),
                (None, None) => "exited abnormally".into(),
            },
        }
    }
}

/// Runs `script` with `sh -c` in `dir` with `env` added, and waits for it, at
/// most `timeout` when one is given. stdin is closed.
pub fn run(
    script: &str,
    dir: &Path,
    env: &[(&str, &str)],
    timeout: Option<Duration>,
) -> Result<Ran> {
    let output = tempfile::tempfile().context("creating a file for the command's output")?;
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(script)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(output.try_clone()?)
        // Its own process group, so a timeout kills what it started too.
        .process_group(0);
    for var in git::REPO_ENV_VARS {
        cmd.env_remove(var);
    }
    cmd.envs(env.iter().copied());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("starting sh -c {script:?} in {}", dir.display()))?;
    let end = match timeout {
        None => End::Exited(child.wait().context("waiting for the command")?),
        Some(limit) => {
            let deadline = Instant::now() + limit;
            loop {
                if let Some(status) = child.try_wait().context("waiting for the command")? {
                    break End::Exited(status);
                }
                if Instant::now() >= deadline {
                    kill_group(child.id());
                    child.wait().context("reaping the timed-out command")?;
                    break End::TimedOut(limit);
                }
                std::thread::sleep(POLL);
            }
        }
    };
    Ok(Ran {
        end,
        first_line: first_line(output)?,
    })
}

fn kill_group(pid: u32) {
    let Ok(pgid) = libc::pid_t::try_from(pid) else {
        return;
    };
    // SAFETY: killpg only sends a signal; the group is the child's own (it
    // was started with process_group(0)) and has not been reaped yet.
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}

fn first_line(mut output: File) -> Result<Option<String>> {
    output.rewind()?;
    let mut raw = Vec::new();
    output
        .read_to_end(&mut raw)
        .context("reading the command's output")?;
    // A command may print bytes that are not UTF-8; only the first line matters.
    Ok(String::from_utf8_lossy(&raw)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(MAX_LINE).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str, timeout: Option<Duration>) -> Ran {
        run(script, Path::new("/"), &[("TETHER_X", "x1")], timeout).unwrap()
    }

    #[test]
    fn success_and_the_first_line_of_either_stream() {
        let ran = sh("echo; echo \"  $TETHER_X said  \" >&2; echo later", None);
        assert!(ran.is_success());
        assert_eq!(ran.first_line.as_deref(), Some("x1 said"));
    }

    #[test]
    fn a_silent_failure_names_its_status() {
        let ran = sh("exit 3", None);
        assert!(!ran.is_success());
        assert_eq!(ran.summary(), "exited with status 3");
    }

    #[test]
    fn a_command_past_its_deadline_is_killed_with_its_children() {
        let started = Instant::now();
        let ran = sh("sleep 5; sleep 5", Some(Duration::from_millis(200)));
        assert_eq!(ran.end, End::TimedOut(Duration::from_millis(200)));
        assert!(started.elapsed() < Duration::from_secs(4), "it waited");
    }

    #[test]
    fn a_background_job_does_not_hold_the_caller() {
        let started = Instant::now();
        let ran = sh("sleep 5 & echo done", Some(Duration::from_secs(3)));
        assert!(ran.is_success());
        assert_eq!(ran.first_line.as_deref(), Some("done"));
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
