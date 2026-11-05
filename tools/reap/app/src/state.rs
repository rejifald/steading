//! What the worker, the commands and the window share, and the snapshot the popover receives
//! after every refresh.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Instant;

use reap_core::Paths;
use reap_core::discovery::{self, View};
use reap_core::report::Report;
use serde::Serialize;

use crate::logic::Event;

pub struct State {
    pub paths: Paths,
    sender: Mutex<Sender<Event>>,
    pub report: Mutex<Option<Report>>,
    pub report_error: Mutex<Option<String>>,
    pub is_refreshing: AtomicBool,
    /// The last failed action, shown until the next refresh.
    pub note: Mutex<Option<String>>,
    /// While the folder picker is up, losing focus must not hide the popover.
    pub is_picking: AtomicBool,
    /// When the popover last hid on focus loss (see `logic::reopens`).
    pub hidden_at: Mutex<Option<Instant>>,
    /// The scan root being walked now (`~/…`); `None` when no scan runs.
    pub scanning: Mutex<Option<String>>,
    /// A clone scan is running (at most one at a time).
    pub is_scanning: AtomicBool,
    /// Why the last scan failed, shown until the next scan starts. Apart from `note`, which
    /// every pass clears: the scan's own final refresh would otherwise wipe it.
    pub scan_error: Mutex<Option<String>>,
}

/// The `report` event's payload and the `report` command's answer.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    /// The same JSON as `git reap status --json`; `None` before the first report or when
    /// repos.toml cannot be read (then `error` says why).
    pub report: Option<Report>,
    pub error: Option<String>,
    pub refreshing: bool,
    pub note: Option<String>,
    /// The same JSON as `git reap discover --json`, with `scanning` filled in by the app.
    pub discovery: View,
}

impl State {
    pub fn new(paths: Paths, sender: Sender<Event>) -> Self {
        Self {
            paths,
            sender: Mutex::new(sender),
            report: Mutex::new(None),
            report_error: Mutex::new(None),
            is_refreshing: AtomicBool::new(false),
            note: Mutex::new(None),
            is_picking: AtomicBool::new(false),
            hidden_at: Mutex::new(None),
            scanning: Mutex::new(None),
            is_scanning: AtomicBool::new(false),
            scan_error: Mutex::new(None),
        }
    }

    /// Wakes the worker; a send after it stopped (the app is quitting) is dropped.
    pub fn send(&self, event: Event) {
        let _ = self.sender.lock().unwrap().send(event);
    }

    pub fn snapshot(&self) -> Snapshot {
        let scanning = self.scanning.lock().unwrap().clone();
        // An invalid repos.toml is reported through `error` (the pass reads the same file), so
        // the popover still gets a `Discovery` here: an empty one.
        let discovery = discovery::view(&self.paths, scanning.clone()).unwrap_or_else(|_| View {
            scanned_at: None,
            scanning,
            roots: Vec::new(),
            suggestions: Vec::new(),
            skipped: Vec::new(),
        });
        Snapshot {
            report: self.report.lock().unwrap().clone(),
            error: self.report_error.lock().unwrap().clone(),
            refreshing: self.is_refreshing.load(Ordering::SeqCst),
            note: self
                .note
                .lock()
                .unwrap()
                .clone()
                .or_else(|| self.scan_error.lock().unwrap().clone()),
            discovery,
        }
    }
}
