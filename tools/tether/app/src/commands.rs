//! The Tauri commands behind the popover (ADR 0002 rule 4). None runs git on
//! the thread that calls it: a sync is only queued for the worker, and the
//! picker, the branch lookup and `add` run on a blocking thread.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde::Deserialize;
use tauri::AppHandle;
use tether_core::Ctx;
use tether_core::add::{self, AddOpts};
use tether_core::config::{Mode, absolute};

use crate::logic::Event;
use crate::osascript;
use crate::state::{Snapshot, State};
use crate::window;

type Shared<'a> = tauri::State<'a, Arc<State>>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("the background task failed: {e}"))
}

#[tauri::command]
pub fn report(state: Shared<'_>) -> Snapshot {
    state.snapshot()
}

/// *Sync now* for one repo (its path as listed), or *Sync all* with `None`.
#[tauri::command]
pub fn sync(state: Shared<'_>, path: Option<String>) {
    state.send(Event::SyncNow(path));
}

#[tauri::command]
pub fn toggle_pause(state: Shared<'_>) -> Result<(), String> {
    let result = state.paths.set_paused(!state.paths.is_paused());
    state.send(Event::Refresh);
    result.map_err(|e| format!("saving Pause failed: {e}"))
}

/// The macOS folder picker, as `~/…`; `None` when cancelled.
#[tauri::command]
pub async fn pick_folder(app: AppHandle, state: Shared<'_>) -> Result<Option<String>, String> {
    let state = state.inner().clone();
    state.picking.store(true, Ordering::SeqCst);
    let picked = blocking(osascript::choose_folder).await;
    state.picking.store(false, Ordering::SeqCst);
    // The picker took the focus; give it back so the form is where it was.
    window::show(&app);
    Ok(picked?.map(|f| state.paths.tilde(Path::new(&f))))
}

/// The branch a clone has checked out, for the add form's default.
#[tauri::command]
pub async fn branch_of(state: Shared<'_>, path: String) -> Result<Option<String>, String> {
    let paths = state.paths.clone();
    blocking(move || add::current_branch(&Ctx::new(paths.clone(), true), &paths.expand(&path)))
        .await
}

/// The add form's fields (the popover's `AddRequest`).
#[derive(Debug, Deserialize)]
pub struct AddRequest {
    path: String,
    branch: String,
    mode: Mode,
    url: Option<String>,
    interval: Option<String>,
    gate: Option<String>,
    after: Option<String>,
}

/// *Add repository*: lists the repo through tether-core, then syncs it.
#[tauri::command]
pub async fn add(state: Shared<'_>, request: AddRequest) -> Result<(), String> {
    let branch = request.branch.trim().to_string();
    if request.path.trim().is_empty() || branch.is_empty() {
        return Err("choose a folder and a branch".into());
    }
    let state = state.inner().clone();
    blocking(move || {
        let opts = AddOpts {
            path: state.paths.expand(&request.path),
            branch,
            mode: request.mode,
            remote: None,
            interval: request.interval,
            url: trimmed(request.url),
            gate: trimmed(request.gate),
            after: trimmed(request.after),
        };
        let result = {
            let _guard = state.work.lock().unwrap();
            add::add(&Ctx::new(state.paths.clone(), false), &opts)
        };
        match result {
            Ok(_) => {
                state.send(Event::SyncNow(Some(
                    state.paths.tilde(&absolute(&opts.path)),
                )));
                Ok(())
            }
            Err(e) => {
                state.send(Event::Refresh);
                Err(format!("{e:#}"))
            }
        }
    })
    .await?
}

/// An optional text field: `None` when left blank.
fn trimmed(field: Option<String>) -> Option<String> {
    field
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
}

/// The answer to an edit of one listed repo; `None` means another writer
/// unlisted it since the popover last refreshed.
fn edited(path: &str, result: anyhow::Result<Option<String>>) -> Result<(), String> {
    match result {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(format!("{path} is no longer listed")),
        Err(e) => Err(format!("{e:#}")),
    }
}

/// *Pull every* in a repo's settings: rewrites its `interval` in repos.toml.
#[tauri::command]
pub async fn set_interval(state: Shared<'_>, path: String, interval: String) -> Result<(), String> {
    let state = state.inner().clone();
    blocking(move || {
        let result = {
            let _guard = state.work.lock().unwrap();
            add::set_interval(&Ctx::new(state.paths.clone(), false), &path, &interval)
        };
        state.send(Event::Refresh);
        edited(&path, result)
    })
    .await?
}

/// *Remove from tether*: unlists the repo; the clone is left as it is.
#[tauri::command]
pub async fn remove(state: Shared<'_>, path: String) -> Result<(), String> {
    let state = state.inner().clone();
    blocking(move || {
        let result = {
            let _guard = state.work.lock().unwrap();
            add::remove(&Ctx::new(state.paths.clone(), false), &path)
        };
        state.send(Event::Refresh);
        edited(&path, result)
    })
    .await?
}

#[tauri::command]
pub fn open_in_finder(state: Shared<'_>, path: String) -> Result<(), String> {
    Command::new("open")
        .arg(state.paths.expand(&path))
        .spawn()
        .map(drop)
        .map_err(|e| format!("opening {path} failed: {e}"))
}

#[tauri::command]
pub fn hide(app: AppHandle) {
    window::hide(&app);
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}
