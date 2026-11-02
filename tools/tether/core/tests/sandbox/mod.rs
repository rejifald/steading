//! A throwaway world for the integration tests: a bare remote, a seed clone
//! that pushes to it, clones under a fake `$HOME`, and a `git` wrapper that
//! logs every command tether runs. git's global config is replaced, so the
//! developer's own config never leaks in. Nothing here touches real paths.

#![allow(dead_code)] // each test file uses a different part of the harness

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;
use std::time::Duration;

use tempfile::TempDir;
use tether_core::config::{Config, Mode, Repo, RepoEntry};
use tether_core::sync::{self, Outcome, Trigger};
use tether_core::{Ctx, Paths};

pub fn isolate_git() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let cfg = std::env::temp_dir().join(format!("tether-test-gitconfig-{}", std::process::id()));
        fs::write(
            &cfg,
            "[user]\n\tname = Tether Test\n\temail = test@example.invalid\n[commit]\n\tgpgsign = false\n\
             [init]\n\tdefaultBranch = main\n[advice]\n\tdetachedHead = false\n[protocol \"file\"]\n\tallow = always\n",
        )
        .unwrap();
        // SAFETY: changing the environment races with any thread reading it. Every test starts
        // with `Sandbox::new`, which calls this before any git runs, and `Once` holds the other
        // callers until it returns, so no test thread reads the environment while it changes.
        unsafe {
            std::env::set_var("GIT_CONFIG_GLOBAL", &cfg);
            std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        }
        // Under a git hook (pre-push runs these tests) git exports the
        // repository it is running in; left set, every `git -C <sandbox>`
        // here would write to that repository instead.
        let local = git_output(&["rev-parse", "--local-env-vars"]);
        for var in local.lines() {
            // SAFETY: as above.
            unsafe { std::env::remove_var(var) };
        }
    });
}

/// Runs git outside any repository and returns its stdout.
fn git_output(args: &[&str]) -> String {
    let out = Command::new("git").args(args).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub fn write(p: &Path, s: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
}

pub struct Sandbox {
    _tmp: TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
    pub config: PathBuf,
    pub seed: PathBuf,
    /// `file://` URL of the bare remote: plain paths make git ignore `--depth`.
    pub url: String,
    pub git_log: PathBuf,
    wrapper: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        isolate_git();
        let tmp = TempDir::new().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        let home = root.join("home");
        let remote = root.join("remote.git");
        let seed = root.join("seed");
        fs::create_dir_all(home.join("Development")).unwrap();
        git(
            &root,
            &[
                "init",
                "--quiet",
                "--bare",
                "-b",
                "main",
                remote.to_str().unwrap(),
            ],
        );
        git(
            &root,
            &["init", "--quiet", "-b", "main", seed.to_str().unwrap()],
        );
        write(&seed.join("README.md"), "readme\n");
        write(&seed.join("notes/a.md"), "one\n");
        write(&seed.join(".gitignore"), "*.log\n");
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "--quiet", "-m", "seed"]);
        let url = format!("file://{}", remote.display());
        git(&seed, &["remote", "add", "origin", &url]);
        git(&seed, &["push", "--quiet", "-u", "origin", "main"]);
        let git_log = root.join("git.log");
        let wrapper = root.join("git-wrapper");
        write(
            &wrapper,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec git \"$@\"\n",
                git_log.display()
            ),
        );
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
        Sandbox {
            config: home.join(".config/tether"),
            _tmp: tmp,
            root,
            home,
            seed,
            url,
            git_log,
            wrapper,
        }
    }

    pub fn paths(&self) -> Paths {
        let mut p = Paths::new(&self.home, &self.config);
        p.git = self.wrapper.clone().into();
        p
    }

    pub fn ctx(&self, dry_run: bool) -> Ctx {
        Ctx::new(self.paths(), dry_run)
    }

    /// Environment for running the CLI binary inside the sandbox.
    pub fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_git-tether"))
            .args(args)
            .env("HOME", &self.home)
            .env("TETHER_CONFIG_DIR", &self.config)
            .env("TETHER_GIT", &self.wrapper)
            .env_remove("TETHER_CONCORD_REPO_DIR")
            .env_remove("CONCORD_REPO_DIR")
            .output()
            .unwrap()
    }

    /// A full clone of the remote at `~/Development/<name>`.
    pub fn clone(&self, name: &str) -> PathBuf {
        let path = self.home.join("Development").join(name);
        git(
            &self.root,
            &["clone", "--quiet", &self.url, path.to_str().unwrap()],
        );
        path
    }

    /// Commits `rel` with `content` upstream and pushes it.
    pub fn upstream(&self, rel: &str, content: &str) -> String {
        write(&self.seed.join(rel), content);
        git(&self.seed, &["add", "-A"]);
        git(
            &self.seed,
            &["commit", "--quiet", "-m", &format!("change {rel}")],
        );
        git(&self.seed, &["push", "--quiet", "origin", "main"]);
        git(&self.seed, &["rev-parse", "HEAD"])
    }

    /// Lists `path` in repos.toml directly, bypassing `add`'s checks.
    pub fn list(&self, path: &Path, mode: Mode) -> Repo {
        let mut cfg = Config::load(&self.paths()).unwrap();
        cfg.repo.push(RepoEntry {
            path: self.paths().tilde(path),
            branch: "main".into(),
            mode,
            remote: None,
            interval: None,
            url: Some(self.url.clone()),
            gate: None,
            after: None,
        });
        cfg.save(&self.paths()).unwrap();
        self.repo(path)
    }

    /// Sets the listed `path`'s `gate` and `after` commands.
    pub fn set_commands(&self, path: &Path, gate: Option<&str>, after: Option<&str>) -> Repo {
        let p = self.paths();
        let mut cfg = Config::load(&p).unwrap();
        let shown = p.tilde(path);
        let entry = cfg
            .repo
            .iter_mut()
            .find(|e| e.path == shown)
            .expect("listed");
        entry.gate = gate.map(str::to_string);
        entry.after = after.map(str::to_string);
        cfg.save(&p).unwrap();
        self.repo(path)
    }

    pub fn repo(&self, path: &Path) -> Repo {
        let p = self.paths();
        Config::load(&p)
            .unwrap()
            .repos(&p)
            .unwrap()
            .into_iter()
            .find(|r| r.path == path)
            .expect("listed")
    }

    /// A background sync: the tray's timers or `sync --due`.
    pub fn sync(&self, repo: &Repo) -> Outcome {
        sync::sync_repo(&self.ctx(false), repo, Trigger::Background).unwrap()
    }

    /// *Sync now* or `git tether sync <repo>`.
    pub fn sync_now(&self, repo: &Repo) -> Outcome {
        let explicit = Trigger::Explicit {
            wait: Duration::ZERO,
        };
        sync::sync_repo(&self.ctx(false), repo, explicit).unwrap()
    }

    pub fn head(&self, dir: &Path) -> String {
        git(dir, &["rev-parse", "HEAD"])
    }

    /// The git commands logged since the last call, one per line.
    pub fn take_git_log(&self) -> Vec<String> {
        let s = fs::read_to_string(&self.git_log).unwrap_or_default();
        let _ = fs::remove_file(&self.git_log);
        s.lines().map(str::to_string).collect()
    }
}

/// git subcommands that change a clone, its refs or its objects.
pub const MUTATING: &[&str] = &[
    "fetch",
    "merge",
    "reset",
    "clean",
    "checkout",
    "clone",
    "gc",
    "reflog",
    "update-ref",
    "stash",
    "rebase",
    "switch",
    "commit",
];

/// The subcommand of a logged `-C <dir> <sub> …` line.
pub fn subcommand(line: &str) -> &str {
    let mut words = line.split_whitespace();
    if words.next() == Some("-C") {
        words.next();
        words.next().unwrap_or("")
    } else {
        line.split_whitespace().next().unwrap_or("")
    }
}
