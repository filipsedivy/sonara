//! The Sonara desktop application (ADR 0009): a Tauri 2 window around one
//! `sonara-app` session, with the IPC server that `sonara mcp` attaches to.
//!
//! The process model: this is the bundle's main executable, `sonara-desktop`; the
//! `sonara` CLI ships beside it in `Contents/MacOS`, and `sonara gui [PROJECT]` starts this
//! executable.
//!
//! Startup, as the prototype's `sonara.ui.app.main`:
//!
//! 1. the options and the logger (stderr and `logs/sonara.log`);
//! 2. the App Nap opt-out;
//! 3. the application with `--audio` or the settings' backend, the project argument (else a
//!    new "Untitled" project from the "empty" template), the IPC server with app kind `gui`
//!    (unless `--no-ipc`) and the autosave timer ([`shell::Shell::start`]);
//! 4. the event pump, the live feed and the main window, whose page asks for the agent label,
//!    offers the recovery of scratch sessions and reads the project view, the live
//!    frames and the `peaks:` scheme;
//! 5. the window's title and menu from the session ([`chrome`], [`menu`]), and live MIDI input
//!    when the backend plays and input ports exist.
//!
//! The File menu's flows ([`lifecycle`]) and quitting ([`quit`]) ask about unsaved changes first.
//! The browser, the inspector and the dialogs are [`dialogs`]; every native prompt and
//! panel goes through [`prompt`]. The menu bar is [`menu`], built from `sonara-app`'s
//! model of it.
#![forbid(unsafe_code)]

#[cfg(feature = "bench")]
mod bench;
mod chrome;
mod commands;
mod dialogs;
mod lifecycle;
mod live;
mod logging;
mod menu;
mod nap;
mod options;
mod prompt;
mod quit;
mod shell;
mod window;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::thread;

use clap::Parser as _;
use sonara_app::events::StatusMessage;
use sonara_app::transport_control;
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

use crate::options::Options;
use crate::shell::Shell;

fn main() -> ExitCode {
    let options = match Options::try_parse() {
        Ok(options) => options,
        Err(error) => {
            // `--help` and `--version` exit with 0, usage errors with 2.
            let code = error.exit_code();
            let _ = error.print();
            return ExitCode::from(u8::try_from(code).unwrap_or(2));
        }
    };
    match logging::init(&options.log_level) {
        Ok(Some(path)) => log::info!("logging to {}", path.display()),
        Ok(None) => {}
        Err(message) => {
            eprintln!("sonara-desktop: {message}");
            return ExitCode::from(2);
        }
    }
    // Held until the process exits: `run` below returns only then.
    let _activity = nap::keep_latency_critical();
    let (shell, autosave_of) = match Shell::start(&options) {
        Ok(started) => started,
        Err(message) => {
            log::error!("{message}");
            return ExitCode::FAILURE;
        }
    };
    let builder = tauri::Builder::default();
    #[cfg(feature = "bench")]
    let builder = builder
        .manage(bench::BenchConfig::from_env())
        .manage(bench::Prompts::default())
        .manage(bench::MenuLog::default());
    let built = builder
        .plugin(tauri_plugin_dialog::init())
        .manage(shell)
        .register_asynchronous_uri_scheme_protocol("peaks", live::peaks_protocol)
        .invoke_handler(invoke_handler())
        .menu(menu::build)
        .on_menu_event(menu::handle)
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } if window.label() == window::MAIN => {
                api.prevent_close();
                quit::request(window.app_handle());
            }
            // Closed by the user: its place is kept, and it stays closed at the next start (A10).
            WindowEvent::CloseRequested { .. } if window.label() == window::MIXER => {
                window::save_mixer_layout(window.app_handle(), false);
            }
            // The mixer goes back into the main window's tab.
            WindowEvent::Destroyed if window.label() == window::MIXER => {
                window::report_mixer(window.app_handle(), false);
            }
            _ => {}
        })
        .setup(move |app| {
            start_window(app.handle(), autosave_of)?;
            Ok(())
        })
        .build(tauri::generate_context!());
    let app = match built {
        Ok(app) => app,
        Err(error) => {
            log::error!("cannot start the window system: {error}");
            return ExitCode::FAILURE;
        }
    };
    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Quitting without the prompt (the Dock's Quit, logging out): the autosave stays
            // for recovery.
            let shell = handle.state::<Shell>();
            if !shell.is_stopped() {
                window::save_layout(handle);
                shell.shutdown(false);
            }
        }
    });
    ExitCode::SUCCESS
}

/// The window's commands ([`commands`]; with the `bench` feature, the benchmarks' too).
fn invoke_handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::shell_status,
        commands::gui_theme,
        commands::recoverable_sessions,
        commands::recover_session,
        commands::project_view,
        commands::project_tracks,
        commands::project_clips,
        commands::project_devices,
        commands::edit,
        commands::undo,
        commands::redo,
        commands::seal_undo,
        commands::set_midi_focus,
        commands::eq_response,
        commands::peaks,
        commands::subscribe_live,
        commands::transport_play,
        commands::transport_stop,
        commands::transport_record,
        commands::transport_seek,
        commands::transport_status,
        commands::import_audio_files,
        commands::import_audio_dialog,
        commands::open_project_file,
        commands::set_pane_layout,
        commands::plugin_menu,
        commands::open_mixer_window,
        commands::drum_map,
        dialogs::new_project_view,
        dialogs::create_project,
        dialogs::settings_view,
        dialogs::apply_settings,
        dialogs::export_view,
        dialogs::export_preset,
        dialogs::export_browse,
        dialogs::export_audio,
        dialogs::agent_connection,
        dialogs::about_info,
        dialogs::open_link,
        dialogs::browser_devices,
        dialogs::browser_root,
        dialogs::list_directory,
        dialogs::delete_track,
        menu::set_menu_context,
        #[cfg(feature = "bench")]
        bench::bench_config,
        #[cfg(feature = "bench")]
        bench::bench_report,
        #[cfg(feature = "bench")]
        bench::bench_clock,
        #[cfg(feature = "bench")]
        bench::bench_live_stamps,
        #[cfg(feature = "bench")]
        bench::bench_frame_cost,
        #[cfg(feature = "bench")]
        bench::bench_main_lag,
        #[cfg(feature = "bench")]
        bench::bench_document_edited,
        #[cfg(feature = "bench")]
        bench::bench_live_subscribers,
        #[cfg(feature = "bench")]
        bench::bench_show_main,
        #[cfg(feature = "bench")]
        bench::bench_answer_prompts,
        #[cfg(feature = "bench")]
        bench::bench_prompts_asked,
        #[cfg(feature = "bench")]
        bench::bench_menu_log,
        #[cfg(feature = "bench")]
        bench::bench_native_menu,
        #[cfg(feature = "bench")]
        bench::bench_activate_menu,
        #[cfg(feature = "bench")]
        bench::bench_post_keys,
        #[cfg(feature = "bench")]
        bench::bench_checkpoint,
    ]
}

/// Starts the event pump, the live feed and the autosave timer (whose autosaves the status bar
/// shows), opens the main window with its title and menu, offers the opened project's
/// newer autosave (`autosave_of`) and starts live MIDI.
fn start_window(
    app: &AppHandle,
    autosave_of: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let shell = app.state::<Shell>();
    let sink = app.clone();
    shell.start_pump(move |notice| commands::forward(&sink, notice))?;
    shell.start_live()?;
    let messages = app.clone();
    shell.start_autosave(Arc::new(move |_: &Path| {
        commands::show_message(&messages, &StatusMessage::autosaved());
    }));
    #[cfg(feature = "bench")]
    let on_top = app
        .try_state::<Option<bench::BenchConfig>>()
        .is_some_and(|config| config.is_some());
    #[cfg(not(feature = "bench"))]
    let on_top = false;
    window::open_main(app, on_top)?;
    chrome::refresh(app, true);
    if let Some(menu) = app.try_state::<menu::MenuHandles>()
        && let Err(error) = menu.show_panes(&shell.panes())
    {
        log::warn!("cannot check the View menu's panes: {error}");
    }
    if let Some(path) = autosave_of {
        lifecycle::spawn(app, "recover-autosave", move |app, shell| {
            lifecycle::offer_autosave(app, shell, &path);
        });
    }
    start_live_midi(app);
    Ok(())
}

/// Opens the MIDI inputs on a thread of its own, since it may open the audio device; a
/// failure is logged, as in the prototype.
fn start_live_midi(app: &AppHandle) {
    let handle = app.clone();
    let spawned = thread::Builder::new()
        .name("sonara-midi-start".to_owned())
        .spawn(move || {
            let shell = handle.state::<Shell>();
            match transport_control::start_live_midi(shell.app()) {
                Ok(true) => log::info!("live MIDI input started"),
                Ok(false) => {}
                Err(error) => log::info!("MIDI input not started: {}", error.message),
            }
        });
    if let Err(error) = spawned {
        log::warn!("cannot start live MIDI: {error}");
    }
}
