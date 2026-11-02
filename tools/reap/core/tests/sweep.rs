//! `git reap sweep` end to end against real repositories (rules 7, 8, 10 to 12).

mod sandbox;

use std::time::Duration;

use chrono::{DateTime, Utc};
use reap_core::classify::State;
use reap_core::ctx::Ctx;
use reap_core::sweep::{self, Outcome};
use sandbox::{Sandbox, commit, git};

fn later(hours: i64) -> DateTime<Utc> {
    Utc::now() + chrono::Duration::hours(hours)
}

fn sweep_at(sandbox: &Sandbox, now: DateTime<Utc>) -> Outcome {
    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let mut outcomes = sweep::sweep(&ctx, None, Duration::ZERO, now).unwrap();
    assert_eq!(outcomes.len(), 1);
    outcomes.remove(0)
}

fn set_policy(sandbox: &Sandbox, policy: &str) {
    let file = sandbox.paths.config_file();
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("policy = \"{policy}\"\n{text}")).unwrap();
}

fn branch_exists(sandbox: &Sandbox, branch: &str) -> bool {
    !git(&sandbox.clone, &["branch", "--list", branch]).is_empty()
}

#[test]
fn a_merged_worktree_is_removed_only_after_its_grace_period() {
    let sandbox = Sandbox::new();
    let empty = sandbox.worktree("empty");
    sandbox.list_clone();

    let first = sweep_at(&sandbox, Utc::now());
    let early = sweep_at(&sandbox, later(23));
    let due = sweep_at(&sandbox, later(25));

    assert_eq!(first.started.len(), 1);
    assert!(first.started[0].removes_at.is_some());
    assert_eq!(first.report.worktrees[0].state, State::Removable);
    assert!(first.report.worktrees[0].removes_at.is_some());
    assert_eq!(early.removed.len(), 0);
    assert_eq!(due.removed.len(), 1);
    assert!(!empty.exists());
    // No commits of its own: `git branch -d` accepts it.
    assert_eq!(due.removed[0].branch_kept, None);
    assert!(!branch_exists(&sandbox, "empty"));
    assert!(due.report.worktrees.is_empty());
}

#[test]
fn a_branch_git_will_not_delete_with_d_is_kept() {
    let sandbox = Sandbox::new();
    let merged = sandbox.worktree("merged");
    commit(&merged, "done");
    sandbox.merge_on_remote("merged", "main");
    sandbox.list_clone();

    sweep_at(&sandbox, Utc::now());
    let due = sweep_at(&sandbox, later(25));

    assert!(!merged.exists());
    // The local main never moved, so `-d` sees unmerged commits; only gh's proof allows -D.
    assert!(
        due.removed[0]
            .branch_kept
            .as_deref()
            .unwrap()
            .contains("not fully merged")
    );
    assert!(branch_exists(&sandbox, "merged"));
}

#[test]
fn a_squash_merged_branch_is_deleted_on_ghs_proof() {
    let mut sandbox = Sandbox::new();
    let squashed = sandbox.worktree("squashed");
    let head = commit(&squashed, "squashed");
    sandbox.use_github_url();
    sandbox.fake_gh(&format!(
        r#"[{{"number":12,"state":"MERGED","headRefName":"squashed","headRefOid":"{head}","baseRefName":"main","url":"u"}}]"#
    ));
    sandbox.list_clone();

    sweep_at(&sandbox, Utc::now());
    let due = sweep_at(&sandbox, later(25));

    assert!(!squashed.exists());
    assert_eq!(due.removed[0].branch_kept, None);
    assert!(!branch_exists(&sandbox, "squashed"));
}

#[test]
fn a_branch_behind_its_merged_pull_request_is_deleted_on_ghs_proof() {
    let mut sandbox = Sandbox::new();
    let behind = sandbox.worktree("behind");
    let head = commit(&behind, "first");
    let fixer_head = commit(&behind, "pushed by a fixer");
    git(&behind, &["reset", "--quiet", "--hard", &head]);
    sandbox.use_github_url();
    sandbox.fake_gh_routes(&[(
        "pr*",
        &format!(
            r#"[{{"number":14,"state":"MERGED","headRefName":"behind","headRefOid":"{fixer_head}","baseRefName":"main","url":"u"}}]"#
        ),
    )]);
    sandbox.list_clone();

    sweep_at(&sandbox, Utc::now());
    let due = sweep_at(&sandbox, later(25));

    assert!(!behind.exists());
    assert_eq!(due.removed[0].branch_kept, None);
    assert!(!branch_exists(&sandbox, "behind"));
}

#[test]
fn a_pushed_stale_worktree_is_removed_and_its_branch_kept() {
    let sandbox = Sandbox::new();
    let idle = sandbox.worktree("idle");
    commit(&idle, "idle");
    git(&sandbox.clone, &["push", "--quiet", "origin", "idle"]);
    sandbox.list_clone();

    sweep_at(&sandbox, later(21 * 24));
    let due = sweep_at(&sandbox, later(22 * 24 + 1));

    assert!(!idle.exists());
    assert_eq!(due.removed[0].branch_kept.as_deref(), Some("not merged"));
    assert!(branch_exists(&sandbox, "idle"));
}

#[test]
fn guards_keep_dirty_locked_and_busy_worktrees_past_their_grace() {
    let sandbox = Sandbox::new();
    let dirty = sandbox.worktree("dirty");
    let locked = sandbox.worktree("locked");
    let busy = sandbox.worktree("busy");
    std::fs::write(dirty.join("notes"), "work in progress").unwrap();
    git(
        &sandbox.clone,
        &["worktree", "lock", locked.to_str().unwrap()],
    );
    let mut process = std::process::Command::new("sleep")
        .arg("30")
        .current_dir(&busy)
        .spawn()
        .unwrap();
    sandbox.list_clone();

    sweep_at(&sandbox, Utc::now());
    let due = sweep_at(&sandbox, later(25));
    process.kill().unwrap();
    process.wait().unwrap();

    assert!(due.removed.is_empty(), "{:?}", due.removed);
    assert!(dirty.exists() && locked.exists() && busy.exists());
}

#[test]
fn a_kept_worktree_stays_until_it_gets_new_commits() {
    let sandbox = Sandbox::new();
    let kept = sandbox.worktree("kept");
    sandbox.list_clone();

    sweep::keep(&sandbox.paths, kept.to_str().unwrap()).unwrap();
    sweep_at(&sandbox, Utc::now());
    let held = sweep_at(&sandbox, later(25));
    commit(&kept, "more work");
    sweep_at(&sandbox, later(26));
    let unkept = sweep::unkeep(&sandbox.paths, kept.to_str().unwrap()).unwrap();

    assert_eq!(held.removed.len(), 0);
    assert_eq!(held.report.worktrees[0].state, State::Kept);
    assert!(kept.exists());
    assert_eq!(unkept, None, "new commits ended the Keep");
}

#[test]
fn ask_never_removes_on_its_own_but_remove_now_does() {
    let sandbox = Sandbox::new();
    let asked = sandbox.worktree("asked");
    sandbox.list_clone();
    set_policy(&sandbox, "ask");

    let first = sweep_at(&sandbox, Utc::now());
    let after_grace = sweep_at(&sandbox, later(48));
    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let removed = sweep::remove_now(&ctx, asked.to_str().unwrap(), later(48)).unwrap();

    assert_eq!(first.started[0].removes_at, None);
    assert_eq!(after_grace.removed.len(), 0);
    assert_eq!(removed.branch.as_deref(), Some("asked"));
    assert!(!asked.exists());
}

#[test]
fn remove_now_refuses_a_worktree_that_is_not_removable() {
    let sandbox = Sandbox::new();
    let open = sandbox.worktree("open");
    commit(&open, "work");
    sandbox.list_clone();

    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let error = sweep::remove_now(&ctx, open.to_str().unwrap(), Utc::now()).unwrap_err();

    assert!(error.to_string().contains("is not removable"), "{error}");
    assert!(open.exists());
}

#[test]
fn a_dry_run_prints_the_removal_and_changes_nothing() {
    let sandbox = Sandbox::new();
    let empty = sandbox.worktree("empty");
    sandbox.list_clone();
    sweep_at(&sandbox, Utc::now());
    let state_before = std::fs::read_to_string(sandbox.paths.state_file()).unwrap();

    let ctx = Ctx::new(sandbox.paths.clone(), true);
    let outcome = sweep::sweep(&ctx, None, Duration::ZERO, later(25))
        .unwrap()
        .remove(0);

    assert!(empty.exists());
    assert!(branch_exists(&sandbox, "empty"));
    assert!(
        outcome
            .log
            .iter()
            .any(|line| line.starts_with("would run: git -C") && line.contains("worktree remove")),
        "{:?}",
        outcome.log
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.paths.state_file()).unwrap(),
        state_before
    );
}

#[test]
fn missing_entries_are_pruned_after_their_grace() {
    let sandbox = Sandbox::new();
    let gone = sandbox.worktree("gone");
    std::fs::remove_dir_all(&gone).unwrap();
    sandbox.list_clone();

    sweep_at(&sandbox, Utc::now());
    let due = sweep_at(&sandbox, later(25));

    assert!(due.pruned);
    assert_eq!(
        git(&sandbox.clone, &["worktree", "list"]).lines().count(),
        1
    );
}

#[test]
fn a_kept_entry_whose_folder_is_gone_is_not_pruned_with_its_sibling() {
    let sandbox = Sandbox::new();
    let gone = sandbox.worktree("gone");
    let kept = sandbox.worktree("kept");
    sandbox.list_clone();
    sweep::keep(&sandbox.paths, kept.to_str().unwrap()).unwrap();
    std::fs::remove_dir_all(&gone).unwrap();
    std::fs::remove_dir_all(&kept).unwrap();

    sweep_at(&sandbox, Utc::now());
    let due = sweep_at(&sandbox, later(25));

    assert!(!due.pruned);
    let listed = git(&sandbox.clone, &["worktree", "list"]);
    assert!(listed.contains("kept"), "{listed}");
}

#[test]
fn a_locked_worktree_whose_folder_is_gone_gets_no_grace_clock() {
    let sandbox = Sandbox::new();
    let away = sandbox.worktree("away");
    git(
        &sandbox.clone,
        &["worktree", "lock", away.to_str().unwrap()],
    );
    std::fs::remove_dir_all(&away).unwrap();
    sandbox.list_clone();

    let first = sweep_at(&sandbox, Utc::now());
    let second = sweep_at(&sandbox, later(25));

    assert!(first.started.is_empty(), "{:?}", first.started);
    assert!(second.started.is_empty(), "{:?}", second.started);
    assert!(!second.pruned);
    assert!(
        second
            .report
            .worktrees
            .iter()
            .all(|row| row.state != State::Missing),
        "{:?}",
        second.report.worktrees
    );
}

#[test]
fn a_pass_fetches_a_base_that_moved_on_the_remote() {
    let sandbox = Sandbox::new();
    let merged = sandbox.worktree("merged");
    commit(&merged, "done");
    let stale_main = git(&sandbox.clone, &["rev-parse", "origin/main"]);
    sandbox.merge_on_remote("merged", "main");
    git(
        &sandbox.clone,
        &["update-ref", "refs/remotes/origin/main", &stale_main],
    );
    sandbox.list_clone();

    let outcome = sweep_at(&sandbox, Utc::now());

    assert_eq!(outcome.fetch_note, None);
    assert!(
        outcome.log.iter().any(|line| line.contains("fetch")),
        "{:?}",
        outcome.log
    );
    assert_eq!(outcome.report.worktrees[0].state, State::Removable);
}

#[test]
fn a_clone_tether_is_syncing_is_skipped() {
    let sandbox = Sandbox::new();
    let empty = sandbox.worktree("empty");
    sandbox.list_clone();
    sweep_at(&sandbox, Utc::now());
    let tether = std::fs::File::create(sandbox.clone.join(".git/tether.lock")).unwrap();
    tether.lock().unwrap();

    let busy = sweep_at(&sandbox, later(25));

    assert_eq!(busy.skipped.as_deref(), Some("git-tether is syncing it"));
    assert_eq!(busy.removed.len(), 0);
    assert!(empty.exists());
}

#[test]
fn the_main_clone_is_never_touched() {
    let sandbox = Sandbox::new();
    let head = git(&sandbox.clone, &["rev-parse", "HEAD"]);
    sandbox.worktree("empty");
    sandbox.list_clone();

    sweep_at(&sandbox, Utc::now());
    sweep_at(&sandbox, later(25));

    assert!(sandbox.clone.join("readme").exists());
    assert_eq!(git(&sandbox.clone, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&sandbox.clone, &["branch", "--show-current"]), "main");
}

#[test]
fn a_due_sweep_respects_pause_and_the_interval() {
    let sandbox = Sandbox::new();
    sandbox.list_clone();
    let ctx = Ctx::new(sandbox.paths.clone(), false);

    let first = sweep::sweep_due(&ctx, Utc::now()).unwrap();
    let too_soon = sweep::sweep_due(&ctx, later(0)).unwrap();
    sandbox.paths.set_paused(true).unwrap();
    let paused = sweep::sweep_due(&ctx, later(1)).unwrap();

    assert_eq!(first.len(), 1);
    assert!(too_soon.is_empty());
    assert!(paused.is_empty());
}
