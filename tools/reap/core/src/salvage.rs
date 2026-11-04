//! ADR dirty-merged-worktrees-are-salvaged: a merged worktree's uncommitted changes are saved as
//! one commit under `refs/reap/salvage/` in the main clone and checked, so the worktree can then
//! be removed with `--force`; and the listing and pruning of those refs.
//!
//! The save never touches the worktree's own index: `git add -A` runs against a copy of it. It
//! never uses `git stash`, which is one stack shared by every worktree of the clone.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

use crate::classify::Salvage;
use crate::config::{Config, Policy, Repo};
use crate::ctx::Ctx;
use crate::git;
use crate::paths::Paths;
use crate::report::shown_size;

/// Where the saves live, in the main clone.
pub const PREFIX: &str = "refs/reap/salvage/";
/// The subject of a save: `git-reap salvage: <worktree> (<branch>)`.
const SUBJECT: &str = "git-reap salvage: ";
/// What a save of a detached HEAD names as its branch.
const DETACHED: &str = "detached HEAD";

/// One save, as `git reap salvaged` and `status` list it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Salvaged {
    /// `refs/reap/salvage/<folder>/<YYYYMMDD-HHMMSS>`.
    pub refname: String,
    pub commit: String,
    /// The worktree it was saved from, as its subject names it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The save's commit date, RFC 3339.
    pub saved_at: String,
    /// When a sweep deletes the ref, RFC 3339; under `policy = "auto"` only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prunes_at: Option<String>,
    /// The commands that bring the changes back as uncommitted, in a new worktree.
    pub restore: String,
}

/// One listed repo's saves.
#[derive(Debug, Clone, Serialize)]
pub struct RepoSalvaged {
    pub repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub salvaged: Vec<Salvaged>,
}

/// The guards of decision 1 and the newest change of decision 2, for a merged worktree with
/// changes. `last_activity` is its last commit or checkout.
pub(crate) fn check(
    paths: &Paths,
    worktree: &Path,
    max_untracked: u64,
    last_activity: Option<DateTime<Utc>>,
) -> Salvage {
    match inspect(paths, worktree, max_untracked) {
        Ok(Ok(newest)) => Salvage::Ready {
            last_change: newest
                .max(last_activity)
                .unwrap_or(DateTime::<Utc>::MIN_UTC),
        },
        Ok(Err(why)) => Salvage::Held(why),
        Err(e) => Salvage::Held(format!("its changes could not be read: {e:#}")),
    }
}

/// `Ok(Ok(newest change))`, or `Ok(Err(the guard that holds it))`.
fn inspect(
    paths: &Paths,
    worktree: &Path,
    max_untracked: u64,
) -> Result<Result<Option<DateTime<Utc>>, String>> {
    let untracked = files(
        paths,
        worktree,
        &["ls-files", "-z", "--others", "--exclude-standard"],
    )?;
    // git lists an untracked folder whole, with a trailing slash, only when it holds a
    // repository; `add -A` would then record a gitlink and none of its files.
    if let Some(folder) = untracked.iter().find(|file| file.ends_with('/')) {
        return Ok(Err(if worktree.join(folder).join(".git").exists() {
            format!("an untracked repository at {folder} would be lost")
        } else {
            format!("git lists the untracked folder {folder} without its files")
        }));
    }
    let untracked_bytes: u64 = untracked
        .iter()
        .filter_map(|file| std::fs::symlink_metadata(worktree.join(file)).ok())
        .map(|metadata| metadata.len())
        .sum();
    if untracked_bytes > max_untracked {
        return Ok(Err(format!(
            "its untracked files take {}, over the {} a save may copy",
            shown_size(untracked_bytes),
            shown_size(max_untracked)
        )));
    }
    let tracked = files(paths, worktree, &["ls-files", "-z"])?;
    let newest = tracked
        .iter()
        .chain(&untracked)
        .filter_map(|file| std::fs::symlink_metadata(worktree.join(file)).ok())
        .filter_map(|metadata| metadata.modified().ok())
        .max()
        .map(DateTime::<Utc>::from);
    Ok(Ok(newest))
}

/// The NUL-separated paths a `git ls-files -z` prints.
fn files(paths: &Paths, worktree: &Path, args: &[&str]) -> Result<Vec<String>> {
    let output = git::run(&paths.git, worktree, args)?;
    Ok(output
        .split('\0')
        .filter(|file| !file.is_empty())
        .map(str::to_string)
        .collect())
}

/// What [`save`] needs to know about the worktree.
pub(crate) struct Source<'a> {
    /// As shown, with `~`.
    pub key: &'a str,
    pub branch: Option<&'a str>,
    pub head: &'a str,
}

/// Saves the worktree's state as a commit under a new salvage ref in `repo`'s clone, and checks
/// it (decision 3). Returns the ref, or `Ok(Err(why))` when a step or a check failed, in which
/// case nothing may be removed.
pub(crate) fn save(
    ctx: &Ctx,
    repo: &Repo,
    source: &Source,
    now: DateTime<Utc>,
) -> Result<Result<String, String>> {
    let paths = &ctx.paths;
    let worktree = paths.expand(source.key);
    let folder = worktree.file_name().map_or_else(
        || "worktree".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    let stamp = now.with_timezone(&chrono::Local).format("%Y%m%d-%H%M%S");
    let refname = format!("{PREFIX}{}/{stamp}", ref_component(&folder));
    // The guards again: the inspection is seconds old.
    match check(paths, &worktree, repo.salvage_max_untracked, None) {
        Salvage::Held(why) => return Ok(Err(why)),
        Salvage::Ready { last_change } => {
            let grace =
                chrono::Duration::from_std(repo.dirty_grace).unwrap_or(chrono::Duration::MAX);
            if now < last_change + grace {
                return Ok(Err("it changed since reap looked at it".into()));
            }
        }
    }
    let index = git::path_of(
        &paths.git,
        &worktree,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    )?;
    let copy = Copy::of(&index, ctx.dry_run)?;
    let with_copy = [("GIT_INDEX_FILE", copy.path.as_os_str())];
    let branch = source.branch.unwrap_or(DETACHED);
    let subject = format!("{SUBJECT}{} ({branch})", source.key);
    if ctx.dry_run {
        ctx.write_output_with(&worktree, &["add", "-A"], &with_copy)?;
        ctx.write_output_with(&worktree, &["write-tree"], &with_copy)?;
        ctx.write_output(
            &worktree,
            &["commit-tree", "<tree>", "-p", source.head, "-m", &subject],
        )?;
        ctx.write_output(&repo.path, &["update-ref", &refname, "<commit>", "''"])?;
        return Ok(Ok(refname));
    }
    if let Err(why) = step(ctx.write_output_with(&worktree, &["add", "-A"], &with_copy)?) {
        return Ok(Err(why));
    }
    let tree = match step(ctx.write_output_with(&worktree, &["write-tree"], &with_copy)?) {
        Ok(tree) => tree,
        Err(why) => return Ok(Err(why)),
    };
    let body = format!(
        "Uncommitted state of the worktree before removal on {stamp}.\n\nHEAD: {}",
        source.head
    );
    let date = format!("@{} +0000", now.timestamp());
    let dated: [(&str, &OsStr); 2] = [
        ("GIT_AUTHOR_DATE", OsStr::new(&date)),
        ("GIT_COMMITTER_DATE", OsStr::new(&date)),
    ];
    let commit = step(ctx.write_output_with(
        &worktree,
        &[
            "commit-tree",
            &tree,
            "-p",
            source.head,
            "-m",
            &subject,
            "-m",
            &body,
        ],
        &dated,
    )?);
    let commit = match commit {
        Ok(commit) => commit,
        Err(why) => return Ok(Err(why)),
    };
    if let Err(why) = matches_worktree(paths, &worktree, &copy.path) {
        return Ok(Err(why));
    }
    if let Err(why) = step(ctx.write_output(&repo.path, &["update-ref", &refname, &commit, ""])?) {
        return Ok(Err(why));
    }
    let stored = git::resolve(&paths.git, &repo.path, &refname);
    let stored_tree = git::run(
        &paths.git,
        &repo.path,
        &["rev-parse", "--verify", &format!("{refname}^{{tree}}")],
    )
    .ok();
    if stored.as_deref() != Some(commit.as_str()) || stored_tree.as_deref() != Some(tree.as_str()) {
        return Ok(Err(format!("{refname} does not hold the save")));
    }
    Ok(Ok(refname))
}

/// A write's stdout, trimmed, or git's message when it failed. Never called in a dry run.
fn step(output: Option<std::process::Output>) -> Result<String, String> {
    let output = output.ok_or_else(|| "not run in a dry run".to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(git::first_error_line(&output))
    }
}

/// Decision 3.3: against the copy of the index, nothing differs and nothing is untracked, so
/// every change and untracked file is in the saved tree.
fn matches_worktree(paths: &Paths, worktree: &Path, index: &Path) -> Result<(), String> {
    let with_copy = |args: &[&str]| {
        git::command(&paths.git, worktree, args)
            .env("GIT_INDEX_FILE", index)
            .output()
            .map_err(|e| format!("running git {}: {e}", args.join(" ")))
    };
    let diff = with_copy(&["diff", "--quiet", "--ignore-submodules=none"])?;
    if !diff.status.success() {
        return Err("the save does not match the worktree; it changed while saving".into());
    }
    let others = with_copy(&["ls-files", "--others", "--exclude-standard"])?;
    if !others.status.success() || !others.stdout.is_empty() {
        return Err("files the save does not hold appeared while saving".into());
    }
    Ok(())
}

/// The copy of a worktree's index a save stages into, deleted when dropped.
struct Copy {
    path: PathBuf,
}

impl Copy {
    /// Beside `index`, so it lives and goes with the worktree's git dir. A worktree with no index
    /// yet stages into an empty one, which `add -A` fills from the whole tree.
    fn of(index: &Path, dry_run: bool) -> Result<Self> {
        let path = index.with_file_name("reap-salvage.index");
        if dry_run {
            return Ok(Self { path });
        }
        let _ = std::fs::remove_file(&path);
        if index.exists() {
            std::fs::copy(index, &path)
                .with_context(|| format!("copying {} for the save", index.display()))?;
        }
        Ok(Self { path })
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A folder name as one ref component: what `git check-ref-format` refuses becomes `-`.
fn ref_component(name: &str) -> String {
    let mut component: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    while component.contains("..") {
        component = component.replace("..", "-");
    }
    let component = component.trim_start_matches('.').to_string();
    let component = match component.strip_suffix(".lock") {
        Some(stem) => format!("{stem}-lock"),
        None => component,
    };
    let component = component.trim_end_matches('.').to_string();
    if component.is_empty() {
        "worktree".into()
    } else {
        component
    }
}

/// Every salvage ref in `repo`'s clone, oldest first, with its commit date.
pub(crate) fn list(paths: &Paths, repo: &Repo) -> Result<Vec<(Salvaged, DateTime<Utc>)>> {
    let output = git::run(
        &paths.git,
        &repo.path,
        &[
            "for-each-ref",
            "--sort=committerdate",
            "--format=%(refname)%00%(objectname)%00%(committerdate:unix)%00%(contents:subject)",
            PREFIX,
        ],
    )?;
    let max_age = chrono::Duration::from_std(repo.salvage_max_age).unwrap_or(chrono::Duration::MAX);
    Ok(output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            let (refname, commit, date, subject) = (
                fields.next()?,
                fields.next()?,
                fields.next()?,
                fields.next()?,
            );
            let saved_at = DateTime::from_timestamp(date.parse().ok()?, 0)?;
            let (worktree, branch) = parse_subject(subject);
            let restore = restore(paths, repo, refname, worktree.as_deref());
            let salvaged = Salvaged {
                refname: refname.to_string(),
                commit: commit.to_string(),
                worktree,
                branch,
                saved_at: rfc3339(saved_at),
                prunes_at: (repo.policy == Policy::Auto).then(|| rfc3339(saved_at + max_age)),
                restore,
            };
            Some((salvaged, saved_at))
        })
        .collect())
}

/// `git-reap salvage: <worktree> (<branch>)` as its worktree and branch.
fn parse_subject(subject: &str) -> (Option<String>, Option<String>) {
    let Some(rest) = subject.strip_prefix(SUBJECT) else {
        return (None, None);
    };
    match rest
        .strip_suffix(')')
        .and_then(|rest| rest.rsplit_once(" ("))
    {
        Some((worktree, DETACHED)) => (Some(worktree.to_string()), None),
        Some((worktree, branch)) => (Some(worktree.to_string()), Some(branch.to_string())),
        None => (Some(rest.to_string()), None),
    }
}

/// `git -C <clone> worktree add <path> <ref> && git -C <path> reset HEAD~1`: the save checked
/// out detached, then its commit undone so the changes are uncommitted again. The path is the
/// one it was saved from, else a folder beside the clone named for the ref.
fn restore(paths: &Paths, repo: &Repo, refname: &str, worktree: Option<&str>) -> String {
    let path = worktree.map_or_else(
        || {
            let folder = refname
                .strip_prefix(PREFIX)
                .and_then(|rest| rest.split('/').next())
                .unwrap_or("salvage");
            paths.tilde(
                &repo
                    .path
                    .with_file_name(format!("{}-{folder}", repo.name())),
            )
        },
        str::to_string,
    );
    let path = quoted(&path);
    let refname = quoted(refname);
    format!(
        "git -C {} worktree add {path} {refname} && git -C {path} reset HEAD~1",
        quoted(&paths.tilde(&repo.path))
    )
}

/// A path as a shell word: as it is when it is plain, else in single quotes (`~/` left outside
/// them so the shell still expands it).
fn quoted(path: &str) -> String {
    let is_plain = |text: &str| {
        text.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '~' | '+'))
    };
    if is_plain(path) {
        return path.to_string();
    }
    let (home, rest) = path
        .strip_prefix("~/")
        .map_or(("", path), |rest| ("~/", rest));
    format!("{home}'{}'", rest.replace('\'', "'\\''"))
}

/// Decision 4: deletes the salvage refs older than `salvage_max_age`, only under `policy =
/// "auto"`. Returns the refs deleted (or, in a dry run, that would be).
pub(crate) fn prune(ctx: &Ctx, repo: &Repo, now: DateTime<Utc>) -> Result<Vec<String>> {
    if repo.policy != Policy::Auto {
        return Ok(Vec::new());
    }
    let max_age = chrono::Duration::from_std(repo.salvage_max_age).unwrap_or(chrono::Duration::MAX);
    let mut pruned = Vec::new();
    for (salvaged, saved_at) in list(&ctx.paths, repo)? {
        if now < saved_at + max_age {
            continue;
        }
        // The old value makes git refuse if the ref moved since it was listed.
        let args = ["update-ref", "-d", &salvaged.refname, &salvaged.commit];
        match ctx.write_output(&repo.path, &args)? {
            Some(output) if !output.status.success() => {
                // One stuck ref must not abort the pass; the next pass tries it again.
                ctx.note(format!(
                    "could not delete {}: {}",
                    salvaged.refname,
                    crate::git::first_error_line(&output)
                ));
            }
            _ => pruned.push(salvaged.refname),
        }
    }
    Ok(pruned)
}

/// `git reap salvaged`: the saves of every listed repo.
///
/// # Errors
///
/// Fails when `repos.toml` cannot be read; a repo that cannot be read only fills its own
/// `error`.
pub fn salvaged(paths: &Paths) -> Result<Vec<RepoSalvaged>> {
    let repos = Config::load(paths)?.repos(paths)?;
    Ok(repos
        .iter()
        .map(|repo| {
            let (salvaged, error) = match list(paths, repo) {
                Ok(listed) => (
                    listed.into_iter().map(|(salvaged, _)| salvaged).collect(),
                    None,
                ),
                Err(e) => (Vec::new(), Some(format!("{e:#}"))),
            };
            RepoSalvaged {
                repo: paths.tilde(&repo.path),
                error,
                salvaged,
            }
        })
        .collect())
}

/// Human-readable `salvaged`.
#[must_use]
pub fn render(repos: &[RepoSalvaged]) -> String {
    let mut lines = Vec::new();
    for repo in repos {
        lines.push(repo.repo.clone());
        if let Some(error) = &repo.error {
            lines.push(format!("  cannot read: {error}"));
        }
        if repo.error.is_none() && repo.salvaged.is_empty() {
            lines.push("  nothing salvaged".into());
        }
        lines.extend(repo.salvaged.iter().flat_map(describe));
    }
    if repos.is_empty() {
        lines.push("no repos listed; add a main clone with `git reap add <path>`".into());
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// One save in three lines: the ref, what it came from and when, and how to restore it.
pub(crate) fn describe(salvaged: &Salvaged) -> Vec<String> {
    let local = |time: &str| {
        DateTime::parse_from_rfc3339(time).map_or_else(
            |_| time.to_string(),
            |time| {
                time.with_timezone(&chrono::Local)
                    .format("%a %-d %b %H:%M")
                    .to_string()
            },
        )
    };
    let from = match (&salvaged.worktree, &salvaged.branch) {
        (Some(worktree), Some(branch)) => format!("{worktree} ({branch}), "),
        (Some(worktree), None) => format!("{worktree}, "),
        (None, _) => String::new(),
    };
    let prunes = salvaged
        .prunes_at
        .as_deref()
        .map(|time| format!("; deleted after {}", local(time)))
        .unwrap_or_default();
    vec![
        format!("  salvaged   {}", salvaged.refname),
        format!(
            "             {from}saved {}{prunes}",
            local(&salvaged.saved_at)
        ),
        format!("             restore: {}", salvaged.restore),
    ]
}

fn rfc3339(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_restore_quotes_a_refname_with_shell_characters() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path(), &dir.path().join("cfg"));
        let repo = Repo {
            path: dir.path().join("app"),
            remote: "origin".into(),
            base: None,
            policy: Policy::Auto,
            grace: std::time::Duration::ZERO,
            stale_after: std::time::Duration::ZERO,
            dirty_grace: std::time::Duration::ZERO,
            salvage_max_untracked: 0,
            salvage_max_age: std::time::Duration::ZERO,
        };

        let line = restore(&paths, &repo, "refs/reap/salvage/x$(id)/1", Some("/w"));

        assert!(line.contains("'refs/reap/salvage/x$(id)/1'"), "{line}");
    }

    #[test]
    fn a_folder_name_becomes_one_valid_ref_component() {
        assert_eq!(ref_component("app-feat-b00f95"), "app-feat-b00f95");
        assert_eq!(ref_component("my work"), "my-work");
        assert_eq!(ref_component("..hidden..x"), "-hidden-x");
        assert_eq!(ref_component(".dot"), "dot");
        assert_eq!(ref_component("x.lock"), "x-lock");
        assert_eq!(ref_component("a:b~c^d?e*f[g\\h"), "a-b-c-d-e-f-g-h");
        assert_eq!(ref_component("..."), "-");
        assert_eq!(ref_component("."), "worktree");
    }

    #[test]
    fn a_subject_names_the_worktree_and_branch() {
        assert_eq!(
            parse_subject("git-reap salvage: ~/app/.claude/worktrees/x (claude/x)"),
            (
                Some("~/app/.claude/worktrees/x".into()),
                Some("claude/x".into())
            )
        );
        assert_eq!(
            parse_subject("git-reap salvage: ~/w (detached HEAD)"),
            (Some("~/w".into()), None)
        );
        assert_eq!(
            parse_subject("git-reap salvage: ~/odd (name) (feat)"),
            (Some("~/odd (name)".into()), Some("feat".into()))
        );
        assert_eq!(parse_subject("something else"), (None, None));
    }

    #[test]
    fn a_path_is_quoted_only_when_the_shell_needs_it() {
        assert_eq!(quoted("~/Development/app-feat"), "~/Development/app-feat");
        assert_eq!(quoted("~/my work"), "~/'my work'");
        assert_eq!(quoted("/srv/it's"), "'/srv/it'\\''s'");
    }
}
