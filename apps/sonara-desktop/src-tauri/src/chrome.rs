//! The window's chrome follows the session: the title with the dirty marker,
//! the Undo and Redo items, and Open Recent ([`sonara_app::chrome::WindowChrome`] words them).
//!
//! The title carries the unsaved changes as the prototype's does ("name • — path — Sonara"),
//! and on macOS the close button's dot does too (the prototype's `setWindowModified`). The
//! windowing library has no call for the dot, so it is set through `objc2-app-kit`'s safe
//! `NSWindow::setDocumentEdited` (QG-10). The window is found among the application's
//! windows by comparing its address with the one Tauri reports, which needs no `unsafe`.
//!
//! The chrome is read on the calling thread (the event pump's) and applied on the main thread,
//! posted without waiting: the window and its menu wait for the main thread, which may itself be
//! waiting for the pump to stop when Sonara quits.

use sonara_app::chrome::WindowChrome;
use tauri::{AppHandle, Manager};

use crate::menu::MenuHandles;
use crate::shell::Shell;
use crate::window::MAIN;

/// Shows the session's state in the title and the Edit menu; `recent` refreshes Open Recent
/// too (after a project was opened, saved or closed). Failures are logged.
pub fn refresh(app: &AppHandle, recent: bool) {
    let shell = app.state::<Shell>();
    let chrome = WindowChrome::of(shell.app().session());
    let recent = recent.then(|| shell.app().projects().recent_projects());
    let handle = app.clone();
    let posted = app.run_on_main_thread(move || apply(&handle, &chrome, recent));
    if let Err(error) = posted {
        log::warn!("cannot update the window title and menu: {error}");
    }
}

fn apply(app: &AppHandle, chrome: &WindowChrome, recent: Option<Vec<String>>) {
    if let Some(window) = app.get_webview_window(MAIN) {
        if let Err(error) = window.set_title(&chrome.title) {
            log::warn!("cannot set the window title: {error}");
        }
        show_edited(&window, chrome.dirty);
    }
    let Some(menu) = app.try_state::<MenuHandles>() else {
        return;
    };
    if let Err(error) = menu.show_history(chrome) {
        log::warn!("cannot update Undo and Redo: {error}");
    }
    if let Some(paths) = recent
        && let Err(error) = menu.show_recent(app, paths)
    {
        log::warn!("cannot update Open Recent: {error}");
    }
}

/// The application's `NSWindow` of `window`, found among its windows by address (comparing
/// addresses dereferences nothing). On the main thread only; `None` elsewhere or for a window the
/// application does not list.
#[cfg(target_os = "macos")]
pub(crate) fn ns_window_of(
    window: &tauri::WebviewWindow,
) -> Option<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2_app_kit::NSApplication;

    let target = window.ns_window().ok()?;
    let main_thread = MainThreadMarker::new()?;
    NSApplication::sharedApplication(main_thread)
        .windows()
        .iter()
        .find(|ns_window| {
            Retained::as_ptr(ns_window).cast::<std::ffi::c_void>() == target.cast_const()
        })
}

/// The macOS close button's dot of unsaved changes (`NSWindow.documentEdited`). Runs on the main
/// thread; elsewhere it does nothing and says so.
#[cfg(target_os = "macos")]
fn show_edited(window: &tauri::WebviewWindow, edited: bool) {
    match ns_window_of(window) {
        Some(ns_window) if ns_window.isDocumentEdited() != edited => {
            ns_window.setDocumentEdited(edited);
        }
        Some(_) => {}
        None => log::warn!("cannot find the window for the unsaved-changes dot"),
    }
}

#[cfg(not(target_os = "macos"))]
fn show_edited(_window: &tauri::WebviewWindow, _edited: bool) {}

/// Whether the main window's close button shows the dot of unsaved changes (the end-to-end
/// check of the `bench` build). On the main thread only; `None` elsewhere.
#[cfg(all(feature = "bench", target_os = "macos"))]
pub fn shows_edited(app: &AppHandle) -> Option<bool> {
    let window = app.get_webview_window(MAIN)?;
    ns_window_of(&window).map(|ns_window| ns_window.isDocumentEdited())
}
