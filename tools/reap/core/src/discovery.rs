//! Clone discovery (repo ADR 0004): reap suggests the clones that have linked worktrees, the
//! operator lists them. A scan only reads. It takes no lock on any clone, runs no `git`, `gh`,
//! fetch or `ls-remote`, and deletes nothing; the one thing it writes is `discovery.json`.
//!
//! The walk is `find-clones`'. The file holds what passes reap's filter (rule 9: at least one
//! entry under `<git dir>/worktrees/`) but not the listed-and-skipped filter, which [`view`]
//! applies whenever it reads, so *Add* and *Skip* take a row away at once and *Unskip* brings it
//! back at once if the last scan found it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use chrono::{Local, SecondsFormat};
use serde::{Deserialize, Serialize};

use crate::add;
use crate::config::Config;
use crate::paths::{Paths, absolute, same_path};

pub use find_clones::Bounds;
use find_clones::{RootOutcome, default_roots, scan_root};

/// A clone worth listing: it has worktrees to reap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    /// `~/…`.
    pub path: String,
    /// The folder's name.
    pub name: String,
    /// The scan root it was found under, `~/…`.
    pub root: String,
    /// Linked worktrees it has now.
    pub worktrees: usize,
}

/// What `discovery.json` holds.
pub type Found = find_clones::Discovery<Suggestion>;

/// One scan root with what the last scan made of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RootInfo {
    pub path: String,
    /// Clones the walk found, before reap's filter; `None` when the last scan did not cover it
    /// (or none ran).
    pub clones: Option<usize>,
    pub stopped_early: bool,
    pub error: Option<String>,
}

/// A path in `skip`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedClone {
    pub path: String,
    pub name: String,
    /// The longest scan root that contains it.
    pub root: Option<String>,
}

/// What the popover and `git reap discover --json` show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    pub scanned_at: Option<String>,
    /// The root being walked now; the caller knows, the file does not.
    pub scanning: Option<String>,
    pub roots: Vec<RootInfo>,
    /// Listed and skipped clones already removed; most worktrees first.
    pub suggestions: Vec<Suggestion>,
    pub skipped: Vec<SkippedClone>,
}

/// The resolved scan roots in order: `scan` if the file has it, else the usual folders that
/// exist under home (rule 4).
///
/// # Errors
///
/// Fails when a `scan` entry is neither absolute nor `~/…`.
pub fn roots(config: &Config, paths: &Paths) -> Result<Vec<PathBuf>> {
    Ok(config
        .scan_roots(paths)?
        .unwrap_or_else(|| default_roots(&paths.home)))
}

/// Walks every root, one at a time, and replaces the suggestions in `discovery.json`. After each
/// root the file holds what is found so far and `on_root` is told the root it just left (`~/…`);
/// `scanned_at` is set with the last root's result. Facts are local: the entries under
/// `<git dir>/worktrees/`, nothing from a remote.
///
/// # Errors
///
/// Fails when `repos.toml` is not valid or `discovery.json` cannot be written. A root that
/// cannot be read is not an error: its outcome says so.
pub fn scan(paths: &Paths, bounds: &Bounds, on_root: &mut dyn FnMut(&str)) -> Result<Found> {
    let config = Config::load(paths)?;
    let roots = roots(&config, paths)?;
    let file = paths.discovery_file();
    let mut found = Found::update(&file, |all| {
        *all = Found::default();
        if roots.is_empty() {
            all.scanned_at = Some(now());
        }
    })?;
    // Roots can overlap; a clone is suggested once, under the first root that has it.
    let mut seen = HashSet::new();
    for (index, root) in roots.iter().enumerate() {
        let walked = scan_root(root, bounds);
        let name = paths.tilde(root);
        let mut suggestions: Vec<Suggestion> = walked
            .clones
            .iter()
            .filter_map(|clone| suggestion(paths, clone, &name))
            .filter(|s| seen.insert(absolute(&paths.expand(&s.path))))
            .collect();
        suggestions.sort_by(|a, b| b.worktrees.cmp(&a.worktrees).then_with(|| order(a, b)));
        let outcome = RootOutcome {
            root: name.clone(),
            clones: walked.clones.len(),
            stopped_early: walked.stopped_early,
            error: walked.error,
        };
        let last = index + 1 == roots.len();
        found = Found::update(&file, |all| {
            all.roots.push(outcome);
            all.suggestions.extend(suggestions);
            if last {
                all.scanned_at = Some(now());
            }
        })?;
        on_root(&name);
    }
    Ok(found)
}

fn now() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn order(a: &Suggestion, b: &Suggestion) -> std::cmp::Ordering {
    a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path))
}

/// The clone as a suggestion, or `None` when it has no linked worktree (rule 9).
fn suggestion(paths: &Paths, clone: &Path, root: &str) -> Option<Suggestion> {
    let worktrees = worktrees(clone);
    (worktrees > 0).then(|| Suggestion {
        path: paths.tilde(clone),
        name: name_of(clone),
        root: root.to_owned(),
        worktrees,
    })
}

/// How many linked worktrees the main clone at `clone` has: the folders under
/// `<clone>/.git/worktrees/`. Plain file reads; the walk only reports clones whose `.git` is a
/// directory.
fn worktrees(clone: &Path) -> usize {
    std::fs::read_dir(clone.join(".git").join("worktrees")).map_or(0, |entries| {
        entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .count()
    })
}

/// What `discovery.json` and `repos.toml` say now, with the listed and skipped clones taken out.
/// `scanning` is the root being walked, if the caller is walking one.
///
/// # Errors
///
/// Fails when `repos.toml` is not valid (a bad `scan` or `skip` entry included); the caller
/// shows the message.
pub fn view(paths: &Paths, scanning: Option<String>) -> Result<View> {
    let config = Config::load(paths)?;
    let listed = config.repos(paths)?;
    let roots = roots(&config, paths)?;
    let skipped = config.skipped(paths)?;
    let found = Found::load(&paths.discovery_file()).unwrap_or_default();

    let hidden: Vec<PathBuf> = listed
        .iter()
        .map(|repo| absolute(&repo.path))
        .chain(skipped.iter().map(|path| absolute(path)))
        .collect();
    let root_names: Vec<String> = roots.iter().map(|root| paths.tilde(root)).collect();

    let root_infos = roots
        .iter()
        .zip(&root_names)
        .map(|(root, name)| {
            let outcome = found
                .roots
                .iter()
                .find(|outcome| same_path(&paths.expand(&outcome.root), root));
            RootInfo {
                path: name.clone(),
                clones: outcome.map(|outcome| outcome.clones),
                stopped_early: outcome.is_some_and(|outcome| outcome.stopped_early),
                error: outcome.and_then(|outcome| outcome.error.clone()),
            }
        })
        .collect();

    let suggestions = found
        .suggestions
        .into_iter()
        .filter(|s| {
            let in_a_root = roots
                .iter()
                .any(|root| same_path(&paths.expand(&s.root), root));
            in_a_root && !hidden.contains(&absolute(&paths.expand(&s.path)))
        })
        .collect();

    let skipped = skipped
        .iter()
        .map(|path| {
            let at = absolute(path);
            let root = roots
                .iter()
                .zip(&root_names)
                .filter(|(root, _)| at.starts_with(absolute(root)))
                .max_by_key(|(root, _)| root.components().count())
                .map(|(_, name)| name.clone());
            SkippedClone {
                path: paths.tilde(path),
                name: name_of(path),
                root,
            }
        })
        .collect();

    Ok(View {
        scanned_at: found.scanned_at,
        scanning,
        roots: root_infos,
        suggestions,
        skipped,
    })
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| "repo".into(), |name| name.to_string_lossy().into_owned())
}

/// The path as an absolute one, `~/…` expanded.
fn absolute_arg(paths: &Paths, text: &str) -> Result<PathBuf> {
    let path = paths.expand(text);
    if !path.is_absolute() {
        bail!("{text} must be an absolute path or start with ~/");
    }
    Ok(path)
}

/// Stops suggesting the clone at `path` (adds it to `skip` once). Allowed for a listed clone: it
/// only edits `skip`.
///
/// # Errors
///
/// Fails when `path` is relative or `repos.toml` cannot be read or written.
pub fn skip(paths: &Paths, path: &str) -> Result<()> {
    let written = paths.tilde(&absolute(&absolute_arg(paths, path)?));
    let mut config = Config::load(paths)?;
    config.repos(paths)?;
    if config.add_skip(paths, &written) {
        config.save(paths)?;
    }
    Ok(())
}

/// Takes `path` out of `skip`. Nothing happens when it is not there.
///
/// # Errors
///
/// Fails when `path` is relative or `repos.toml` cannot be read or written.
pub fn unskip(paths: &Paths, path: &str) -> Result<()> {
    let path = absolute_arg(paths, path)?;
    let mut config = Config::load(paths)?;
    config.repos(paths)?;
    if config.remove_skip(paths, &path) {
        config.save(paths)?;
    }
    Ok(())
}

/// Adds a scan root. The first edit of the roots writes the resolved list, so the usual folders
/// become explicit. Runs nothing: the clones under it show after the next scan.
///
/// # Errors
///
/// Fails when `path` is relative or `repos.toml` cannot be read or written.
pub fn add_root(paths: &Paths, path: &str) -> Result<()> {
    let root = absolute_arg(paths, path)?;
    let mut config = Config::load(paths)?;
    config.repos(paths)?;
    let mut entries = explicit_roots(&config, paths);
    if entries
        .iter()
        .any(|entry| same_path(&paths.expand(entry), &root))
    {
        return Ok(());
    }
    entries.push(paths.tilde(&absolute(&root)));
    config.scan = Some(entries);
    config.save(paths)
}

/// Removes a scan root and drops its suggestions and outcome from `discovery.json` at once.
/// Nothing happens when it is not a root.
///
/// # Errors
///
/// Fails when `path` is relative or `repos.toml` or `discovery.json` cannot be read or written.
pub fn remove_root(paths: &Paths, path: &str) -> Result<()> {
    let root = absolute_arg(paths, path)?;
    let mut config = Config::load(paths)?;
    config.repos(paths)?;
    let mut entries = explicit_roots(&config, paths);
    let before = entries.len();
    entries.retain(|entry| !same_path(&paths.expand(entry), &root));
    if entries.len() == before {
        return Ok(());
    }
    config.scan = Some(entries);
    config.save(paths)?;
    let file = paths.discovery_file();
    if Found::load(&file).is_some() {
        Found::update(&file, |all| {
            all.roots
                .retain(|outcome| !same_path(&paths.expand(&outcome.root), &root));
            all.suggestions
                .retain(|s| !same_path(&paths.expand(&s.root), &root));
        })?;
    }
    Ok(())
}

/// The roots as written in `scan`, or the usual folders written out while it is absent.
fn explicit_roots(config: &Config, paths: &Paths) -> Vec<String> {
    config.scan.clone().unwrap_or_else(|| {
        default_roots(&paths.home)
            .iter()
            .map(|root| paths.tilde(root))
            .collect()
    })
}

/// Accepts a suggestion: reap's own `add`, with every check it makes (a folder, a clone's top
/// level, not a linked worktree, not listed). Its error text is for the operator as it is.
/// Returns the path as written to `repos.toml`.
///
/// # Errors
///
/// Fails as [`add::add`] does.
pub fn add_suggestion(paths: &Paths, path: &str) -> Result<String> {
    add::add(paths, &absolute_arg(paths, path)?, None, None)
}

/// The terminal form of `git reap discover`: the roots, then a line per suggestion.
#[must_use]
pub fn render(view: &View) -> String {
    let mut lines = Vec::new();
    if view.roots.is_empty() {
        lines.push("no folders to look in; `scan` in repos.toml names them".to_owned());
    } else {
        let names: Vec<&str> = view.roots.iter().map(|root| root.path.as_str()).collect();
        lines.push(format!("looked in {}", names.join(", ")));
    }
    for root in &view.roots {
        if let Some(error) = &root.error {
            lines.push(format!("  {}: {error}", root.path));
        } else if root.stopped_early {
            lines.push(format!(
                "  {}: stopped early, the list may be incomplete",
                root.path
            ));
        }
    }
    if view.suggestions.is_empty() {
        lines.push("no unlisted clone with worktrees found".to_owned());
    }
    for found in &view.suggestions {
        let noun = if found.worktrees == 1 {
            "worktree"
        } else {
            "worktrees"
        };
        lines.push(format!(
            "{}  {} {noun}  {}",
            found.name, found.worktrees, found.path
        ));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suggestion(name: &str, worktrees: usize) -> Suggestion {
        Suggestion {
            path: format!("~/Code/{name}"),
            name: name.into(),
            root: "~/Code".into(),
            worktrees,
        }
    }

    fn view_of(suggestions: Vec<Suggestion>, roots: Vec<RootInfo>) -> View {
        View {
            scanned_at: None,
            scanning: None,
            roots,
            suggestions,
            skipped: Vec::new(),
        }
    }

    fn root(path: &str, error: Option<&str>) -> RootInfo {
        RootInfo {
            path: path.into(),
            clones: None,
            stopped_early: false,
            error: error.map(Into::into),
        }
    }

    #[test]
    fn a_suggestion_line_is_name_count_path() {
        let view = view_of(
            vec![suggestion("app", 2), suggestion("api", 1)],
            vec![root("~/Code", None)],
        );

        assert_eq!(
            render(&view),
            "looked in ~/Code\napp  2 worktrees  ~/Code/app\napi  1 worktree  ~/Code/api\n"
        );
    }

    #[test]
    fn the_text_says_when_there_is_nothing_to_look_in_or_nothing_found() {
        assert!(render(&view_of(Vec::new(), Vec::new())).starts_with("no folders to look in"));
        let view = view_of(Vec::new(), vec![root("~/gone", Some("no such folder"))]);
        assert_eq!(
            render(&view),
            "looked in ~/gone\n  ~/gone: no such folder\nno unlisted clone with worktrees found\n"
        );
    }

    #[test]
    fn the_view_serialises_with_the_contract_names() {
        let view = View {
            scanned_at: Some("2026-11-04T10:00:00+02:00".into()),
            scanning: None,
            roots: vec![RootInfo {
                clones: Some(5),
                ..root("~/Code", None)
            }],
            suggestions: vec![suggestion("app", 2)],
            skipped: vec![SkippedClone {
                path: "~/Code/scratch".into(),
                name: "scratch".into(),
                root: None,
            }],
        };

        assert_eq!(
            serde_json::to_value(&view).unwrap(),
            serde_json::json!({
                "scanned_at": "2026-11-04T10:00:00+02:00",
                "scanning": null,
                "roots": [{"path": "~/Code", "clones": 5, "stopped_early": false, "error": null}],
                "suggestions": [{"path": "~/Code/app", "name": "app", "root": "~/Code", "worktrees": 2}],
                "skipped": [{"path": "~/Code/scratch", "name": "scratch", "root": null}],
            })
        );
    }
}
