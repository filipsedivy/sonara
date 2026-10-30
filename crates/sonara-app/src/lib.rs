//! Application services on top of the session: projects, media, render and export jobs,
//! transport, templates and settings. The composition root of the
//! application.
//!
//! [`context::AppContext`] wires the services together, as the prototype's
//! `sonara.application.context` does. The transport service ([`transport`]) plays,
//! records and takes live MIDI input; its MIDI part, the devices and the input hub, is in
//! [`midi`].
//!
//! An interactive session (the desktop application) adds the autosave timer
//! ([`autosave`]), the delivery of session events, job changes and status messages to its
//! window ([`events`]) and the saved window layout ([`window_layout`]). Its data layer
//! is the project view ([`view`]), the edit boundary ([`edit`]), the live frame ([`live`]), the
//! waveform peaks ([`peaks`]) and the EQ curve ([`eq_response`]). Its main window adds
//! the title and the Undo and Redo items ([`chrome`]) and the transport bar's calls
//! ([`transport_control`]). The mixer adds the plugin pickers ([`catalog`]). The
//! browser, the inspector and the dialogs add the browser's tree and folder listing
//! ([`browser`]), the New Project form ([`projects`]), the Settings dialog
//! ([`settings_dialog`]), the Export dialog ([`export_dialog`]), the prompts and reports
//! ([`prompts`]) and About and Agent Connection ([`about`]).
//! The menus and shortcuts add the native menu bar's tree ([`menus`]). The AI composer
//! ([`composer`]) runs a headless Claude Code as an agent that composes in the open project;
//! the AI Chat ([`chat`]) is a conversation with such an agent, as a queue of tasks.
#![forbid(unsafe_code)]

pub mod about;
pub mod autosave;
pub mod browser;
pub mod catalog;
pub mod chat;
pub mod chrome;
pub mod clock;
pub mod composer;
pub mod context;
pub mod edit;
pub mod eq_response;
pub mod error;
pub mod events;
pub mod export_dialog;
pub mod ids;
pub mod instruments;
pub mod jobs;
pub mod live;
pub mod log_level;
pub mod media;
pub mod menus;
pub mod midi;
pub mod paths;
pub mod peaks;
pub mod projects;
pub mod prompts;
pub mod pyjson;
pub mod quality;
pub mod render;
pub mod samples;
pub mod session;
pub mod settings;
pub mod settings_dialog;
mod sync;
pub mod templates;
pub mod transport;
pub mod transport_control;
pub mod view;
pub mod vocabulary;
pub mod window_layout;
