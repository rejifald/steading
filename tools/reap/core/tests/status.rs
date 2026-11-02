//! `git reap status` end to end against real repositories (rules 2 to 6, 9, 13).

mod sandbox;

use chrono::Utc;
use reap_core::classify::State;
use reap_core::report::{self, Overall, RepoReport, Row};
use sandbox::{Sandbox, commit, git};

fn status(sandbox: &Sandbox) -> RepoReport {
    let mut report = report::report(&sandbox.paths, Utc::now()).unwrap();
    assert_eq!(report.repos.len(), 1);
    report.repos.remove(0)
}

fn row<'a>(repo: &'a RepoReport, name: &str) -> &'a Row {
    repo.worktrees
        .iter()
        .find(|row| row.name == name)
        .unwrap_or_else(|| panic!("no worktree {name} in {:?}", repo.worktrees))
}

#[test]
fn lists_linked_worktrees_but_never_the_main_clone() {
    let sandbox = Sandbox::new();
    sandbox.worktree("feat");
    sandbox.list_clone();

    let repo = status(&sandbox);

    assert_eq!(repo.base, "origin/main");
    let names: Vec<_> = repo.worktrees.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["feat"]);
}

#[test]
fn a_worktree_merged_into_the_base_is_removable_and_unmerged_work_is_open() {
    let sandbox = Sandbox::new();
    let merged = sandbox.worktree("merged");
    let open = sandbox.worktree("open");
    commit(&merged, "done");
    commit(&open, "wip");
    sandbox.merge_on_remote("merged", "main");
    sandbox.list_clone();

    let repo = status(&sandbox);

    assert_eq!(row(&repo, "merged").state, State::Removable);
    assert_eq!(row(&repo, "merged").reason, "merged into origin/main");
    assert_eq!(row(&repo, "open").state, State::Open);
    assert_eq!(row(&repo, "open").reason, "not merged into origin/main");
}

#[test]
fn changes_a_lock_or_a_process_keep_a_merged_worktree() {
    let sandbox = Sandbox::new();
    let dirty = sandbox.worktree("dirty");
    let untracked = sandbox.worktree("untracked");
    let locked = sandbox.worktree("locked");
    let busy = sandbox.worktree("busy");
    std::fs::write(dirty.join("readme"), "edited").unwrap();
    std::fs::write(untracked.join("notes"), "new").unwrap();
    git(
        &sandbox.clone,
        &[
            "worktree",
            "lock",
            "--reason",
            "on usb",
            locked.to_str().unwrap(),
        ],
    );
    let mut process = std::process::Command::new("sleep")
        .arg("30")
        .current_dir(&busy)
        .spawn()
        .unwrap();
    sandbox.list_clone();

    let repo = status(&sandbox);
    process.kill().unwrap();
    process.wait().unwrap();

    assert_eq!(row(&repo, "dirty").state, State::Dirty);
    assert_eq!(row(&repo, "untracked").state, State::Dirty);
    assert_eq!(
        row(&repo, "locked").reason,
        "merged into origin/main, but locked: on usb"
    );
    assert_eq!(row(&repo, "busy").state, State::InUse);
}

#[test]
fn a_worktree_whose_folder_is_gone_is_missing() {
    let sandbox = Sandbox::new();
    let gone = sandbox.worktree("gone");
    std::fs::remove_dir_all(&gone).unwrap();
    sandbox.list_clone();

    let repo = status(&sandbox);

    assert_eq!(row(&repo, "gone").state, State::Missing);
}

#[test]
fn a_squash_merged_pull_request_at_head_counts_as_merged() {
    let mut sandbox = Sandbox::new();
    let squashed = sandbox.worktree("squashed");
    let moved_on = sandbox.worktree("moved-on");
    let squashed_head = commit(&squashed, "squashed");
    let merged_head = commit(&moved_on, "first");
    commit(&moved_on, "after the merge");
    sandbox.use_github_url();
    sandbox.fake_gh(&format!(
        r#"[{{"number":12,"state":"MERGED","headRefName":"squashed","headRefOid":"{squashed_head}","baseRefName":"main","url":"https://github.com/acme/app/pull/12"}},
            {{"number":13,"state":"MERGED","headRefName":"moved-on","headRefOid":"{merged_head}","baseRefName":"main","url":"https://github.com/acme/app/pull/13"}}]"#
    ));
    sandbox.list_clone();

    let repo = status(&sandbox);

    assert_eq!(repo.gh_note, None);
    assert_eq!(row(&repo, "squashed").state, State::Removable);
    assert_eq!(row(&repo, "squashed").reason, "pull request #12 merged");
    assert_eq!(row(&repo, "squashed").pr.as_ref().unwrap().number, 12);
    // Commits after the merged head are not in the pull request: the worktree stays.
    assert_eq!(row(&repo, "moved-on").state, State::Open);
}

/// A worktree on a branch with two commits, reset back to the first: the clone holds the
/// second, as it does after a push someone else built on. Returns (HEAD, the later head).
fn behind_its_pull_request(sandbox: &Sandbox, name: &str) -> (String, String) {
    let worktree = sandbox.worktree(name);
    let head = commit(&worktree, "first");
    let later = commit(&worktree, "pushed by a fixer");
    git(&worktree, &["reset", "--quiet", "--hard", &head]);
    (head, later)
}

#[test]
fn a_pull_request_merged_after_more_commits_still_counts_as_merged() {
    let mut sandbox = Sandbox::new();
    let (_, later) = behind_its_pull_request(&sandbox, "behind");
    sandbox.use_github_url();
    sandbox.fake_gh_routes(&[(
        "pr*",
        &format!(
            r#"[{{"number":14,"state":"MERGED","headRefName":"behind","headRefOid":"{later}","baseRefName":"main","url":"u"}}]"#
        ),
    )]);
    sandbox.list_clone();

    let repo = status(&sandbox);

    assert_eq!(row(&repo, "behind").state, State::Removable);
    assert_eq!(row(&repo, "behind").reason, "pull request #14 merged");
}

#[test]
fn a_merged_head_the_clone_lacks_is_asked_of_github() {
    const ABSENT: &str = "1111111111111111111111111111111111111111";
    let mut sandbox = Sandbox::new();
    let contained = sandbox.worktree("contained");
    let diverged = sandbox.worktree("diverged");
    let contained_head = commit(&contained, "contained");
    let diverged_head = commit(&diverged, "diverged");
    sandbox.use_github_url();
    sandbox.fake_gh_routes(&[
        (
            "pr*",
            &format!(
                r#"[{{"number":15,"state":"MERGED","headRefName":"contained","headRefOid":"{ABSENT}","baseRefName":"main","url":"u"}},
                    {{"number":16,"state":"MERGED","headRefName":"diverged","headRefOid":"{ABSENT}","baseRefName":"main","url":"u"}}]"#
            ),
        ),
        (&format!("*compare/{contained_head}...{ABSENT}*"), "ahead\n"),
        (&format!("*compare/{diverged_head}...{ABSENT}*"), "diverged\n"),
    ]);
    sandbox.list_clone();

    let repo = status(&sandbox);

    assert_eq!(row(&repo, "contained").state, State::Removable);
    assert_eq!(row(&repo, "diverged").state, State::Open);
}

#[test]
fn a_pull_request_older_than_the_list_is_looked_up_by_branch() {
    let mut sandbox = Sandbox::new();
    let old = sandbox.worktree("old");
    let head = commit(&old, "old");
    sandbox.use_github_url();
    sandbox.fake_gh_routes(&[
        ("pr*", "[]"),
        (
            "api*graphql*old*",
            &format!(
                r#"{{"data":{{"repository":{{"b0":{{"nodes":[{{"number":3,"state":"MERGED","headRefName":"old","headRefOid":"{head}","baseRefName":"main","url":"u"}}]}}}}}}}}"#
            ),
        ),
    ]);
    sandbox.list_clone();

    let repo = status(&sandbox);

    assert_eq!(row(&repo, "old").state, State::Removable);
    assert_eq!(row(&repo, "old").pr.as_ref().unwrap().number, 3);
}

#[test]
fn a_worktree_merged_into_its_pull_requests_target_counts_as_merged() {
    let mut sandbox = Sandbox::new();
    let base = sandbox.worktree("feat-base");
    commit(&base, "base");
    git(&sandbox.clone, &["push", "--quiet", "origin", "feat-base"]);
    let top = sandbox.root.join("feat-top");
    git(
        &sandbox.clone,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "feat-top",
            top.to_str().unwrap(),
            "feat-base",
        ],
    );
    let top_head = commit(&top, "top");
    sandbox.merge_on_remote("feat-top", "feat-base");
    sandbox.use_github_url();
    sandbox.fake_gh(&format!(
        r#"[{{"number":21,"state":"MERGED","headRefName":"feat-top","headRefOid":"0000000000000000000000000000000000000000","baseRefName":"feat-base","url":"u"}},
            {{"number":20,"state":"OPEN","headRefName":"feat-base","headRefOid":"{top_head}","baseRefName":"main","url":"u"}}]"#
    ));
    sandbox.list_clone();

    let repo = status(&sandbox);

    assert_eq!(row(&repo, "feat-top").state, State::Removable);
    assert_eq!(
        row(&repo, "feat-top").reason,
        "merged into origin/feat-base, the target of pull request #21"
    );
    // The bottom of the stack has the top merged into it, but is not merged itself.
    assert_eq!(row(&repo, "feat-base").state, State::Open);
    assert_eq!(row(&repo, "feat-base").reason, "pull request #20 is open");
}

#[test]
fn a_pull_request_closed_without_merging_is_stale() {
    let mut sandbox = Sandbox::new();
    let abandoned = sandbox.worktree("abandoned");
    let head = commit(&abandoned, "idea");
    sandbox.use_github_url();
    sandbox.fake_gh(&format!(
        r#"[{{"number":5,"state":"CLOSED","headRefName":"abandoned","headRefOid":"{head}","baseRefName":"main","url":"u"}}]"#
    ));
    sandbox.list_clone();

    let report = report::report(&sandbox.paths, Utc::now()).unwrap();

    assert_eq!(report.overall, Overall::NeedsYou);
    let repo = &report.repos[0];
    assert_eq!(row(repo, "abandoned").state, State::Stale);
    assert_eq!(
        row(repo, "abandoned").reason,
        "pull request #5 closed without merging"
    );
}

#[test]
fn without_gh_only_ancestry_merges_are_seen_and_the_report_says_why() {
    let sandbox = Sandbox::new();
    let squashed = sandbox.worktree("squashed");
    commit(&squashed, "squashed");
    sandbox.use_github_url();
    sandbox.list_clone();

    let repo = status(&sandbox);

    assert_eq!(
        repo.gh_note.as_deref(),
        Some("gh is not installed: only merges into origin/main are seen")
    );
    assert_eq!(row(&repo, "squashed").state, State::Open);
}

#[test]
fn a_remote_that_is_not_on_github_skips_gh() {
    let sandbox = Sandbox::new();
    sandbox.list_clone();

    let repo = status(&sandbox);

    assert_eq!(
        repo.gh_note.as_deref(),
        Some("origin is not on GitHub: only merges into origin/main are seen")
    );
}

#[test]
fn a_repo_that_cannot_be_read_needs_you_without_failing_the_report() {
    let sandbox = Sandbox::new();
    sandbox.list_clone();
    git(
        &sandbox.clone,
        &["remote", "set-head", "origin", "--delete"],
    );

    let report = report::report(&sandbox.paths, Utc::now()).unwrap();

    assert_eq!(report.overall, Overall::NeedsYou);
    let error = report.repos[0].error.as_deref().unwrap();
    assert!(error.contains("set `base` in repos.toml"), "{error}");
}

#[test]
fn add_refuses_a_linked_worktree_and_a_subfolder() {
    let sandbox = Sandbox::new();
    let feat = sandbox.worktree("feat");
    std::fs::create_dir(sandbox.clone.join("src")).unwrap();

    let worktree = reap_core::add::add(&sandbox.paths, &feat, None, None).unwrap_err();
    let subfolder =
        reap_core::add::add(&sandbox.paths, &sandbox.clone.join("src"), None, None).unwrap_err();

    assert!(
        worktree.to_string().contains("is a linked worktree"),
        "{worktree}"
    );
    assert!(
        subfolder.to_string().contains("is inside a clone"),
        "{subfolder}"
    );
}

#[test]
fn a_row_carries_the_disk_its_folder_takes() {
    let sandbox = Sandbox::new();
    let big = sandbox.worktree("big");
    sandbox.worktree("small");
    std::fs::write(big.join("blob"), vec![b'x'; 1024 * 1024]).unwrap();
    sandbox.list_clone();

    let repo = status(&sandbox);

    let big_bytes = row(&repo, "big").size_bytes.unwrap();
    let small_bytes = row(&repo, "small").size_bytes.unwrap();
    assert!(
        big_bytes >= small_bytes + 1024 * 1024,
        "{big_bytes} vs {small_bytes}"
    );
}

#[test]
fn a_stale_worktree_is_removable_only_when_every_commit_is_pushed() {
    let sandbox = Sandbox::new();
    let pushed = sandbox.worktree("pushed");
    let local = sandbox.worktree("local");
    let dirty = sandbox.worktree("dirty");
    commit(&pushed, "pushed");
    commit(&local, "local");
    commit(&dirty, "dirty");
    git(
        &sandbox.clone,
        &["push", "--quiet", "origin", "pushed", "dirty"],
    );
    std::fs::write(dirty.join("dirty"), "changed").unwrap();
    sandbox.list_clone();

    let in_three_weeks = Utc::now() + chrono::Duration::days(21);
    let mut report = report::report(&sandbox.paths, in_three_weeks).unwrap();
    let repo = report.repos.remove(0);

    assert_eq!(row(&repo, "pushed").state, State::Removable);
    assert!(
        row(&repo, "pushed")
            .reason
            .ends_with("; every commit is on origin, so its branch is kept"),
        "{}",
        row(&repo, "pushed").reason
    );
    assert_eq!(row(&repo, "local").state, State::Stale);
    assert_eq!(row(&repo, "dirty").state, State::Stale);
}
