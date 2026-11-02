//! The menu-bar app (ADR 0001 rules 14 and 15), in git-tether's shape: a tray icon with three
//! states, the popover on a left click, a minimal native menu on a right click, and the
//! worker thread. It calls reap-core directly; there is no IPC beyond the popover's commands.
//!
//! The event loop never runs git: clicks and commands only send an [`Event`] to the worker,
//! which builds the report and publishes it here.

use std::sync::Arc;
use std::sync::mpsc;

use reap_core::Paths;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, RunEvent};

use crate::logic::{self, Event, Icon};
use crate::state::State;
use crate::{commands, window, worker};

const TRAY_ID: &str = "reap";
/// The event the popover listens to; its payload is a [`crate::state::Snapshot`].
const REPORT_EVENT: &str = "report";

pub fn run() {
    let paths = match Paths::from_env() {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("reap: {e:#}");
            std::process::exit(1);
        }
    };
    let (sender, receiver) = mpsc::channel::<Event>();
    let state = Arc::new(State::new(paths, sender));

    let app = tauri::Builder::default()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::report,
            commands::refresh,
            commands::toggle_pause,
            commands::pick_folder,
            commands::add,
            commands::keep,
            commands::unkeep,
            commands::remove_now,
            commands::open_in_finder,
            commands::hide,
            commands::quit,
        ])
        .setup({
            let state = state.clone();
            move |app| {
                #[cfg(target_os = "macos")]
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                let handle = app.handle().clone();
                // The fallback for a broken web view.
                let menu = Menu::with_items(
                    &handle,
                    &[
                        &MenuItem::with_id(&handle, "refresh", "Refresh", true, None::<&str>)?,
                        &MenuItem::with_id(&handle, "quit", "Quit reap", true, None::<&str>)?,
                    ],
                )?;
                TrayIconBuilder::with_id(TRAY_ID)
                    .icon(icon(Icon::Pending))
                    .icon_as_template(true)
                    .tooltip("reap")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event({
                        let state = state.clone();
                        move |app, event| match event.id().as_ref() {
                            "refresh" => state.send(Event::Refresh),
                            "quit" => app.exit(0),
                            _ => {}
                        }
                    })
                    .on_tray_icon_event({
                        let state = state.clone();
                        move |tray, event| {
                            if let TrayIconEvent::Click {
                                button: MouseButton::Left,
                                button_state: MouseButtonState::Up,
                                rect,
                                ..
                            } = event
                            {
                                window::toggle(tray.app_handle(), &state, &rect);
                            }
                        }
                    })
                    .build(app)?;
                window::watch_focus(&handle, &state);
                std::thread::spawn({
                    let state = state.clone();
                    move || worker::run(&handle, &state, &receiver)
                });
                Ok(())
            }
        })
        .build(tauri::generate_context!())
        .expect("building the reap app");
    app.run(|_, event| {
        // The popover only hides: never quit because "the last window closed".
        if let RunEvent::ExitRequested {
            api, code: None, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}

/// Shows the current state on the icon and sends it to the popover.
pub fn publish(app: &AppHandle, state: &Arc<State>) {
    let snapshot = state.snapshot();
    if let Err(e) = app.emit(REPORT_EVENT, &snapshot) {
        eprintln!("reap: sending the report to the popover failed: {e}");
    }
    let (app_handle, shared) = (app.clone(), state.clone());
    // The icon belongs to the main thread; the report is already built.
    let _ = app.run_on_main_thread(move || {
        if let Err(e) = apply_icon(&app_handle, &shared) {
            eprintln!("reap: updating the icon failed: {e}");
        }
    });
}

fn apply_icon(app: &AppHandle, state: &State) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let report = state.report.lock().unwrap();
    let has_error = state.report_error.lock().unwrap().is_some();
    let shown = if has_error {
        Icon::NeedsYou
    } else {
        logic::icon_for(report.as_ref())
    };
    tray.set_icon(Some(icon(shown)))?;
    tray.set_icon_as_template(shown != Icon::NeedsYou)?;
    let (title, tooltip) = match report.as_ref() {
        _ if has_error => (None, "reap — cannot read repos.toml".to_string()),
        Some(report) => (
            (report.needs_you > 0).then(|| report.needs_you.to_string()),
            format!("reap — {}", report.overall.label()),
        ),
        None => (None, "reap — checking…".to_string()),
    };
    tray.set_title(title)?;
    tray.set_tooltip(Some(tooltip))?;
    Ok(())
}

/// 18pt status icons at 2x, rendered from `icons/src/tray-*.svg` by
/// `scripts/render-icons.sh`: the mark, four cubes with the back one lifted out (tidy), the
/// mark in outline round its notch (removal pending), the mark in orange (needs you, not a
/// template image).
fn icon(state: Icon) -> Image<'static> {
    let png: &[u8] = match state {
        Icon::Tidy => include_bytes!("../icons/tray/tidy.png"),
        Icon::Pending => include_bytes!("../icons/tray/pending.png"),
        Icon::NeedsYou => include_bytes!("../icons/tray/needs-you.png"),
    };
    Image::from_bytes(png).expect("the tray PNGs are embedded at build time and decode")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tray_icon_decodes_at_18pt_2x() {
        for state in [Icon::Tidy, Icon::Pending, Icon::NeedsYou] {
            let image = icon(state);
            assert_eq!((image.width(), image.height()), (36, 36), "{state:?}");
        }
    }
}
