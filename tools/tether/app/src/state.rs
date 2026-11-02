//! What the worker, the commands and the window share, and the snapshot the
//! popover receives after every refresh.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Instant;

use serde::Serialize;
use tether_core::Paths;
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
        Snapshot {
            report,
            error: self.report_error.lock().unwrap().clone(),
            syncing: self.syncing.load(Ordering::SeqCst),
            busy: self.busy.lock().unwrap().clone(),
            note: self.note.lock().unwrap().clone(),
            details,
        }
    }
}
