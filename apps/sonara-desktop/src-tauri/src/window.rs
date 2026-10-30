//! The main window: opened at its saved place and size (`<config>/window.json`), else at the
//! prototype's 1440 × 900, centred; its layout, with the panes the page reported, is
//! saved when Sonara quits. The mixer can move to a window of its own, which the
//! main window's page is told about when it opens and closes (`mixer-window`). That window's
//! place and size are saved when it closes and when Sonara quits, with whether it was open then,
//! and it opens there again, at startup too when it was open (the prototype's
//! `restoreState` reopened a floating Editors dock). The AI Chat has a window of its own too, which
//! opens centred ([`open_chat`]).

use serde::Serialize;
use sonara_app::window_layout::{
    MixerWindowLayout, ScreenArea, WindowLayout, load_mixer_window_layout, load_window_layout,
    save_mixer_window_layout, save_window_layout, window_layout_path,
};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::shell::Shell;

/// The label of the main window (`capabilities/default.json`).
pub const MAIN: &str = "main";

/// The label of the mixer's own window (`capabilities/default.json`). It loads the same
/// page, which shows only the mixer for this label.
pub const MIXER: &str = "mixer";

/// The label of the AI Chat's window (`capabilities/default.json`). It loads the same page,
/// which shows only the chat for this label.
pub const CHAT: &str = "chat";

/// The chat window's first size.
const CHAT_SIZE: (f64, f64) = (520.0, 720.0);

/// The smallest size the chat window can be given.
const CHAT_MIN_SIZE: (f64, f64) = (380.0, 480.0);

/// The mixer window's first size.
const MIXER_SIZE: (f64, f64) = (1000.0, 560.0);

/// The prototype's initial size (`MainWindow.resize(1440, 900)`).
const DEFAULT_SIZE: (f64, f64) = (1440.0, 900.0);

/// The smallest size the window can be given.
const MIN_SIZE: (f64, f64) = (960.0, 640.0);

/// The displays' areas, in logical pixels.
fn screens(app: &AppHandle) -> Vec<ScreenArea> {
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|monitor| {
            let scale = monitor.scale_factor();
            let origin = monitor.position().to_logical::<f64>(scale);
            let size = monitor.size().to_logical::<f64>(scale);
            ScreenArea {
                x: origin.x,
                y: origin.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect()
}

/// The saved layout, when there is one that a display still shows.
fn saved_layout(app: &AppHandle) -> Option<WindowLayout> {
    let path = window_layout_path().ok()?;
    let layout = load_window_layout(&path)?;
    layout.is_visible_on(&screens(app)).then_some(layout)
}

/// Opens the main window, hidden until it is placed. `on_top` keeps it above the others and on
/// every Space (the timeline benchmark: the web view stops drawing a hidden window). Another
/// app's full-screen Space on its display still hides it: `FullScreenAuxiliary` does not change
/// that (tried at fix), so the harnesses check for one.
///
/// # Errors
///
/// A Tauri error when the window cannot be created or shown.
pub fn open_main(app: &AppHandle, on_top: bool) -> tauri::Result<WebviewWindow> {
    let layout = saved_layout(app);
    let (width, height) = layout.map_or(DEFAULT_SIZE, |l| (l.width, l.height));
    let mut builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("Sonara")
        .inner_size(width, height)
        .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
        .always_on_top(on_top)
        .visible_on_all_workspaces(on_top)
        .visible(false);
    builder = match layout {
        Some(layout) => builder.position(layout.x, layout.y),
        None => builder.center(),
    };
    let window = builder.build()?;
    if layout.is_some_and(|layout| layout.maximized) {
        window.maximize()?;
    }
    window.show()?;
    window.set_focus()?;
    Ok(window)
}

/// The mixer window's saved layout, when there is one that a display still shows.
fn saved_mixer_layout(app: &AppHandle) -> Option<MixerWindowLayout> {
    let path = window_layout_path().ok()?;
    let layout = load_mixer_window_layout(&path)?;
    layout.is_visible_on(&screens(app)).then_some(layout)
}

/// Whether the mixer's window was open when Sonara last quit, so that the main window's page
/// opens it again (A10).
pub fn mixer_was_open(app: &AppHandle) -> bool {
    saved_mixer_layout(app).is_some_and(|layout| layout.open)
}

/// Saves the mixer window's place and size, with `open` (whether it opens again at startup),
/// when the window exists; a failure is logged (the layout is no contract).
pub fn save_mixer_layout(app: &AppHandle, open: bool) {
    let Some(window) = app.get_webview_window(MIXER) else {
        return;
    };
    let layout = (|| -> tauri::Result<MixerWindowLayout> {
        let scale = window.scale_factor()?;
        let position = window.outer_position()?.to_logical::<f64>(scale);
        let size = window.inner_size()?.to_logical::<f64>(scale);
        Ok(MixerWindowLayout {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
            open,
        })
    })();
    let saved = match (layout, window_layout_path()) {
        (Ok(layout), Ok(path)) => {
            save_mixer_window_layout(&layout, &path).map_err(|e| e.to_string())
        }
        (Err(error), _) => Err(error.to_string()),
        (_, Err(error)) => Err(error.to_string()),
    };
    if let Err(error) = saved {
        log::warn!("cannot save the mixer window's layout: {error}");
    }
}

/// Saves the main window's place and size, and the mixer window's as open when it is; a failure
/// is logged (the layout is no contract).
pub fn save_layout(app: &AppHandle) {
    save_mixer_layout(app, true);
    let Some(window) = app.get_webview_window(MAIN) else {
        return;
    };
    let layout = (|| -> tauri::Result<WindowLayout> {
        let scale = window.scale_factor()?;
        let position = window.outer_position()?.to_logical::<f64>(scale);
        let size = window.inner_size()?.to_logical::<f64>(scale);
        Ok(WindowLayout {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
            maximized: window.is_maximized()?,
            panes: app.state::<Shell>().panes(),
        })
    })();
    let saved = match (layout, window_layout_path()) {
        (Ok(layout), Ok(path)) => save_window_layout(&layout, &path).map_err(|e| e.to_string()),
        (Err(error), _) => Err(error.to_string()),
        (_, Err(error)) => Err(error.to_string()),
    };
    if let Err(error) = saved {
        log::warn!("cannot save the window layout: {error}");
    }
}

/// `mixer-window`: whether the mixer's own window is open.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MixerWindowState {
    /// True once the window opened, false once it closed.
    pub open: bool,
}

/// Tells the main window's page whether the mixer's window is open; a failure is logged.
pub fn report_mixer(app: &AppHandle, open: bool) {
    if let Err(error) = app.emit_to(MAIN, "mixer-window", MixerWindowState { open }) {
        log::warn!("cannot tell the window about the mixer's window: {error}");
    }
}

/// Opens the mixer in a window of its own, or brings that window to the front.
///
/// # Errors
///
/// A Tauri error when the window cannot be created or shown.
pub fn open_mixer(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MIXER) {
        window.unminimize()?;
        window.show()?;
        return window.set_focus();
    }
    let saved = saved_mixer_layout(app);
    let (width, height) = saved.map_or(MIXER_SIZE, |l| (l.width, l.height));
    let mut builder = WebviewWindowBuilder::new(app, MIXER, WebviewUrl::App("index.html".into()))
        .title("Mixer — Sonara")
        .inner_size(width, height)
        .min_inner_size(480.0, 360.0);
    if let Some(layout) = saved {
        builder = builder.position(layout.x, layout.y);
    }
    let window = builder.build()?;
    window.set_focus()?;
    report_mixer(app, true);
    Ok(())
}

/// Opens the AI Chat in a window of its own, or brings that window to the front. The window
/// has no saved place: it opens centred, and closing it wipes the chat ([`crate::chat`]).
///
/// # Errors
///
/// A Tauri error when the window cannot be created or shown.
pub fn open_chat(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(CHAT) {
        window.unminimize()?;
        window.show()?;
        return window.set_focus();
    }
    let window = WebviewWindowBuilder::new(app, CHAT, WebviewUrl::App("index.html".into()))
        .title("AI Chat (Beta)")
        .inner_size(CHAT_SIZE.0, CHAT_SIZE.1)
        .min_inner_size(CHAT_MIN_SIZE.0, CHAT_MIN_SIZE.1)
        .center()
        .build()?;
    window.set_focus()
}
