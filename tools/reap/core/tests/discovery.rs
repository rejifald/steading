//! Clone discovery end to end against real repositories (repo ADR 0004): which clones reap
//! suggests, what hides them, and that a scan only reads.

#[allow(dead_code)] // the shared sandbox has more helpers than this file uses
mod sandbox;

use std::path::{Path, PathBuf};
use std::process::Command;

use reap_core::add;
use reap_core::config::Config;
use reap_core::discovery::{self, Bounds, Found, View};
use sandbox::{Sandbox, commit, git, make_executable};

/// A main clone at `<dir>/<name>` with `worktrees` linked worktrees, which live in `<root>/wt`
/// so the walk does not meet them.
fn clone_with_worktrees(sandbox: &Sandbox, dir: &Path, name: &str, worktrees: usize) -> PathBuf {
    let clone = dir.join(name);
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "--quiet", "--initial-branch=main"]);
    commit(&clone, "readme");
    for index in 0..worktrees {
        let path = sandbox.root.join("wt").join(format!("{name}-{index}"));
        git(
            &clone,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                &format!("{name}-{index}"),
                path.to_str().unwrap(),
                "main",
            ],
        );
    }
    clone
}

/// Writes `scan` into `repos.toml`.
fn set_scan(sandbox: &Sandbox, roots: &[&str]) {
    let mut config = Config::load(&sandbox.paths).unwrap();
    config.scan = Some(roots.iter().map(|root| (*root).to_owned()).collect());
    config.save(&sandbox.paths).unwrap();
}

fn scan(sandbox: &Sandbox) -> Found {
    discovery::scan(&sandbox.paths, &Bounds::default(), &mut |_| {}).unwrap()
}

fn view(sandbox: &Sandbox) -> View {
    discovery::view(&sandbox.paths, None).unwrap()
}

fn names(view: &View) -> Vec<&str> {
    view.suggestions.iter().map(|s| s.name.as_str()).collect()
}

/// `~/Code` under the sandbox home with `alpha` and `zeta` (2 worktrees each), `beta` (1) and
/// `none` (0), scanned.
fn code_sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    let code = sandbox.root.join("Code");
    for (name, worktrees) in [("zeta", 2), ("beta", 1), ("none", 0), ("alpha", 2)] {
        clone_with_worktrees(&sandbox, &code, name, worktrees);
    }
    set_scan(&sandbox, &["~/Code"]);
    scan(&sandbox);
    sandbox
}

#[test]
fn a_clone_with_worktrees_is_suggested_most_first_and_one_without_is_not() {
    let sandbox = code_sandbox();

    let view = view(&sandbox);

    assert_eq!(names(&view), ["alpha", "zeta", "beta"]);
    let counts: Vec<_> = view.suggestions.iter().map(|s| s.worktrees).collect();
    assert_eq!(counts, [2, 2, 1]);
    assert_eq!(view.suggestions[0].path, "~/Code/alpha");
    assert_eq!(view.suggestions[0].root, "~/Code");
    assert_eq!(view.roots.len(), 1);
    assert_eq!(view.roots[0].path, "~/Code");
    assert_eq!(
        view.roots[0].clones,
        Some(4),
        "the walk's count, before the filter"
    );
    assert_eq!(view.roots[0].error, None);
    assert!(view.scanned_at.is_some());
    assert!(sandbox.paths.discovery_file().is_file());
    assert!(
        !sandbox.paths.state_file().exists(),
        "state.json is not touched"
    );
}

#[test]
fn a_linked_worktree_folder_is_never_suggested() {
    let sandbox = Sandbox::new();
    let code = sandbox.root.join("Code");
    let clone = clone_with_worktrees(&sandbox, &code, "app", 0);
    git(
        &clone,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "feat",
            code.join("app-feat").to_str().unwrap(),
            "main",
        ],
    );
    set_scan(&sandbox, &["~/Code"]);
    scan(&sandbox);

    let view = view(&sandbox);

    assert_eq!(names(&view), ["app"]);
    assert_eq!(view.suggestions[0].worktrees, 1);
    assert_eq!(view.roots[0].clones, Some(1), "the worktree is not a clone");
}

#[test]
fn a_listed_clone_and_a_skipped_one_are_hidden_and_unskip_brings_one_back_at_once() {
    let sandbox = code_sandbox();

    add::add(&sandbox.paths, &sandbox.root.join("Code/alpha"), None, None).unwrap();
    discovery::skip(&sandbox.paths, "~/Code/zeta").unwrap();
    let hidden = view(&sandbox);

    assert_eq!(names(&hidden), ["beta"]);
    assert_eq!(hidden.skipped.len(), 1);
    assert_eq!(hidden.skipped[0].path, "~/Code/zeta");
    assert_eq!(hidden.skipped[0].name, "zeta");
    assert_eq!(hidden.skipped[0].root.as_deref(), Some("~/Code"));
    let stored = Found::load(&sandbox.paths.discovery_file()).unwrap();
    assert_eq!(
        stored.suggestions.len(),
        3,
        "the file keeps the tool filter only"
    );

    discovery::unskip(&sandbox.paths, "~/Code/zeta").unwrap();

    assert_eq!(names(&view(&sandbox)), ["zeta", "beta"]);
    assert_eq!(view(&sandbox).skipped, []);
}

#[test]
fn skipping_twice_writes_one_entry_and_an_outside_path_has_no_root() {
    let sandbox = code_sandbox();

    discovery::skip(&sandbox.paths, "~/Code/zeta").unwrap();
    discovery::skip(
        &sandbox.paths,
        &sandbox.root.join("Code/zeta").display().to_string(),
    )
    .unwrap();
    discovery::skip(&sandbox.paths, "/srv/elsewhere").unwrap();

    let config = Config::load(&sandbox.paths).unwrap();
    assert_eq!(config.skip, ["~/Code/zeta", "/srv/elsewhere"]);
    let view = view(&sandbox);
    assert_eq!(view.skipped[1].root, None);
    assert!(discovery::skip(&sandbox.paths, "relative/path").is_err());
}

#[test]
fn remove_also_skips_the_clone_and_touches_no_state() {
    let sandbox = code_sandbox();
    add::add(&sandbox.paths, &sandbox.root.join("Code/alpha"), None, None).unwrap();

    let removed = add::remove(&sandbox.paths, "alpha").unwrap();

    assert_eq!(removed.as_deref(), Some("~/Code/alpha"));
    let config = Config::load(&sandbox.paths).unwrap();
    assert_eq!(config.repo, []);
    assert_eq!(config.skip, ["~/Code/alpha"]);
    assert_eq!(
        names(&view(&sandbox)),
        ["zeta", "beta"],
        "not suggested again"
    );
    assert!(!sandbox.paths.state_file().exists());
    assert_eq!(add::remove(&sandbox.paths, "alpha").unwrap(), None);
    assert_eq!(Config::load(&sandbox.paths).unwrap().skip, ["~/Code/alpha"]);
}

#[test]
fn removing_an_already_skipped_clone_keeps_one_skip_entry() {
    let sandbox = code_sandbox();
    add::add(&sandbox.paths, &sandbox.root.join("Code/alpha"), None, None).unwrap();
    discovery::skip(&sandbox.paths, "~/Code/alpha").unwrap();

    add::remove(&sandbox.paths, "alpha").unwrap();

    assert_eq!(Config::load(&sandbox.paths).unwrap().skip, ["~/Code/alpha"]);
}

#[test]
fn an_empty_scan_list_turns_discovery_off() {
    let sandbox = Sandbox::new();
    clone_with_worktrees(&sandbox, &sandbox.root.join("Development"), "app", 1);
    set_scan(&sandbox, &[]);
    let mut left = Vec::new();

    let found = discovery::scan(&sandbox.paths, &Bounds::default(), &mut |root| {
        left.push(root.to_owned());
    })
    .unwrap();

    assert_eq!(left, Vec::<String>::new());
    assert_eq!((found.roots, found.suggestions), (vec![], vec![]));
    let view = view(&sandbox);
    assert_eq!((view.roots, view.suggestions), (vec![], vec![]));
    assert!(
        view.scanned_at.is_some(),
        "a scan ran, it just had nowhere to look"
    );
}

#[test]
fn without_scan_the_usual_folders_that_exist_are_walked() {
    let sandbox = Sandbox::new();
    clone_with_worktrees(&sandbox, &sandbox.root.join("Development"), "app", 1);
    clone_with_worktrees(&sandbox, &sandbox.root.join("Desktop"), "hidden", 1);

    scan(&sandbox);
    let view = view(&sandbox);

    assert_eq!(names(&view), ["app"], "Desktop is never a default root");
    let roots: Vec<_> = view.roots.iter().map(|root| root.path.as_str()).collect();
    assert_eq!(roots, ["~/Development"]);
}

#[test]
fn an_unreadable_root_yields_an_error_and_another_root_still_yields() {
    let sandbox = Sandbox::new();
    clone_with_worktrees(&sandbox, &sandbox.root.join("Code"), "app", 3);
    set_scan(&sandbox, &["~/missing", "~/Code"]);
    let mut left = Vec::new();
    let mut seen = Vec::new();

    discovery::scan(&sandbox.paths, &Bounds::default(), &mut |root| {
        left.push(root.to_owned());
        seen.push(discovery::view(&sandbox.paths, None).unwrap());
    })
    .unwrap();

    assert_eq!(left, ["~/missing", "~/Code"]);
    assert!(
        seen[0].suggestions.is_empty(),
        "partial results are on disk after each root"
    );
    assert_eq!(
        seen[0].scanned_at, None,
        "scanned_at waits for the last root"
    );
    assert_eq!(seen[0].roots[1].clones, None, "~/Code is not walked yet");
    assert_eq!(names(&seen[1]), ["app"]);
    assert!(seen[1].scanned_at.is_some());
    let view = view(&sandbox);
    assert_eq!(view.roots[0].path, "~/missing");
    assert_eq!(view.roots[0].clones, Some(0));
    assert_eq!(view.roots[0].error.as_deref(), Some("no such folder"));
    assert_eq!(view.roots[1].error, None);
    assert_eq!(view.suggestions[0].worktrees, 3);
}

#[test]
fn a_new_scan_replaces_the_old_suggestions() {
    let sandbox = code_sandbox();
    std::fs::remove_dir_all(sandbox.root.join("Code/beta")).unwrap();

    scan(&sandbox);

    assert_eq!(names(&view(&sandbox)), ["alpha", "zeta"]);
}

#[test]
fn a_clone_under_two_overlapping_roots_is_suggested_once() {
    let sandbox = code_sandbox();
    set_scan(&sandbox, &["~/Code", "~/Code/alpha/.."]);

    scan(&sandbox);

    assert_eq!(names(&view(&sandbox)), ["alpha", "zeta", "beta"]);
}

#[test]
fn the_first_root_edit_writes_the_usual_folders_out_and_removing_a_root_drops_its_suggestions() {
    let sandbox = Sandbox::new();
    clone_with_worktrees(&sandbox, &sandbox.root.join("Development"), "dev-app", 1);
    clone_with_worktrees(&sandbox, &sandbox.root.join("Elsewhere"), "other", 1);
    assert_eq!(Config::load(&sandbox.paths).unwrap().scan, None);

    discovery::add_root(&sandbox.paths, "~/Elsewhere").unwrap();

    let config = Config::load(&sandbox.paths).unwrap();
    assert_eq!(
        config.scan.as_deref(),
        Some(&["~/Development".to_owned(), "~/Elsewhere".to_owned()][..])
    );
    assert!(
        !sandbox.paths.discovery_file().exists(),
        "adding a root runs nothing"
    );
    scan(&sandbox);
    assert_eq!(names(&view(&sandbox)), ["dev-app", "other"]);

    discovery::remove_root(&sandbox.paths, "~/Elsewhere").unwrap();

    let view = view(&sandbox);
    assert_eq!(names(&view), ["dev-app"]);
    assert_eq!(view.roots.len(), 1);
    let stored = Found::load(&sandbox.paths.discovery_file()).unwrap();
    assert_eq!(stored.suggestions.len(), 1);
    assert_eq!(stored.roots.len(), 1);
    discovery::add_root(&sandbox.paths, "~/Development").unwrap();
    assert_eq!(
        Config::load(&sandbox.paths).unwrap().scan.unwrap(),
        ["~/Development"]
    );
}

#[test]
fn accepting_a_suggestion_lists_it_with_the_checks_add_makes() {
    let sandbox = code_sandbox();

    let listed = discovery::add_suggestion(&sandbox.paths, "~/Code/alpha").unwrap();

    assert_eq!(listed, "~/Code/alpha");
    assert_eq!(names(&view(&sandbox)), ["zeta", "beta"]);
    let again = discovery::add_suggestion(&sandbox.paths, "~/Code/alpha").unwrap_err();
    assert_eq!(again.to_string(), "~/Code/alpha is already listed");
    let worktree = discovery::add_suggestion(
        &sandbox.paths,
        sandbox.root.join("wt/zeta-0").to_str().unwrap(),
    )
    .unwrap_err();
    assert!(
        worktree.to_string().contains("is a linked worktree"),
        "{worktree}"
    );
}

#[test]
fn a_bad_skip_or_scan_entry_makes_view_fail_with_its_name() {
    let sandbox = Sandbox::new();
    std::fs::create_dir_all(&sandbox.paths.config).unwrap();
    std::fs::write(sandbox.paths.config_file(), "skip = [\"scratch\"]\n").unwrap();

    let error = discovery::view(&sandbox.paths, None)
        .unwrap_err()
        .to_string();

    assert!(error.contains("skip entry scratch"), "{error}");
}

/// The `git-reap` binary in the sandbox, with `git` and `gh` replaced by scripts that log every
/// call, so a test can see that discovery ran neither.
struct Cli<'a> {
    sandbox: &'a Sandbox,
    calls: PathBuf,
}

impl<'a> Cli<'a> {
    fn new(sandbox: &'a Sandbox) -> Self {
        let calls = sandbox.root.join("calls.log");
        let logger = |name: &str, real: &str| {
            let file = sandbox.root.join(name);
            std::fs::write(
                &file,
                format!(
                    "#!/bin/sh\necho \"{name} $*\" >> '{}'\n{real}\n",
                    calls.display()
                ),
            )
            .unwrap();
            make_executable(&file);
            file
        };
        logger("logged-git", "exec git \"$@\"");
        logger("logged-gh", "exit 1");
        Self { sandbox, calls }
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_git-reap"));
        for name in reap_core::git::REPOSITORY_ENV_VARS {
            command.env_remove(name);
        }
        command
            .args(args)
            .env("HOME", &self.sandbox.root)
            .env("REAP_CONFIG_DIR", &self.sandbox.paths.config)
            .env("REAP_GIT", self.sandbox.root.join("logged-git"))
            .env("REAP_GH", self.sandbox.root.join("logged-gh"))
            .output()
            .unwrap()
    }

    fn ran_git_or_gh(&self) -> bool {
        self.calls.exists()
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn discover_prints_the_suggestions_and_runs_no_git_or_gh() {
    let sandbox = Sandbox::new();
    let code = sandbox.root.join("Code");
    clone_with_worktrees(&sandbox, &code, "app", 2);
    clone_with_worktrees(&sandbox, &code, "api", 1);
    clone_with_worktrees(&sandbox, &code, "bare", 0);
    set_scan(&sandbox, &["~/Code"]);
    let cli = Cli::new(&sandbox);

    let output = cli.run(&["discover"]);

    assert!(output.status.success(), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "looked in ~/Code\napp  2 worktrees  ~/Code/app\napi  1 worktree  ~/Code/api\n"
    );
    assert!(
        !cli.ran_git_or_gh(),
        "{}",
        std::fs::read_to_string(&cli.calls).unwrap_or_default()
    );
    assert!(
        Config::load(&sandbox.paths).unwrap().repo.is_empty(),
        "nothing is listed"
    );
}

#[test]
fn discover_json_has_the_contract_shape() {
    let sandbox = Sandbox::new();
    clone_with_worktrees(&sandbox, &sandbox.root.join("Code"), "app", 2);
    set_scan(&sandbox, &["~/Code", "~/missing"]);
    discovery::skip(&sandbox.paths, "~/Code/scratch").unwrap();
    let cli = Cli::new(&sandbox);

    let output = cli.run(&["discover", "--json"]);

    assert!(output.status.success(), "{}", text(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["scanned_at"].is_string());
    assert_eq!(json["scanning"], serde_json::Value::Null);
    assert_eq!(
        json["roots"],
        serde_json::json!([
            {"path": "~/Code", "clones": 1, "stopped_early": false, "error": null},
            {"path": "~/missing", "clones": 0, "stopped_early": false, "error": "no such folder"},
        ])
    );
    assert_eq!(
        json["suggestions"],
        serde_json::json!([{"path": "~/Code/app", "name": "app", "root": "~/Code", "worktrees": 2}])
    );
    assert_eq!(
        json["skipped"],
        serde_json::json!([{"path": "~/Code/scratch", "name": "scratch", "root": "~/Code"}])
    );
    assert!(!cli.ran_git_or_gh());
}

#[test]
fn skip_and_unskip_edit_the_skip_list() {
    let sandbox = code_sandbox();
    let cli = Cli::new(&sandbox);

    let skipped = cli.run(&["skip", "~/Code/zeta"]);
    assert!(skipped.status.success(), "{}", text(&skipped.stderr));
    assert_eq!(Config::load(&sandbox.paths).unwrap().skip, ["~/Code/zeta"]);
    assert_eq!(names(&view(&sandbox)), ["alpha", "beta"]);

    let unskipped = cli.run(&["unskip", "~/Code/zeta"]);
    assert!(unskipped.status.success(), "{}", text(&unskipped.stderr));
    assert_eq!(
        Config::load(&sandbox.paths).unwrap().skip,
        Vec::<String>::new()
    );
    assert_eq!(names(&view(&sandbox)), ["alpha", "zeta", "beta"]);
    assert!(!cli.ran_git_or_gh());
}

#[test]
fn a_relative_or_unknown_path_is_a_usage_error() {
    let sandbox = code_sandbox();
    let cli = Cli::new(&sandbox);

    assert_eq!(cli.run(&["skip", "zeta"]).status.code(), Some(2));
    assert_eq!(cli.run(&["unskip", "zeta"]).status.code(), Some(2));
    assert_eq!(cli.run(&["unskip", "~/Code/zeta"]).status.code(), Some(2));
    assert_eq!(
        Config::load(&sandbox.paths).unwrap().skip,
        Vec::<String>::new()
    );
}
