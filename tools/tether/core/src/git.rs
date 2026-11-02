//! Thin wrappers over the `git` binary. tether shells out rather than linking
//! libgit2 so it sees exactly what the user's own `git` sees (config,
//! credential helpers, worktrees).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

/// Variables that point git at a repository (from `git rev-parse
/// --local-env-vars`). A caller inside a git hook or alias exports them, and
/// they win over `-C`, so git would act on the caller's repo instead of `dir`.
/// A repo's `gate` and `after` commands run without them too.
pub const REPO_ENV_VARS: [&str; 10] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_SHALLOW_FILE",
    "GIT_GRAFT_FILE",
    "GIT_PREFIX",
];

pub fn command(bin: &OsStr, dir: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(bin);
    cmd.arg("-C").arg(dir).args(args);
    for var in REPO_ENV_VARS {
        cmd.env_remove(var);
    }
    // A poll must never stop on a credential prompt nobody can see.
    cmd.env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C");
    cmd
}

/// Runs git and returns its raw output, whatever the exit status.
pub fn output(bin: &OsStr, dir: &Path, args: &[&str]) -> Result<Output> {
    command(bin, dir, args)
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))
}

/// The first line of git's stderr, for skip reasons and errors.
pub fn first_error_line(out: &Output) -> String {
    let err = String::from_utf8_lossy(&out.stderr);
    err.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no error output")
        .to_string()
}

/// Runs git, failing with its stderr on a non-zero exit; returns trimmed stdout.
pub fn run(bin: &OsStr, dir: &Path, args: &[&str]) -> Result<String> {
    let out = output(bin, dir, args)?;
    if !out.status.success() {
        bail!(
            "git {} failed in {}: {}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

/// True when git exits zero.
pub fn ok(bin: &OsStr, dir: &Path, args: &[&str]) -> bool {
    output(bin, dir, args)
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `git rev-parse` output as an absolute path (git may answer relative to `dir`).
pub fn path_of(bin: &OsStr, dir: &Path, args: &[&str]) -> Result<PathBuf> {
    let p = PathBuf::from(run(bin, dir, args)?);
    Ok(if p.is_absolute() { p } else { dir.join(p) })
}

/// The commit `refname` points at, or `None` when it does not exist.
pub fn resolve(bin: &OsStr, dir: &Path, refname: &str) -> Option<String> {
    run(
        bin,
        dir,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{refname}^{{commit}}"),
        ],
    )
    .ok()
}

/// The branch checked out in `dir`, or `None` on a detached HEAD.
pub fn current_branch(bin: &OsStr, dir: &Path) -> Option<String> {
    run(bin, dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok()
}

pub fn is_ancestor(bin: &OsStr, dir: &Path, ancestor: &str, of: &str) -> bool {
    ok(bin, dir, &["merge-base", "--is-ancestor", ancestor, of])
}

/// The tip of `branch` on `remote` by `git ls-remote` (rule 3): one request, no
/// objects. `Ok(None)` when the remote has no such branch.
pub fn ls_remote(bin: &OsStr, dir: &Path, remote: &str, branch: &str) -> Result<Option<String>> {
    let refname = format!("refs/heads/{branch}");
    let out = output(bin, dir, &["ls-remote", "--quiet", remote, &refname])?;
    if !out.status.success() {
        bail!("{}", first_error_line(&out));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .find(|(_, r)| *r == refname)
        .map(|(sha, _)| sha.to_string()))
}

/// Local branches, as short names.
pub fn local_branches(bin: &OsStr, dir: &Path) -> Result<Vec<String>> {
    let out = run(
        bin,
        dir,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads/"],
    )?;
    Ok(out
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_callers_repository_never_reaches_git() {
        let cmd = command(OsStr::new("git"), Path::new("/Users/me/repo"), &["status"]);
        let removed: Vec<_> = cmd
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        for var in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_COMMON_DIR",
        ] {
            assert!(removed.iter().any(|r| r == var), "{var} is passed through");
        }
    }
}
