//! Clone discovery (repository ADR 0004): `git tether discover` and the popover's *Find clones* walk the
//! folders in `scan`, and tether *suggests* the clones it finds. A suggestion reaches
//! `repos.toml` only through [`accept`], which is `add` with every check it makes.
//!
//! The walk is the `find-clones` crate's. tether's filter is local facts only (no fetch, no
//! `ls-remote`): a clone is suggested when it has a remote and a branch known from its
//! remote-tracking refs, and it is not concord's clone while concord is installed (rule 8 of that ADR).
//!
//! `discovery.json` keeps the suggestions that pass that filter. Whether one is already listed
//! or skipped is decided when the file is read ([`view`]), so *Add*, *Skip* and *Unskip* change
//! what shows at once, without a scan.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use find_clones::{Bounds, RootOutcome, default_roots, scan_root};
use serde::{Deserialize, Serialize};

use crate::add::{self, AddOpts};
use crate::config::{Config, DEFAULT_REMOTE, Mode, Repo, absolute, same_path};
use crate::ctx::Ctx;
use crate::git;
use crate::paths::Paths;
use crate::record;

/// One clone discovery found: *Add* lists it following `branch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    /// `~/…` when under `$HOME`.
    pub path: String,
    /// The folder's name.
    pub name: String,
    /// The scan root it was found under, as [`Suggestion::path`] is written.
    pub root: String,
    /// The branch it would follow.
    pub branch: String,
}

/// What the last scan left in `discovery.json`.
pub type Found = find_clones::Discovery<Suggestion>;

/// A scan root with what the last scan found in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RootInfo {
    pub path: String,
    /// Main clones the walk found, before the filter; `None` when the last scan did not cover it.
    pub clones: Option<usize>,
    pub stopped_early: bool,
    pub error: Option<String>,
}

/// A skipped clone, for *Unskip*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Skipped {
    pub path: String,
    pub name: String,
    /// The longest scan root it sits under, if any.
    pub root: Option<String>,
}

/// What the popover and `discover --json` show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    pub scanned_at: Option<String>,
    /// The root being walked now; the caller knows, so it passes it to [`view`].
    pub scanning: Option<String>,
    pub roots: Vec<RootInfo>,
    /// Listed and skipped clones already removed.
    pub suggestions: Vec<Suggestion>,
    pub skipped: Vec<Skipped>,
}

/// The folders a scan walks: `scan` as written, or the default roots that exist while it is absent.
///
/// # Errors
///
/// Fails, naming the entry, when a `scan` entry is not an absolute or `~/` path.
pub fn roots(config: &Config, paths: &Paths) -> Result<Vec<PathBuf>> {
    Ok(config
        .scan_roots(paths)?
        .unwrap_or_else(|| default_roots(&paths.home)))
}

/// Walks the roots one at a time, in order, and replaces the suggestions in `discovery.json`.
/// After each root the file holds what is found so far and `on_root` is called with that root
/// (`~/…`), so the caller can publish partial results. `scanned_at` is set with the last root.
/// Runs no fetch and no `ls-remote`.
///
/// # Errors
///
/// Fails when `repos.toml` is invalid or `discovery.json` cannot be written.
pub fn scan(paths: &Paths, bounds: &Bounds, on_root: &mut dyn FnMut(&str)) -> Result<Found> {
    let config = Config::load(paths)?;
    config.repos(paths)?;
    let walk = roots(&config, paths)?;
    let file = paths.discovery_file();
    let mut found = Found::update(&file, |d| *d = Found::default())?;
    let last = walk.len().saturating_sub(1);
    for (i, root) in walk.iter().enumerate() {
        let shown = paths.tilde(root);
        let result = scan_root(root, bounds);
        let mut suggestions: Vec<Suggestion> = result
            .clones
            .iter()
            .filter_map(|clone| suggest(paths, clone, &shown))
            .collect();
        suggestions.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
        let outcome = RootOutcome {
            root: shown.clone(),
            clones: result.clones.len(),
            stopped_early: result.stopped_early,
            error: result.error,
        };
        found = Found::update(&file, |d| {
            d.roots.push(outcome);
            for s in suggestions {
                // Roots may nest; a clone under two of them is one suggestion.
                if !d.suggestions.iter().any(|o| o.path == s.path) {
                    d.suggestions.push(s);
                }
            }
            if i == last {
                d.scanned_at = Some(record::now());
            }
        })?;
        on_root(&shown);
    }
    if walk.is_empty() {
        found = Found::update(&file, |d| d.scanned_at = Some(record::now()))?;
    }
    Ok(found)
}

/// tether's filter for one clone the walk found; `None` means it is not suggested.
fn suggest(paths: &Paths, clone: &Path, root: &str) -> Option<Suggestion> {
    if paths.concord_installed() && same_path(clone, &paths.concord_repo) {
        return None;
    }
    let remote = remote_of(paths, clone)?;
    let branch = branch_of(paths, clone, &remote)?;
    Some(Suggestion {
        path: paths.tilde(clone),
        name: clone.file_name()?.to_string_lossy().into_owned(),
        root: root.to_string(),
        branch,
    })
}

/// `origin` if the clone has it, else its first remote.
fn remote_of(paths: &Paths, clone: &Path) -> Option<String> {
    let out = git::run(&paths.git, clone, &["remote"]).ok()?;
    let names: Vec<&str> = out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    names
        .iter()
        .find(|n| **n == DEFAULT_REMOTE)
        .or_else(|| names.first())
        .map(|n| (*n).to_string())
}

/// The current branch if `remote` tracks it, else the branch `refs/remotes/<remote>/HEAD`
/// points to (when that ref exists too). Local refs only.
fn branch_of(paths: &Paths, clone: &Path, remote: &str) -> Option<String> {
    let tracked = |branch: &str| {
        git::resolve(
            &paths.git,
            clone,
            &format!("refs/remotes/{remote}/{branch}"),
        )
        .is_some()
    };
    if let Some(b) = git::current_branch(&paths.git, clone)
        && tracked(&b)
    {
        return Some(b);
    }
    let head = format!("refs/remotes/{remote}/HEAD");
    let target = git::run(&paths.git, clone, &["symbolic-ref", "--quiet", &head]).ok()?;
    let branch = target.strip_prefix(&format!("refs/remotes/{remote}/"))?;
    tracked(branch).then(|| branch.to_string())
}

/// What the UI and `discover --json` show: the last scan, minus the clones that are listed or
/// skipped now, with each scan root joined to what the scan found in it. `scanning` is the root
/// being walked, which only the caller knows.
///
/// # Errors
///
/// Fails when `repos.toml` is invalid, which includes a bad `scan` or `skip` entry.
pub fn view(paths: &Paths, scanning: Option<String>) -> Result<View> {
    let config = Config::load(paths)?;
    let listed = config.repos(paths)?;
    let walk = roots(&config, paths)?;
    let skipped = config.skipped(paths)?;
    let found = Found::load(&paths.discovery_file()).unwrap_or_default();
    let on_walk = |root: &str| walk.iter().any(|r| same_path(r, &paths.expand(root)));
    let roots = walk
        .iter()
        .map(|r| {
            let outcome = found
                .roots
                .iter()
                .find(|o| same_path(&paths.expand(&o.root), r));
            RootInfo {
                path: paths.tilde(r),
                clones: outcome.map(|o| o.clones),
                stopped_early: outcome.is_some_and(|o| o.stopped_early),
                error: outcome.and_then(|o| o.error.clone()),
            }
        })
        .collect();
    let suggestions = found
        .suggestions
        .into_iter()
        .filter(|s| on_walk(&s.root))
        .filter(|s| {
            let at = paths.expand(&s.path);
            !listed.iter().any(|r| same_path(&r.path, &at))
                && !skipped.iter().any(|k| same_path(k, &at))
        })
        .collect();
    let skipped = skipped
        .iter()
        .map(|k| Skipped {
            path: paths.tilde(k),
            name: name_of(k),
            root: walk
                .iter()
                .filter(|r| absolute(k).starts_with(absolute(r)))
                .max_by_key(|r| r.components().count())
                .map(|r| paths.tilde(r)),
        })
        .collect();
    Ok(View {
        scanned_at: found.scanned_at,
        scanning,
        roots,
        suggestions,
        skipped,
    })
}

fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// `path` as `repos.toml` keeps it: `~/…` under `$HOME`, always absolute.
fn entry_of(paths: &Paths, path: &str) -> Result<String> {
    let expanded = paths.expand(path.trim());
    if !expanded.is_absolute() {
        bail!("{path}: path must be absolute or start with ~/");
    }
    Ok(paths.tilde(&expanded))
}

/// Never suggest `path` again (*Skip*). Skipping a listed clone is allowed; it only edits `skip`.
///
/// # Errors
///
/// Fails when `path` is relative or `repos.toml` cannot be read, validated or saved.
pub fn skip(paths: &Paths, path: &str) -> Result<()> {
    let entry = entry_of(paths, path)?;
    let mut config = Config::load(paths)?;
    config.repos(paths)?;
    if push_skip(&mut config, paths, &entry) {
        config.save(paths)?;
    }
    Ok(())
}

/// Adds `entry` to `skip` unless it is there; whether it changed.
pub(crate) fn push_skip(config: &mut Config, paths: &Paths, entry: &str) -> bool {
    let at = paths.expand(entry);
    if config.skip.iter().any(|k| same_path(&paths.expand(k), &at)) {
        return false;
    }
    config.skip.push(entry.to_string());
    true
}

/// Takes `path` out of `skip` (*Unskip*). A clone the last scan found shows again at once.
///
/// # Errors
///
/// Fails when `path` is relative or not skipped, or `repos.toml` cannot be read, validated or saved.
pub fn unskip(paths: &Paths, path: &str) -> Result<()> {
    let entry = entry_of(paths, path)?;
    let at = paths.expand(&entry);
    let mut config = Config::load(paths)?;
    config.repos(paths)?;
    let before = config.skip.len();
    config.skip.retain(|k| !same_path(&paths.expand(k), &at));
    if config.skip.len() == before {
        bail!("{entry} is not skipped");
    }
    config.save(paths)
}

/// Adds a folder to the scan roots. The first edit writes the resolved list, defaults included,
/// into `scan`. Nothing is scanned until the next [`scan`].
///
/// # Errors
///
/// Fails when `path` is relative or `repos.toml` cannot be read, validated or saved.
pub fn add_root(paths: &Paths, path: &str) -> Result<()> {
    let entry = entry_of(paths, path)?;
    let at = paths.expand(&entry);
    let mut config = Config::load(paths)?;
    config.repos(paths)?;
    let mut list = roots(&config, paths)?;
    if list.iter().any(|r| same_path(r, &at)) {
        return Ok(());
    }
    list.push(at);
    config.scan = Some(list.iter().map(|r| paths.tilde(r)).collect());
    config.save(paths)
}

/// Removes a folder from the scan roots and drops what the last scan found in it at once.
///
/// # Errors
///
/// Fails when `path` is relative or `repos.toml` or `discovery.json` cannot be read, validated or written.
pub fn remove_root(paths: &Paths, path: &str) -> Result<()> {
    let entry = entry_of(paths, path)?;
    let at = paths.expand(&entry);
    let mut config = Config::load(paths)?;
    config.repos(paths)?;
    let list = roots(&config, paths)?;
    let kept: Vec<&PathBuf> = list.iter().filter(|r| !same_path(r, &at)).collect();
    if kept.len() == list.len() {
        return Ok(());
    }
    config.scan = Some(kept.iter().map(|r| paths.tilde(r)).collect());
    config.save(paths)?;
    let file = paths.discovery_file();
    if Found::load(&file).is_some() {
        Found::update(&file, |d| {
            d.roots.retain(|o| !same_path(&paths.expand(&o.root), &at));
            d.suggestions
                .retain(|s| !same_path(&paths.expand(&s.root), &at));
        })?;
    }
    Ok(())
}

/// *Add* on a suggestion: lists it through `add` in `follow` mode on the suggestion's branch,
/// with nothing else set, so every check `add` makes still runs (listed, concord's gate, a main
/// clone, the branch on the remote) and its refusal comes back as it is. Never `mirror`: a
/// mirror throws local work away (rule 8 of that ADR).
///
/// The remote is the one the branch was read from (`origin`, else the clone's first remote),
/// so a clone with no `origin` is not listed against one it does not have.
///
/// # Errors
///
/// Fails when `path` is not in the last scan's suggestions, or with `add`'s own error.
pub fn accept(ctx: &Ctx, path: &str) -> Result<Repo> {
    let paths = &ctx.paths;
    let at = absolute(&paths.expand(path.trim()));
    let found = Found::load(&paths.discovery_file()).unwrap_or_default();
    let Some(suggestion) = found
        .suggestions
        .iter()
        .find(|s| same_path(&paths.expand(&s.path), &at))
    else {
        bail!(
            "{} is not a suggestion: look again, or list it with `git tether add`",
            paths.tilde(&at)
        );
    };
    let remote = remote_of(paths, &at).filter(|r| r != DEFAULT_REMOTE);
    add::add(
        ctx,
        &AddOpts {
            path: at,
            branch: suggestion.branch.clone(),
            mode: Mode::Follow,
            remote,
            interval: None,
            url: None,
            gate: None,
            after: None,
        },
    )
}
