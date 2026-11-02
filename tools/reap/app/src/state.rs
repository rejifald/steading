//! What the worker, the commands and the window share, and the snapshot the popover receives
//! after every refresh.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Instant;

use reap_core::Paths;
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
        }
    }

    /// Wakes the worker; a send after it stopped (the app is quitting) is dropped.
    pub fn send(&self, event: Event) {
        let _ = self.sender.lock().unwrap().send(event);
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            report: self.report.lock().unwrap().clone(),
            error: self.report_error.lock().unwrap().clone(),
            refreshing: self.is_refreshing.load(Ordering::SeqCst),
            note: self.note.lock().unwrap().clone(),
        }
    }
}
