//! A repo's `gate` and `after` commands (ADR 0004) against a throwaway remote.

mod sandbox;

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use sandbox::Sandbox;
use tether_core::config::Mode;
use tether_core::inspect::{self, Health, Reason};
use tether_core::sync::{self, Trigger};
use tether_core::{lock, report};

fn lines(file: &Path) -> Vec<String> {
    fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_failing_gate_skips_with_its_output_and_runs_no_git() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let repo = s.set_commands(
        &notes,
        Some("echo; echo 'a session is active'; exit 1"),
        None,
    );
    let before = s.head(&notes);
    s.upstream("README.md", "readme v2\n");
    s.take_git_log();
    let out = s.sync(&repo);
    assert_eq!(
        out.reason,
        Some(Reason::Gated {
            output: "a session is active".into()
        })
    );
    assert_eq!(out.health, Health::Skipped);
    assert_eq!(s.take_git_log(), Vec::<String>::new());
    assert_eq!(s.head(&notes), before);
    let row = &report::report(&s.ctx(true)).unwrap().repos[0];
    assert_eq!(
        (row.health, row.reason.clone()),
        (Health::Skipped, out.reason)
    );
}

#[test]
fn a_gate_that_hangs_is_skipped_after_the_timeout_and_killed() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let marker = s.root.join("gate-finished");
    let gate = format!("sleep 2; touch '{}'", marker.display());
    let repo = s.set_commands(&notes, Some(&gate), None);
    let mut ctx = s.ctx(false);
    ctx.gate_timeout = Duration::from_millis(300);
    let started = Instant::now();
    let out = sync::sync_repo(&ctx, &repo, Trigger::Background).unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "it waited for the gate"
    );
    assert_eq!(
        out.reason,
        Some(Reason::Gated {
            output: "no exit within 0s".into()
        })
    );
    std::thread::sleep(Duration::from_millis(2500));
    assert!(!marker.exists(), "the gate kept running after the timeout");
}

#[test]
fn an_explicit_sync_does_not_ask_the_gate() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let repo = s.set_commands(&notes, Some("exit 1"), None);
    let tip = s.upstream("README.md", "readme v2\n");
    let out = s.sync_now(&repo);
    assert_eq!((out.health, out.reason.clone()), (Health::InSync, None));
    assert_eq!(s.head(&notes), tip);
    assert!(!out.log.iter().any(|l| l.contains("gate")), "{:?}", out.log);
}

#[test]
fn a_passing_gate_lets_the_sync_run() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let repo = s.set_commands(&notes, Some("test \"$TETHER_BRANCH\" = main"), None);
    let tip = s.upstream("README.md", "readme v2\n");
    assert_eq!(s.sync(&repo).health, Health::InSync);
    assert_eq!(s.head(&notes), tip);
}

#[test]
fn after_runs_once_per_move_with_the_range_and_the_lock_free() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let ran = s.root.join("after-ran");
    let after = format!(
        "printf '%s %s %s %s\\n' \"$TETHER_REPO\" \"$TETHER_BRANCH\" \"$TETHER_FROM\" \"$TETHER_TO\" >> '{}'",
        ran.display()
    );
    let repo = s.set_commands(&notes, None, Some(&after));
    let before = s.head(&notes);
    let tip = s.upstream("README.md", "readme v2\n");
    assert_eq!(s.sync(&repo).health, Health::InSync);
    assert_eq!(
        lines(&ran),
        [format!("~/Development/notes main {before} {tip}")]
    );
    assert_eq!(s.sync(&repo).health, Health::InSync);
    assert_eq!(lines(&ran).len(), 1, "ran on a sync with nothing to pull");
}

#[test]
fn after_can_take_the_lock_itself() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let started = s.root.join("after-started");
    let release = s.root.join("after-release");
    let after = format!(
        "touch '{}'; while [ ! -f '{}' ]; do sleep 0.05; done",
        started.display(),
        release.display()
    );
    let repo = s.set_commands(&notes, None, Some(&after));
    s.upstream("README.md", "readme v2\n");
    let paths = s.paths();
    let syncing = std::thread::spawn(move || {
        sync::sync_repo(
            &tether_core::Ctx::new(paths, false),
            &repo,
            Trigger::Background,
        )
        .unwrap()
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while !started.exists() {
        assert!(Instant::now() < deadline, "after never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    let git_dir = inspect::git_dir(&s.ctx(true), &notes).unwrap();
    let taken = lock::acquire(&git_dir, Duration::ZERO).unwrap();
    assert!(
        taken.is_some(),
        "tether still held tether.lock while after ran"
    );
    drop(taken);
    fs::write(&release, "").unwrap();
    assert_eq!(syncing.join().unwrap().health, Health::InSync);
}

#[test]
fn a_background_sync_during_another_runs_after_is_busy_and_keeps_its_failure() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let started = s.root.join("after-started");
    let release = s.root.join("after-release");
    let after = format!(
        "touch '{}'; while [ ! -f '{}' ]; do sleep 0.05; done; echo 'reconcile failed'; exit 1",
        started.display(),
        release.display()
    );
    let repo = s.set_commands(&notes, None, Some(&after));
    s.upstream("README.md", "readme v2\n");
    let paths = s.paths();
    let first = repo.clone();
    let syncing = std::thread::spawn(move || {
        sync::sync_repo(
            &tether_core::Ctx::new(paths, false),
            &first,
            Trigger::Background,
        )
        .unwrap()
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while !started.exists() {
        assert!(Instant::now() < deadline, "after never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(s.sync(&repo).reason, Some(Reason::Busy));
    fs::write(&release, "").unwrap();
    let failed = Some(Reason::AfterFailed {
        output: "reconcile failed".into(),
    });
    assert_eq!(syncing.join().unwrap().reason, failed);
    let row = &report::report(&s.ctx(true)).unwrap().repos[0];
    assert_eq!((row.health, row.reason.clone()), (Health::NeedsYou, failed));
}

#[test]
fn a_failing_after_needs_you_until_a_later_move_runs_it() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let fixed = s.root.join("fixed");
    let ran = s.root.join("after-ran");
    let after = format!(
        "test -f '{}' || {{ echo 'reconcile failed'; exit 1; }}; echo \"$TETHER_FROM $TETHER_TO\" >> '{}'",
        fixed.display(),
        ran.display()
    );
    let repo = s.set_commands(&notes, None, Some(&after));
    let before = s.head(&notes);
    let first = s.upstream("README.md", "readme v2\n");
    let failed = Some(Reason::AfterFailed {
        output: "reconcile failed".into(),
    });
    let out = s.sync(&repo);
    assert_eq!(
        (out.health, out.reason.clone()),
        (Health::NeedsYou, failed.clone())
    );
    assert_eq!(s.head(&notes), first, "the pull itself stands");
    // A background sync with nothing to pull keeps it, and so does status.
    assert_eq!(s.sync(&repo).reason, failed);
    let row = &report::report(&s.ctx(true)).unwrap().repos[0];
    assert_eq!((row.health, row.reason.clone()), (Health::NeedsYou, failed));
    assert!(row.hint.is_some());
    fs::write(&fixed, "").unwrap();
    let second = s.upstream("README.md", "readme v3\n");
    assert_eq!(s.sync(&repo).health, Health::InSync);
    assert_eq!(
        lines(&ran),
        [format!("{before} {second}")],
        "catches up from the last good tip"
    );
    let row = &report::report(&s.ctx(true)).unwrap().repos[0];
    assert_eq!((row.health, row.reason.clone()), (Health::InSync, None));
}

#[test]
fn an_explicit_sync_runs_a_failed_after_again() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let fixed = s.root.join("fixed");
    let after = format!("test -f '{}'", fixed.display());
    let repo = s.set_commands(&notes, None, Some(&after));
    s.upstream("README.md", "readme v2\n");
    let out = s.sync(&repo);
    assert_eq!(
        out.reason,
        Some(Reason::AfterFailed {
            output: "exited with status 1".into()
        })
    );
    assert_eq!(s.sync_now(&repo).health, Health::NeedsYou, "still broken");
    fs::write(&fixed, "").unwrap();
    assert_eq!(s.sync_now(&repo).health, Health::InSync);
    assert_eq!(s.sync(&repo).health, Health::InSync);
}

#[test]
fn a_dry_run_runs_neither_and_says_what_it_would() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let gate_ran = s.root.join("gate-ran");
    let after_ran = s.root.join("after-ran");
    let gate = format!("touch '{}'", gate_ran.display());
    let after = format!("touch '{}'", after_ran.display());
    let repo = s.set_commands(&notes, Some(&gate), Some(&after));
    s.upstream("README.md", "readme v2\n");
    let out = sync::sync_repo(&s.ctx(true), &repo, Trigger::Background).unwrap();
    assert_eq!(out.health, Health::Behind, "{out:#?}");
    assert!(
        out.log.contains(&format!("would run gate: {gate}")),
        "{:?}",
        out.log
    );
    assert!(
        out.log.contains(&format!("would run after: {after}")),
        "{:?}",
        out.log
    );
    assert!(!gate_ran.exists() && !after_ran.exists());
}

#[test]
fn status_json_reports_both_commands() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    s.set_commands(&notes, Some("tool idle"), Some("tool reconcile"));
    let out = s.cli(&["status", "--json"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["repos"][0]["gate"], "tool idle");
    assert_eq!(json["repos"][0]["after"], "tool reconcile");
}
