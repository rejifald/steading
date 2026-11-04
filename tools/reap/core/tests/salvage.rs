//! Saving a merged worktree's uncommitted changes to a ref before removing it, end to end
//! against real repositories (ADR dirty-merged-worktrees-are-salvaged).

// The GitHub fakes of the sandbox are for the other test crates.
#[allow(dead_code)]
mod sandbox;

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use reap_core::classify::State;
use reap_core::ctx::Ctx;
use reap_core::report::Row;
use reap_core::salvage;
use reap_core::sweep::{self, Outcome};
use sandbox::{Sandbox, commit, git, make_executable};

const DAY: i64 = 24;

fn later(hours: i64) -> DateTime<Utc> {
    Utc::now() + chrono::Duration::hours(hours)
}

fn sweep_at(sandbox: &Sandbox, now: DateTime<Utc>) -> Outcome {
    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let mut outcomes = sweep::sweep(&ctx, None, Duration::ZERO, now).unwrap();
    assert_eq!(outcomes.len(), 1);
    outcomes.remove(0)
}

fn row<'a>(outcome: &'a Outcome, name: &str) -> &'a Row {
    outcome
        .report
        .worktrees
        .iter()
        .find(|row| row.name == name)
        .unwrap_or_else(|| panic!("no worktree {name} in {:?}", outcome.report.worktrees))
}

fn salvage_refs(sandbox: &Sandbox) -> Vec<String> {
    git(
        &sandbox.clone,
        &["for-each-ref", "--format=%(refname)", "refs/reap/salvage/"],
    )
    .lines()
    .map(str::to_string)
    .collect()
}

/// Prepends `line` to `repos.toml`, as a global setting.
fn set_global(sandbox: &Sandbox, line: &str) {
    let file = sandbox.paths.config_file();
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{line}\n{text}")).unwrap();
}

/// A linked worktree with a commit of its own that has merged into `origin/main`.
fn merged_worktree(sandbox: &Sandbox, name: &str) -> PathBuf {
    let worktree = sandbox.worktree(name);
    commit(&worktree, &format!("{name}.txt"));
    sandbox.merge_on_remote(name, "main");
    worktree
}

/// Sweeps past the dirty grace (the clock starts) and then past the removal grace.
fn sweep_past_both_graces(sandbox: &Sandbox) -> (Outcome, Outcome) {
    let started = sweep_at(sandbox, later(8 * DAY));
    let due = sweep_at(sandbox, later(9 * DAY + 2));
    (started, due)
}

/// One entry of a folder as a restore must reproduce it.
#[derive(Debug, PartialEq, Eq)]
enum Entry {
    File { bytes: Vec<u8>, is_executable: bool },
    Link(PathBuf),
    Folder,
}

/// Everything under `root` but `.git`, without following symlinks.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Entry> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<PathBuf, Entry>) {
        for item in std::fs::read_dir(dir).unwrap() {
            let path = item.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if relative == Path::new(".git") {
                continue;
            }
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            let entry = if metadata.file_type().is_symlink() {
                Entry::Link(std::fs::read_link(&path).unwrap())
            } else if metadata.is_dir() {
                walk(root, &path, into);
                Entry::Folder
            } else {
                Entry::File {
                    bytes: std::fs::read(&path).unwrap(),
                    is_executable: metadata.permissions().mode() & 0o111 != 0,
                }
            };
            into.insert(relative, entry);
        }
    }
    let mut entries = BTreeMap::new();
    walk(root, root, &mut entries);
    entries
}

/// Runs a restore command the way the operator would paste it, with `~` as the sandbox.
fn run_in_shell(sandbox: &Sandbox, line: &str) {
    let mut command = Command::new("sh");
    for name in reap_core::git::REPOSITORY_ENV_VARS {
        command.env_remove(name);
    }
    let output = command
        .args(["-c", line])
        .env("HOME", &sandbox.root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{line}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_restore_brings_back_the_worktree_byte_for_byte() {
    let sandbox = Sandbox::new();
    let shared = sandbox.root.join("shared-modules");
    std::fs::create_dir_all(shared.join("left-pad")).unwrap();
    std::fs::write(shared.join("left-pad/index.js"), "module.exports = 1;\n").unwrap();
    let worktree = sandbox.worktree("feat");
    std::fs::write(worktree.join(".gitignore"), "node_modules/\n*.log\n").unwrap();
    std::fs::write(worktree.join("tool.sh"), "#!/bin/sh\necho tool\n").unwrap();
    std::fs::write(worktree.join("doc.md"), "first\n").unwrap();
    std::fs::write(worktree.join("gone.txt"), "deleted later\n").unwrap();
    git(&worktree, &["add", "."]);
    git(&worktree, &["commit", "--quiet", "-m", "files"]);
    let head = git(&worktree, &["rev-parse", "HEAD"]);
    sandbox.merge_on_remote("feat", "main");
    // Every kind of change: edited, staged, deleted, a new exec bit, and untracked files,
    // folders, symlinks (one a symlinked node_modules) and binary bytes.
    std::fs::write(worktree.join("doc.md"), "first\nsecond\n").unwrap();
    std::fs::write(worktree.join("staged.txt"), "staged\n").unwrap();
    git(&worktree, &["add", "staged.txt"]);
    std::fs::remove_file(worktree.join("gone.txt")).unwrap();
    make_executable(&worktree.join("tool.sh"));
    std::fs::create_dir_all(worktree.join("notes/deep")).unwrap();
    std::fs::write(worktree.join("notes/deep/draft.md"), "a draft ADR\n").unwrap();
    std::fs::write(worktree.join("run.sh"), "#!/bin/sh\necho run\n").unwrap();
    make_executable(&worktree.join("run.sh"));
    std::os::unix::fs::symlink("doc.md", worktree.join("link")).unwrap();
    std::os::unix::fs::symlink("../shared-modules", worktree.join("node_modules")).unwrap();
    std::fs::write(worktree.join("blob.bin"), (0..=255u8).collect::<Vec<u8>>()).unwrap();
    std::fs::write(worktree.join("build.log"), "ignored, so dropped\n").unwrap();
    let mut before = snapshot(&worktree);
    sandbox.list_clone();

    let (started, due) = sweep_past_both_graces(&sandbox);

    assert_eq!(row(&started, "feat").state, State::Removable);
    assert_eq!(due.removed.len(), 1, "{:?}", due.refused);
    let refname = due.removed[0].salvaged_to.clone().unwrap();
    assert!(refname.starts_with("refs/reap/salvage/feat/"), "{refname}");
    assert!(!worktree.exists());
    assert_eq!(salvage_refs(&sandbox), std::slice::from_ref(&refname));
    let listed = &due.report.salvaged;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].refname, refname);
    assert_eq!(listed[0].worktree.as_deref(), Some("~/feat"));
    assert_eq!(listed[0].branch.as_deref(), Some("feat"));
    assert_eq!(
        listed[0].restore,
        format!("git -C ~/app worktree add ~/feat {refname} && git -C ~/feat reset HEAD~1")
    );

    run_in_shell(&sandbox, &listed[0].restore);

    before.remove(Path::new("build.log"));
    assert_eq!(snapshot(&worktree), before);
    assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), head);
    let status = git(&worktree, &["status", "--porcelain"]);
    assert!(
        status.contains("notes/") && status.contains("doc.md"),
        "{status}"
    );
}

#[test]
fn salvaged_lists_saves_made_by_hand_with_the_same_subject() {
    let sandbox = Sandbox::new();
    let head = git(&sandbox.clone, &["rev-parse", "HEAD"]);
    let tree = git(&sandbox.clone, &["rev-parse", "HEAD^{tree}"]);
    let commit = git(
        &sandbox.clone,
        &[
            "commit-tree",
            &tree,
            "-p",
            &head,
            "-m",
            "git-reap salvage: ~/app/.claude/worktrees/old (claude/old)",
        ],
    );
    git(
        &sandbox.clone,
        &[
            "update-ref",
            "refs/reap/salvage/old/20261103-231018",
            &commit,
        ],
    );
    sandbox.list_clone();

    let repos = salvage::salvaged(&sandbox.paths).unwrap();
    let text = salvage::render(&repos);

    assert_eq!(repos[0].salvaged.len(), 1);
    assert_eq!(
        repos[0].salvaged[0].worktree.as_deref(),
        Some("~/app/.claude/worktrees/old")
    );
    assert!(repos[0].salvaged[0].prunes_at.is_some());
    assert!(
        text.contains("restore: git -C ~/app worktree add ~/app/.claude/worktrees/old refs/reap/salvage/old/20261103-231018 && git -C ~/app/.claude/worktrees/old reset HEAD~1"),
        "{text}"
    );
}

#[test]
fn changes_wait_for_the_dirty_grace_counted_from_the_newest_file() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "paused");
    let notes = worktree.join("notes.md");
    std::fs::write(&notes, "half a thought\n").unwrap();
    sandbox.list_clone();

    let now = sweep_at(&sandbox, Utc::now());
    let six_days = sweep_at(&sandbox, later(6 * DAY));
    // The session came back on day five and touched the file.
    let touched = SystemTime::from(later(5 * DAY));
    std::fs::File::options()
        .append(true)
        .open(&notes)
        .unwrap()
        .set_modified(touched)
        .unwrap();
    let eight_days = sweep_at(&sandbox, later(8 * DAY));
    let twelve_days = sweep_at(&sandbox, later(12 * DAY + 1));
    let due = sweep_at(&sandbox, later(13 * DAY + 2));

    assert_eq!(row(&now, "paused").state, State::Dirty);
    assert!(
        row(&now, "paused")
            .reason
            .ends_with("last changed today; they are saved to a ref once unchanged for 7 days"),
        "{}",
        row(&now, "paused").reason
    );
    assert!(row(&now, "paused").saves_at.is_some());
    assert_eq!(row(&six_days, "paused").state, State::Dirty);
    assert_eq!(six_days.started.len(), 0);
    assert_eq!(row(&eight_days, "paused").state, State::Dirty);
    assert!(
        row(&eight_days, "paused")
            .reason
            .contains("last changed 3 days ago"),
        "{}",
        row(&eight_days, "paused").reason
    );
    assert_eq!(row(&twelve_days, "paused").state, State::Removable);
    assert_eq!(twelve_days.started.len(), 1);
    assert_eq!(due.removed.len(), 1);
    assert!(!worktree.exists());
}

#[test]
fn a_change_during_the_removal_grace_holds_it_again() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "back");
    let notes = worktree.join("notes.md");
    std::fs::write(&notes, "draft\n").unwrap();
    sandbox.list_clone();

    let started = sweep_at(&sandbox, later(8 * DAY));
    std::fs::write(&notes, "draft, continued\n").unwrap();
    std::fs::File::options()
        .append(true)
        .open(&notes)
        .unwrap()
        .set_modified(SystemTime::from(later(8 * DAY + 1)))
        .unwrap();
    let after = sweep_at(&sandbox, later(9 * DAY + 2));

    assert_eq!(started.started.len(), 1);
    assert_eq!(after.removed.len(), 0);
    assert_eq!(row(&after, "back").state, State::Dirty);
    assert!(worktree.exists());
    assert_eq!(salvage_refs(&sandbox).len(), 0);
}

/// Asserts that a guard kept the worktree, with its changes, and saved nothing.
fn assert_held(sandbox: &Sandbox, worktree: &Path, due: &Outcome, state: State, reason: &str) {
    let name = worktree.file_name().unwrap().to_str().unwrap();
    assert!(due.removed.is_empty(), "{:?}", due.removed);
    assert!(worktree.exists());
    assert_eq!(salvage_refs(sandbox).len(), 0);
    assert_eq!(row(due, name).state, state);
    assert!(
        row(due, name).reason.contains(reason),
        "{}",
        row(due, name).reason
    );
}

/// Starts a merge in `worktree` that conflicts on the file its own commit added.
fn conflict(sandbox: &Sandbox, worktree: &Path) {
    let file = format!("{}.txt", worktree.file_name().unwrap().to_str().unwrap());
    let other = sandbox.worktree("other");
    std::fs::write(other.join(&file), "theirs").unwrap();
    git(&other, &["add", &file]);
    git(&other, &["commit", "--quiet", "-m", "theirs"]);
    git(
        &sandbox.clone,
        &["worktree", "remove", other.to_str().unwrap()],
    );
    let mut command = Command::new("git");
    for name in reap_core::git::REPOSITORY_ENV_VARS {
        command.env_remove(name);
    }
    let output = command
        .arg("-C")
        .arg(worktree)
        .args(["merge", "other"])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(!output.status.success(), "the merge was meant to conflict");
}

#[test]
fn a_merge_in_progress_holds_it() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "merging");
    // A clean merge in progress: no conflict, nothing committed yet.
    let other = sandbox.worktree("side");
    commit(&other, "side.txt");
    git(
        &sandbox.clone,
        &["worktree", "remove", other.to_str().unwrap()],
    );
    git(
        &worktree,
        &["merge", "--quiet", "--no-commit", "--no-ff", "side"],
    );
    sandbox.list_clone();

    let (_, due) = sweep_past_both_graces(&sandbox);

    assert_held(
        &sandbox,
        &worktree,
        &due,
        State::Dirty,
        "a merge is in progress",
    );
}

#[test]
fn unmerged_paths_hold_it_with_no_operation_under_way() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "conflicted");
    conflict(&sandbox, &worktree);
    // Forget the merge but keep its conflicts, as a conflicted `stash pop` leaves them.
    git(&worktree, &["merge", "--quit"]);
    sandbox.list_clone();

    let (_, due) = sweep_past_both_graces(&sandbox);

    assert_held(
        &sandbox,
        &worktree,
        &due,
        State::Dirty,
        "has unmerged paths",
    );
}

#[test]
fn a_lock_holds_it() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "locked");
    std::fs::write(worktree.join("notes.md"), "wip").unwrap();
    git(
        &sandbox.clone,
        &[
            "worktree",
            "lock",
            "--reason",
            "agent",
            worktree.to_str().unwrap(),
        ],
    );
    sandbox.list_clone();

    let (_, due) = sweep_past_both_graces(&sandbox);

    assert_held(&sandbox, &worktree, &due, State::Locked, "locked: agent");
}

#[test]
fn a_process_inside_holds_it() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "busy");
    std::fs::write(worktree.join("notes.md"), "wip").unwrap();
    let mut process = Command::new("sleep")
        .arg("30")
        .current_dir(&worktree)
        .spawn()
        .unwrap();
    sandbox.list_clone();

    let (_, due) = sweep_past_both_graces(&sandbox);
    process.kill().unwrap();
    process.wait().unwrap();

    assert_held(
        &sandbox,
        &worktree,
        &due,
        State::InUse,
        "a process is running inside it",
    );
}

#[test]
fn an_untracked_repository_inside_holds_it() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "nested");
    let inner = worktree.join("vendor/lib");
    std::fs::create_dir_all(&inner).unwrap();
    git(&inner, &["init", "--quiet"]);
    std::fs::write(inner.join("only-copy.rs"), "fn main() {}").unwrap();
    sandbox.list_clone();

    let (_, due) = sweep_past_both_graces(&sandbox);

    assert_held(
        &sandbox,
        &worktree,
        &due,
        State::Dirty,
        "an untracked repository at vendor/lib/ would be lost",
    );
}

#[test]
fn untracked_files_over_the_cap_hold_it() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "big");
    std::fs::write(worktree.join("dataset.csv"), vec![b'x'; 2_000]).unwrap();
    sandbox.list_clone();
    set_global(&sandbox, "salvage_max_untracked = \"1kB\"");

    let (_, due) = sweep_past_both_graces(&sandbox);

    assert_held(
        &sandbox,
        &worktree,
        &due,
        State::Dirty,
        "its untracked files take 2.0 kB, over the 1.0 kB a save may copy",
    );
}

#[test]
fn a_save_that_fails_keeps_the_worktree_and_is_shown_as_refused() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "taken");
    std::fs::write(worktree.join("notes.md"), "wip").unwrap();
    sandbox.list_clone();
    sweep_at(&sandbox, later(8 * DAY));
    // A ref already at the name this save would take: `update-ref` must not overwrite it.
    let when = later(9 * DAY + 2);
    let stamp = when.with_timezone(&chrono::Local).format("%Y%m%d-%H%M%S");
    let taken = format!("refs/reap/salvage/taken/{stamp}");
    git(&sandbox.clone, &["update-ref", &taken, "HEAD"]);

    let due = sweep_at(&sandbox, when);

    assert_eq!(due.removed.len(), 0);
    assert!(worktree.join("notes.md").exists());
    assert_eq!(due.refused.len(), 1);
    assert!(
        due.refused[0]
            .message
            .starts_with("saving its changes failed"),
        "{}",
        due.refused[0].message
    );
    assert_eq!(row(&due, "taken").state, State::Refused);
}

#[test]
fn a_dry_run_shows_the_save_and_changes_nothing() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "dry");
    std::fs::write(worktree.join("notes.md"), "wip").unwrap();
    sandbox.list_clone();
    sweep_at(&sandbox, later(8 * DAY));

    let ctx = Ctx::new(sandbox.paths.clone(), true);
    let outcome = sweep::sweep(&ctx, None, Duration::ZERO, later(9 * DAY + 2))
        .unwrap()
        .remove(0);

    assert!(worktree.join("notes.md").exists());
    assert_eq!(salvage_refs(&sandbox).len(), 0);
    for command in [
        "add -A",
        "commit-tree",
        "update-ref",
        "worktree remove --force",
    ] {
        assert!(
            outcome
                .log
                .iter()
                .any(|line| line.starts_with("would run:") && line.contains(command)),
            "{command}: {:?}",
            outcome.log
        );
    }
}

#[test]
fn a_save_is_deleted_after_salvage_max_age() {
    let sandbox = Sandbox::new();
    let worktree = merged_worktree(&sandbox, "aging");
    std::fs::write(worktree.join("notes.md"), "wip").unwrap();
    sandbox.list_clone();
    let (_, due) = sweep_past_both_graces(&sandbox);
    let refname = due.removed[0].salvaged_to.clone().unwrap();

    let day_29 = sweep_at(&sandbox, later(38 * DAY));
    let day_31 = sweep_at(&sandbox, later(40 * DAY + 3));

    assert_eq!(day_29.pruned_salvage.len(), 0);
    assert_eq!(day_29.report.salvaged.len(), 1);
    assert_eq!(day_31.pruned_salvage, [refname]);
    assert_eq!(salvage_refs(&sandbox).len(), 0);
    assert_eq!(day_31.report.salvaged.len(), 0);
}
