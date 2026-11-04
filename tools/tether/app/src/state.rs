//! What the worker, the commands and the window share, and the snapshot the
//! popover receives after every refresh.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Instant;

use serde::Serialize;
use tether_core::Paths;
use tether_core::discovery::{self, View};
use tether_core::report::{self, Report};

use crate::logic::Event;

pub struct State {
    pub paths: Paths,
    tx: Mutex<Sender<Event>>,
    /// Serialises the worker's syncs with *Add repository*.
    pub work: Mutex<()>,
    pub report: Mutex<Option<Report>>,
    pub report_error: Mutex<Option<String>>,
    pub syncing: AtomicBool,
    /// Repos the worker is syncing right now, as listed (`~/…`).
    pub busy: Mutex<Vec<String>>,
    /// The last failed sync of this round, shown until the next round.
    pub note: Mutex<Option<String>>,
    /// While the folder picker is up, losing focus must not hide the popover.
    pub picking: AtomicBool,
    /// When the popover last hid on focus loss (see `logic::reopens`).
    pub hidden_at: Mutex<Option<Instant>>,
    /// The scan root being walked now (`~/…`); `None` when idle.
    pub scanning: Mutex<Option<String>>,
    /// Whether a scan thread is running, so a second *Find clones* is ignored.
    pub scan_running: AtomicBool,
    /// Why the last scan failed. Apart from `note`, which the worker clears at the
    /// start of every round, so the Refresh a scan ends with would erase it.
    pub scan_note: Mutex<Option<String>>,
}

/// The `report` event's payload and the `report` command's answer.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    /// The same JSON as `git tether status --json`; `None` before the first
    /// report or when repos.toml cannot be read (then `error` says why).
    pub report: Option<Report>,
    pub error: Option<String>,
    pub syncing: bool,
    pub busy: Vec<String>,
    pub note: Option<String>,
    /// Per path, the CLI's own detail line: the skip reason or the commits to pull.
    pub details: BTreeMap<String, String>,
    /// What the last scan found, for the suggestions (`git tether discover --json`).
    pub discovery: View,
}

/// What the popover shows when the roots, `scan` or `skip` cannot be read: the
/// reason is already in `error`, so there is nothing to list.
fn empty_view(scanning: Option<String>) -> View {
    View {
        scanned_at: None,
        scanning,
        roots: Vec::new(),
        suggestions: Vec::new(),
        skipped: Vec::new(),
    }
}

impl State {
    pub fn new(paths: Paths, tx: Sender<Event>) -> Self {
        Self {
            paths,
            tx: Mutex::new(tx),
            work: Mutex::new(()),
            report: Mutex::new(None),
            report_error: Mutex::new(None),
            syncing: AtomicBool::new(false),
            busy: Mutex::new(Vec::new()),
            note: Mutex::new(None),
            picking: AtomicBool::new(false),
            hidden_at: Mutex::new(None),
            scanning: Mutex::new(None),
            scan_running: AtomicBool::new(false),
            scan_note: Mutex::new(None),
        }
    }

    pub fn send(&self, e: Event) {
        let _ = self.tx.lock().unwrap().send(e);
    }

    pub fn snapshot(&self) -> Snapshot {
        let report = self.report.lock().unwrap().clone();
        let details = report
            .iter()
            .flat_map(|r| &r.repos)
            .filter_map(|row| {
                let line = row
                    .reason_text()
                    .or_else(|| row.behind.map(report::pending_text))?;
                Some((row.path.clone(), line))
            })
            .collect();
        let scanning = self.scanning.lock().unwrap().clone();
        let discovery =
            discovery::view(&self.paths, scanning.clone()).unwrap_or_else(|_| empty_view(scanning));
        let note = self
            .note
            .lock()
            .unwrap()
            .clone()
            .or_else(|| self.scan_note.lock().unwrap().clone());
        Snapshot {
            report,
            error: self.report_error.lock().unwrap().clone(),
            syncing: self.syncing.load(Ordering::SeqCst),
            busy: self.busy.lock().unwrap().clone(),
            note,
            details,
            discovery,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    /// A state over a throwaway home and config folder.
    fn state_in(dir: &Path) -> State {
        let (tx, _rx) = std::sync::mpsc::channel();
        let home = dir.join("home");
        let config = dir.join("config");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&config).unwrap();
        State::new(Paths::new(&home, &config), tx)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tether-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_snapshot_carries_discovery_with_the_root_being_walked() {
        let dir = scratch("snapshot");
        let state = state_in(&dir);
        std::fs::write(
            state.paths.config_file(),
            "scan = [\"~/Code\"]\nskip = [\"~/Code/old\"]\n",
        )
        .unwrap();
        *state.scanning.lock().unwrap() = Some("~/Code".into());
        let d = state.snapshot().discovery;
        assert_eq!(d.scanning.as_deref(), Some("~/Code"));
        assert_eq!(d.roots.len(), 1);
        assert_eq!(d.roots[0].path, "~/Code");
        assert_eq!(d.roots[0].clones, None);
        assert_eq!(d.skipped.len(), 1);
        assert_eq!(d.skipped[0].root.as_deref(), Some("~/Code"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unreadable_repos_toml_leaves_an_empty_discovery_and_keeps_scanning() {
        let dir = scratch("invalid");
        let state = state_in(&dir);
        std::fs::write(state.paths.config_file(), "scan = not toml").unwrap();
        *state.scanning.lock().unwrap() = Some("~/Code".into());
        let d = state.snapshot().discovery;
        assert_eq!(d, empty_view(Some("~/Code".into())));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_scan_failure_shows_as_the_note_until_a_sync_failure_replaces_it() {
        let dir = scratch("note");
        let state = state_in(&dir);
        *state.scan_note.lock().unwrap() = Some("scan failed".into());
        assert_eq!(state.snapshot().note.as_deref(), Some("scan failed"));
        // The worker clears `note` every round; the scan's own survives it.
        *state.note.lock().unwrap() = None;
        assert_eq!(state.snapshot().note.as_deref(), Some("scan failed"));
        *state.note.lock().unwrap() = Some("~/r: failed".into());
        assert_eq!(state.snapshot().note.as_deref(), Some("~/r: failed"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
