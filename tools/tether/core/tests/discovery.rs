//! Clone discovery (ADR 0004): what a scan suggests, what `view` hides, and how a suggestion is
//! accepted. Every walk takes the sandbox's fake `$HOME`; nothing here reads the real one.

mod sandbox;

use std::fs;
use std::path::{Path, PathBuf};

use find_clones::Bounds;
use sandbox::{Sandbox, git, subcommand, write};
use serde_json::Value;
use tether_core::add;
use tether_core::config::{Config, Mode};
use tether_core::discovery::{self, Found};
use tether_core::paths::CONCORD_LABEL;

fn scan(s: &Sandbox) -> Found {
    discovery::scan(&s.paths(), &Bounds::default(), &mut |_| {}).unwrap()
}

fn names(found: &Found) -> Vec<&str> {
    found.suggestions.iter().map(|x| x.name.as_str()).collect()
}

fn view_names(s: &Sandbox) -> Vec<String> {
    discovery::view(&s.paths(), None)
        .unwrap()
        .suggestions
        .into_iter()
        .map(|x| x.name)
        .collect()
}

/// A repo with a commit and no remote, at `~/Development/<name>`.
fn local_only(s: &Sandbox, name: &str) -> PathBuf {
    let path = s.home.join("Development").join(name);
    git(
        &s.root,
        &["init", "--quiet", "-b", "main", path.to_str().unwrap()],
    );
    write(&path.join("a.txt"), "a\n");
    git(&path, &["add", "-A"]);
    git(&path, &["commit", "--quiet", "-m", "first"]);
    path
}

fn set_config(s: &Sandbox, body: &str) {
    write(&s.config.join("repos.toml"), body);
}

fn config(s: &Sandbox) -> Config {
    Config::load(&s.paths()).unwrap()
}

#[test]
fn a_scan_suggests_clones_with_a_remote_and_the_current_branch() {
    let s = Sandbox::new();
    s.clone("notes");
    s.clone("api");
    let dev = s.clone("dev-branch");
    git(&s.seed, &["push", "--quiet", "origin", "main:dev"]);
    git(&dev, &["fetch", "--quiet", "origin"]);
    git(&dev, &["checkout", "--quiet", "-b", "dev", "origin/dev"]);

    let mut left = Vec::new();
    let found = discovery::scan(&s.paths(), &Bounds::default(), &mut |r| {
        left.push(r.to_string());
    })
    .unwrap();

    assert_eq!(left, ["~/Development"]);
    assert_eq!(names(&found), ["api", "dev-branch", "notes"]);
    let by = |n: &str| found.suggestions.iter().find(|x| x.name == n).unwrap();
    assert_eq!(by("notes").path, "~/Development/notes");
    assert_eq!(by("notes").root, "~/Development");
    assert_eq!(by("notes").branch, "main");
    assert_eq!(by("dev-branch").branch, "dev");
    assert!(found.scanned_at.is_some());
    assert_eq!(found.roots.len(), 1);
    assert_eq!(found.roots[0].clones, 3);
    assert_eq!(
        discovery::view(&s.paths(), None).unwrap().suggestions,
        found.suggestions
    );
}

#[test]
fn a_clone_with_no_remote_is_not_suggested_but_is_counted() {
    let s = Sandbox::new();
    s.clone("notes");
    local_only(&s, "scratch");
    let found = scan(&s);
    assert_eq!(names(&found), ["notes"]);
    assert_eq!(found.roots[0].clones, 2, "the count is before the filter");
}

#[test]
fn the_branch_falls_back_to_the_remote_head() {
    let s = Sandbox::new();
    let feature = s.clone("feature-work");
    git(&feature, &["checkout", "--quiet", "-b", "feature"]);
    let found = scan(&s);
    assert_eq!(found.suggestions.len(), 1);
    assert_eq!(found.suggestions[0].branch, "main");
}

#[test]
fn a_clone_with_neither_branch_is_not_suggested() {
    let s = Sandbox::new();
    let lost = s.clone("lost");
    git(&lost, &["checkout", "--quiet", "-b", "feature"]);
    git(&lost, &["remote", "set-head", "origin", "-d"]);
    s.clone("fine");
    assert_eq!(names(&scan(&s)), ["fine"]);
}

#[test]
fn a_detached_head_falls_back_to_the_remote_head() {
    let s = Sandbox::new();
    let d = s.clone("detached");
    git(&d, &["checkout", "--quiet", "--detach"]);
    assert_eq!(scan(&s).suggestions[0].branch, "main");
}

#[test]
fn a_remote_that_is_not_origin_is_used_when_origin_is_absent() {
    let s = Sandbox::new();
    let c = s.clone("renamed");
    git(&c, &["remote", "rename", "origin", "upstream"]);
    let found = scan(&s);
    assert_eq!(names(&found), ["renamed"]);
    assert_eq!(found.suggestions[0].branch, "main");
}

#[test]
fn concords_clone_is_not_suggested_while_concord_is_installed() {
    let s = Sandbox::new();
    s.clone("handbook");
    s.clone("notes");
    assert_eq!(names(&scan(&s)), ["handbook", "notes"]);
    let agent = s
        .home
        .join(format!("Library/LaunchAgents/{CONCORD_LABEL}.plist"));
    write(&agent, "");
    let found = scan(&s);
    assert_eq!(names(&found), ["notes"]);
    assert_eq!(found.roots[0].clones, 2);
}

#[test]
fn listed_and_skipped_clones_are_not_in_the_view() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.clone("api");
    s.clone("docs");
    scan(&s);
    assert_eq!(view_names(&s), ["api", "docs", "notes"]);

    s.list(&notes, Mode::Follow);
    assert_eq!(view_names(&s), ["api", "docs"]);

    discovery::skip(&s.paths(), "~/Development/api").unwrap();
    assert_eq!(view_names(&s), ["docs"]);
    let v = discovery::view(&s.paths(), None).unwrap();
    assert_eq!(v.skipped.len(), 1);
    assert_eq!(v.skipped[0].path, "~/Development/api");
    assert_eq!(v.skipped[0].name, "api");
    assert_eq!(v.skipped[0].root.as_deref(), Some("~/Development"));

    // Unskip brings it back at once, with no new scan.
    discovery::unskip(&s.paths(), "~/Development/api").unwrap();
    assert_eq!(view_names(&s), ["api", "docs"]);
    assert!(
        discovery::view(&s.paths(), None)
            .unwrap()
            .skipped
            .is_empty(),
        "nothing skipped now"
    );
    // The file itself kept every clone that passed the tool's filter.
    assert_eq!(
        names(&Found::load(&s.paths().discovery_file()).unwrap()).len(),
        3
    );
}

#[test]
fn skipping_twice_writes_one_entry_and_a_skip_outside_every_root_has_no_root() {
    let s = Sandbox::new();
    discovery::skip(&s.paths(), "~/Development/api").unwrap();
    discovery::skip(&s.paths(), "/srv/elsewhere").unwrap();
    discovery::skip(&s.paths(), "~/Development/api/").unwrap();
    assert_eq!(config(&s).skip, ["~/Development/api", "/srv/elsewhere"]);
    let v = discovery::view(&s.paths(), None).unwrap();
    assert_eq!(v.skipped[1].root, None);
    assert!(discovery::skip(&s.paths(), "api").is_err());
    assert!(discovery::unskip(&s.paths(), "~/Development/never").is_err());
}

#[test]
fn remove_adds_the_path_to_skip_once() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    add::add(
        &s.ctx(false),
        &add::AddOpts {
            path: notes.clone(),
            branch: "main".into(),
            mode: Mode::Follow,
            remote: None,
            interval: None,
            url: None,
            gate: None,
            after: None,
        },
    )
    .unwrap();
    scan(&s);
    assert!(view_names(&s).is_empty(), "listed");

    add::remove(&s.ctx(false), "notes").unwrap();
    assert_eq!(config(&s).skip, ["~/Development/notes"]);
    assert!(config(&s).repo.is_empty(), "unlisted");
    assert!(view_names(&s).is_empty(), "removed, so not suggested again");

    // Listed again by hand and removed again: still one entry.
    s.list(&notes, Mode::Follow);
    add::remove(&s.ctx(false), "notes").unwrap();
    assert_eq!(config(&s).skip, ["~/Development/notes"]);
}

#[test]
fn skip_and_unskip_run_from_the_cli() {
    let s = Sandbox::new();
    s.clone("notes");
    let out = s.cli(&["skip", "~/Development/notes"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(config(&s).skip, ["~/Development/notes"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("~/Development/notes"), "{text}");

    assert_eq!(s.cli(&["skip", "notes"]).status.code(), Some(2), "relative");
    assert_eq!(
        s.cli(&["unskip", "notes"]).status.code(),
        Some(2),
        "relative"
    );
    assert_eq!(
        s.cli(&["unskip", "~/Development/never"]).status.code(),
        Some(2),
        "not skipped"
    );

    let out = s.cli(&["unskip", "~/Development/notes"]);
    assert!(out.status.success(), "{out:?}");
    assert!(config(&s).skip.is_empty(), "unskipped");
    // Skipping a listed clone is allowed: it only edits skip.
    let notes = s.home.join("Development/notes");
    s.list(&notes, Mode::Follow);
    assert!(s.cli(&["skip", "~/Development/notes"]).status.success());
    assert_eq!(config(&s).skip, ["~/Development/notes"]);
    assert_eq!(config(&s).repo.len(), 1);
}

#[test]
fn discover_json_has_the_contracts_shape() {
    let s = Sandbox::new();
    s.clone("notes");
    s.clone("api");
    discovery::skip(&s.paths(), "~/Development/api").unwrap();
    let out = s.cli(&["discover", "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    let mut want = ["scanned_at", "scanning", "roots", "suggestions", "skipped"];
    want.sort_unstable();
    let mut got = keys.clone();
    got.sort_unstable();
    assert_eq!(got, want);
    assert!(v["scanned_at"].is_string());
    assert!(v["scanning"].is_null());
    assert_eq!(
        v["roots"][0],
        serde_json::json!({"path": "~/Development", "clones": 2, "stopped_early": false, "error": null})
    );
    assert_eq!(
        v["suggestions"],
        serde_json::json!([{"path": "~/Development/notes", "name": "notes", "root": "~/Development", "branch": "main"}])
    );
    assert_eq!(
        v["skipped"],
        serde_json::json!([{"path": "~/Development/api", "name": "api", "root": "~/Development"}])
    );
}

#[test]
fn discover_prints_plain_lines() {
    let s = Sandbox::new();
    s.clone("notes");
    let out = s.cli(&["discover"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("looked in ~/Development: 1 clone"), "{text}");
    assert!(text.contains("1 suggestion"), "{text}");
    assert!(text.contains("notes  main  ~/Development/notes"), "{text}");
}

#[test]
fn an_empty_scan_turns_discovery_off() {
    let s = Sandbox::new();
    s.clone("notes");
    set_config(&s, "scan = []\n");
    let found = scan(&s);
    assert!(found.suggestions.is_empty(), "discovery is off");
    assert!(found.roots.is_empty(), "no root walked");
    assert!(found.scanned_at.is_some());
    let v = discovery::view(&s.paths(), None).unwrap();
    assert!(v.roots.is_empty() && v.suggestions.is_empty());
    let out = s.cli(&["discover"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("no folder to look in"));
}

#[test]
fn an_absent_scan_uses_the_default_roots_that_exist() {
    let s = Sandbox::new();
    fs::create_dir_all(s.home.join("Code")).unwrap();
    // A protected folder is never a default root.
    let docs = s.home.join("Documents/old");
    git(
        &s.root,
        &["clone", "--quiet", &s.url, docs.to_str().unwrap()],
    );
    s.clone("notes");
    let v = discovery::view(&s.paths(), None).unwrap();
    let paths: Vec<&str> = v.roots.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(
        paths,
        ["~/Development", "~/Code"]
            .map(String::from)
            .map(|p| {
                // Order follows find_clones::default_roots.
                p
            })
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    assert!(v.roots.iter().all(|r| r.clones.is_none()), "no scan ran");
    assert_eq!(names(&scan(&s)), ["notes"]);
}

#[test]
fn a_bad_root_is_an_error_outcome_and_the_next_root_still_yields() {
    let s = Sandbox::new();
    s.clone("notes");
    let extra = s.home.join("Code");
    write(&extra.join("x"), "");
    git(
        &s.root,
        &[
            "clone",
            "--quiet",
            &s.url,
            extra.join("api").to_str().unwrap(),
        ],
    );
    set_config(
        &s,
        "scan = [\"~/Missing\", \"~/Development\", \"~/Code\"]\n",
    );
    let file = s.paths().discovery_file();
    let mut seen = Vec::new();
    let found = discovery::scan(&s.paths(), &Bounds::default(), &mut |root| {
        // The file holds what is found so far each time the caller is told.
        let now = Found::load(&file).unwrap();
        seen.push((
            root.to_string(),
            now.suggestions.len(),
            now.scanned_at.is_some(),
        ));
    })
    .unwrap();
    assert_eq!(
        seen,
        [
            ("~/Missing".to_string(), 0, false),
            ("~/Development".to_string(), 1, false),
            ("~/Code".to_string(), 2, true),
        ]
    );
    assert_eq!(found.roots[0].error.as_deref(), Some("no such folder"));
    assert_eq!(found.roots[0].clones, 0);
    assert_eq!(found.roots[1].error, None);
    assert_eq!(
        names(&found),
        ["notes", "api"],
        "config order, then by name"
    );

    let v = discovery::view(&s.paths(), Some("~/Code".into())).unwrap();
    assert_eq!(v.scanning.as_deref(), Some("~/Code"));
    assert_eq!(v.roots[0].error.as_deref(), Some("no such folder"));
}

#[test]
fn a_new_scan_replaces_the_suggestions() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    s.clone("api");
    assert_eq!(scan(&s).suggestions.len(), 2);
    fs::remove_dir_all(&notes).unwrap();
    assert_eq!(names(&scan(&s)), ["api"]);
}

#[test]
fn adding_and_removing_a_root_edit_scan_and_drop_its_suggestions() {
    let s = Sandbox::new();
    s.clone("notes");
    // Nothing written yet: the defaults apply.
    assert!(config(&s).scan.is_none());
    discovery::add_root(&s.paths(), "~/Code").unwrap();
    assert_eq!(
        config(&s).scan,
        Some(vec!["~/Development".to_string(), "~/Code".to_string()])
    );
    let code = s.home.join("Code");
    git(
        &s.root,
        &[
            "clone",
            "--quiet",
            &s.url,
            code.join("api").to_str().unwrap(),
        ],
    );
    assert!(
        Found::load(&s.paths().discovery_file()).is_none(),
        "adding a root runs nothing"
    );
    scan(&s);
    assert_eq!(view_names(&s), ["notes", "api"]);

    discovery::remove_root(&s.paths(), "~/Development").unwrap();
    assert_eq!(config(&s).scan, Some(vec!["~/Code".to_string()]));
    let found = Found::load(&s.paths().discovery_file()).unwrap();
    assert_eq!(names(&found), ["api"], "its suggestions went at once");
    assert_eq!(found.roots.len(), 1);
    assert_eq!(view_names(&s), ["api"]);

    // Both are quiet no-ops when there is nothing to do.
    discovery::add_root(&s.paths(), "~/Code").unwrap();
    discovery::remove_root(&s.paths(), "~/Never").unwrap();
    assert_eq!(config(&s).scan, Some(vec!["~/Code".to_string()]));
    assert!(discovery::add_root(&s.paths(), "relative").is_err());
}

#[test]
fn a_bad_scan_entry_is_an_error_the_view_reports() {
    let s = Sandbox::new();
    set_config(&s, "scan = [\"code\"]\n");
    let e = discovery::view(&s.paths(), None).unwrap_err();
    assert!(format!("{e:#}").contains("code"), "{e:#}");
    assert!(discovery::scan(&s.paths(), &Bounds::default(), &mut |_| {}).is_err());
}

#[test]
fn accepting_a_suggestion_lists_it_in_follow_mode_on_its_branch() {
    let s = Sandbox::new();
    let dev = s.clone("dev-branch");
    git(&s.seed, &["push", "--quiet", "origin", "main:dev"]);
    git(&dev, &["fetch", "--quiet", "origin"]);
    git(&dev, &["checkout", "--quiet", "-b", "dev", "origin/dev"]);
    scan(&s);

    let repo = discovery::accept(&s.ctx(false), "~/Development/dev-branch").unwrap();
    assert_eq!((repo.mode, repo.branch.as_str()), (Mode::Follow, "dev"));
    let cfg = config(&s);
    assert_eq!(cfg.repo.len(), 1);
    let e = &cfg.repo[0];
    assert_eq!(e.path, "~/Development/dev-branch");
    assert_eq!(e.mode, Mode::Follow);
    assert_eq!(
        (&e.remote, &e.interval, &e.url, &e.gate, &e.after),
        (&None, &None, &None, &None, &None)
    );
    assert!(view_names(&s).is_empty(), "listed, so no longer suggested");
}

#[test]
fn accepting_lists_the_remote_the_branch_was_read_from() {
    let s = Sandbox::new();
    let c = s.clone("renamed");
    git(&c, &["remote", "rename", "origin", "upstream"]);
    scan(&s);
    let repo = discovery::accept(&s.ctx(false), "~/Development/renamed").unwrap();
    assert_eq!(repo.remote, "upstream");
    assert_eq!(config(&s).repo[0].remote.as_deref(), Some("upstream"));
}

#[test]
fn accepting_a_path_that_is_not_a_suggestion_fails() {
    let s = Sandbox::new();
    s.clone("notes");
    let e = discovery::accept(&s.ctx(false), "~/Development/notes").unwrap_err();
    assert!(
        format!("{e:#}").contains("not a suggestion"),
        "no scan yet: {e:#}"
    );
    scan(&s);
    discovery::skip(&s.paths(), "~/Development/notes").unwrap();
    // A skipped clone is still in the file, so it can be accepted by path; add decides.
    let e = discovery::accept(&s.ctx(false), "~/Development/other").unwrap_err();
    assert!(format!("{e:#}").contains("not a suggestion"), "{e:#}");
}

#[test]
fn accepting_runs_every_check_add_makes_and_returns_its_text() {
    let s = Sandbox::new();
    let notes = s.clone("notes");
    scan(&s);
    // Listed by hand after the scan: add refuses a second time, in its own words.
    s.list(&notes, Mode::Follow);
    let e = discovery::accept(&s.ctx(false), "~/Development/notes").unwrap_err();
    assert!(format!("{e:#}").contains("is already listed"), "{e:#}");

    // The branch the suggestion carries is gone from the remote by the time it is accepted.
    let gone = s.clone("gone");
    git(&s.seed, &["push", "--quiet", "origin", "main:gone"]);
    git(&gone, &["fetch", "--quiet", "origin"]);
    git(&gone, &["checkout", "--quiet", "-b", "gone", "origin/gone"]);
    scan(&s);
    git(&s.seed, &["push", "--quiet", "origin", ":gone"]);
    let e = discovery::accept(&s.ctx(false), "~/Development/gone").unwrap_err();
    assert!(format!("{e:#}").contains("has no branch gone"), "{e:#}");
    assert_eq!(config(&s).repo.len(), 1, "nothing was listed");
}

#[test]
fn a_scan_runs_no_fetch_and_no_ls_remote() {
    let s = Sandbox::new();
    s.clone("notes");
    s.clone("api");
    let lonely = s.clone("feature");
    git(&lonely, &["checkout", "--quiet", "-b", "feature"]);
    s.take_git_log();

    scan(&s);
    let out = s.cli(&["discover", "--json"]);
    assert!(out.status.success());
    let log = s.take_git_log();
    assert!(!log.is_empty(), "the wrapper saw the scan's git calls");
    for line in &log {
        let sub = subcommand(line);
        assert!(
            !["fetch", "ls-remote", "pull", "clone", "remote-update"].contains(&sub)
                && !sandbox::MUTATING.contains(&sub),
            "a scan ran {line}"
        );
    }
}

#[test]
fn the_walk_stays_under_the_roots() {
    let s = Sandbox::new();
    // Not under a root: never found, even though it is a clone.
    let outside: &Path = &s.root.join("elsewhere/outside");
    git(
        &s.root,
        &["clone", "--quiet", &s.url, outside.to_str().unwrap()],
    );
    s.clone("inside");
    assert_eq!(names(&scan(&s)), ["inside"]);
}
