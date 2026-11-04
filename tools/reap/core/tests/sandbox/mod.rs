//! A throwaway world for integration tests (rule 17): a bare remote, a main clone and its
//! linked worktrees in a temp dir, and a fake `gh` that answers from a fixture. Nothing here
//! reaches GitHub or a clone outside the temp dir.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use reap_core::Paths;
use tempfile::TempDir;

pub const GITHUB_URL: &str = "https://github.com/acme/app.git";

pub struct Sandbox {
    _dir: TempDir,
    pub root: PathBuf,
    pub remote: PathBuf,
    pub clone: PathBuf,
    pub paths: Paths,
}

/// Runs git in `dir` with no user or system config, failing the test on a non-zero exit.
/// Drops the repository variables a git hook exports, so a test run from pre-push builds its
/// repositories in the temp dir instead of committing into the clone being pushed.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    for name in reap_core::git::REPOSITORY_ENV_VARS {
        command.env_remove(name);
    }
    let output = command
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

impl Sandbox {
    /// A remote whose `main` has one commit, cloned, with `origin/HEAD` set.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let remote = root.join("remote.git");
        let clone = root.join("app");
        git(
            &root,
            &[
                "init",
                "--quiet",
                "--bare",
                "--initial-branch=main",
                "remote.git",
            ],
        );
        git(
            &root,
            &["clone", "--quiet", remote.to_str().unwrap(), "app"],
        );
        // reap's own `commit-tree` (a salvage) needs an identity; the clone's config gives one.
        git(&clone, &["config", "user.name", "Test"]);
        git(&clone, &["config", "user.email", "test@example.com"]);
        commit(&clone, "readme");
        git(&clone, &["push", "--quiet", "origin", "main"]);
        git(&clone, &["remote", "set-head", "origin", "main"]);
        let mut paths = Paths::new(&root, &root.join("config"));
        paths.gh = root.join("no-gh").into();
        Self {
            _dir: dir,
            root,
            remote,
            clone,
            paths,
        }
    }

    /// A linked worktree at `<root>/<name>` on a new branch `name` from `main`.
    pub fn worktree(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        git(
            &self.clone,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                name,
                path.to_str().unwrap(),
                "main",
            ],
        );
        path
    }

    /// Pushes `branch` from the clone and merges it into `into` on the remote with a merge
    /// commit, then fetches, so `origin/<into>` contains it.
    pub fn merge_on_remote(&self, branch: &str, into: &str) {
        git(&self.clone, &["push", "--quiet", "origin", branch]);
        let merger = self.root.join("merger");
        if !merger.exists() {
            git(
                &self.root,
                &["clone", "--quiet", self.remote.to_str().unwrap(), "merger"],
            );
        }
        git(&merger, &["fetch", "--quiet", "origin"]);
        git(
            &merger,
            &["checkout", "--quiet", "-B", into, &format!("origin/{into}")],
        );
        git(
            &merger,
            &[
                "merge",
                "--quiet",
                "--no-ff",
                "-m",
                "merge",
                &format!("origin/{branch}"),
            ],
        );
        git(&merger, &["push", "--quiet", "origin", into]);
        git(&self.clone, &["fetch", "--quiet", "origin"]);
    }

    /// Points `origin` at GitHub, so reap asks `gh`; nothing is fetched after this.
    /// git may not use https in the clone, so a sweep's `ls-remote` fails at once instead of
    /// reaching GitHub.
    pub fn use_github_url(&self) {
        git(&self.clone, &["remote", "set-url", "origin", GITHUB_URL]);
        git(&self.clone, &["config", "protocol.https.allow", "never"]);
    }

    /// Installs a fake `gh` that prints `json` for any command.
    pub fn fake_gh(&mut self, json: &str) {
        self.fake_gh_routes(&[("*", json)]);
    }

    /// Installs a fake `gh` that prints the answer of the first route whose shell `case`
    /// pattern matches its arguments, joined by spaces, and fails when none does.
    pub fn fake_gh_routes(&mut self, routes: &[(&str, &str)]) {
        let mut script = String::from("#!/bin/sh\ncase \"$*\" in\n");
        for (index, (pattern, answer)) in routes.iter().enumerate() {
            let fixture = self.root.join(format!("gh-fixture-{index}"));
            std::fs::write(&fixture, answer).unwrap();
            let _ = writeln!(script, "  {pattern}) cat '{}' ;;", fixture.display());
        }
        script.push_str("  *) exit 1 ;;\nesac\n");
        let file = self.root.join("gh");
        std::fs::write(&file, script).unwrap();
        make_executable(&file);
        self.paths.gh = file.into();
    }

    /// Lists the clone in `repos.toml`.
    pub fn list_clone(&self) {
        reap_core::add::add(&self.paths, &self.clone, None, None).unwrap();
    }
}

/// Writes a file named `name` and commits it.
pub fn commit(dir: &Path, name: &str) -> String {
    std::fs::write(dir.join(name), name).unwrap();
    git(dir, &["add", name]);
    git(dir, &["commit", "--quiet", "-m", name]);
    git(dir, &["rev-parse", "HEAD"])
}

pub fn make_executable(file: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
}
