//! The provider-neutral part of the safety boundary of a chat run.
//!
//! The Sonara tools below save, export, open or replace a project, or render and analyse audio
//! files. The user does these, so no chat run may call them, whatever agent runs it. They are
//! listed here as bare tool names, once. The Claude adapter keeps its own, prefixed lists
//! ([`crate::composer::invocation::DENIED_TOOLS`]) and a test checks that the two agree; an
//! adapter whose agent has another way to deny tools (Codex) builds its deny rules from this
//! list.

/// The Sonara tools a chat run must never use, by their bare names (`save_project`, not
/// `mcp__sonara__save_project`).
pub const DENIED_SONARA_TOOLS: [&str; 13] = [
    "save_project",
    "open_project",
    "create_project",
    "export_audio",
    "export_midi",
    "export_stems",
    "render_preview",
    "relink_media",
    "start_recording",
    "set_midi_input",
    "verify_project",
    "analyze_audio",
    "get_audio_analysis",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composer::invocation::DENIED_TOOLS;

    const PREFIX: &str = "mcp__sonara__";

    #[test]
    fn the_bare_names_are_exactly_the_sonara_entries_of_the_claude_deny_list() {
        let claude: Vec<&str> = DENIED_TOOLS
            .iter()
            .filter_map(|tool| tool.strip_prefix(PREFIX))
            .collect();
        assert_eq!(claude.len(), DENIED_SONARA_TOOLS.len(), "same count");
        for name in DENIED_SONARA_TOOLS {
            assert!(claude.contains(&name), "{name} is not denied for Claude");
        }
        for name in &claude {
            assert!(
                DENIED_SONARA_TOOLS.contains(name),
                "{name} is denied for Claude only"
            );
        }
    }

    #[test]
    fn no_name_repeats_and_none_carries_the_prefix() {
        let mut names = DENIED_SONARA_TOOLS.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), DENIED_SONARA_TOOLS.len());
        assert!(DENIED_SONARA_TOOLS.iter().all(|n| !n.starts_with(PREFIX)));
    }

    #[test]
    fn every_name_is_a_tool_of_the_agent_api() {
        let docs = include_str!("../../../../docs/agent-api.md");
        for name in DENIED_SONARA_TOOLS {
            assert!(
                docs.lines().any(|line| line == format!("### `{name}`")),
                "{name} has no `### {name}` heading in docs/agent-api.md"
            );
        }
    }

    #[test]
    fn the_tools_that_edit_clip_content_are_not_denied() {
        // They change only the open project, undoably, like any other edit of a chat run.
        for name in [
            "create_midi_clip",
            "add_midi_notes",
            "edit_midi_notes",
            "write_midi_controllers",
            "create_automation",
        ] {
            assert!(!DENIED_SONARA_TOOLS.contains(&name), "{name} is denied");
        }
    }
}
