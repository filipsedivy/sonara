//! The tool implementations (`sonara.agent.tools.handlers`), attached to the registry one tool at
//! a time. Handlers only call application services
//! and commands.
//!
//! Modules follow the prototype's sections. Every tool of the contract is attached: the state
//! views, the plugin list, the history tools and the capabilities; the tracks, clips, MIDI,
//! devices, routing and automation; the project, media, operation and real-time tools; and the
//! render, export and analysis tools.

mod automation;
mod capabilities;
mod clips;
mod devices;
mod effects;
mod history;
mod media;
mod midi;
mod operations;
mod positions;
mod project;
mod projects;
mod render;
mod tracks;
mod transport;

use crate::registry::{RegistryError, ToolRegistry};

/// Attaches every built-in handler to `registry`.
///
/// # Errors
///
/// [`RegistryError`] when one of those tools already has a handler in `registry`.
pub fn register_builtin(registry: &mut ToolRegistry) -> Result<(), RegistryError> {
    registry.register("get_project_state", project::get_project_state)?;
    registry.register("list_tracks", project::list_tracks)?;
    registry.register("list_plugins", devices::list_plugins)?;
    registry.register("undo", history::undo)?;
    registry.register("redo", history::redo)?;
    registry.register("get_history", history::get_history)?;
    registry.register("get_capabilities", capabilities::get_capabilities)?;
    // Tracks, clips, MIDI, devices, routing and automation.
    registry.register("create_track", tracks::create_track)?;
    registry.register("rename_track", tracks::rename_track)?;
    registry.register("delete_track", tracks::delete_track)?;
    registry.register("set_track_volume", tracks::set_track_volume)?;
    registry.register("set_track_pan", tracks::set_track_pan)?;
    registry.register("set_track_state", tracks::set_track_state)?;
    registry.register("set_instrument", tracks::set_instrument)?;
    registry.register("configure_routing", tracks::configure_routing)?;
    registry.register("create_midi_clip", clips::create_midi_clip)?;
    registry.register("add_midi_notes", clips::add_midi_notes)?;
    registry.register("edit_midi_notes", clips::edit_midi_notes)?;
    registry.register("generate_midi", midi::generate_midi)?;
    registry.register("move_clip", clips::move_clip)?;
    registry.register("edit_clip", clips::edit_clip)?;
    registry.register("split_clip", clips::split_clip)?;
    registry.register("duplicate_clip", clips::duplicate_clip)?;
    registry.register("delete_clips", clips::delete_clips)?;
    registry.register("add_effect", effects::add_effect)?;
    registry.register("set_effect_parameter", effects::set_effect_parameter)?;
    registry.register("remove_effect", effects::remove_effect)?;
    registry.register("create_automation", automation::create_automation)?;
    // Project, media, operations and the real-time tools on the null backend.
    registry.register("create_project", projects::create_project)?;
    registry.register("open_project", projects::open_project)?;
    registry.register("save_project", projects::save_project)?;
    registry.register("set_tempo", projects::set_tempo)?;
    registry.register("set_time_signature", projects::set_time_signature)?;
    registry.register("set_project_length", projects::set_project_length)?;
    registry.register("add_section", projects::add_section)?;
    registry.register("add_marker", projects::add_marker)?;
    registry.register("set_metadata", projects::set_metadata)?;
    registry.register("set_loop", projects::set_loop)?;
    registry.register("verify_project", projects::verify_project)?;
    registry.register("import_audio", media::import_audio)?;
    registry.register("import_midi", media::import_midi)?;
    registry.register("load_impulse_response", media::load_impulse_response)?;
    registry.register("export_midi", media::export_midi)?;
    registry.register("relink_media", media::relink_media)?;
    registry.register("play_preview", transport::play_preview)?;
    registry.register("start_recording", transport::start_recording)?;
    registry.register("list_midi_devices", transport::list_midi_devices)?;
    registry.register("set_midi_input", transport::set_midi_input)?;
    registry.register("stop_playback", transport::stop_playback)?;
    registry.register("get_operation_status", operations::get_operation_status)?;
    registry.register("cancel_operation", operations::cancel_operation)?;
    registry.register("list_operations", operations::list_operations)?;
    // Render, export and analysis (with ).
    registry.register("render_preview", render::render_preview)?;
    registry.register("export_audio", render::export_audio)?;
    registry.register("export_stems", render::export_stems)?;
    registry.register("analyze_audio", render::analyze_audio)?;
    registry.register("get_audio_analysis", render::get_audio_analysis)?;
    Ok(())
}

/// A registry with every built-in handler.
///
/// # Panics
///
/// Never: each built-in handler is attached once, to its own tool, with that tool's argument
/// struct (a unit test builds this registry).
#[must_use]
pub fn builtin() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    register_builtin(&mut registry).expect(
        "each built-in handler is attached once, to its own tool, with its argument struct",
    );
    registry
}

#[cfg(test)]
mod tests {
    use super::{builtin, register_builtin};
    use crate::contract::TOOLS;
    use crate::registry::RegistryError;

    #[test]
    fn builtin_handlers_attach_once() {
        let mut registry = builtin();
        let names: Vec<&str> = registry.specs().iter().map(|spec| spec.name).collect();
        // Every tool of the contract, in contract order.
        let contract: Vec<&str> = TOOLS.iter().map(|spec| spec.name).collect();
        assert_eq!(names, contract);
        assert_eq!(names.len(), 57);
        assert_eq!(
            register_builtin(&mut registry),
            Err(RegistryError::Duplicate("get_project_state"))
        );
    }
}
