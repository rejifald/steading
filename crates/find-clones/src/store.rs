//! What a scan found, kept in `discovery.json` beside a tool's `state.json` (ADR 0004 rule 10).
//!
//! It is its own file, never a key in `state.json`: that loader treats a file it cannot parse as
//! empty, so a new key there would silently drop every record. This loader does the same, which
//! here only means "no scan yet".

use std::fs::{self, OpenOptions};
use std::path::Path;

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// What one root yielded in a scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootOutcome {
    /// As the operator wrote it (`~/Code`).
    pub root: String,
    /// Main clones found under it, before the tool's filter.
    pub clones: usize,
    #[serde(default)]
    pub stopped_early: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A scan's result. `S` is a tool's suggestion: its facts differ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Discovery<S> {
    /// RFC 3339, local time; when the last scan finished. `None` while the first one runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scanned_at: Option<String>,
    pub roots: Vec<RootOutcome>,
    pub suggestions: Vec<S>,
}

impl<S> Default for Discovery<S> {
    fn default() -> Self {
        Self {
            scanned_at: None,
            roots: Vec::new(),
            suggestions: Vec::new(),
        }
    }
}

impl<S: Serialize + DeserializeOwned> Discovery<S> {
    /// `None` when there is no file yet or it cannot be parsed: either way, no scan has run.
    #[must_use]
    pub fn load(file: &Path) -> Option<Self> {
        let body = fs::read_to_string(file).ok()?;
        serde_json::from_str(&body).ok()
    }

    /// Writes beside and renames, so a crash never leaves half a file.
    ///
    /// # Errors
    ///
    /// Fails when the file or its folder cannot be written.
    pub fn save(&self, file: &Path) -> Result<()> {
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let tmp = file.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("writing {}", tmp.display()))?;
        fs::rename(&tmp, file).with_context(|| format!("replacing {}", file.display()))
    }

    /// Read-modify-write under a lock on `<file>.lock`, so a scan publishing a root and the
    /// popover dropping a root's suggestions never lose each other's write. A missing or
    /// unreadable file starts from the default.
    ///
    /// # Errors
    ///
    /// Fails when the lock cannot be taken or the file cannot be written.
    pub fn update(file: &Path, change: impl FnOnce(&mut Self)) -> Result<Self> {
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let lock_path = file.with_extension("json.lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .with_context(|| format!("opening {}", lock_path.display()))?;
        lock.lock()
            .with_context(|| format!("locking {}", lock_path.display()))?;
        let mut all = Self::load(file).unwrap_or_default();
        change(&mut all);
        all.save(file)?;
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Plain = Discovery<String>;

    #[test]
    fn round_trips_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("cfg/discovery.json");
        let d = Plain {
            scanned_at: Some("2026-11-04T10:00:00+02:00".into()),
            roots: vec![RootOutcome {
                root: "~/Code".into(),
                clones: 2,
                stopped_early: true,
                error: None,
            }],
            suggestions: vec!["a".into()],
        };
        d.save(&file).unwrap();
        assert_eq!(Plain::load(&file), Some(d));
        assert!(!file.with_extension("json.tmp").exists());
    }

    #[test]
    fn a_missing_or_garbled_file_is_no_scan_yet() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("discovery.json");
        assert_eq!(Plain::load(&file), None);
        fs::write(&file, "{ not json").unwrap();
        assert_eq!(Plain::load(&file), None);
    }

    #[test]
    fn update_starts_from_the_default_and_keeps_the_change() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("discovery.json");
        Plain::update(&file, |d| d.suggestions.push("a".into())).unwrap();
        Plain::update(&file, |d| d.suggestions.push("b".into())).unwrap();
        assert_eq!(Plain::load(&file).unwrap().suggestions, ["a", "b"]);
    }
}
