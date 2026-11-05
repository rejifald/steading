//! `tether add` / `set` / `remove` refusals (rule 2) and the config they write.

mod sandbox;

use std::fs;
use std::path::Path;

use sandbox::{Sandbox, git};
use tether_core::add::{self, AddOpts};
use tether_core::config::Mode;
use tether_core::paths::CONCORD_LABEL;

fn add_follow(s: &Sandbox, path: &Path) -> anyhow::Result<tether_core::config::Repo> {
    let opts = AddOpts {
        path: path.to_path_buf(),
        branch: "main".into(),
        mode: Mode::Follow,
        remote: None,
        interval: Some("10m".into()),
        url: None,
        gate: None,
        after: None,
    };
    add::add(&s.ctx(false), &opts)
}

#[test]
fn concords_clone_needs_a_gate_while_concord_is_installed() {
    let s = Sandbox::new();
    let handbook = s.clone("handbook");
    let agent = s
        .home
        .join(format!("Library/LaunchAgents/{CONCORD_LABEL}.plist"));
    fs::create_dir_all(agent.parent().unwrap()).unwrap();
    fs::write(&agent, "").unwrap();
    let e = add_follow(&s, &handbook).unwrap_err();
    assert!(format!("{e:#}").contains("--gate"), "{e:#}");
    let gated = AddOpts {
        path: handbook.clone(),
        branch: "main".into(),
        mode: Mode::Follow,
        remote: None,
        interval: None,
        url: None,
        gate: Some("concord idle".into()),
        after: Some("concord reconcile".into()),
    };
    let repo = add::add(&s.ctx(false), &gated).unwrap();
    assert_eq!(repo.gate.as_deref(), Some("concord idle"));
    let toml = fs::read_to_string(s.config.join("repos.toml")).unwrap();
    assert!(toml.contains("gate = \"concord idle\""), "{toml}");
    assert!(toml.contains("after = \"concord reconcile\""), "{toml}");
    // Without concord the clone is just another repo.
    add::remove(&s.ctx(false), "handbook").unwrap();
    fs::remove_file(&agent).unwrap();
    add_follow(&s, &handbook).unwrap();
}

#[test]
fn an_empty_gate_is_refused() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let opts = AddOpts {
        path: notes,
        branch: "main".into(),
        mode: Mode::Follow,
        remote: None,
        interval: None,
        url: None,
        gate: Some(" ".into()),
        after: None,
    };
    let e = add::add(&s.ctx(false), &opts).unwrap_err();
    assert!(format!("{e:#}").contains("gate is empty"), "{e:#}");
}

#[test]
fn concords_clone_follows_the_override() {
    let s = Sandbox::new();
    let elsewhere = s.clone("my-handbook");
    fs::create_dir_all(s.home.join("Applications/concord.app")).unwrap();
    let mut paths = s.paths();
    paths.concord_repo = elsewhere.clone();
    let opts = AddOpts {
        path: elsewhere,
        branch: "main".into(),
        mode: Mode::Follow,
        remote: None,
        interval: None,
        url: None,
        gate: None,
        after: None,
    };
    let e = add::add(&tether_core::Ctx::new(paths, false), &opts).unwrap_err();
    assert!(format!("{e:#}").contains("concord's clone"), "{e:#}");
}

#[test]
fn a_linked_worktree_is_refused() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let wt = s.home.join("Development/notes-wt");
    git(
        &notes,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "wt",
            wt.to_str().unwrap(),
        ],
    );
    let e = add_follow(&s, &wt).unwrap_err();
    assert!(format!("{e:#}").contains("linked worktree"), "{e:#}");
}

#[test]
fn a_subdirectory_or_a_plain_folder_is_refused() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    assert!(
        format!("{:#}", add_follow(&s, &notes.join("notes")).unwrap_err())
            .contains("not a clone's top level")
    );
    let plain = s.home.join("plain");
    fs::create_dir_all(&plain).unwrap();
    assert!(format!("{:#}", add_follow(&s, &plain).unwrap_err()).contains("not a git clone"));
}

#[test]
fn a_branch_the_remote_lacks_is_refused() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    let opts = AddOpts {
        path: notes,
        branch: "nope".into(),
        mode: Mode::Follow,
        remote: None,
        interval: None,
        url: None,
        gate: None,
        after: None,
    };
    let e = add::add(&s.ctx(false), &opts).unwrap_err();
    assert!(
        format!("{e:#}").contains("origin has no branch nope"),
        "{e:#}"
    );
}

#[test]
fn add_writes_the_entry_and_remove_unlists_it() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    add_follow(&s, &notes).unwrap();
    assert!(format!("{:#}", add_follow(&s, &notes).unwrap_err()).contains("already listed"));
    let toml = fs::read_to_string(s.config.join("repos.toml")).unwrap();
    assert!(toml.contains("path = \"~/Development/notes\""), "{toml}");
    assert!(
        toml.contains("interval = \"10m\"") && !toml.contains("remote"),
        "{toml}"
    );
    s.sync(&s.repo(&notes));
    let out = s.cli(&["remove", "notes"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Unlisted, and kept out of the suggestions (ADR 0004 rule 3).
    let toml = fs::read_to_string(s.config.join("repos.toml")).unwrap();
    assert!(!toml.contains("\n[[repo]]"), "{toml}");
    assert!(toml.contains("skip = [\"~/Development/notes\"]"), "{toml}");
    assert!(
        !fs::read_to_string(s.config.join("state.json"))
            .unwrap()
            .contains("notes")
    );
    assert!(notes.join(".git").exists(), "the clone is untouched");
}

#[test]
fn set_changes_the_interval_and_keeps_the_rest() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    add_follow(&s, &notes).unwrap();
    let out = s.cli(&["set", "notes", "--interval", "1h"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let toml = fs::read_to_string(s.config.join("repos.toml")).unwrap();
    assert!(
        toml.contains("interval = \"1h\"") && toml.contains("branch = \"main\""),
        "{toml}"
    );
    assert!(add::set_interval(&s.ctx(false), "notes", "5s").is_err());
    assert_eq!(
        add::set_interval(&s.ctx(false), "elsewhere", "5m").unwrap(),
        None
    );
}

#[test]
fn the_cli_add_lists_a_repo() {
    let s = Sandbox::new();
    s.clone("notes");
    let out = s.cli(&["add", "~/Development/notes", "--branch", "main"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let st = s.cli(&["status"]);
    let text = String::from_utf8_lossy(&st.stdout);
    assert!(
        text.contains("~/Development/notes  main (follow, every 5m)"),
        "{text}"
    );
}
