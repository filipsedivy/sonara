//! `get_capabilities`: what this build can do.

use serde_json::json;
use sonara_app::render::export_formats;
use sonara_core::model::project::MAX_TRACKS;

use crate::context::AgentContext;
use crate::contract::API_VERSION;
use crate::contract::args::Empty;
use crate::registry::ToolResult;

/// The longest render, in seconds (three hours).
const MAX_RENDER_SECONDS: u32 = 3 * 3600;
/// The longest preview render, in seconds.
const MAX_PREVIEW_SECONDS: u32 = 900;
/// The most notes one call may carry (the `max_length` of the note lists in the contract).
const MAX_NOTES_PER_CALL: u32 = 20_000;

/// `get_capabilities`: the API version, export formats, audio engine status, allowed file
/// locations and limits.
///
/// `export_formats` lists the containers this build writes, sorted (MP3 only when libmp3lame can
/// be loaded), as the prototype listed libsndfile's. `ffmpeg_import` says whether the import
/// fallback finds `ffmpeg` now (`ffmpeg_path() is not None`, [`MediaService::ffmpeg`]).
///
/// [`MediaService::ffmpeg`]: sonara_app::media::MediaService::ffmpeg
#[expect(
    clippy::unnecessary_wraps,
    reason = "every handler returns a ToolResult (ToolRegistry::register)"
)]
pub(super) fn get_capabilities(ctx: &AgentContext, _args: Empty) -> ToolResult {
    Ok(json!({
        "api_version": API_VERSION,
        "export_formats": export_formats(),
        "ffmpeg_import": ctx.app().media().ffmpeg().is_some(),
        "audio_engine": super::transport::engine_status(ctx.app()),
        "allowed_paths": ctx.policy().roots(),
        "confirmations_enabled": ctx.confirmations().is_enabled(),
        "limits": {
            "max_render_seconds": MAX_RENDER_SECONDS,
            "max_preview_seconds": MAX_PREVIEW_SECONDS,
            "max_notes_per_call": MAX_NOTES_PER_CALL,
            "max_tracks": MAX_TRACKS,
        },
        "third_party_plugins": "not supported yet",
    }))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use sonara_app::context::{AppContext, AppOptions};
    use sonara_app::media::{Ffmpeg, FfmpegLocator};
    use sonara_app::settings::Settings;

    use crate::context::PolicyEnvironment;
    use crate::service::{AgentOptions, AgentService, AuditTarget};

    /// `get_capabilities` on an application whose import finds `FFmpeg` with `locator`.
    fn capabilities(locator: FfmpegLocator) -> serde_json::Value {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("workspaces")),
            settings: Some(Settings::default()),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ffmpeg: Some(locator),
            ..AppOptions::default()
        })
        .expect("the application");
        let service = AgentService::new(
            Arc::new(app),
            AgentOptions {
                base_dir: Some(dir.path().to_owned()),
                audit: AuditTarget::Disabled,
                environment: Some(PolicyEnvironment {
                    allowed: None,
                    projects_dir: dir.path().join("projects"),
                }),
                ..AgentOptions::default()
            },
        )
        .expect("the service");
        service.call("get_capabilities", None)
    }

    #[test]
    fn ffmpeg_import_reports_whether_the_import_finds_ffmpeg() {
        let found = Ffmpeg::new(PathBuf::from("/opt/ffmpeg/bin/ffmpeg"), None);
        let with = capabilities(Arc::new(move || Some(found.clone())));
        assert_eq!(with["ok"], true);
        assert_eq!(with["ffmpeg_import"], true);
        let without = capabilities(Arc::new(|| None));
        assert_eq!(without["ffmpeg_import"], false);
    }
}
