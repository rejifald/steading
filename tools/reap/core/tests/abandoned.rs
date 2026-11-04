//! ADR 0006 end to end against real repositories: merged work found by HEAD's commit (rule
//! 3(d)), and abandoned worktrees that are offered, never swept, and saved to a ref on request.

// The merge helpers and the plain `gh` fake are for the other test crates.
#[allow(dead_code)]
mod sandbox;

use std::time::Duration;

use chrono::{DateTime, Utc};
use reap_core::classify::State;
use reap_core::ctx::Ctx;
use reap_core::report::{self, RepoReport, Row};
use reap_core::salvage::{self, Kind};
use reap_core::sweep::{self, Outcome};
use sandbox::{Sandbox, commit, git};

const DAY: i64 = 24;

fn later(hours: i64) -> DateTime<Utc> {
    Utc::now() + chrono::Duration::hours(hours)
}

fn status_at(sandbox: &Sandbox, now: DateTime<Utc>) -> RepoReport {
    let mut report = report::report(&sandbox.paths, now).unwrap();
    assert_eq!(report.repos.len(), 1);
    report.repos.remove(0)
}

fn sweep_at(sandbox: &Sandbox, now: DateTime<Utc>) -> Outcome {
    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let mut outcomes = sweep::sweep(&ctx, None, Duration::ZERO, now).unwrap();
    assert_eq!(outcomes.len(), 1);
    outcomes.remove(0)
}

fn row<'a>(rows: &'a [Row], name: &str) -> &'a Row {
    rows.iter()
        .find(|row| row.name == name)
        .unwrap_or_else(|| panic!("no worktree {name} in {rows:?}"))
}

fn set_global(sandbox: &Sandbox, line: &str) {
    let file = sandbox.paths.config_file();
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{line}\n{text}")).unwrap();
}

fn branch_exists(sandbox: &Sandbox, branch: &str) -> bool {
    !git(&sandbox.clone, &["branch", "--list", branch]).is_empty()
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

/// A pull request as GraphQL spells it.
fn node(number: u64, branch: &str, head: &str) -> String {
    format!(
        r#"{{"number":{number},"state":"MERGED","headRefName":"{branch}","headRefOid":"{head}","baseRefName":"main","url":"https://github.com/acme/app/pull/{number}"}}"#
    )
}

/// The answer to an `associatedPullRequests` lookup: `c<i>` per commit in the order reap asks
/// (sorted), `null` for a commit GitHub does not have.
fn associated(commits: &[(&str, Option<&[String]>)]) -> String {
    let mut commits = commits.to_vec();
    commits.sort_by_key(|(sha, _)| *sha);
    let fields: Vec<String> = commits
        .iter()
        .enumerate()
        .map(|(index, (_, nodes))| match nodes {
            None => format!(r#""c{index}":null"#),
            Some(nodes) => format!(
                r#""c{index}":{{"associatedPullRequests":{{"nodes":[{}]}}}}"#,
                nodes.join(",")
            ),
        })
        .collect();
    format!(r#"{{"data":{{"repository":{{{}}}}}}}"#, fields.join(","))
}

/// A fake `gh` that lists no pull request by branch and answers the commit lookup.
fn github(sandbox: &mut Sandbox, answer: &str, extra: &[(&str, &str)]) {
    sandbox.use_github_url();
    let mut routes = vec![
        ("pr*", "[]"),
        ("*associatedPullRequests*", answer),
        ("*graphql*", r#"{"data":{"repository":{}}}"#),
    ];
    routes.extend_from_slice(extra);
    sandbox.fake_gh_routes(&routes);
}

// Rule 3(d): merged, found by commit.

#[test]
fn a_detached_worktree_in_a_merged_pull_request_is_removed() {
    let mut sandbox = Sandbox::new();
    let branch = sandbox.worktree("fixer");
    let head = commit(&branch, "fix");
    git(&sandbox.clone, &["push", "--quiet", "origin", "fixer"]);
    let detached = sandbox.detached("review-fix", &head);
    git(
        &sandbox.clone,
        &["worktree", "remove", branch.to_str().unwrap()],
    );
    let nodes = [node(212, "fixer", &head)];
    github(&mut sandbox, &associated(&[(&head, Some(&nodes))]), &[]);
    sandbox.list_clone();

    let first = sweep_at(&sandbox, Utc::now());
    // Weeks later it is still merged, never abandoned: 3(d) is decided before staleness.
    let month = status_at(&sandbox, later(30 * DAY));
    let due = sweep_at(&sandbox, later(25));

    let found = row(&first.report.worktrees, "review-fix");
    assert_eq!(found.state, State::Removable);
    assert_eq!(found.reason, "in #212, merged");
    let pull_request = found.pr.as_ref().unwrap();
    assert_eq!((pull_request.number, pull_request.by_commit), (212, true));
    assert_eq!(row(&month.worktrees, "review-fix").state, State::Removable);
    assert_eq!(due.removed.len(), 1, "{:?}", due.refused);
    assert_eq!(due.removed[0].branch, None);
    assert!(!detached.exists());
}

#[test]
fn a_branch_merged_through_another_pull_request_is_removed_with_its_branch() {
    let mut sandbox = Sandbox::new();
    let issue = sandbox.worktree("issue-7");
    let head = commit(&issue, "issue");
    // A batch branch takes the issue branch and more, and its own pull request merges.
    let batch = sandbox.root.join("batch");
    git(
        &sandbox.clone,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "batch",
            batch.to_str().unwrap(),
            "issue-7",
        ],
    );
    let batch_head = commit(&batch, "batch");
    git(
        &sandbox.clone,
        &["worktree", "remove", batch.to_str().unwrap()],
    );
    let nodes = [node(300, "batch", &batch_head)];
    github(&mut sandbox, &associated(&[(&head, Some(&nodes))]), &[]);
    sandbox.list_clone();

    let first = sweep_at(&sandbox, Utc::now());
    let due = sweep_at(&sandbox, later(25));

    assert_eq!(
        row(&first.report.worktrees, "issue-7").reason,
        "in #300, merged"
    );
    assert_eq!(due.removed.len(), 1, "{:?}", due.refused);
    // The merged pull request holds every commit on the branch: `-D` (ADR 0002 rule 2).
    assert_eq!(due.removed[0].branch_kept, None);
    assert!(!issue.exists());
    assert!(!branch_exists(&sandbox, "issue-7"));
}

#[test]
fn a_commit_github_does_not_have_stays_unmerged() {
    let mut sandbox = Sandbox::new();
    let local = sandbox.worktree("local-only");
    let head = commit(&local, "local");
    github(&mut sandbox, &associated(&[(&head, None)]), &[]);
    sandbox.list_clone();

    let repo = status_at(&sandbox, Utc::now());

    assert_eq!(row(&repo.worktrees, "local-only").state, State::Open);
    assert!(row(&repo.worktrees, "local-only").pr.is_none());
}

#[test]
fn a_merged_pull_request_whose_head_lacks_head_does_not_count() {
    const ABSENT: &str = "2222222222222222222222222222222222222222";
    let mut sandbox = Sandbox::new();
    let ahead = sandbox.worktree("ahead");
    let diverged = sandbox.worktree("diverged");
    let merged_head = commit(&ahead, "merged");
    let ahead_head = commit(&ahead, "after the merge");
    let diverged_head = commit(&diverged, "diverged");
    let ahead_nodes = [node(400, "other", &merged_head)];
    let diverged_nodes = [node(401, "other", ABSENT)];
    github(
        &mut sandbox,
        &associated(&[
            (&ahead_head, Some(&ahead_nodes)),
            (&diverged_head, Some(&diverged_nodes)),
        ]),
        &[(
            &format!("*compare/{diverged_head}...{ABSENT}*"),
            "diverged\n",
        )],
    );
    sandbox.list_clone();

    let repo = status_at(&sandbox, Utc::now());

    // HEAD has a commit the merged head lacks, in the clone and by the compare API.
    assert_eq!(row(&repo.worktrees, "ahead").state, State::Open);
    assert_eq!(row(&repo.worktrees, "diverged").state, State::Open);
    assert!(row(&repo.worktrees, "ahead").pr.is_none());
}

// Abandoned: offered, never swept.

#[test]
fn an_abandoned_worktree_is_never_removed_by_a_sweep_under_any_policy() {
    for policy in ["auto", "ask", "off"] {
        let sandbox = Sandbox::new();
        let idle = sandbox.worktree("idle");
        commit(&idle, "never pushed");
        std::fs::write(idle.join("notes.md"), "half a thought").unwrap();
        sandbox.list_clone();
        set_global(&sandbox, &format!("policy = \"{policy}\""));

        let stale = sweep_at(&sandbox, later(21 * DAY));
        let much_later = sweep_at(&sandbox, later(90 * DAY));

        for outcome in [&stale, &much_later] {
            assert!(
                outcome.removed.is_empty(),
                "{policy}: {:?}",
                outcome.removed
            );
            assert!(outcome.started.is_empty(), "{policy}: no grace clock");
            assert_eq!(
                row(&outcome.report.worktrees, "idle").state,
                State::Abandoned,
                "{policy}"
            );
        }
        assert!(idle.join("notes.md").exists(), "{policy}");
        assert!(salvage_refs(&sandbox).is_empty(), "{policy}");
        let report = report::report(&sandbox.paths, later(21 * DAY)).unwrap();
        assert_eq!(report.needs_you, 1, "{policy}");
    }
}

#[test]
fn a_stale_row_names_what_is_local_and_what_holds_it() {
    let mut sandbox = Sandbox::new();
    let idle = sandbox.worktree("idle");
    commit(&idle, "one");
    commit(&idle, "two");
    std::fs::write(idle.join("one"), "edited").unwrap();
    std::fs::write(idle.join("draft.md"), "new").unwrap();
    let reviewing = sandbox.worktree("reviewing");
    let reviewing_head = commit(&reviewing, "review");
    git(&sandbox.clone, &["push", "--quiet", "origin", "reviewing"]);
    let side = sandbox.worktree("side");
    commit(&side, "side.txt");
    git(
        &sandbox.clone,
        &["worktree", "remove", side.to_str().unwrap()],
    );
    git(
        &reviewing,
        &["merge", "--quiet", "--no-commit", "--no-ff", "side"],
    );
    sandbox.use_github_url();
    sandbox.fake_gh_routes(&[
        (
            "pr*",
            &format!(
                r#"[{{"number":15,"state":"OPEN","headRefName":"reviewing","headRefOid":"{reviewing_head}","baseRefName":"main","url":"u"}}]"#
            ),
        ),
        ("*graphql*", r#"{"data":{"repository":{}}}"#),
    ]);
    sandbox.list_clone();

    let repo = status_at(&sandbox, later(21 * DAY));

    let idle = row(&repo.worktrees, "idle");
    assert_eq!(idle.state, State::Abandoned);
    assert_eq!(
        idle.reason,
        "no commit or checkout for 21 days · 2 uncommitted changes · 2 commits on no remote"
    );
    assert_eq!(idle.held_by, None);
    let held = row(&repo.worktrees, "reviewing");
    assert_eq!(held.state, State::Stale);
    assert_eq!(
        held.reason,
        "no commit or checkout for 21 days · 1 uncommitted change · pull request #15 open · a \
         merge is in progress"
    );
    assert_eq!(held.held_by.as_deref(), Some("a merge is in progress"));
}

#[test]
fn the_guards_hold_an_abandoned_worktree_and_salvage_refuses_it() {
    let sandbox = Sandbox::new();
    let locked = sandbox.worktree("locked");
    let busy = sandbox.worktree("busy");
    let nested = sandbox.worktree("nested");
    let big = sandbox.worktree("big");
    for worktree in [&locked, &busy, &nested, &big] {
        commit(worktree, "local");
    }
    git(
        &sandbox.clone,
        &[
            "worktree",
            "lock",
            "--reason",
            "agent",
            locked.to_str().unwrap(),
        ],
    );
    let mut process = std::process::Command::new("sleep")
        .arg("30")
        .current_dir(&busy)
        .spawn()
        .unwrap();
    let inner = nested.join("vendor/lib");
    std::fs::create_dir_all(&inner).unwrap();
    git(&inner, &["init", "--quiet"]);
    std::fs::write(inner.join("only-copy.rs"), "fn main() {}").unwrap();
    std::fs::write(big.join("dataset.csv"), vec![b'x'; 2_000]).unwrap();
    sandbox.list_clone();
    set_global(&sandbox, "salvage_max_untracked = \"1kB\"");
    let when = later(21 * DAY);

    let repo = status_at(&sandbox, when);
    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let refusals: Vec<String> = [&locked, &busy, &nested, &big]
        .iter()
        .map(|worktree| {
            sweep::salvage_now(&ctx, worktree.to_str().unwrap(), when)
                .unwrap_err()
                .to_string()
        })
        .collect();
    process.kill().unwrap();
    process.wait().unwrap();

    for (name, guard) in [
        ("locked", "locked: agent"),
        ("busy", "a process is running inside it"),
        (
            "nested",
            "an untracked repository at vendor/lib/ would be lost",
        ),
        (
            "big",
            "its untracked files take 2.0 kB, over the 1.0 kB a save may copy",
        ),
    ] {
        let held = row(&repo.worktrees, name);
        assert_eq!(held.state, State::Stale, "{name}");
        assert_eq!(held.held_by.as_deref(), Some(guard), "{name}");
        assert!(
            held.reason.ends_with(&format!(" · {guard}")),
            "{}",
            held.reason
        );
    }
    for message in &refusals {
        assert!(message.contains("is not abandoned"), "{message}");
    }
    assert!(locked.exists() && busy.exists() && nested.exists() && big.exists());
    assert_eq!(salvage_refs(&sandbox), Vec::<String>::new());
}

// Save and remove.

/// The `Reap-Reason` trailer of a commit.
fn reason_of(sandbox: &Sandbox, refname: &str) -> String {
    git(
        &sandbox.clone,
        &[
            "log",
            "-1",
            "--format=%(trailers:key=Reap-Reason,valueonly)",
            refname,
        ],
    )
}

#[test]
fn saving_a_dirty_abandoned_worktree_keeps_every_change_and_the_branch() {
    let sandbox = Sandbox::new();
    let idle = sandbox.worktree("idle");
    let head = commit(&idle, "never pushed");
    std::fs::write(idle.join("never pushed"), "edited").unwrap();
    std::fs::write(idle.join("draft.md"), "a draft ADR").unwrap();
    sandbox.list_clone();
    let when = later(21 * DAY);

    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let removed = sweep::salvage_now(&ctx, idle.to_str().unwrap(), when).unwrap();

    let refname = removed.salvaged_to.unwrap();
    assert!(refname.starts_with("refs/reap/salvage/idle/"), "{refname}");
    assert!(!idle.exists());
    assert!(branch_exists(&sandbox, "idle"));
    assert_eq!(removed.branch_kept.as_deref(), Some("not merged"));
    assert_eq!(
        git(&sandbox.clone, &["rev-parse", &format!("{refname}^")]),
        head
    );
    assert_eq!(
        git(&sandbox.clone, &["show", &format!("{refname}:draft.md")]),
        "a draft ADR"
    );
    assert_eq!(
        reason_of(&sandbox, &refname),
        "abandoned (no commit or checkout for 21 days)"
    );
    let listed = salvage::salvaged(&sandbox.paths)
        .unwrap()
        .remove(0)
        .salvaged;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].kind, Kind::Abandoned);
    assert_eq!(listed[0].prunes_at, None);
    assert_eq!(
        listed[0].reason.as_deref(),
        Some("abandoned (no commit or checkout for 21 days)")
    );
}

#[test]
fn a_clean_unpushed_worktree_is_saved_with_heads_tree() {
    let sandbox = Sandbox::new();
    let idle = sandbox.worktree("idle");
    let head = commit(&idle, "never pushed");
    sandbox.list_clone();
    let when = later(21 * DAY);

    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let removed = sweep::salvage_now(&ctx, idle.to_str().unwrap(), when).unwrap();

    let refname = removed.salvaged_to.unwrap();
    assert!(!idle.exists());
    assert_eq!(
        git(
            &sandbox.clone,
            &["rev-parse", &format!("{refname}^{{tree}}")]
        ),
        git(&sandbox.clone, &["rev-parse", &format!("{head}^{{tree}}")])
    );
    assert_eq!(
        git(&sandbox.clone, &["rev-parse", &format!("{refname}^")]),
        head
    );
    assert!(branch_exists(&sandbox, "idle"));
}

#[test]
fn a_detached_abandoned_worktree_is_saved_and_its_commits_kept() {
    let sandbox = Sandbox::new();
    let main = git(&sandbox.clone, &["rev-parse", "HEAD"]);
    let probe = sandbox.detached("probe", &main);
    let head = commit(&probe, "experiment");
    sandbox.list_clone();
    let when = later(21 * DAY);

    let repo = status_at(&sandbox, when);
    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let removed = sweep::salvage_now(&ctx, probe.to_str().unwrap(), when).unwrap();

    assert_eq!(
        row(&repo.worktrees, "probe").reason,
        "no commit or checkout for 21 days · 1 commit on no remote · a detached HEAD"
    );
    let refname = removed.salvaged_to.unwrap();
    assert!(!probe.exists());
    assert_eq!(removed.branch, None);
    assert_eq!(
        git(&sandbox.clone, &["rev-parse", &format!("{refname}^")]),
        head
    );
    assert!(reason_of(&sandbox, &refname).starts_with("abandoned ("));
}

#[test]
fn salvage_refuses_open_work_and_a_dry_run_changes_nothing() {
    let sandbox = Sandbox::new();
    let recent = sandbox.worktree("recent");
    commit(&recent, "today");
    sandbox.list_clone();
    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let state_before = std::fs::read(sandbox.paths.state_file()).ok();

    let open = sweep::salvage_now(&ctx, recent.to_str().unwrap(), Utc::now()).unwrap_err();
    let dry = Ctx::new(sandbox.paths.clone(), true);
    let would = sweep::salvage_now(&dry, recent.to_str().unwrap(), later(21 * DAY)).unwrap();
    let log = dry.take_log();

    assert!(open.to_string().contains("is not abandoned"), "{open}");
    assert!(recent.exists());
    assert_eq!(salvage_refs(&sandbox), Vec::<String>::new());
    assert_eq!(std::fs::read(sandbox.paths.state_file()).ok(), state_before);
    assert!(
        would
            .salvaged_to
            .unwrap()
            .starts_with("refs/reap/salvage/recent/")
    );
    for command in [
        "add -A",
        "commit-tree",
        "Reap-Reason: abandoned",
        "update-ref",
        "worktree remove --force",
    ] {
        assert!(
            log.iter()
                .any(|line| line.starts_with("would run:") && line.contains(command)),
            "{command}: {log:?}"
        );
    }
}

#[test]
fn an_abandoned_save_outlives_salvage_max_age_until_it_is_deleted() {
    let sandbox = Sandbox::new();
    let idle = sandbox.worktree("idle");
    commit(&idle, "never pushed");
    sandbox.list_clone();
    let ctx = Ctx::new(sandbox.paths.clone(), false);
    let refname = sweep::salvage_now(&ctx, idle.to_str().unwrap(), later(21 * DAY))
        .unwrap()
        .salvaged_to
        .unwrap();

    let year = sweep_at(&sandbox, later(400 * DAY));
    let not_a_save = salvage::delete(&ctx, None, "refs/heads/main").unwrap_err();
    let deleted = salvage::delete(&ctx, None, &refname).unwrap();

    assert_eq!(year.pruned_salvage, Vec::<String>::new());
    assert_eq!(year.report.salvaged.len(), 1);
    assert_eq!(year.report.salvaged[0].kind, Kind::Abandoned);
    assert!(
        not_a_save.to_string().contains("is not a save"),
        "{not_a_save}"
    );
    assert_eq!(deleted, "~/app");
    assert_eq!(salvage_refs(&sandbox), Vec::<String>::new());
    assert!(branch_exists(&sandbox, "idle"));
}
