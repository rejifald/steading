//! The last sync result per repo, kept in `state.json` in the config dir so
//! `status`, the tray and the `--due` tick know when a repo last synced and why
//! it was skipped. Keyed by the repo's path as written in `repos.toml`.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::inspect::{Health, Reason};
use crate::paths::Paths;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// RFC 3339, local time.
    pub last_sync: String,
    pub health: Health,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    /// The commit the branch was on after the sync.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip: Option<String>,
    /// The move whose `after` command failed, kept across syncs until a run
    /// of it succeeds (ADR 0004 rule 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_failed: Option<AfterFailed>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AfterFailed {
    /// The tip before the move (empty for a fresh clone).
    pub from: String,
    pub to: String,
    /// The command's first line of output, or how it ended.
    pub output: String,
}

impl Record {
    pub fn last_sync_time(&self) -> Option<DateTime<Local>> {
        DateTime::parse_from_rfc3339(&self.last_sync)
            .ok()
            .map(|t| t.with_timezone(&Local))
    }
}

pub type Records = BTreeMap<String, Record>;

pub fn load(paths: &Paths) -> Records {
    // A missing or unreadable file only loses "last sync" times: every repo is
    // then due, which is the safe direction.
    std::fs::read_to_string(paths.state_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Read-modify-write under a lock on `state.lock`, so two processes syncing
/// different repos never drop each other's record.
pub fn save(paths: &Paths, key: &str, record: Record) -> Result<()> {
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
    let mut all = load(paths);
    all.insert(key.to_string(), record);
    write_atomic(&paths.state_file(), &serde_json::to_string_pretty(&all)?)
}

/// Drops a repo's record (`git tether remove`).
pub fn forget(paths: &Paths, key: &str) -> Result<()> {
    let mut all = load(paths);
    if all.remove(key).is_some() {
        write_atomic(&paths.state_file(), &serde_json::to_string_pretty(&all)?)?;
    }
    Ok(())
}

fn write_atomic(file: &Path, body: &str) -> Result<()> {
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, body).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, file).with_context(|| format!("replacing {}", file.display()))
}

pub fn now() -> String {
    Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}
