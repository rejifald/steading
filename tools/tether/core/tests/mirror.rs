//! `mirror` mode end to end (rules 7–9): fresh shallow clones, forced syncs
//! with a backup patch, and the adoption guards.

mod sandbox;

use std::fs;
use std::path::Path;

use sandbox::{Sandbox, git, write};
use tether_core::add::{self, AddOpts};
use tether_core::config::Mode;
use tether_core::inspect::Health;

fn add_mirror(
    s: &Sandbox,
    path: &Path,
    url: Option<&str>,
) -> anyhow::Result<tether_core::config::Repo> {
    let opts = AddOpts {
        path: path.to_path_buf(),
        branch: "main".into(),
        mode: Mode::Mirror,
        remote: None,
        interval: None,
        url: url.map(str::to_string),
        gate: None,
        after: None,
    };
    add::add(&s.ctx(false), &opts)
}

fn commit_count(dir: &Path) -> String {
    git(dir, &["rev-list", "--count", "--all"])
}

#[test]
fn a_new_path_is_cloned_at_depth_one() {
    let s = Sandbox::new();
    s.upstream("README.md", "readme v2\n");
    let docs = s.home.join("Mirrors/docs");
    let repo = add_mirror(&s, &docs, Some(&s.url)).unwrap();
    assert_eq!(commit_count(&docs), "1");
    assert!(docs.join(".git/shallow").exists());
    assert_eq!(git(&docs, &["symbolic-ref", "--short", "HEAD"]), "main");
    assert_eq!(s.head(&docs), git(&s.seed, &["rev-parse", "HEAD"]));
    let out = s.sync(&repo);
    assert_eq!(
        (out.health, out.fetched, out.moved.is_none()),
        (Health::InSync, false, true),
        "{out:#?}"
    );
}

#[test]
fn a_new_path_without_a_url_is_refused() {
    let s = Sandbox::new();
    let e = add_mirror(&s, &s.home.join("Mirrors/docs"), None).unwrap_err();
    assert!(format!("{e:#}").contains("--url"), "{e:#}");
}

#[test]
fn a_sync_discards_everything_and_keeps_a_patch_of_tracked_changes() {
    let s = Sandbox::new();
    let docs = s.home.join("Mirrors/docs");
    let repo = add_mirror(&s, &docs, Some(&s.url)).unwrap();
    write(&docs.join("notes/a.md"), "edited in the mirror\n");
    write(&docs.join("untracked.md"), "stray\n");
    write(&docs.join("build.log"), "ignored\n");
    let tip = s.upstream("README.md", "readme v2\n");
    let out = s.sync(&repo);
    assert_eq!(out.health, Health::InSync, "{out:#?}");
    assert!(out.discarded && out.fetched);
    assert_eq!(s.head(&docs), tip);
    assert_eq!(
        fs::read_to_string(docs.join("notes/a.md")).unwrap(),
        "one\n"
    );
    assert!(!docs.join("untracked.md").exists());
    assert!(
        !docs.join("build.log").exists(),
        "clean -x removes ignored files too"
    );
    assert_eq!(git(&docs, &["status", "--porcelain", "--ignored"]), "");
    let patch = out.backup.expect("backup patch");
    assert!(
        patch.starts_with(s.config.join("backups/docs")),
        "{}",
        patch.display()
    );
    assert!(patch.extension().is_some_and(|e| e == "diff"));
    let body = fs::read_to_string(&patch).unwrap();
    assert!(body.contains("+edited in the mirror"), "{body}");
    // Rule 9: the reflog is pruned and the clone stays at one commit.
    assert_eq!(commit_count(&docs), "1");
    assert_eq!(git(&docs, &["reflog", "show", "--all"]), "");
}

#[test]
fn local_edits_are_discarded_even_when_the_remote_did_not_move() {
    let s = Sandbox::new();
    let docs = s.home.join("Mirrors/docs");
    let repo = add_mirror(&s, &docs, Some(&s.url)).unwrap();
    write(&docs.join("untracked.md"), "stray\n");
    let out = s.sync(&repo);
    assert!(
        !out.fetched && out.discarded && out.backup.is_none(),
        "{out:#?}"
    );
    assert!(!docs.join("untracked.md").exists());
}

#[test]
fn an_adopted_clone_becomes_shallow_after_the_tip_moves() {
    let s = Sandbox::new();
    s.upstream("README.md", "readme v2\n");
    let notes = s.clone("mirror-me");
    assert_eq!(commit_count(&notes), "2");
    let repo = add_mirror(&s, &notes, None).unwrap();
    let tip = s.upstream("README.md", "readme v3\n");
    s.sync(&repo);
    assert_eq!(s.head(&notes), tip);
    assert_eq!(commit_count(&notes), "1");
}

#[test]
fn adoption_refuses_a_linked_worktree() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    git(
        &notes,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            s.root.join("wt").to_str().unwrap(),
        ],
    );
    let e = add_mirror(&s, &notes, None).unwrap_err();
    assert!(format!("{e:#}").contains("1 linked worktree(s)"), "{e:#}");
}

#[test]
fn adoption_refuses_a_stash() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    write(&notes.join("README.md"), "wip\n");
    git(&notes, &["stash", "--quiet"]);
    let e = add_mirror(&s, &notes, None).unwrap_err();
    assert!(format!("{e:#}").contains("1 stash entry"), "{e:#}");
}

#[test]
fn adoption_refuses_another_local_branch() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    git(&notes, &["branch", "spike"]);
    let e = add_mirror(&s, &notes, None).unwrap_err();
    assert!(format!("{e:#}").contains("other than main: spike"), "{e:#}");
}

#[test]
fn adoption_refuses_an_unpushed_commit() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    write(&notes.join("local.md"), "local\n");
    git(&notes, &["add", "local.md"]);
    git(&notes, &["commit", "--quiet", "-m", "local"]);
    let e = add_mirror(&s, &notes, None).unwrap_err();
    assert!(
        format!("{e:#}").contains("1 commit(s) not on origin"),
        "{e:#}"
    );
    assert!(
        !s.config.join("repos.toml").exists(),
        "a refused add lists nothing"
    );
}

#[test]
fn a_clean_clone_is_adopted() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let repo = add_mirror(&s, &notes, None).unwrap();
    assert_eq!(repo.mode, Mode::Mirror);
    let toml = fs::read_to_string(s.config.join("repos.toml")).unwrap();
    assert!(
        toml.contains("path = \"~/Development/notes\"") && toml.contains("mode = \"mirror\""),
        "{toml}"
    );
}

#[test]
fn a_mirror_with_a_new_tip_is_behind_until_synced() {
    let s = Sandbox::new();
    let docs = s.home.join("Mirrors/docs");
    let repo = add_mirror(&s, &docs, Some(&s.url)).unwrap();
    s.upstream("README.md", "readme v2\n");
    let planned =
        tether_core::sync::sync_repo(&s.ctx(true), &repo, tether_core::sync::Trigger::Background)
            .unwrap();
    assert_eq!(planned.health, Health::Behind, "{planned:#?}");
    git(
        &docs,
        &[
            "fetch",
            "--quiet",
            "--depth=1",
            "origin",
            "+refs/heads/main:refs/remotes/origin/main",
        ],
    );
    let r = tether_core::report::report(&s.ctx(true)).unwrap();
    assert_eq!(r.repos[0].health, Health::Behind, "{:#?}", r.repos[0]);
    assert_eq!(s.sync(&repo).health, Health::InSync);
    let r = tether_core::report::report(&s.ctx(true)).unwrap();
    assert_eq!(r.repos[0].health, Health::InSync);
}
