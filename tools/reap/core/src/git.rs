//! Thin wrappers over the `git` binary. reap shells out rather than linking libgit2 so it
//! sees exactly what the user's own `git` sees (config, credential helpers, worktrees).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

/// The variables that tie git to one repository, as `git rev-parse --local-env-vars` lists them.
/// A git hook exports them for its own repository, and `-C` does not override them, so a reap
/// run from a hook (its tests in pre-push, for one) would act on that repository instead of
/// `dir`.
pub const REPOSITORY_ENV_VARS: [&str; 15] = [
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

/// A `git -C <dir>` command that never prompts, speaks C-locale English, and acts on `dir`
/// whatever repository the caller's environment points at.
#[must_use]
pub fn command(bin: &OsStr, dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(bin);
    command.arg("-C").arg(dir).args(args);
    for name in REPOSITORY_ENV_VARS {
        command.env_remove(name);
    }
    // A pass must never stop on a credential prompt nobody can see.
    command.env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C");
    command
}

/// Runs git and returns its raw output, whatever the exit status.
///
/// # Errors
///
/// Fails when git cannot be started.
pub fn output(bin: &OsStr, dir: &Path, args: &[&str]) -> Result<Output> {
    command(bin, dir, args)
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))
}

/// The first non-empty line of a command's stderr, for reasons and errors.
pub fn first_error_line(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no error output")
        .to_string()
}

/// Runs git, failing with its stderr on a non-zero exit; returns stdout without its trailing
/// newline.
///
/// # Errors
///
/// Fails when git cannot be started or exits non-zero.
pub fn run(bin: &OsStr, dir: &Path, args: &[&str]) -> Result<String> {
    let output = output(bin, dir, args)?;
    if !output.status.success() {
        bail!(
            "git {} failed in {}: {}",
            args.join(" "),
            dir.display(),
            first_error_line(&output)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

/// True when git exits zero.
#[must_use]
pub fn is_ok(bin: &OsStr, dir: &Path, args: &[&str]) -> bool {
    output(bin, dir, args).is_ok_and(|output| output.status.success())
}

/// `git rev-parse` output as an absolute path (git may answer relative to `dir`).
///
/// # Errors
///
/// Fails when git fails.
pub fn path_of(bin: &OsStr, dir: &Path, args: &[&str]) -> Result<PathBuf> {
    let path = PathBuf::from(run(bin, dir, args)?);
    Ok(if path.is_absolute() {
        path
    } else {
        dir.join(path)
    })
}

/// The commit `refname` points at, or `None` when it does not exist.
#[must_use]
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

#[must_use]
pub fn is_ancestor(bin: &OsStr, dir: &Path, ancestor: &str, of: &str) -> bool {
    is_ok(bin, dir, &["merge-base", "--is-ancestor", ancestor, of])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_drops_the_variables_that_point_git_at_another_repository() {
        let command = command(OsStr::new("git"), Path::new("/Users/me/app"), &["status"]);
        let removed: Vec<_> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        for name in REPOSITORY_ENV_VARS {
            assert!(
                removed.iter().any(|removed| removed == name),
                "{name} is not removed"
            );
        }
    }
}
