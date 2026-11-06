//! Dev servers inside listed clones' linked worktrees, and the operator's *Stop* (ADR 0005 rules
//! 3 to 6). A *server* is a process whose current directory is inside a linked worktree and whose
//! process group listens on a TCP port. Every pass lists them; reap stops one only when the
//! operator asks, with `git reap stop` or the popover's *Stop*, and never on its own.
//!
//! The current directories come from the pass's one `lsof -d cwd` ([`WorkingDirectories`]), the
//! process tree from one `ps`, and the ports from one `lsof -iTCP -sTCP:LISTEN`. Parsing is pure;
//! only the `read` functions and [`stop`] touch the system.

#![allow(
    clippy::similar_names,
    reason = "pid, ppid and pgid are the names ps and POSIX give them"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use serde::Serialize;

use crate::config::Config;
use crate::in_use::WorkingDirectories;
use crate::paths::Paths;
use crate::report;

/// How long *Stop* waits for a server's process group to exit after `SIGTERM` (rule 4).
pub const STOP_WAIT: Duration = Duration::from_secs(10);

/// How often *Stop* looks again while it waits.
const POLL: Duration = Duration::from_millis(100);

/// The programs that count as a shell when reap looks for a server's launching shell (rule 3).
const SHELLS: &[&str] = &["sh", "bash", "zsh", "fish", "dash", "ksh", "tcsh", "csh"];

/// How far up the process tree reap looks for a launching shell.
const MAX_ANCESTORS: usize = 64;

/// `ps`'s `lstart` once its padding is collapsed: `Wed Nov 4 10:26:37 2026`, in local time.
const LSTART: &str = "%a %b %d %H:%M:%S %Y";

/// One process as `ps` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub pid: u32,
    pub ppid: u32,
    pub pgid: u32,
    pub uid: u32,
    /// It has exited and waits for its parent to reap it; it holds nothing.
    pub is_zombie: bool,
    /// To the second, as `ps` gives it; with the pid, it tells a process from a later one that
    /// reuses the pid.
    pub started_at: DateTime<Utc>,
    /// The full command line.
    pub command: String,
}

impl Process {
    /// The program's name: the last part of the command line's first word, without the `-` a
    /// login shell carries.
    #[must_use]
    pub fn name(&self) -> &str {
        let first = self.command.split_whitespace().next().unwrap_or_default();
        let base = first.rsplit('/').next().unwrap_or(first);
        base.strip_prefix('-').unwrap_or(base)
    }

    fn is_shell(&self) -> bool {
        SHELLS.contains(&self.name())
    }
}

/// Every process on the Mac, by pid.
#[derive(Debug, Clone, Default)]
pub struct ProcessTable(BTreeMap<u32, Process>);

impl ProcessTable {
    /// Asks `ps` for every process.
    ///
    /// # Errors
    ///
    /// Fails when `ps` cannot run or exits non-zero.
    pub fn read() -> Result<Self> {
        let output = Command::new("ps")
            .args([
                "-A",
                "-ww",
                "-o",
                "pid=,ppid=,pgid=,uid=,stat=,lstart=,command=",
            ])
            // `lstart` is in the C locale's words only under it.
            .env("LC_ALL", "C")
            .output()
            .context("running ps")?;
        if !output.status.success() {
            bail!(
                "ps failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(Self::parse(
            &String::from_utf8_lossy(&output.stdout),
            &local_to_utc,
        ))
    }

    /// Parses `ps -o pid=,ppid=,pgid=,uid=,stat=,lstart=,command=` output; `to_utc` reads
    /// `lstart`'s local time. A line that does not parse is skipped.
    #[must_use]
    pub fn parse(output: &str, to_utc: &dyn Fn(NaiveDateTime) -> Option<DateTime<Utc>>) -> Self {
        Self(
            output
                .lines()
                .filter_map(|line| parse_process(line, to_utc))
                .map(|process| (process.pid, process))
                .collect(),
        )
    }

    /// The process `pid`, unless it has exited.
    #[must_use]
    pub fn live(&self, pid: u32) -> Option<&Process> {
        self.0.get(&pid).filter(|process| !process.is_zombie)
    }

    fn members(&self, pgid: u32) -> impl Iterator<Item = &Process> {
        self.0
            .values()
            .filter(move |process| process.pgid == pgid && !process.is_zombie)
    }

    fn children(&self, pid: u32) -> impl Iterator<Item = &Process> {
        self.0
            .values()
            .filter(move |process| process.ppid == pid && !process.is_zombie)
    }
}

fn parse_process(
    line: &str,
    to_utc: &dyn Fn(NaiveDateTime) -> Option<DateTime<Utc>>,
) -> Option<Process> {
    // pid, ppid, pgid, uid, stat, then lstart's five words, then the command line.
    let (words, command) = split_words(line, 10)?;
    let number = |index: usize| words[index].parse::<u32>().ok();
    let started = words[5..10].join(" ");
    let started_at = NaiveDateTime::parse_from_str(&started, LSTART)
        .ok()
        .and_then(to_utc)?;
    Some(Process {
        pid: number(0)?,
        ppid: number(1)?,
        pgid: number(2)?,
        uid: number(3)?,
        is_zombie: words[4].starts_with('Z'),
        started_at,
        command: command.to_string(),
    })
}

/// The first `count` whitespace-separated words of `line`, and the rest of it, trimmed.
fn split_words(line: &str, count: usize) -> Option<(Vec<&str>, &str)> {
    let mut words = Vec::with_capacity(count);
    let mut rest = line.trim_start();
    for _ in 0..count {
        if rest.is_empty() {
            return None;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        words.push(&rest[..end]);
        rest = rest[end..].trim_start();
    }
    Some((words, rest.trim_end()))
}

fn local_to_utc(time: NaiveDateTime) -> Option<DateTime<Utc>> {
    Local
        .from_local_datetime(&time)
        .earliest()
        .map(|time| time.with_timezone(&Utc))
}

/// The TCP ports each of this user's processes listens on.
#[derive(Debug, Clone, Default)]
pub struct Listeners(BTreeMap<u32, BTreeSet<u16>>);

impl Listeners {
    /// Asks `lsof` for the listening TCP sockets of `uid`'s processes.
    ///
    /// # Errors
    ///
    /// Fails when `lsof` cannot run, or exits non-zero with an error and no output.
    pub fn read(uid: u32) -> Result<Self> {
        let output = Command::new("lsof")
            .args(["-n", "-P", "-w", "-a", "-iTCP", "-sTCP:LISTEN", "-u"])
            .arg(uid.to_string())
            .args(["-F", "pn"])
            .output()
            .context("running lsof for listening ports")?;
        // lsof exits 1 with no output when nothing listens.
        if !output.status.success() && output.stdout.is_empty() && !output.stderr.is_empty() {
            bail!(
                "lsof failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(Self::parse(&String::from_utf8_lossy(&output.stdout)))
    }

    /// Parses `lsof -F pn` output for listening sockets: a `p<pid>` line, then `n<address>:<port>`
    /// lines such as `n*:5173`, `n127.0.0.1:3000` or `n[::1]:8080`.
    #[must_use]
    pub fn parse(output: &str) -> Self {
        let mut pid = None;
        let mut ports: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new();
        for line in output.lines() {
            if let Some(number) = line.strip_prefix('p') {
                pid = number.parse().ok();
            } else if let (Some(pid), Some(address)) = (pid, line.strip_prefix('n')) {
                let port = address
                    .rsplit_once(':')
                    .and_then(|(_, port)| port.parse().ok());
                if let Some(port) = port {
                    ports.entry(pid).or_default().insert(port);
                }
            }
        }
        Self(ports)
    }

    fn of(&self, pid: u32) -> Option<&BTreeSet<u16>> {
        self.0.get(&pid).filter(|ports| !ports.is_empty())
    }
}

/// What reap knows about the processes at one moment.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Each process's current directory, where lsof could read it.
    pub directories: BTreeMap<u32, PathBuf>,
    pub table: ProcessTable,
    pub listeners: Listeners,
    /// The user reap runs as; only their processes can be servers.
    pub uid: u32,
    /// reap's own pid; its process group is never a server.
    pub own_pid: u32,
}

impl Snapshot {
    /// The process table and listening ports, joined to the current directories of the pass.
    ///
    /// # Errors
    ///
    /// Fails when `ps` or `lsof` fails.
    pub fn read(directories: &WorkingDirectories) -> Result<Self> {
        let uid = current_uid();
        Ok(Self {
            directories: directories
                .entries()
                .filter(|(pid, _)| *pid != 0)
                .map(|(pid, path)| (pid, path.to_path_buf()))
                .collect(),
            table: ProcessTable::read()?,
            listeners: Listeners::read(uid)?,
            uid,
            own_pid: std::process::id(),
        })
    }

    /// The most specific of `worktrees` that holds `pid`'s current directory.
    fn worktree_of<'w>(&self, pid: u32, worktrees: &'w [PathBuf]) -> Option<&'w PathBuf> {
        let directory = self.directories.get(&pid)?;
        worktrees
            .iter()
            .filter(|worktree| directory.starts_with(worktree))
            .max_by_key(|worktree| worktree.components().count())
    }

    fn own_pgid(&self) -> Option<u32> {
        self.table.live(self.own_pid).map(|process| process.pgid)
    }

    /// Whether any process in group `pgid` listens on a TCP port.
    fn listens(&self, pgid: u32) -> bool {
        self.table
            .members(pgid)
            .any(|member| self.listeners.of(member.pid).is_some())
    }

    /// Whether `process` can be a server at all: this user's, and not reap's own group.
    fn may_serve(&self, process: &Process) -> bool {
        process.uid == self.uid
            && process.pid != self.own_pid
            && Some(process.pgid) != self.own_pgid()
    }

    /// What runs inside each of `worktrees` that has anything running (rule 3).
    #[must_use]
    pub fn inside(&self, worktrees: &[PathBuf]) -> BTreeMap<PathBuf, Inside> {
        let mut pids: BTreeMap<&PathBuf, Vec<u32>> = BTreeMap::new();
        for pid in self.directories.keys() {
            if let Some(worktree) = self.worktree_of(*pid, worktrees) {
                pids.entry(worktree).or_default().push(*pid);
            }
        }
        pids.into_iter()
            .map(|(worktree, pids)| {
                (
                    worktree.clone(),
                    self.inside_one(worktree, worktrees, &pids),
                )
            })
            .collect()
    }

    fn inside_one(&self, worktree: &Path, worktrees: &[PathBuf], pids: &[u32]) -> Inside {
        let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for pid in pids {
            if let Some(process) = self.table.live(*pid)
                && self.may_serve(process)
                && self.listens(process.pgid)
            {
                groups.entry(process.pgid).or_default().push(*pid);
            }
        }
        let servers: Vec<Found> = groups
            .into_iter()
            .map(|(pgid, inside)| self.found(worktree, worktrees, pgid, &inside))
            .collect();
        let others = pids
            .iter()
            .copied()
            .filter(|pid| {
                let is_server = self.table.live(*pid).is_some_and(|process| {
                    servers.iter().any(|server| server.pgid == process.pgid)
                });
                let is_shell = servers.iter().any(|server| server.shell == Some(*pid));
                !is_server && !is_shell && self.table.live(*pid).is_some()
            })
            .collect();
        Inside { servers, others }
    }

    fn found(&self, worktree: &Path, worktrees: &[PathBuf], pgid: u32, inside: &[u32]) -> Found {
        let members: Vec<u32> = self.table.members(pgid).map(|member| member.pid).collect();
        // The group's leader when it is still there, else its eldest member inside the worktree.
        let pid = self.table.live(pgid).map_or_else(
            || {
                inside
                    .iter()
                    .copied()
                    .min_by_key(|pid| self.table.live(*pid).map(|process| process.started_at))
                    .unwrap_or(pgid)
            },
            |leader| leader.pid,
        );
        let ports: BTreeSet<u16> = members
            .iter()
            .filter_map(|member| self.listeners.of(*member))
            .flatten()
            .copied()
            .collect();
        let listener = members
            .iter()
            .copied()
            .find(|member| self.listeners.of(*member).is_some())
            .unwrap_or(pid);
        let shell = self.launching_shell(pid, pgid, worktree, worktrees);
        Found {
            worktree: worktree.to_path_buf(),
            pid,
            pgid,
            listener,
            members,
            ports: ports.into_iter().collect(),
            shell,
        }
    }

    /// Rule 3: the nearest ancestor of `pid` that is a shell, when the server group is all it is
    /// waiting for and it sits in the same worktree, so stopping the server leaves it idle.
    fn launching_shell(
        &self,
        pid: u32,
        pgid: u32,
        worktree: &Path,
        worktrees: &[PathBuf],
    ) -> Option<u32> {
        let mut parent = self.table.live(pid)?.ppid;
        for _ in 0..MAX_ANCESTORS {
            let process = self.table.live(parent).filter(|_| parent > 1)?;
            if process.is_shell() {
                let is_idle = self.table.children(parent).all(|child| child.pgid == pgid);
                let is_here = self
                    .worktree_of(parent, worktrees)
                    .is_some_and(|holder| holder == worktree);
                return (process.pgid != pgid && is_idle && is_here && self.may_serve(process))
                    .then_some(parent);
            }
            parent = process.ppid;
        }
        None
    }

    /// Every check *Stop* makes before it signals `pid`'s group (rules 4 and 5): it runs, it is
    /// this user's, it is the process that was listed, it is inside a listed clone's worktree,
    /// its group is a server, and every process in that group is this user's and inside that
    /// worktree, so nothing reap cannot tie to the worktree gets the signal.
    ///
    /// # Errors
    ///
    /// The first check that fails, as a [`Refusal`].
    pub fn check(
        &self,
        pid: u32,
        listed_start: Option<DateTime<Utc>>,
        worktrees: &[PathBuf],
    ) -> Result<Found, Refusal> {
        let process = self.table.live(pid).ok_or(Refusal::NotRunning { pid })?;
        if process.uid != self.uid {
            return Err(Refusal::OtherUser { pid });
        }
        if let Some(listed) = listed_start
            && listed != process.started_at
        {
            return Err(Refusal::Restarted {
                pid,
                listed,
                running: process.started_at,
            });
        }
        if !self.may_serve(process) {
            return Err(Refusal::Own { pid });
        }
        let worktree = self
            .worktree_of(pid, worktrees)
            .ok_or(Refusal::Outside { pid })?;
        let found = self
            .inside(worktrees)
            .remove(worktree)
            .and_then(|inside| {
                inside
                    .servers
                    .into_iter()
                    .find(|server| server.pgid == process.pgid)
            })
            .ok_or(Refusal::NotServer { pid })?;
        for member in &found.members {
            let is_tied = self
                .table
                .live(*member)
                .is_some_and(|process| self.may_serve(process))
                && self.worktree_of(*member, worktrees) == Some(worktree);
            if !is_tied {
                return Err(Refusal::GroupOutside {
                    pid,
                    pgid: found.pgid,
                    member: *member,
                });
            }
        }
        Ok(found)
    }
}

/// What runs inside one linked worktree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inside {
    pub servers: Vec<Found>,
    /// Every other live process inside it: shells, editors, agent sessions. reap never stops
    /// these (rule 3).
    pub others: Vec<u32>,
}

/// One server: a process group that listens, with at least one process inside the worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub worktree: PathBuf,
    /// The group's leader, or its eldest member inside the worktree.
    pub pid: u32,
    pub pgid: u32,
    /// The first process in the group that listens.
    pub listener: u32,
    /// Every live process in the group, wherever its directory is.
    pub members: Vec<u32>,
    pub ports: Vec<u16>,
    /// Its idle launching shell (rule 3).
    pub shell: Option<u32>,
}

/// A server as the report shows it (ADR 0005 rule 6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Server {
    /// The process *Stop* is asked about: the group's leader, or its eldest member inside the
    /// worktree.
    pub pid: u32,
    /// Its process group; *Stop* signals the whole group.
    pub pgid: u32,
    /// The name of the process that listens, such as `node`.
    pub name: String,
    /// `pid`'s full command line, such as `npm run dev`.
    pub command: String,
    /// The TCP ports the group listens on, ascending.
    pub ports: Vec<u16>,
    /// When `pid` started, RFC 3339 to the second; *Stop* takes it back to tell the process from
    /// a later one that reuses its pid.
    pub started_at: String,
    /// The linked worktree it runs in, with `~`: the `path` of a row in the same repo.
    pub worktree: String,
    /// Its idle launching shell, which *Stop* hangs up once the server has exited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<Shell>,
}

/// A server's idle launching shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Shell {
    pub pid: u32,
    /// Such as `zsh`.
    pub name: String,
}

impl Snapshot {
    /// `found` as the report shows it.
    #[must_use]
    pub fn server(&self, paths: &Paths, found: &Found) -> Server {
        let process = self.table.live(found.pid);
        let name = self
            .table
            .live(found.listener)
            .or(process)
            .map(|process| process.name().to_string())
            .unwrap_or_default();
        Server {
            pid: found.pid,
            pgid: found.pgid,
            name,
            command: process
                .map(|process| process.command.clone())
                .unwrap_or_default(),
            ports: found.ports.clone(),
            started_at: process
                .map(|process| rfc3339(process.started_at))
                .unwrap_or_default(),
            worktree: paths.tilde(&found.worktree),
            shell: found.shell.and_then(|pid| {
                Some(Shell {
                    pid,
                    name: self.table.live(pid)?.name().to_string(),
                })
            }),
        }
    }

    /// The names of `pids`, for a row's other processes.
    #[must_use]
    pub fn names(&self, pids: &[u32]) -> Vec<String> {
        pids.iter()
            .filter_map(|pid| self.table.live(*pid))
            .map(|process| process.name().to_string())
            .collect()
    }
}

/// Why *Stop* refused to signal a process. Nothing was signalled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    NotRunning {
        pid: u32,
    },
    OtherUser {
        pid: u32,
    },
    /// The pid now belongs to a process that started at another time than the one listed.
    Restarted {
        pid: u32,
        listed: DateTime<Utc>,
        running: DateTime<Utc>,
    },
    /// reap itself, or a process in reap's own group.
    Own {
        pid: u32,
    },
    Outside {
        pid: u32,
    },
    NotServer {
        pid: u32,
    },
    /// A process in the group is not this user's or not inside the worktree.
    GroupOutside {
        pid: u32,
        pgid: u32,
        member: u32,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRunning { pid } => write!(formatter, "pid {pid} is not running"),
            Self::OtherUser { pid } => write!(
                formatter,
                "pid {pid} belongs to another user; reap stops only your own processes"
            ),
            Self::Restarted {
                pid,
                listed,
                running,
            } => write!(
                formatter,
                "pid {pid} started at {}, not at {} as listed: it is another process now",
                rfc3339(*running),
                rfc3339(*listed)
            ),
            Self::Own { pid } => write!(
                formatter,
                "pid {pid} is reap itself or in reap's own process group"
            ),
            Self::Outside { pid } => write!(
                formatter,
                "pid {pid} is not inside a linked worktree of a listed clone"
            ),
            Self::NotServer { pid } => write!(
                formatter,
                "pid {pid} is not a server: nothing in its process group listens on a TCP port; reap stops only servers"
            ),
            Self::GroupOutside { pid, pgid, member } => write!(
                formatter,
                "pid {pid}'s process group {pgid} also holds pid {member}, which is not yours or not inside the same worktree"
            ),
        }
    }
}

impl std::error::Error for Refusal {}

/// What a *Stop* did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StopState {
    /// Every process in the group has exited.
    Stopped,
    /// Something in the group was still running when the wait ended; reap does not escalate.
    StillRunning,
    /// A dry run: nothing was signalled.
    WouldStop,
}

/// The result of `git reap stop` and the app's `stop_process`.
#[derive(Debug, Clone, Serialize)]
pub struct Stopped {
    pub pid: u32,
    pub pgid: u32,
    /// With `~`.
    pub worktree: String,
    pub state: StopState,
    /// The group's pids still running after the wait.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub still_running: Vec<u32>,
    /// The idle launching shell that got `SIGHUP`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hung_up: Option<u32>,
    /// The signals sent (`ran:`) or, in a dry run, that would be (`would run:`).
    pub log: Vec<String>,
}

/// The linked worktrees of every listed clone (rule 1); a clone that cannot be read adds none.
///
/// # Errors
///
/// Fails when `repos.toml` cannot be read.
pub fn watched(paths: &Paths) -> Result<Vec<PathBuf>> {
    let repos = Config::load(paths)?.repos(paths)?;
    Ok(repos
        .iter()
        .filter_map(|repo| report::list_worktrees(paths, &repo.path).ok())
        .flat_map(|worktrees| {
            worktrees
                .into_iter()
                .skip(1)
                .filter(|worktree| !worktree.is_bare)
                .map(|worktree| worktree.path)
        })
        .collect())
}

/// Every server in a listed clone's linked worktrees, for `git reap ps`: the report's `servers`,
/// without the rest of a status pass.
///
/// # Errors
///
/// Fails when `repos.toml`, `lsof` or `ps` cannot be read.
pub fn list(paths: &Paths) -> Result<Vec<Server>> {
    let worktrees = watched(paths)?;
    let snapshot = Snapshot::read(&WorkingDirectories::read()?)?;
    Ok(snapshot
        .inside(&worktrees)
        .values()
        .flat_map(|inside| &inside.servers)
        .map(|found| snapshot.server(paths, found))
        .collect())
}

/// *Stop* (rules 4 to 6): on the operator's explicit request only, checks `pid` again (see
/// [`Snapshot::check`]), sends `SIGTERM` to its process group, and waits up to `wait` for the group
/// to exit. Then, if its launching shell is idle, inside the same worktree and still the same
/// process, sends it `SIGHUP`. Never `SIGKILL`: a group still running is reported as such.
/// `listed_start` is the `started_at` the operator saw; without it, the check uses the process
/// as it is now. A dry run checks everything and signals nothing.
///
/// # Errors
///
/// A [`Refusal`] when a check fails; an error when `lsof`, `ps` or `repos.toml` cannot be read,
/// `listed_start` is not RFC 3339, or the signal fails.
pub fn stop(
    paths: &Paths,
    pid: u32,
    listed_start: Option<&str>,
    dry_run: bool,
    wait: Duration,
) -> Result<Stopped> {
    let listed_start = listed_start
        .map(|time| {
            DateTime::parse_from_rfc3339(time)
                .map(|time| time.with_timezone(&Utc))
                .with_context(|| format!("{time} is not an RFC 3339 time"))
        })
        .transpose()?;
    let worktrees = watched(paths)?;
    let directories = WorkingDirectories::read()?;
    let snapshot = Snapshot::read(&directories)?;
    let found = snapshot.check(pid, listed_start, &worktrees)?;
    let mut stopped = Stopped {
        pid,
        pgid: found.pgid,
        worktree: paths.tilde(&found.worktree),
        state: StopState::WouldStop,
        still_running: Vec::new(),
        hung_up: None,
        log: Vec::new(),
    };
    let term = format!("kill -TERM -{}", found.pgid);
    let hangup = found.shell.map(|shell| format!("kill -HUP {shell}"));
    if dry_run {
        stopped.log.push(format!("would run: {term}"));
        stopped.log.extend(
            hangup.map(|hangup| format!("would run: {hangup} (once the server has exited)")),
        );
        return Ok(stopped);
    }
    signal_group(found.pgid, libc::SIGTERM)?;
    stopped.log.push(format!("ran: {term}"));
    stopped.still_running = wait_for_exit(&snapshot, &found.members, wait)?;
    if !stopped.still_running.is_empty() {
        stopped.state = StopState::StillRunning;
        return Ok(stopped);
    }
    stopped.state = StopState::Stopped;
    if let Some(shell) = found.shell
        && is_still_idle(&snapshot, shell, &found.worktree, &worktrees)?
    {
        signal(shell, libc::SIGHUP)?;
        stopped
            .log
            .extend(hangup.map(|hangup| format!("ran: {hangup}")));
        stopped.hung_up = Some(shell);
    }
    Ok(stopped)
}

/// The members of a signalled group still running after up to `wait`; a pid now held by a
/// process that started later is not one of them.
fn wait_for_exit(before: &Snapshot, members: &[u32], wait: Duration) -> Result<Vec<u32>> {
    let deadline = Instant::now() + wait;
    loop {
        let table = ProcessTable::read()?;
        let running: Vec<u32> = members
            .iter()
            .copied()
            .filter(|pid| {
                let then = before.table.live(*pid).map(|process| process.started_at);
                table
                    .live(*pid)
                    .is_some_and(|now| Some(now.started_at) == then)
            })
            .collect();
        if running.is_empty() || Instant::now() >= deadline {
            return Ok(running);
        }
        thread::sleep(POLL);
    }
}

/// Whether the launching shell found before the signal is the same process, still a shell, still
/// inside the worktree, and has no child left.
fn is_still_idle(
    before: &Snapshot,
    shell: u32,
    worktree: &Path,
    worktrees: &[PathBuf],
) -> Result<bool> {
    let then = before.table.live(shell).map(|process| process.started_at);
    let now = Snapshot::read(&WorkingDirectories::read()?)?;
    let is_same = now
        .table
        .live(shell)
        .is_some_and(|process| Some(process.started_at) == then && process.is_shell());
    let is_here = now
        .worktree_of(shell, worktrees)
        .is_some_and(|holder| holder == worktree);
    let has_no_child = now.table.children(shell).next().is_none();
    Ok(is_same && is_here && has_no_child)
}

fn signal_group(pgid: u32, signal: libc::c_int) -> Result<()> {
    let group = libc::pid_t::try_from(pgid).context("process group out of range")?;
    // SAFETY: killpg only sends a signal. `check` tied every process in the group to the worktree
    // and to this user just before, and refused reap's own group.
    let result = unsafe { libc::killpg(group, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    // The group exited between the check and the signal: that is what Stop wanted.
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(error).with_context(|| format!("signalling process group {pgid}"))
}

fn signal(pid: u32, signal: libc::c_int) -> Result<()> {
    let target = libc::pid_t::try_from(pid).context("pid out of range")?;
    // SAFETY: kill only sends a signal, to a shell `is_still_idle` has just checked.
    let result = unsafe { libc::kill(target, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(error).with_context(|| format!("signalling pid {pid}"))
}

fn current_uid() -> u32 {
    // SAFETY: getuid has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}

fn rfc3339(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// A server's age as the operator reads it: `45 s`, `12 min`, `3 h`, `2 d`.
#[must_use]
pub fn shown_age(started_at: &str, now: DateTime<Utc>) -> String {
    let Ok(started) = DateTime::parse_from_rfc3339(started_at) else {
        return "?".into();
    };
    let seconds = (now - started.with_timezone(&Utc)).num_seconds().max(0);
    match seconds {
        0..60 => format!("{seconds} s"),
        60..3600 => format!("{} min", seconds / 60),
        3600..86_400 => format!("{} h", seconds / 3600),
        _ => format!("{} d", seconds / 86_400),
    }
}

/// One line per server for `git reap ps`: `vite :5173 · pid 4121 · up 2 d · in ~/w/feat`.
#[must_use]
pub fn describe(server: &Server, now: DateTime<Utc>) -> String {
    let ports: Vec<String> = server.ports.iter().map(|port| format!(":{port}")).collect();
    let mut line = format!(
        "{} {} · pid {} · up {} · in {}",
        server.name,
        ports.join(" "),
        server.pid,
        shown_age(&server.started_at, now),
        server.worktree
    );
    if let Some(shell) = &server.shell {
        let _ = write!(line, " · from {} {}", shell.name, shell.pid);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + seconds, 0).unwrap()
    }

    fn process(pid: u32, ppid: u32, pgid: u32, command: &str) -> Process {
        Process {
            pid,
            ppid,
            pgid,
            uid: 501,
            is_zombie: false,
            started_at: at(i64::from(pid)),
            command: command.into(),
        }
    }

    /// A worktree `/w/feat` with an idle zsh (10) that ran `npm run dev` (20, its own group) whose
    /// child node (21) listens on 5173; a second zsh (30) with an agent (31) in it; and reap (99)
    /// in its own group with a cwd elsewhere.
    fn snapshot() -> Snapshot {
        let processes = [
            process(1, 0, 1, "/sbin/launchd"),
            process(10, 1, 10, "-zsh"),
            process(20, 10, 20, "npm run dev"),
            process(21, 20, 20, "node /w/feat/node_modules/.bin/vite"),
            process(30, 1, 30, "/bin/zsh -il"),
            process(31, 30, 30, "claude"),
            process(99, 1, 99, "git-reap stop 20"),
        ];
        let directories = [10, 20, 21, 30, 31]
            .into_iter()
            .map(|pid| (pid, PathBuf::from("/w/feat")))
            .chain([(99, PathBuf::from("/"))])
            .collect();
        Snapshot {
            directories,
            table: ProcessTable(processes.into_iter().map(|p| (p.pid, p)).collect()),
            listeners: Listeners::parse("p21\nf20\nn*:5173\nf21\nn[::1]:5173\n"),
            uid: 501,
            own_pid: 99,
        }
    }

    fn worktrees() -> Vec<PathBuf> {
        vec![PathBuf::from("/w/feat"), PathBuf::from("/w/other")]
    }

    #[test]
    fn parses_ps_lines_with_lstart_and_a_command_with_spaces() {
        let output = "    1     0     1     0 Ss   Wed Nov  4 10:26:37 2026     /sbin/launchd\n\
                      4121  4100  4121   501 S+   Thu Nov  5 09:01:02 2026     node /a b/vite --port 5173\n\
                      4122  4121  4121   501 Z    Thu Nov  5 09:01:03 2026     (node)\n\
                      garbage\n";
        let utc = |time: NaiveDateTime| Some(time.and_utc());

        let table = ProcessTable::parse(output, &utc);

        let node = table.live(4121).unwrap();
        assert_eq!((node.ppid, node.pgid, node.uid), (4100, 4121, 501));
        assert_eq!(node.command, "node /a b/vite --port 5173");
        assert_eq!(node.name(), "node");
        assert_eq!(rfc3339(node.started_at), "2026-11-05T09:01:02Z");
        assert!(table.live(4122).is_none(), "a zombie is not live");
        assert!(table.live(1).is_some());
        assert_eq!(table.0.len(), 3);
    }

    #[test]
    fn parses_listening_ports_of_every_address_form() {
        let listeners = Listeners::parse(
            "p741\nf9\nn*:7000\nf10\nn127.0.0.1:5000\np822\nf11\nn[::1]:49153\nf12\nn*:7000\n",
        );

        assert_eq!(
            listeners
                .of(741)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [5000, 7000]
        );
        assert_eq!(
            listeners
                .of(822)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [7000, 49153]
        );
        assert!(listeners.of(1).is_none());
    }

    #[test]
    fn a_listening_group_is_a_server_and_its_idle_shell_goes_with_it() {
        let inside = snapshot().inside(&worktrees());

        let feat = &inside[Path::new("/w/feat")];
        assert_eq!(feat.servers.len(), 1);
        let server = &feat.servers[0];
        assert_eq!((server.pid, server.pgid, server.listener), (20, 20, 21));
        assert_eq!(server.members, [20, 21]);
        assert_eq!(server.ports, [5173]);
        assert_eq!(server.shell, Some(10));
        assert_eq!(
            feat.others,
            [30, 31],
            "the agent's shell and the agent are not servers"
        );
        assert!(!inside.contains_key(Path::new("/w/other")));
    }

    #[test]
    fn a_shell_with_another_child_is_not_a_launching_shell() {
        let mut snapshot = snapshot();
        snapshot
            .table
            .0
            .insert(40, process(40, 10, 40, "vim notes.md"));

        let inside = snapshot.inside(&worktrees());

        let feat = &inside[Path::new("/w/feat")];
        assert_eq!(feat.servers[0].shell, None);
        assert!(feat.others.contains(&10));
    }

    #[test]
    fn nothing_that_does_not_listen_is_a_server() {
        let mut snapshot = snapshot();
        snapshot.listeners = Listeners::default();

        let feat = &snapshot.inside(&worktrees())[Path::new("/w/feat")];

        assert_eq!(feat.servers, Vec::<Found>::new());
        assert_eq!(feat.others, [10, 20, 21, 30, 31]);
    }

    #[test]
    fn reap_and_other_users_processes_are_never_servers() {
        let mut snapshot = snapshot();
        snapshot.own_pid = 21;
        assert_eq!(
            snapshot.inside(&worktrees())[Path::new("/w/feat")].servers,
            Vec::<Found>::new()
        );

        let mut snapshot = self::snapshot();
        for pid in [20, 21] {
            snapshot.table.0.get_mut(&pid).unwrap().uid = 0;
        }
        assert_eq!(
            snapshot.inside(&worktrees())[Path::new("/w/feat")].servers,
            Vec::<Found>::new()
        );
    }

    #[test]
    fn a_process_belongs_to_the_most_specific_worktree() {
        let snapshot = snapshot();
        let nested = vec![PathBuf::from("/w"), PathBuf::from("/w/feat")];

        let inside = snapshot.inside(&nested);

        assert!(inside.contains_key(Path::new("/w/feat")));
        assert!(!inside.contains_key(Path::new("/w")));
    }

    #[test]
    fn check_passes_a_listed_server() {
        let found = snapshot().check(20, Some(at(20)), &worktrees()).unwrap();

        assert_eq!(found.pgid, 20);
        assert_eq!(found.shell, Some(10));
    }

    #[test]
    fn check_refuses_a_process_that_is_not_running() {
        assert_eq!(
            snapshot().check(77, None, &worktrees()),
            Err(Refusal::NotRunning { pid: 77 })
        );
    }

    #[test]
    fn check_refuses_another_users_process() {
        let mut snapshot = snapshot();
        snapshot.table.0.get_mut(&20).unwrap().uid = 0;

        assert_eq!(
            snapshot.check(20, None, &worktrees()),
            Err(Refusal::OtherUser { pid: 20 })
        );
    }

    #[test]
    fn check_refuses_a_pid_that_now_belongs_to_a_later_process() {
        assert_eq!(
            snapshot().check(20, Some(at(5)), &worktrees()),
            Err(Refusal::Restarted {
                pid: 20,
                listed: at(5),
                running: at(20)
            })
        );
    }

    #[test]
    fn check_refuses_a_process_whose_directory_moved_out_of_every_worktree() {
        let mut snapshot = snapshot();
        snapshot.directories.insert(20, PathBuf::from("/tmp"));
        snapshot.directories.insert(21, PathBuf::from("/tmp"));

        assert_eq!(
            snapshot.check(20, Some(at(20)), &worktrees()),
            Err(Refusal::Outside { pid: 20 })
        );
    }

    #[test]
    fn check_refuses_a_process_that_is_not_a_server() {
        assert_eq!(
            snapshot().check(31, None, &worktrees()),
            Err(Refusal::NotServer { pid: 31 })
        );
    }

    #[test]
    fn check_refuses_a_group_with_a_process_outside_the_worktree() {
        let mut snapshot = snapshot();
        snapshot.directories.insert(21, PathBuf::from("/elsewhere"));

        assert_eq!(
            snapshot.check(20, None, &worktrees()),
            Err(Refusal::GroupOutside {
                pid: 20,
                pgid: 20,
                member: 21
            })
        );
    }

    #[test]
    fn check_refuses_reaps_own_group() {
        let mut snapshot = snapshot();
        snapshot.own_pid = 21;

        assert_eq!(
            snapshot.check(20, None, &worktrees()),
            Err(Refusal::Own { pid: 20 })
        );
    }

    #[test]
    fn a_server_is_shown_with_its_listener_and_shell() {
        let snapshot = snapshot();
        let found = snapshot.check(20, None, &worktrees()).unwrap();
        let paths = Paths::new(Path::new("/w"), Path::new("/c"));

        let server = snapshot.server(&paths, &found);

        assert_eq!(server.name, "node");
        assert_eq!(server.command, "npm run dev");
        assert_eq!(server.worktree, "~/feat");
        assert_eq!(server.started_at, rfc3339(at(20)));
        assert_eq!(
            server.shell,
            Some(Shell {
                pid: 10,
                name: "zsh".into()
            })
        );
        assert_eq!(
            describe(&server, at(20 + 2 * 86_400)),
            "node :5173 · pid 20 · up 2 d · in ~/feat · from zsh 10"
        );
    }

    #[test]
    fn ages_read_in_the_largest_whole_unit() {
        let started = rfc3339(at(0));
        assert_eq!(shown_age(&started, at(45)), "45 s");
        assert_eq!(shown_age(&started, at(12 * 60)), "12 min");
        assert_eq!(shown_age(&started, at(3 * 3600 + 5)), "3 h");
        assert_eq!(shown_age("not a time", at(0)), "?");
    }
}
