//! `state.json` in the config dir (rule 10): per repo, the grace clock of every removable
//! worktree, the *Keep*s, the removals git refused, and when the last pass ran. Keyed by paths
//! as shown (`~/…`). Under `volumes`, per mounted volume, the low-disk episode being announced
//! (ADR low-disk-skips-the-grace).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::paths::Paths;

/// When reap first saw a worktree removable (or its folder missing), at which HEAD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clock {
    pub head: String,
    pub since: DateTime<Utc>,
}

/// A removal git refused, at which HEAD; not retried until HEAD moves or *Remove now*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub head: String,
    pub message: String,
}

/// Everything reap remembers about one repo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoMemory {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub clocks: BTreeMap<String, Clock>,
    /// Kept worktrees and the HEAD they were kept at; new commits end the Keep.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keeps: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refusals: BTreeMap<String, Refusal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_pass: Option<DateTime<Utc>>,
}

impl RepoMemory {
    /// Whether `worktree` is kept at `head`.
    #[must_use]
    pub fn is_kept(&self, worktree: &str, head: &str) -> bool {
        self.keeps.get(worktree).is_some_and(|kept| kept == head)
    }

    /// The refusal recorded for `worktree` at `head`.
    #[must_use]
    pub fn refusal(&self, worktree: &str, head: &str) -> Option<&Refusal> {
        self.refusals
            .get(worktree)
            .filter(|refusal| refusal.head == head)
    }

    /// The running clock of `worktree` at `head`.
    #[must_use]
    pub fn clock(&self, worktree: &str, head: &str) -> Option<&Clock> {
        self.clocks.get(worktree).filter(|clock| clock.head == head)
    }
}

/// A volume that is low and what has been said about it so far: an episode lasts until the
/// volume is healthy again, and each notice goes out once in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeMemory {
    /// When the pass first saw it low.
    pub since: DateTime<Utc>,
    /// "Still low, nothing reap can remove" has been sent.
    #[serde(default)]
    pub has_announced_stuck: bool,
    /// The `policy = "ask"` worktrees already announced as low-disk removals.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub asked: BTreeSet<String>,
}

pub type StateFile = BTreeMap<String, RepoMemory>;

/// The whole file: the repos' memories sit at the top level, the volumes' under `volumes`
/// (no repo key can be `volumes`: they are paths).
#[derive(Debug, Default, Serialize, Deserialize)]
struct Document {
    #[serde(flatten)]
    repos: StateFile,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    volumes: BTreeMap<String, VolumeMemory>,
}

fn load_document(paths: &Paths) -> Document {
    std::fs::read_to_string(paths.state_file())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Reads `state.json`. A missing or unreadable file forgets every clock, which only delays
/// removals: each clock starts again with a full grace period.
#[must_use]
pub fn load(paths: &Paths) -> StateFile {
    load_document(paths).repos
}

/// The low-disk episodes in progress, by the volume's mount point.
#[must_use]
pub fn load_volumes(paths: &Paths) -> BTreeMap<String, VolumeMemory> {
    load_document(paths).volumes
}

/// Changes one repo's memory as a read-modify-write under a lock on `state.lock`, so the tray
/// and a CLI run never drop each other's changes.
///
/// # Errors
///
/// Fails when the config folder or the file cannot be written.
pub fn update<T>(
    paths: &Paths,
    repo: &str,
    change: impl FnOnce(&mut RepoMemory) -> T,
) -> Result<T> {
    modify(paths, |document| {
        let memory = document.repos.entry(repo.to_string()).or_default();
        let result = change(memory);
        if document
            .repos
            .get(repo)
            .is_some_and(|memory| *memory == RepoMemory::default())
        {
            document.repos.remove(repo);
        }
        result
    })
}

/// Changes the low-disk episodes, under the same lock as [`update`].
///
/// # Errors
///
/// Fails when the config folder or the file cannot be written.
pub fn update_volumes<T>(
    paths: &Paths,
    change: impl FnOnce(&mut BTreeMap<String, VolumeMemory>) -> T,
) -> Result<T> {
    modify(paths, |document| change(&mut document.volumes))
}

fn modify<T>(paths: &Paths, change: impl FnOnce(&mut Document) -> T) -> Result<T> {
    std::fs::create_dir_all(&paths.config)
        .with_context(|| format!("creating {}", paths.config.display()))?;
    let lock_path = paths.config.join("state.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .with_context(|| format!("opening {}", lock_path.display()))?;
    lock.lock()
        .with_context(|| format!("locking {}", lock_path.display()))?;
    let mut document = load_document(paths);
    let result = change(&mut document);
    write_atomic(
        &paths.state_file(),
        &serde_json::to_string_pretty(&document)?,
    )?;
    Ok(result)
}

fn write_atomic(file: &Path, body: &str) -> Result<()> {
    let temporary = file.with_extension("json.tmp");
    std::fs::write(&temporary, body).with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, file).with_context(|| format!("replacing {}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_round_trips_and_an_empty_repo_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path(), &dir.path().join("cfg"));

        update(&paths, "~/app", |memory| {
            memory.keeps.insert("~/w".into(), "abc".into());
        })
        .unwrap();

        assert!(load(&paths)["~/app"].is_kept("~/w", "abc"));
        assert!(!load(&paths)["~/app"].is_kept("~/w", "def"));
        update(&paths, "~/app", |memory| memory.keeps.clear()).unwrap();
        assert!(load(&paths).is_empty());
    }

    #[test]
    fn volume_episodes_live_beside_the_repos_and_an_old_file_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path(), &dir.path().join("cfg"));
        std::fs::create_dir_all(&paths.config).unwrap();
        std::fs::write(paths.state_file(), r#"{"~/app":{"keeps":{"~/w":"abc"}}}"#).unwrap();
        assert!(load(&paths)["~/app"].is_kept("~/w", "abc"));

        update_volumes(&paths, |volumes| {
            volumes.insert(
                "/Volumes/Backup".into(),
                VolumeMemory {
                    since: Utc::now(),
                    has_announced_stuck: true,
                    asked: BTreeSet::new(),
                },
            );
        })
        .unwrap();
        update(&paths, "~/app", |memory| {
            memory.last_pass = Some(Utc::now());
        })
        .unwrap();

        assert!(load_volumes(&paths)["/Volumes/Backup"].has_announced_stuck);
        assert!(load(&paths)["~/app"].is_kept("~/w", "abc"));
        update_volumes(&paths, BTreeMap::clear).unwrap();
        assert!(load_volumes(&paths).is_empty());
    }

    #[test]
    fn a_clock_or_refusal_at_another_head_does_not_count() {
        let mut memory = RepoMemory::default();
        memory.clocks.insert(
            "~/w".into(),
            Clock {
                head: "abc".into(),
                since: Utc::now(),
            },
        );
        memory.refusals.insert(
            "~/w".into(),
            Refusal {
                head: "abc".into(),
                message: "refused".into(),
            },
        );

        assert!(memory.clock("~/w", "abc").is_some());
        assert!(memory.clock("~/w", "def").is_none());
        assert!(memory.refusal("~/w", "def").is_none());
    }
}
