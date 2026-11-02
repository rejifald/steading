//! The one thread that runs git: a pass every `interval` and whenever the popover opens or asks
//! (rule 7). A burst of requests is handled as one. A pass sweeps every listed repo: it fetches
//! what moved, runs the grace clocks and removes what is due; while paused it only reads. After
//! every pass the report is published to the icon and the popover.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use reap_core::config::{Config, DEFAULT_INTERVAL};
use reap_core::ctx::Ctx;
use reap_core::report::{self, Report};
use reap_core::sweep::{self, Outcome};
use tauri::AppHandle;

use crate::logic::{self, Event};
use crate::osascript::notify;
use crate::state::State;
use crate::tray;

/// Requests arriving within this window are handled as one.
const COALESCE: Duration = Duration::from_millis(300);

pub fn run(app: &AppHandle, state: &Arc<State>, receiver: &Receiver<Event>) {
    let mut needed_you = HashSet::new();
    let mut is_first = true;
    loop {
        if !is_first {
            match receiver.recv_timeout(interval(state)) {
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            loop {
                match receiver.recv_timeout(COALESCE) {
                    Ok(_) => {}
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
        pass(app, state, &mut needed_you, is_first);
        is_first = false;
    }
}

/// The pass interval from repos.toml, re-read each time so an edit takes effect.
fn interval(state: &State) -> Duration {
    Config::load(&state.paths)
        .and_then(|config| config.interval())
        .unwrap_or(DEFAULT_INTERVAL)
}

/// Sweeps (or, while paused, only reads), notifies, and publishes the report. The first pass
/// after start only seeds what needed the owner already, so a restart does not notify again;
/// grace clocks persist in `state.json`, so their notifications are never repeated.
fn pass(app: &AppHandle, state: &Arc<State>, needed_you: &mut HashSet<String>, is_first: bool) {
    state.is_refreshing.store(true, Ordering::SeqCst);
    tray::publish(app, state);
    // Commands that change a repo (Remove now) take the same clone locks, so they and this
    // pass never write one clone at once.
    let result = sweep_or_read(state);
    *state.note.lock().unwrap() = None;
    match result {
        Ok((report, outcomes)) => {
            let mut messages = logic::notifications(&outcomes);
            for (path, why) in logic::new_needs_you(needed_you, &report) {
                if !is_first {
                    messages.push(("reap needs you".into(), format!("{path}: {why}")));
                }
            }
            for (title, message) in logic::batched(messages) {
                notify(&title, &message);
            }
            *state.report_error.lock().unwrap() = None;
            *state.report.lock().unwrap() = Some(report);
        }
        Err(e) => {
            *state.report_error.lock().unwrap() = Some(format!("{e:#}"));
            *state.report.lock().unwrap() = None;
        }
    }
    state.is_refreshing.store(false, Ordering::SeqCst);
    tray::publish(app, state);
}

fn sweep_or_read(state: &State) -> Result<(Report, Vec<Outcome>)> {
    let now = Utc::now();
    if state.paths.is_paused() {
        return Ok((report::report(&state.paths, now)?, Vec::new()));
    }
    let ctx = Ctx::new(state.paths.clone(), false);
    let outcomes = sweep::sweep(&ctx, None, Duration::ZERO, now)?;
    let repos = outcomes
        .iter()
        .map(|outcome| outcome.report.clone())
        .collect();
    Ok((report::summarize(&state.paths, repos), outcomes))
}
