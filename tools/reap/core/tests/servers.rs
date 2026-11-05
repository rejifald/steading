//! Servers and *Stop* end to end (ADR 0005 rules 3 to 6), against processes the test spawns in
//! its own temp repositories. Every signal goes to a process group the test started; a refusal
//! test asserts that nothing was signalled.

#[allow(dead_code)] // each test binary uses part of the sandbox
mod sandbox;

use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use reap_core::report;
use reap_core::servers::{self, ProcessTable, Refusal, Server, StopState};
use sandbox::Sandbox;

/// Listens on a free port on 127.0.0.1, then sleeps; ignores SIGTERM when `IGNORE_TERM` is set.
const LISTENER: &str = "use IO::Socket::INET; $SIG{TERM} = 'IGNORE' if $ENV{IGNORE_TERM}; \
    my $socket = IO::Socket::INET->new(Listen => 5, LocalAddr => '127.0.0.1', LocalPort => 0, \
    Proto => 'tcp') or die \"listen: $!\"; sleep 60 for 1 .. 10;";

const SHORT_WAIT: Duration = Duration::from_secs(5);

/// Kills the child when the test ends, however it ends.
struct Spawned(Child);

impl Drop for Spawned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Spawned {
    fn pid(&self) -> u32 {
        self.0.id()
    }

    fn is_running(&mut self) -> bool {
        self.0.try_wait().unwrap().is_none()
    }

    /// Whether it exits by itself within `wait`.
    fn exits_within(&mut self, wait: Duration) -> bool {
        let deadline = Instant::now() + wait;
        while Instant::now() < deadline {
            if !self.is_running() {
                return true;
            }
            thread::sleep(Duration::from_millis(50));
        }
        false
    }
}

/// A listener in `dir`, in a process group of its own unless `own_group` is false.
fn listener(dir: &Path, own_group: bool, ignore_term: bool) -> Spawned {
    let mut command = Command::new("perl");
    command.args(["-e", LISTENER]).current_dir(dir);
    if own_group {
        command.process_group(0);
    }
    if ignore_term {
        command.env("IGNORE_TERM", "1");
    }
    Spawned(command.spawn().unwrap())
}

/// The server `pid` as `git reap ps` lists it, once its socket is open.
fn listed(sandbox: &Sandbox, pid: u32) -> Server {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(server) = servers::list(&sandbox.paths)
            .unwrap()
            .into_iter()
            .find(|server| server.pid == pid)
        {
            return server;
        }
        assert!(
            Instant::now() < deadline,
            "pid {pid} never listed as a server"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// Waits until `pid` listens, without asking for it to be inside a worktree.
fn wait_until_listening(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let output = Command::new("lsof")
            .args(["-n", "-P", "-w", "-a", "-iTCP", "-sTCP:LISTEN", "-p"])
            .arg(pid.to_string())
            .output()
            .unwrap();
        if !output.stdout.is_empty() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("pid {pid} never listened");
}

fn refusal(result: anyhow::Result<servers::Stopped>) -> Refusal {
    result
        .expect_err("stop must refuse")
        .downcast::<Refusal>()
        .expect("a refusal, not another error")
}

fn sandbox_with(worktree: &str) -> (Sandbox, std::path::PathBuf) {
    let sandbox = Sandbox::new();
    let path = sandbox.worktree(worktree);
    sandbox.list_clone();
    (sandbox, path)
}

#[test]
fn a_listener_in_a_worktree_is_listed_in_the_report_and_ps() {
    let (sandbox, feat) = sandbox_with("feat");
    let child = listener(&feat, true, false);

    let server = listed(&sandbox, child.pid());
    let status = report::report(&sandbox.paths, Utc::now()).unwrap();

    assert_eq!(server.pgid, child.pid());
    assert_eq!(server.name, "perl");
    assert_eq!(server.worktree, "~/feat");
    assert_eq!(server.ports.len(), 1);
    assert!(server.command.starts_with("perl -e"));
    assert!(status.servers_error.is_none());
    let repo = &status.repos[0];
    assert!(repo.servers.iter().any(|shown| shown.pid == child.pid()));
    assert_eq!(repo.worktrees[0].other_processes, Vec::<String>::new());
}

#[test]
fn a_process_that_does_not_listen_is_another_process_not_a_server() {
    let (sandbox, feat) = sandbox_with("feat");
    let mut command = Command::new("sleep");
    command.arg("30").current_dir(&feat).process_group(0);
    let mut sleeper = Spawned(command.spawn().unwrap());

    let status = report::report(&sandbox.paths, Utc::now()).unwrap();
    let stopped = servers::stop(&sandbox.paths, sleeper.pid(), None, false, SHORT_WAIT);

    assert_eq!(status.repos[0].servers, Vec::<Server>::new());
    assert_eq!(status.repos[0].worktrees[0].other_processes, ["sleep"]);
    assert_eq!(refusal(stopped), Refusal::NotServer { pid: sleeper.pid() });
    assert!(sleeper.is_running());
}

#[test]
fn stop_terminates_the_listed_server() {
    let (sandbox, feat) = sandbox_with("feat");
    let mut child = listener(&feat, true, false);
    let server = listed(&sandbox, child.pid());

    let stopped = servers::stop(
        &sandbox.paths,
        server.pid,
        Some(&server.started_at),
        false,
        SHORT_WAIT,
    )
    .unwrap();

    assert_eq!(stopped.state, StopState::Stopped);
    assert_eq!(stopped.log, [format!("ran: kill -TERM -{}", server.pgid)]);
    assert!(child.exits_within(SHORT_WAIT));
}

#[test]
fn a_dry_run_checks_and_signals_nothing() {
    let (sandbox, feat) = sandbox_with("feat");
    let mut child = listener(&feat, true, false);
    let server = listed(&sandbox, child.pid());

    let stopped = servers::stop(&sandbox.paths, server.pid, None, true, SHORT_WAIT).unwrap();

    assert_eq!(stopped.state, StopState::WouldStop);
    assert_eq!(
        stopped.log,
        [format!("would run: kill -TERM -{}", server.pgid)]
    );
    thread::sleep(Duration::from_millis(300));
    assert!(child.is_running());
}

#[test]
fn a_server_that_ignores_sigterm_is_reported_still_running_never_killed() {
    let (sandbox, feat) = sandbox_with("feat");
    let mut child = listener(&feat, true, true);
    let server = listed(&sandbox, child.pid());

    let stopped = servers::stop(
        &sandbox.paths,
        server.pid,
        Some(&server.started_at),
        false,
        Duration::from_millis(500),
    )
    .unwrap();

    assert_eq!(stopped.state, StopState::StillRunning);
    assert_eq!(stopped.still_running, [child.pid()]);
    assert!(child.is_running());
}

#[test]
fn stop_refuses_a_start_time_other_than_the_one_listed() {
    let (sandbox, feat) = sandbox_with("feat");
    let mut child = listener(&feat, true, false);
    listed(&sandbox, child.pid());

    let refused = refusal(servers::stop(
        &sandbox.paths,
        child.pid(),
        Some("2000-01-01T00:00:00Z"),
        false,
        SHORT_WAIT,
    ));

    assert!(matches!(refused, Refusal::Restarted { pid, .. } if pid == child.pid()));
    assert!(child.is_running());
}

#[test]
fn stop_refuses_a_server_outside_every_listed_worktree() {
    let (sandbox, _) = sandbox_with("feat");
    let elsewhere = sandbox.root.join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    let mut outside = listener(&elsewhere, true, false);
    // The main clone is never reap's to act in (rule 11).
    let mut in_clone = listener(&sandbox.clone, true, false);
    wait_until_listening(outside.pid());
    wait_until_listening(in_clone.pid());

    for child in [&mut outside, &mut in_clone] {
        let refused = refusal(servers::stop(
            &sandbox.paths,
            child.pid(),
            None,
            false,
            SHORT_WAIT,
        ));

        assert_eq!(refused, Refusal::Outside { pid: child.pid() });
        assert!(child.is_running());
    }
}

#[test]
fn stop_refuses_a_server_in_reaps_own_process_group() {
    let (sandbox, feat) = sandbox_with("feat");
    // Without a group of its own the child shares the test's, which `killpg` would also hit.
    let mut child = listener(&feat, false, false);
    wait_until_listening(child.pid());

    let refused = refusal(servers::stop(
        &sandbox.paths,
        child.pid(),
        None,
        false,
        SHORT_WAIT,
    ));

    assert_eq!(refused, Refusal::Own { pid: child.pid() });
    assert!(child.is_running());
}

#[test]
fn stop_refuses_another_users_process() {
    let sandbox = Sandbox::new();
    sandbox.list_clone();
    let init = ProcessTable::read().unwrap();
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    let Some(other) = init.live(1).filter(|process| process.uid != uid) else {
        // In a container where pid 1 is this user's there is no other user's process to try.
        return;
    };

    let refused = refusal(servers::stop(
        &sandbox.paths,
        other.pid,
        None,
        false,
        SHORT_WAIT,
    ));

    assert_eq!(refused, Refusal::OtherUser { pid: 1 });
}

#[cfg(target_os = "macos")]
#[test]
fn stop_hangs_up_the_idle_launching_shell_once_the_server_is_gone() {
    let (sandbox, feat) = sandbox_with("feat");
    // A shell that, like a terminal tab, runs the server as a job in its own group and sits idle
    // once it ends; it spins without a child until something hangs it up.
    let listener = sandbox.root.join("listener.pl");
    std::fs::write(&listener, LISTENER).unwrap();
    let script = format!(
        "set -m; perl '{}' & wait; while :; do :; done",
        listener.display()
    );
    let mut command = Command::new("sh");
    command
        .args(["-c", &script])
        .current_dir(&feat)
        .process_group(0);
    let mut shell = Spawned(command.spawn().unwrap());
    let server = {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let found = servers::list(&sandbox.paths)
                .unwrap()
                .into_iter()
                .find(|server| server.shell.as_ref().map(|shell| shell.pid) == Some(shell.pid()));
            if let Some(server) = found {
                break server;
            }
            assert!(Instant::now() < deadline, "the shell's server never listed");
            thread::sleep(Duration::from_millis(100));
        }
    };

    let stopped = servers::stop(
        &sandbox.paths,
        server.pid,
        Some(&server.started_at),
        false,
        SHORT_WAIT,
    )
    .unwrap();

    assert_eq!(stopped.state, StopState::Stopped);
    assert_eq!(stopped.hung_up, Some(shell.pid()));
    assert!(shell.exits_within(SHORT_WAIT));
}
