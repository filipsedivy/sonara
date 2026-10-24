//! Quitting Sonara: the unsaved-changes prompt, then the shutdown.
//!
//! The prompt is [`lifecycle::confirm_discard`]'s "Save changes to “name”?" with Save, Discard
//! and Cancel; a failed or cancelled save keeps Sonara open. After Save or Discard, the window
//! layout is saved, the [`Shell`] shuts down (a discarded project drops its autosave) and the
//! application exits. It runs as a lifecycle flow, so it never stacks on another flow's panel.

use tauri::AppHandle;

use crate::lifecycle::{self, Decision};
use crate::shell::Shell;
use crate::window;

/// Asks to save unsaved changes and quits unless the user cancels. Does nothing while another
/// prompt or panel is open.
pub fn request(app: &AppHandle) {
    lifecycle::spawn(app, "quit", |app, shell: &Shell| {
        if let Decision::Proceed { discarded } = lifecycle::confirm_discard(app, shell) {
            window::save_layout(app);
            shell.shutdown(discarded);
            app.exit(0);
        }
    });
}
