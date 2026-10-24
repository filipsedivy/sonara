//! Render, export and analysis tools (the prototype's "render, playback, analysis, export"
//! section): previews, exports to one or more formats, stems, file and render analysis, and the
//! stored analysis of earlier operations. The work runs in `sonara-app`'s render service; long
//! operations are background jobs that the tools wait for (`wait`, `timeout_seconds`).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sonara_app::paths::{split_suffix, with_suffix};
use sonara_app::render::{
    Container, Dither, ExportSettings, Normalize, NormalizeSpec, OutputSpec, RenderRequest,
    StemOptions, Tail, range_frames,
};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::JsonMap;
use sonara_core::pyfmt::py_round;

use super::positions::Positions;
use crate::context::AgentContext;
use crate::contract::args::{
    self, AnalyzeAudio, ExportAudio, ExportStems, FormatSpec, GetAudioAnalysis, Position,
    RenderPreview,
};
use crate::permissions::Access;
use crate::registry::{ToolError, ToolResult};

/// The suffix of a preview's analysis sidecar.
const SIDECAR_SUFFIX: &str = ".analysis.json";

/// The range part of a render tool's arguments (the prototype's `RenderRange`).
struct Range<'a> {
    start: &'a Position,
    end: Option<&'a Position>,
    tail: args::Tail,
    track_id: Option<&'a str>,
    preroll_seconds: f64,
}

/// `_render_request`: the positions in beats, and the target track checked against the open
/// project.
///
/// # Errors
///
/// The errors of converting the positions, [`ErrorCode::NoProject`], and `NOT_FOUND` for an
/// unknown track.
fn render_request(ctx: &AgentContext, range: &Range<'_>) -> Result<RenderRequest, DomainError> {
    let positions = Positions::new(ctx);
    let start_beat = positions.beats(range.start)?;
    let end_beat = range.end.map(|end| positions.beats(end)).transpose()?;
    if let Some(track_id) = range.track_id {
        positions.project()?.track(track_id)?;
    }
    let tail = match range.tail {
        args::Tail::Mode(args::TailMode::Auto) => Tail::Auto,
        args::Tail::Mode(args::TailMode::Off) => Tail::None,
        args::Tail::Seconds(seconds) => Tail::Seconds(seconds),
    };
    Ok(RenderRequest {
        start_beat,
        end_beat,
        tail,
        target: range.track_id.map(str::to_owned),
        preroll_seconds: range.preroll_seconds,
        ..RenderRequest::default()
    })
}

/// `render_preview`: the project (or a range, or one track) rendered to a WAV file and
/// analyzed, as a background operation.
pub(super) fn render_preview(ctx: &AgentContext, args: RenderPreview) -> ToolResult {
    let RenderPreview {
        start,
        end,
        tail,
        track_id,
        preroll_seconds,
        path,
        per_track_levels,
        wait,
        timeout_seconds,
    } = args;
    let request = render_request(
        ctx,
        &Range {
            start: &start,
            end: end.as_ref(),
            tail,
            track_id: track_id.as_deref(),
            preroll_seconds,
        },
    )?;
    let path = match path.as_deref().filter(|path| !path.is_empty()) {
        Some(path) => Some(ctx.path(path, Access::Write)?),
        None => None,
    };
    let job = ctx
        .app()
        .render()
        .preview_job(request, path, per_track_levels)?;
    Ok(ctx.wait_job(&job, wait, timeout_seconds)?)
}

/// The export container of a format.
fn container(format: args::AudioFormat) -> Container {
    match format {
        args::AudioFormat::Wav => Container::Wav,
        args::AudioFormat::Bwf => Container::Bwf,
        args::AudioFormat::Rf64 => Container::Rf64,
        args::AudioFormat::W64 => Container::W64,
        args::AudioFormat::Aiff => Container::Aiff,
        args::AudioFormat::Caf => Container::Caf,
        args::AudioFormat::Flac => Container::Flac,
        args::AudioFormat::Mp3 => Container::Mp3,
        args::AudioFormat::Ogg => Container::Ogg,
        args::AudioFormat::Opus => Container::Opus,
    }
}

/// `OutputSpec(path, Container(f.format), …)` of a format.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for a sample rate outside `0..2^32`. Deviation (TYP-06): the prototype
/// passed such a rate on and failed in the resampler with an internal error.
fn output_spec(path: PathBuf, format: &FormatSpec) -> Result<OutputSpec, DomainError> {
    let sample_rate = format
        .sample_rate
        .map(|rate| {
            u32::try_from(rate).map_err(|_| {
                DomainError::invalid(format!("sample rate {rate} Hz is out of range"))
                    .with_detail("sample_rate", rate)
            })
        })
        .transpose()?;
    Ok(OutputSpec {
        path,
        container: container(format.format),
        bit_depth: u32::from(format.bit_depth),
        floating_point: format.floating_point,
        sample_rate,
        channels: Some(u32::from(format.channels)),
        bitrate_kbps: format.bitrate_kbps.map(u32::from),
        variable_bitrate: format.variable_bitrate,
        dither: match format.dither {
            args::Dither::Tpdf => Dither::Tpdf,
            args::Dither::Off => Dither::None,
        },
    })
}

/// `_outputs`: one output per format, at the format's own path or at the main path with the
/// format's extension; a path taken twice gets `-<format>` before its suffix.
fn outputs(
    ctx: &AgentContext,
    main: &str,
    formats: &[FormatSpec],
) -> Result<Vec<OutputSpec>, DomainError> {
    let main = ctx.path(main, Access::Write)?;
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut out = Vec::with_capacity(formats.len());
    for format in formats {
        let container = container(format.format);
        let mut path = match format.path.as_deref().filter(|path| !path.is_empty()) {
            Some(path) => ctx.path(path, Access::Write)?,
            None => with_suffix(&main, container.extension()),
        };
        if seen.contains(&path) {
            let name = file_name(&path);
            let (stem, suffix) = split_suffix(&name);
            path = path.with_file_name(format!("{stem}-{}{suffix}", container.value()));
        }
        seen.insert(path.clone());
        out.push(output_spec(path, format)?);
    }
    Ok(out)
}

/// A path's last component as text (`Path.name`).
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a render's frame count stays far below 2^52"
)]
fn frames_to_f64(frames: i64) -> f64 {
    frames as f64
}

/// `export_audio`: the mix (or a range, or one track) rendered and exported to one or more
/// formats, as a background operation. Replacing existing files needs `overwrite` and
/// confirmation.
pub(super) fn export_audio(ctx: &AgentContext, args: ExportAudio) -> ToolResult {
    let outputs = outputs(ctx, &args.path, &args.formats)?;
    let existing: Vec<String> = outputs
        .iter()
        .filter(|output| output.path.exists())
        .map(|output| output.path.display().to_string())
        .collect();
    if !existing.is_empty() && !args.overwrite {
        return Err(DomainError::new(
            ErrorCode::Conflict,
            "output files exist; set overwrite=true to replace them",
        )
        .with_detail("existing", existing)
        .into());
    }
    let needs = ctx.confirm(
        "export_audio",
        &args,
        !existing.is_empty(),
        &format!("{} existing file(s) will be overwritten", existing.len()),
    )?;
    let request = render_request(
        ctx,
        &Range {
            start: &args.start,
            end: args.end.as_ref(),
            tail: args.tail,
            track_id: args.track_id.as_deref(),
            preroll_seconds: args.preroll_seconds,
        },
    )?;
    if args.dry_run {
        let project = ctx.project()?;
        let (start, end) = range_frames(&project, &request)?;
        let seconds = frames_to_f64(end - start) / f64::from(project.settings.sample_rate);
        let paths: Vec<String> = outputs
            .iter()
            .map(|output| output.path.display().to_string())
            .collect();
        return Ok(json!({
            "dry_run": true,
            "committed": false,
            "outputs": paths,
            "seconds": py_round(seconds, 3),
            "requires_confirmation": needs,
        }));
    }
    let settings = ExportSettings {
        normalize: NormalizeSpec {
            mode: match args.normalize {
                args::Normalize::Off => Normalize::None,
                args::Normalize::Peak => Normalize::Peak,
                args::Normalize::TruePeak => Normalize::TruePeak,
                args::Normalize::Loudness => Normalize::Loudness,
            },
            target_db: args.target_db,
            true_peak_ceiling_db: Some(args.true_peak_ceiling_db),
        },
        metadata: args.metadata.into_iter().collect(),
        overwrite: args.overwrite,
    };
    let job = ctx.app().render().export_job(outputs, request, settings)?;
    Ok(ctx.wait_job(&job, args.wait, args.timeout_seconds)?)
}

/// `export_stems`: each track or bus exported as its own file, rendered alone through its
/// output path (master processing optional), as a background operation.
pub(super) fn export_stems(ctx: &AgentContext, args: ExportStems) -> ToolResult {
    let directory = ctx.path(&args.directory, Access::Write)?;
    let template = output_spec(directory.join("stem"), &args.format)?;
    // `directory.is_dir() and any(directory.iterdir())`.
    let occupied = directory.is_dir()
        && fs::read_dir(&directory).is_ok_and(|mut entries| entries.next().is_some());
    let needs = ctx.confirm(
        "export_stems",
        &args,
        args.overwrite && occupied,
        &format!(
            "existing files in {} may be overwritten",
            directory.display()
        ),
    )?;
    let request = render_request(
        ctx,
        &Range {
            start: &args.start,
            end: args.end.as_ref(),
            tail: args.tail,
            track_id: args.track_id.as_deref(),
            preroll_seconds: args.preroll_seconds,
        },
    )?;
    if args.dry_run {
        return Ok(json!({
            "dry_run": true,
            "committed": false,
            "directory": directory.display().to_string(),
            "requires_confirmation": needs,
        }));
    }
    let job = ctx.app().render().export_stems_job(
        directory,
        args.track_ids,
        template,
        request,
        StemOptions {
            through_master: args.through_master,
            overwrite: args.overwrite,
        },
    )?;
    Ok(ctx.wait_job(&job, args.wait, args.timeout_seconds)?)
}

/// `analyze_audio`: the analysis of an audio file, of a media asset of the project, or of a
/// fresh render of the project (exactly one of them).
pub(super) fn analyze_audio(ctx: &AgentContext, args: AnalyzeAudio) -> ToolResult {
    let AnalyzeAudio {
        path,
        asset_id,
        render: range,
        per_track_levels,
    } = args;
    let chosen = [path.is_some(), asset_id.is_some(), range.is_some()]
        .into_iter()
        .filter(|&given| given)
        .count();
    if chosen != 1 {
        return Err(DomainError::invalid("give exactly one of path, asset_id or render").into());
    }
    let render = ctx.app().render();
    if let Some(path) = path.as_deref().filter(|path| !path.is_empty()) {
        let path = ctx.path(path, Access::Read)?;
        return Ok(Value::Object(render.analyze_file(&path)?));
    }
    if let Some(asset_id) = asset_id.as_deref().filter(|id| !id.is_empty()) {
        return Ok(Value::Object(render.analyze_asset(asset_id)?));
    }
    let Some(range) = range else {
        // The prototype's `assert a.render is not None` (an empty `path` or `asset_id`).
        return Err(ToolError::Internal(
            "AssertionError: give a non-empty path or asset_id".to_owned(),
        ));
    };
    let request = render_request(
        ctx,
        &Range {
            start: &range.start,
            end: range.end.as_ref(),
            tail: range.tail,
            track_id: range.track_id.as_deref(),
            preroll_seconds: range.preroll_seconds,
        },
    )?;
    let preview = render.preview(&request, None, per_track_levels, None)?;
    Ok(Value::Object(preview.to_json()))
}

/// A stored sidecar as JSON.
fn read_json(path: &Path) -> Result<Value, ToolError> {
    let bytes = fs::read(path)
        .map_err(|err| ToolError::Internal(format!("cannot read {}: {err}", path.display())))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| ToolError::Internal(format!("cannot parse {}: {err}", path.display())))
}

/// Python's truth value of a JSON value.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

/// `get_audio_analysis`: the analysis of a finished preview or export operation, or of a
/// preview file rendered earlier (from its sidecar, else analyzed now).
pub(super) fn get_audio_analysis(ctx: &AgentContext, args: GetAudioAnalysis) -> ToolResult {
    let GetAudioAnalysis {
        operation_id,
        path,
        include_series,
    } = args;
    if let Some(operation_id) = operation_id.as_deref().filter(|id| !id.is_empty()) {
        return operation_analysis(ctx, operation_id, include_series);
    }
    let Some(path) = path.as_deref().filter(|path| !path.is_empty()) else {
        return Err(DomainError::invalid("give operation_id or path").into());
    };
    let path = ctx.path(path, Access::Read)?;
    let sidecar = with_suffix(&path, SIDECAR_SUFFIX);
    if sidecar.exists() {
        let Value::Object(mut data) = read_json(&sidecar)? else {
            return Err(ToolError::Internal(format!(
                "{} does not hold an object",
                sidecar.display()
            )));
        };
        if !include_series && let Some(Value::Object(analysis)) = data.get_mut("analysis") {
            analysis.shift_remove("short_term_lufs");
        }
        // `{"ready": True, **data}`: a stored `ready` replaces the value in place.
        let mut out = JsonMap::new();
        out.insert("ready".to_owned(), Value::Bool(true));
        out.extend(data);
        return Ok(Value::Object(out));
    }
    let mut out = ctx.app().render().analyze_file(&path)?;
    out.insert("ready".to_owned(), Value::Bool(true));
    Ok(Value::Object(out))
}

/// The analysis part of an operation's state for `get_audio_analysis`.
fn operation_analysis(ctx: &AgentContext, operation_id: &str, include_series: bool) -> ToolResult {
    let snapshot = ctx.app().jobs().get(operation_id)?.snapshot();
    if !snapshot.status.is_finished() {
        return Ok(json!({
            "status": snapshot.status.as_str(),
            "progress": snapshot.progress,
            "ready": false,
        }));
    }
    let Some(result) = snapshot.result else {
        return Err(
            DomainError::new(ErrorCode::NotFound, "operation has no analysis")
                .with_detail("status", snapshot.status.as_str())
                .with_detail(
                    "error",
                    snapshot.error.map_or(Value::Null, |error| error.to_json()),
                )
                .into(),
        );
    };
    let field = |key: &str| result.get(key).cloned().unwrap_or(Value::Null);
    if let Some(analysis) = result.get("analysis") {
        let mut analysis = analysis.clone();
        let path = result
            .get("path")
            .filter(|path| truthy(path))
            .and_then(Value::as_str);
        if include_series && let Some(path) = path {
            let sidecar = with_suffix(Path::new(path), SIDECAR_SUFFIX);
            if sidecar.exists() {
                let stored = read_json(&sidecar)?;
                if let Some(series) = stored.get("analysis") {
                    analysis = series.clone();
                }
            }
        }
        return Ok(json!({
            "ready": true,
            "analysis": analysis,
            "issues": result.get("issues").cloned().unwrap_or_else(|| json!([])),
            "path": field("path"),
        }));
    }
    if let Some(before) = result.get("analysis_before_gain") {
        return Ok(json!({
            "ready": true,
            "analysis_before_gain": before,
            "gain_db": field("gain_db"),
            "final_integrated_lufs": field("final_integrated_lufs"),
            "final_true_peak_dbtp": field("final_true_peak_dbtp"),
        }));
    }
    Ok(json!({"ready": true, "result": result}))
}
