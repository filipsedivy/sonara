//! The Sonara tools a Codex run may call.
//!
//! Codex has no "only these tools" switch for an MCP server except its allow-list
//! (`enabled_tools`), so the chat lists every tool it allows by name and, as a second layer,
//! denies the tools of [`DENIED_SONARA_TOOLS`] in `disabled_tools`. The allow-list is every
//! tool of the Agent API that is not denied.
//!
//! `sonara-app` cannot depend on `sonara-agent`, which holds the tool registry (ARC-11: the
//! agent depends on the application), so [`ALL_SONARA_TOOLS`] is a copy of the tool
//! headings of `docs/agent-api.md`, which that registry generates. A test checks the copy
//! against the document in both directions, so a tool that is added or renamed fails the build
//! until the list is updated (and the new tool is a conscious decision for the boundary).

use crate::chat::boundary::DENIED_SONARA_TOOLS;

/// Every tool of the Agent API, by name, in the order of `docs/agent-api.md`.
pub const ALL_SONARA_TOOLS: [&str; 58] = [
    "add_effect",
    "add_marker",
    "add_midi_notes",
    "add_section",
    "analyze_audio",
    "cancel_operation",
    "configure_routing",
    "create_automation",
    "create_midi_clip",
    "create_project",
    "create_track",
    "delete_clips",
    "delete_track",
    "duplicate_clip",
    "edit_clip",
    "edit_midi_notes",
    "export_audio",
    "export_midi",
    "export_stems",
    "generate_midi",
    "get_audio_analysis",
    "get_capabilities",
    "get_history",
    "get_operation_status",
    "get_project_state",
    "import_audio",
    "import_midi",
    "list_midi_devices",
    "list_operations",
    "list_plugins",
    "list_tracks",
    "load_impulse_response",
    "move_clip",
    "open_project",
    "play_preview",
    "redo",
    "relink_media",
    "remove_effect",
    "rename_track",
    "render_preview",
    "save_project",
    "set_effect_parameter",
    "set_instrument",
    "set_loop",
    "set_metadata",
    "set_midi_input",
    "set_project_length",
    "set_tempo",
    "set_time_signature",
    "set_track_pan",
    "set_track_state",
    "set_track_volume",
    "split_clip",
    "start_recording",
    "stop_playback",
    "undo",
    "verify_project",
    "write_midi_controllers",
];

/// The tools Codex may call: every tool of the Agent API that the chat does not deny.
#[must_use]
pub fn allowed_sonara_tools() -> Vec<&'static str> {
    ALL_SONARA_TOOLS
        .into_iter()
        .filter(|tool| !DENIED_SONARA_TOOLS.contains(tool))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tool names of the headings of `docs/agent-api.md`.
    fn documented() -> Vec<&'static str> {
        include_str!("../../../../../../docs/agent-api.md")
            .lines()
            .filter_map(|line| line.strip_prefix("### `")?.strip_suffix('`'))
            .collect()
    }

    #[test]
    fn the_list_is_the_tools_of_the_agent_api_in_both_directions() {
        let docs = documented();
        for name in ALL_SONARA_TOOLS {
            assert!(
                docs.contains(&name),
                "{name} has no heading in docs/agent-api.md"
            );
        }
        for name in &docs {
            assert!(
                ALL_SONARA_TOOLS.contains(name),
                "{name} is documented but missing from ALL_SONARA_TOOLS"
            );
        }
        assert_eq!(docs.len(), ALL_SONARA_TOOLS.len(), "no heading repeats");
        let mut names = ALL_SONARA_TOOLS.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL_SONARA_TOOLS.len(), "no name repeats");
    }

    #[test]
    fn the_allowed_and_the_denied_tools_are_disjoint_and_make_up_all_of_them() {
        let allowed = allowed_sonara_tools();
        for name in DENIED_SONARA_TOOLS {
            assert!(!allowed.contains(&name), "{name} is denied and allowed");
        }
        let mut union: Vec<&str> = allowed.iter().copied().chain(DENIED_SONARA_TOOLS).collect();
        union.sort_unstable();
        let mut all = ALL_SONARA_TOOLS.to_vec();
        all.sort_unstable();
        assert_eq!(union, all);
        assert_eq!(allowed.len(), 58 - 13);
    }

    #[test]
    fn the_tools_that_change_what_leaves_the_project_are_not_allowed() {
        let allowed = allowed_sonara_tools();
        for name in [
            "save_project",
            "export_audio",
            "export_midi",
            "export_stems",
            "open_project",
            "create_project",
        ] {
            assert!(!allowed.contains(&name), "{name}");
        }
        // The tools the chat is for stay.
        for name in [
            "get_project_state",
            "create_track",
            "set_tempo",
            "generate_midi",
            "write_midi_controllers",
            "undo",
        ] {
            assert!(allowed.contains(&name), "{name}");
        }
    }
}
