//! The Tauri commands behind the popover. None runs git on the thread that calls it: a
//! refresh is only queued for the worker, and the picker and the commands that change a repo
//! or `state.json` run on a blocking thread.

// Tauri hands every command its arguments by value; that is its calling convention.
#![allow(clippy::needless_pass_by_value)]

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::logic::Event;
use crate::osascript;
use crate::state::{Snapshot, State};
use crate::window;

type Shared<'a> = tauri::State<'a, Arc<State>>;

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("the background task failed: {e}"))
}

#[tauri::command]
pub fn report(state: Shared<'_>) -> Snapshot {
    state.snapshot()
}

#[tauri::command]
pub fn refresh(state: Shared<'_>) {
    state.send(Event::Refresh);
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
    state.is_picking.store(true, Ordering::SeqCst);
    let picked = blocking(osascript::choose_folder).await;
    state.is_picking.store(false, Ordering::SeqCst);
    // The picker took the focus; give it back so the form is where it was.
    window::show(&app);
    Ok(picked?.map(|folder| state.paths.tilde(Path::new(&folder))))
}

/// *Add repository*: lists the main clone through reap-core, then refreshes.
#[tauri::command]
pub async fn add(state: Shared<'_>, path: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("choose a folder".into());
    }
    let state = state.inner().clone();
    blocking(move || {
        let result =
            reap_core::add::add(&state.paths, &state.paths.expand(path.trim()), None, None);
        state.send(Event::Refresh);
        result.map(drop).map_err(|e| format!("{e:#}"))
    })
    .await?
}

/// *Keep*: holds a worktree until it gets new commits.
#[tauri::command]
pub async fn keep(state: Shared<'_>, path: String) -> Result<(), String> {
    let state = state.inner().clone();
    blocking(move || {
        let result = reap_core::sweep::keep(&state.paths, &path);
        state.send(Event::Refresh);
        result.map(drop).map_err(|e| format!("{e:#}"))
    })
    .await?
}

/// *Unkeep*: ends a Keep.
#[tauri::command]
pub async fn unkeep(state: Shared<'_>, path: String) -> Result<(), String> {
    let state = state.inner().clone();
    blocking(move || {
        let result = reap_core::sweep::unkeep(&state.paths, &path);
        state.send(Event::Refresh);
        result.map(drop).map_err(|e| format!("{e:#}"))
    })
    .await?
}

/// *Remove now*: removes one removable worktree at once, with the same guards and no grace.
#[tauri::command]
pub async fn remove_now(state: Shared<'_>, path: String) -> Result<(), String> {
    let state = state.inner().clone();
    blocking(move || {
        let ctx = reap_core::ctx::Ctx::new(state.paths.clone(), false);
        let result = reap_core::sweep::remove_now(&ctx, &path, chrono::Utc::now());
        state.send(Event::Refresh);
        result.map(drop).map_err(|e| format!("{e:#}"))
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
