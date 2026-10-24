//! Sonara's domain layer: the musical time base, the project model, IDs, routing rules, typed
//! errors, commands, undo history and the session. Pure: no I/O, no audio, no GUI.
#![forbid(unsafe_code)]

pub mod commands;
pub mod error;
pub mod ids;
pub mod midi;
pub mod model;
pub mod plugins;
pub mod pyfmt;
pub mod routing;
pub mod session;
pub mod time;
pub mod undo;

/// Tolerance for comparing beat positions, as in the prototype (`sonara.domain.common`).
pub const EPSILON: f64 = 1e-9;
