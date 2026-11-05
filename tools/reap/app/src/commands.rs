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

use crate::logic::{self, Event};
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

/// *Save and remove*: saves an abandoned worktree to a ref, then removes it (ADR 0006); the
/// click is the operator's confirmation.
#[tauri::command]
pub async fn save_and_remove(state: Shared<'_>, path: String) -> Result<(), String> {
    let state = state.inner().clone();
    blocking(move || {
        let ctx = reap_core::ctx::Ctx::new(state.paths.clone(), false);
        let result = reap_core::sweep::salvage_now(&ctx, &path, chrono::Utc::now());
        state.send(Event::Refresh);
        result.map(drop).map_err(|e| format!("{e:#}"))
    })
    .await?
}

/// *Delete* on a save: deletes one salvage ref in the repo the popover lists it under.
#[tauri::command]
pub async fn delete_salvaged(
    state: Shared<'_>,
    repo: String,
    refname: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    blocking(move || {
        let ctx = reap_core::ctx::Ctx::new(state.paths.clone(), false);
        let result = reap_core::salvage::delete(&ctx, Some(&repo), &refname);
        state.send(Event::Refresh);
        result.map(drop).map_err(|e| format!("{e:#}"))
    })
    .await?
}

/// *Find clones*: walks the scan roots on a background thread and returns at once; the
/// popover follows it through the refreshes, one per root. Ignored while a scan runs. It only
/// reads (ADR 0004), so it takes no lock.
#[tauri::command]
pub fn find_clones(state: Shared<'_>) {
    let state = state.inner().clone();
    if state
        .is_scanning
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || {
        // Clears the scan whichever way it ends, a panic included, so the button never sticks.
        let _done = ScanDone(state.clone());
        *state.scan_error.lock().unwrap() = None;
        let roots = reap_core::config::Config::load(&state.paths)
            .and_then(|config| reap_core::discovery::roots(&config, &state.paths))
            .map(|roots| {
                roots
                    .iter()
                    .map(|root| state.paths.tilde(root))
                    .collect::<Vec<_>>()
            });
        let roots = match roots {
            Ok(roots) => roots,
            Err(e) => {
                *state.scan_error.lock().unwrap() = Some(format!("{e:#}"));
                return;
            }
        };
        *state.scanning.lock().unwrap() = roots.first().cloned();
        state.send(Event::Refresh);
        let mut on_root = |left: &str| {
            *state.scanning.lock().unwrap() = logic::next_scanning(&roots, left);
            state.send(Event::Refresh);
        };
        let bounds = reap_core::discovery::Bounds::default();
        if let Err(e) = reap_core::discovery::scan(&state.paths, &bounds, &mut on_root) {
            *state.scan_error.lock().unwrap() = Some(format!("{e:#}"));
        }
    });
}

/// Ends a scan: nothing is being walked, the next may start, and the popover is told.
struct ScanDone(Arc<State>);

impl Drop for ScanDone {
    fn drop(&mut self) {
        *self.0.scanning.lock().unwrap() = None;
        self.0.is_scanning.store(false, Ordering::SeqCst);
        self.0.send(Event::Refresh);
    }
}

/// *Add* on a suggested clone: reap's own `add`, so every check it makes runs, and its error
/// text comes back as it is.
#[tauri::command]
pub async fn add_suggestion(state: Shared<'_>, path: String) -> Result<(), String> {
    edit_discovery(state, path, |state, path| {
        reap_core::discovery::add_suggestion(&state.paths, path).map(drop)
    })
    .await
}

/// *Skip*: stops suggesting a clone.
#[tauri::command]
pub async fn skip_clone(state: Shared<'_>, path: String) -> Result<(), String> {
    edit_discovery(state, path, |state, path| {
        reap_core::discovery::skip(&state.paths, path)
    })
    .await
}

/// *Unskip*: suggests a skipped clone again.
#[tauri::command]
pub async fn unskip_clone(state: Shared<'_>, path: String) -> Result<(), String> {
    edit_discovery(state, path, |state, path| {
        reap_core::discovery::unskip(&state.paths, path)
    })
    .await
}

/// *Add folder*: a new scan root. Runs nothing; its clones show after the next scan.
#[tauri::command]
pub async fn add_scan_root(state: Shared<'_>, path: String) -> Result<(), String> {
    edit_discovery(state, path, |state, path| {
        reap_core::discovery::add_root(&state.paths, path)
    })
    .await
}

/// *Remove* on a scan root: its suggestions go with it.
#[tauri::command]
pub async fn remove_scan_root(state: Shared<'_>, path: String) -> Result<(), String> {
    edit_discovery(state, path, |state, path| {
        reap_core::discovery::remove_root(&state.paths, path)
    })
    .await
}

/// Runs one discovery edit on a blocking thread and refreshes after it, failed or not. Like
/// `add`, it takes no lock: reap has none around repos.toml.
async fn edit_discovery(
    state: Shared<'_>,
    path: String,
    edit: impl FnOnce(&State, &str) -> anyhow::Result<()> + Send + 'static,
) -> Result<(), String> {
    let state = state.inner().clone();
    blocking(move || {
        let result = edit(&state, path.trim());
        state.send(Event::Refresh);
        result.map_err(|e| format!("{e:#}"))
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
