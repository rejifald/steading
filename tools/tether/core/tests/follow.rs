//! `follow` mode end to end (rules 3–6) against a throwaway remote.

mod sandbox;

use std::fs;
use std::time::Duration;

use sandbox::{MUTATING, Sandbox, git, subcommand, write};
use tether_core::config::Mode;
use tether_core::inspect::{Health, Reason};
use tether_core::{lock, report, sync};

#[test]
fn fast_forwards_a_clean_clone() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    let before = s.head(&notes);
    let tip = s.upstream("notes/a.md", "two\n");
    let out = s.sync(&repo);
    assert_eq!(
        (out.health, out.reason.clone()),
        (Health::InSync, None),
        "{out:#?}"
    );
    assert_eq!(out.moved, Some((before, tip.clone())));
    assert!(out.fetched);
    assert_eq!(s.head(&notes), tip);
    assert_eq!(
        fs::read_to_string(notes.join("notes/a.md")).unwrap(),
        "two\n"
    );
}

#[test]
fn tracked_changes_skip_and_keep_the_edit() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    let before = s.head(&notes);
    s.upstream("README.md", "readme v2\n");
    write(&notes.join("notes/a.md"), "my edit\n");
    let out = s.sync(&repo);
    assert_eq!(out.reason, Some(Reason::TrackedChanges { count: 1 }));
    assert_eq!(out.health, Health::NeedsYou);
    assert_eq!(s.head(&notes), before);
    assert_eq!(
        fs::read_to_string(notes.join("notes/a.md")).unwrap(),
        "my edit\n"
    );
}

#[test]
fn an_edited_clone_with_nothing_to_pull_is_in_sync() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    write(&notes.join("notes/a.md"), "my edit\n");
    assert_eq!(s.sync(&repo).health, Health::InSync);
}

#[test]
fn another_branch_skips() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    git(&notes, &["checkout", "--quiet", "-b", "feature"]);
    s.upstream("README.md", "readme v2\n");
    let out = s.sync(&repo);
    assert_eq!(
        out.reason,
        Some(Reason::OtherBranch {
            branch: "feature".into()
        })
    );
    assert_eq!(
        git(&notes, &["symbolic-ref", "--short", "HEAD"]),
        "feature",
        "never switches branches"
    );
}

#[test]
fn a_detached_head_skips() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    git(&notes, &["checkout", "--quiet", "--detach"]);
    s.upstream("README.md", "readme v2\n");
    assert_eq!(s.sync(&repo).reason, Some(Reason::Detached));
}

#[test]
fn a_diverged_branch_skips_with_counts() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    write(&notes.join("local.md"), "local\n");
    git(&notes, &["add", "local.md"]);
    git(&notes, &["commit", "--quiet", "-m", "local"]);
    let mine = s.head(&notes);
    s.upstream("README.md", "readme v2\n");
    let out = s.sync(&repo);
    assert_eq!(
        out.reason,
        Some(Reason::Diverged {
            ahead: 1,
            behind: 1
        })
    );
    assert_eq!(s.head(&notes), mine, "never rebases or resets");
}

#[test]
fn only_ahead_is_in_sync() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    write(&notes.join("local.md"), "local\n");
    git(&notes, &["add", "local.md"]);
    git(&notes, &["commit", "--quiet", "-m", "local"]);
    let out = s.sync(&repo);
    assert_eq!((out.health, out.reason), (Health::InSync, None));
    assert!(out.moved.is_none());
}

#[test]
fn a_rebase_in_progress_skips_before_any_fetch() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    s.upstream("README.md", "readme v2\n");
    fs::create_dir_all(notes.join(".git/rebase-merge")).unwrap();
    s.take_git_log();
    let out = s.sync(&repo);
    assert_eq!(
        out.reason,
        Some(Reason::OperationInProgress {
            operation: "rebase".into()
        })
    );
    assert_eq!(out.health, Health::NeedsYou);
    assert!(s.take_git_log().iter().all(|l| subcommand(l) != "fetch"));
}

#[test]
fn an_index_lock_skips_this_round_only() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    let tip = s.upstream("README.md", "readme v2\n");
    write(&notes.join(".git/index.lock"), "");
    let out = s.sync(&repo);
    assert_eq!(
        (out.health, out.reason),
        (Health::Skipped, Some(Reason::IndexLocked))
    );
    fs::remove_file(notes.join(".git/index.lock")).unwrap();
    s.sync(&repo);
    assert_eq!(s.head(&notes), tip);
}

#[test]
fn another_runs_lock_skips_as_busy() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    let _held = lock::acquire(&notes.join(".git"), Duration::ZERO)
        .unwrap()
        .expect("free");
    assert_eq!(s.sync(&repo).reason, Some(Reason::Busy));
}

#[test]
fn untracked_files_are_allowed() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    write(&notes.join("scratch.txt"), "mine\n");
    let tip = s.upstream("README.md", "readme v2\n");
    let out = s.sync(&repo);
    assert_eq!(out.reason, None);
    assert_eq!(s.head(&notes), tip);
    assert_eq!(
        fs::read_to_string(notes.join("scratch.txt")).unwrap(),
        "mine\n"
    );
}

#[test]
fn an_untracked_file_in_the_way_is_a_refused_fast_forward() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    let before = s.head(&notes);
    write(&notes.join("new.md"), "mine\n");
    s.upstream("new.md", "theirs\n");
    let out = s.sync(&repo);
    assert!(
        matches!(out.reason, Some(Reason::FastForwardRefused { .. })),
        "{out:#?}"
    );
    assert_eq!(s.head(&notes), before);
    assert_eq!(fs::read_to_string(notes.join("new.md")).unwrap(), "mine\n");
}

#[test]
fn an_unchanged_remote_is_not_fetched() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    s.upstream("README.md", "readme v2\n");
    assert!(s.sync(&repo).fetched);
    s.take_git_log();
    let out = s.sync(&repo);
    assert!(!out.fetched);
    let log = s.take_git_log();
    assert!(log.iter().any(|l| subcommand(l) == "ls-remote"), "{log:#?}");
    assert!(log.iter().all(|l| subcommand(l) != "fetch"), "{log:#?}");
}

#[test]
fn an_unreachable_remote_is_a_fetch_failure() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = s.list(&notes, Mode::Follow);
    git(
        &notes,
        &[
            "remote",
            "set-url",
            "origin",
            &format!("file://{}", s.root.join("gone.git").display()),
        ],
    );
    let out = s.sync(&repo);
    assert!(
        matches!(out.reason, Some(Reason::FetchFailed { .. })),
        "{out:#?}"
    );
    let r = report::report(&s.ctx(true)).unwrap();
    assert!(
        matches!(r.repos[0].reason, Some(Reason::FetchFailed { .. })),
        "status keeps the network failure"
    );
}

#[test]
fn pause_stops_the_tick_and_resume_restarts_it() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let before = s.head(&notes);
    let tip = s.upstream("README.md", "readme v2\n");
    s.paths().set_paused(true).unwrap();
    assert!(sync::tick(&s.ctx(false)).unwrap().is_empty());
    assert_eq!(s.head(&notes), before);
    s.paths().set_paused(false).unwrap();
    assert_eq!(sync::tick(&s.ctx(false)).unwrap().len(), 1);
    assert_eq!(s.head(&notes), tip);
    // Just synced: not due again until its interval passes.
    assert!(sync::tick(&s.ctx(false)).unwrap().is_empty());
}

#[test]
fn dry_run_runs_no_mutating_git_command() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    let before = s.head(&notes);
    let tracking = git(&notes, &["rev-parse", "refs/remotes/origin/main"]);
    s.upstream("README.md", "readme v2\n");
    s.take_git_log();
    let out = s.cli(&["sync", "--dry-run"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("would run: git -C ~/Development/notes fetch"),
        "{stdout}"
    );
    let log = s.take_git_log();
    assert!(log.iter().any(|l| subcommand(l) == "ls-remote"), "{log:#?}");
    let mutating: Vec<_> = log
        .iter()
        .filter(|l| MUTATING.contains(&subcommand(l)))
        .collect();
    assert!(mutating.is_empty(), "{mutating:#?}");
    assert_eq!(s.head(&notes), before);
    assert_eq!(
        git(&notes, &["rev-parse", "refs/remotes/origin/main"]),
        tracking
    );
    assert!(
        !s.config.join("state.json").exists(),
        "a dry run records nothing"
    );
}

#[test]
fn status_lists_the_reason_and_the_cli_reports_it() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    git(&notes, &["checkout", "--quiet", "--detach"]);
    let r = report::report(&s.ctx(true)).unwrap();
    assert_eq!((r.overall, r.needs_you), (Health::NeedsYou, 1));
    assert_eq!(r.repos[0].reason, Some(Reason::Detached));
    let out = s.cli(&["status", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["repos"][0]["reason"]["kind"], "detached");
    assert_eq!(v["repos"][0]["health"], "needs-you");
}

#[test]
fn an_unknown_repo_is_a_usage_error() {
    let s = Sandbox::new();
    assert_eq!(s.cli(&["sync", "nope"]).status.code(), Some(2));
    assert_eq!(s.cli(&["remove", "nope"]).status.code(), Some(2));
}

#[test]
fn a_fetched_but_unmerged_clone_is_behind_in_status() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    s.upstream("README.md", "readme v2\n");
    s.upstream("README.md", "readme v3\n");
    git(&notes, &["fetch", "--quiet"]);
    let r = report::report(&s.ctx(true)).unwrap();
    assert_eq!((r.overall, r.needs_you), (Health::Behind, 0));
    assert_eq!(
        (r.repos[0].health, r.repos[0].behind),
        (Health::Behind, Some(2))
    );
    let out = s.cli(&["status"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("2 commit(s) to pull"), "{stdout}");
}

#[test]
fn a_dry_run_with_something_to_pull_says_behind() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.list(&notes, Mode::Follow);
    s.upstream("README.md", "readme v2\n");
    let out = s.cli(&["sync", "--dry-run"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("~/Development/notes: behind"), "{stdout}");
    let fetched =
        sync::sync_repo(&s.ctx(false), &s.repo(&notes), sync::Trigger::Background).unwrap();
    assert!(fetched.moved.is_some(), "{fetched:#?}");
    git(&notes, &["reset", "--quiet", "--hard", "HEAD~1"]);
    let planned =
        sync::sync_repo(&s.ctx(true), &s.repo(&notes), sync::Trigger::Background).unwrap();
    assert_eq!(planned.health, Health::Behind, "{planned:#?}");
    assert_eq!(planned.describe(), "behind");
}
