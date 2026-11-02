//! The one thread that runs git (README § Menu): per-repo timers on each
//! repo's `interval`, and the syncs the popover and the menu queue. A burst of
//! requests is coalesced; after every round the report is rebuilt and
//! published to the icon and the popover.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use chrono::Local;
use tauri::AppHandle;
use tether_core::inspect::Health;
use tether_core::record;
use tether_core::report;
use tether_core::sync::{self, Outcome, Trigger};
use tether_core::{Ctx, Paths};

use crate::logic::{self, Event, Pending};
use crate::osascript::notify;
use crate::state::State;
use crate::tray;

/// Longest sleep between due checks, so a hand-edited repos.toml is seen.
const WAKE_CAP: Duration = Duration::from_secs(60);
/// Shortest sleep between due checks.
const MIN_WAKE: Duration = Duration::from_secs(1);
/// Requests arriving within this window are handled as one.
const COALESCE: Duration = Duration::from_millis(300);
/// *Sync now* waits this long for another run (the CLI) to release a clone.
const EXPLICIT_WAIT: Duration = Duration::from_secs(10);

pub fn run(app: &AppHandle, state: &Arc<State>, rx: &Receiver<Event>) {
    // Seeded from the last recorded results, so a restart does not re-notify
    // a repo that was already *needs you*.
    let mut prev: HashMap<String, Health> = record::load(&state.paths)
        .into_iter()
        .map(|(k, r)| (k, r.health))
        .collect();
    loop {
        let wait = sleep_time(&state.paths);
        let first = match rx.recv_timeout(wait) {
            Ok(e) => e,
            Err(RecvTimeoutError::Timeout) => Event::Tick,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let mut pending = Pending::of(first);
        loop {
            match rx.recv_timeout(COALESCE) {
                Ok(e) => pending.absorb(e),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        *state.note.lock().unwrap() = None;
        let outcomes = run_due_and_requested(app, state, &pending);
        for o in outcomes.iter().filter(|o| o.discarded) {
            let saved = o
                .backup
                .as_ref()
                .map(|b| format!("; tracked changes saved to {}", state.paths.tilde(b)))
                .unwrap_or_default();
            notify(
                "tether discarded changes",
                &format!("{}: mirror reset to its branch tip{saved}", o.path),
            );
        }
        refresh(app, state, &mut prev);
    }
}

fn sleep_time(paths: &Paths) -> Duration {
    let ctx = Ctx::new(paths.clone(), true);
    let Ok(repos) = ctx.config().and_then(|c| c.repos(paths)) else {
        return WAKE_CAP;
    };
    let keyed: Vec<_> = repos
        .into_iter()
        .map(|r| {
            let key = paths.tilde(&r.path);
            (r, key)
        })
        .collect();
    // A floor, so a repo whose sync cannot record its result never spins the loop.
    logic::next_wait(&keyed, &record::load(paths), Local::now(), WAKE_CAP).max(MIN_WAKE)
}

/// Syncs every repo asked for, plus every due repo unless paused.
fn run_due_and_requested(app: &AppHandle, state: &Arc<State>, pending: &Pending) -> Vec<Outcome> {
    let _guard = state.work.lock().unwrap();
    let ctx = Ctx::new(state.paths.clone(), false);
    let repos = match ctx.config().and_then(|c| c.repos(&state.paths)) {
        Ok(r) => r,
        Err(e) => {
            *state.note.lock().unwrap() = Some(format!("{e:#}"));
            return Vec::new();
        }
    };
    let paused = state.paths.is_paused();
    let records = record::load(&state.paths);
    let chosen: Vec<_> = repos
        .into_iter()
        .filter(|r| {
            let requested = pending.requested(&state.paths.tilde(&r.path));
            requested || (!paused && sync::is_due(&ctx, r, &records))
        })
        .collect();
    if chosen.is_empty() {
        return Vec::new();
    }
    *state.busy.lock().unwrap() = chosen.iter().map(|r| state.paths.tilde(&r.path)).collect();
    state.syncing.store(true, Ordering::SeqCst);
    tray::publish(app, state);
    let mut outs = Vec::new();
    for r in &chosen {
        let trigger = if pending.requested(&state.paths.tilde(&r.path)) {
            Trigger::Explicit {
                wait: EXPLICIT_WAIT,
            }
        } else {
            Trigger::Background
        };
        match sync::sync_repo(&ctx, r, trigger) {
            Ok(o) => outs.push(o),
            Err(e) => {
                *state.note.lock().unwrap() =
                    Some(format!("{}: {e:#}", state.paths.tilde(&r.path)));
            }
        }
    }
    state.busy.lock().unwrap().clear();
    state.syncing.store(false, Ordering::SeqCst);
    outs
}

fn refresh(app: &AppHandle, state: &Arc<State>, prev: &mut HashMap<String, Health>) {
    let report = {
        let _guard = state.work.lock().unwrap();
        report::report(&Ctx::new(state.paths.clone(), true))
    };
    match report {
        Ok(r) => {
            for (path, why) in logic::flips(prev, &r) {
                notify("tether needs you", &format!("{path}: {why}"));
            }
            *state.report_error.lock().unwrap() = None;
            *state.report.lock().unwrap() = Some(r);
        }
        Err(e) => {
            *state.report_error.lock().unwrap() = Some(format!("{e:#}"));
            *state.report.lock().unwrap() = None;
        }
    }
    tray::publish(app, state);
}
