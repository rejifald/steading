//! The popover window (ADR 0002 rule 2): toggled by a left click on the tray
//! icon, placed centred under it, hidden on Escape and when it loses focus.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, PhysicalPosition, Rect, WebviewWindow, WindowEvent};

use crate::logic::{self, Area, Event};
use crate::state::State;

/// The window's label in tauri.conf.json.
pub const LABEL: &str = "popover";
/// Gap between the menu bar and the popover, in points.
const GAP: f64 = 4.0;
/// Clicking the icon of an open popover first blurs it (which hides it) and
/// then delivers the click; a click this soon after that hide closes, not reopens.
const REOPEN_GUARD: Duration = Duration::from_millis(300);

fn popover(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Hide on focus loss, except while the folder picker holds the focus.
pub fn watch_focus(app: &AppHandle, state: &Arc<State>) {
    let Some(window) = popover(app) else { return };
    let (handle, state) = (window.clone(), state.clone());
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            if state.picking.load(Ordering::SeqCst) {
                return;
            }
            *state.hidden_at.lock().unwrap() = Some(Instant::now());
            let _ = handle.hide();
        }
    });
}

/// A left click on the icon: open under it, or close.
pub fn toggle(app: &AppHandle, state: &State, icon: &Rect) {
    let Some(window) = popover(app) else { return };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    if !logic::reopens(
        *state.hidden_at.lock().unwrap(),
        Instant::now(),
        REOPEN_GUARD,
    ) {
        return;
    }
    if let Err(e) = place_under(&window, icon) {
        eprintln!("tether: placing the popover failed: {e}");
    }
    let _ = window.show();
    let _ = window.set_focus();
    // The popover shows the last snapshot at once; a refresh re-reads every
    // clone (local git only) so a branch switched a moment ago shows up. It
    // syncs no repo that was not already due.
    state.send(Event::Refresh);
}

/// Brings the popover back where it was, e.g. after the folder picker.
pub fn show(app: &AppHandle) {
    if let Some(window) = popover(app) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = popover(app) {
        let _ = window.hide();
    }
}

fn place_under(window: &WebviewWindow, icon: &Rect) -> tauri::Result<()> {
    let scale = window.scale_factor()?;
    let at = icon.position.to_physical::<f64>(scale);
    let size = icon.size.to_physical::<f64>(scale);
    let icon = Area {
        x: at.x,
        y: at.y,
        w: size.width,
        h: size.height,
    };
    let (cx, cy) = (icon.x + icon.w / 2.0, icon.y + icon.h / 2.0);
    let screen = window
        .available_monitors()?
        .into_iter()
        .map(|m| {
            let work = m.work_area();
            let full = Area {
                x: m.position().x.into(),
                y: m.position().y.into(),
                w: m.size().width.into(),
                h: m.size().height.into(),
            };
            let work = Area {
                x: work.position.x.into(),
                y: work.position.y.into(),
                w: work.size.width.into(),
                h: work.size.height.into(),
            };
            (full, work)
        })
        .find(|(full, _)| full.contains(cx, cy))
        .map(|(_, work)| work);
    let own = window.outer_size()?;
    let (x, y) = logic::popover_origin(
        &icon,
        (own.width.into(), own.height.into()),
        screen.as_ref(),
        GAP * scale,
    );
    // Screen coordinates are far inside i32; the f64s come from them.
    #[allow(clippy::cast_possible_truncation)]
    let position = PhysicalPosition::new(x.round() as i32, y.round() as i32);
    window.set_position(position)
}
