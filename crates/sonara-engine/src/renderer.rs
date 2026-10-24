//! The offline renderer (`audio/engine/renderer.py`): faster-than-real-time rendering on top of
//! the [`RenderGraph`], for previews, exports and the golden renders.
//!
//! Not real-time code: it runs on a worker thread, reads media between blocks and collects the
//! output.
//!
//! - **Range:** `[beat_to_frame(max(0, start_beat)), beat_to_frame(end_beat))`, the end
//!   defaulting to the project end.
//! - **Latency and pre-roll:** the graph is reset; the sources start `min(preroll, start)`
//!   frames early, and the first `latency + min(preroll, start)` output frames are dropped, so
//!   the output is aligned with the sources and reverbs carry in.
//! - **Tail:** [`Tail::Auto`] renders up to `int(min(30, tail_seconds + 0.1) · sr)` frames after
//!   the end and stops once 0.25 s in a row stayed at or below [`SILENCE`]; [`Tail::None`] stops
//!   at the end; [`Tail::Seconds`] renders that long.
//! - **Trim:** [`OfflineRenderer::render`] cuts an auto tail after its last sample above
//!   [`SILENCE`] plus 10 ms; the block stream ([`RenderStream`]) does not trim.

use std::num::NonZeroUsize;
use std::ops::Range;

use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::MASTER_ID;
use sonara_core::model::project::Project;
use sonara_core::plugins::PluginRegistry;
use sonara_media::MediaProvider;

use crate::EngineError;
use crate::graph::{GraphOptions, PlayableNotes, RenderGraph};

/// The default block size in frames.
pub const DEFAULT_BLOCK: NonZeroUsize = match NonZeroUsize::new(512) {
    Some(n) => n,
    None => NonZeroUsize::MIN,
};
/// The longest render a request may ask for, in seconds (three hours).
pub const MAX_RENDER_SECONDS: f64 = 3.0 * 3600.0;
/// The level at or below which the auto tail counts a sample as silent (−100 dBFS).
pub const SILENCE: f64 = 1e-5;
/// How long the auto tail must stay silent before it stops, in seconds.
pub const SILENCE_HOLD_SECONDS: f64 = 0.25;
/// The longest auto tail, in seconds.
pub const MAX_AUTO_TAIL_SECONDS: f64 = 30.0;
/// The silence an auto tail keeps after its last loud sample when trimmed, in seconds.
const TRIM_KEEP_SECONDS: f64 = 0.01;

/// How a render ends after its range.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Tail {
    /// Effect and instrument tails until they decay to silence, bounded by the graph's tail
    /// and [`MAX_AUTO_TAIL_SECONDS`].
    #[default]
    Auto,
    /// Exactly at the range end.
    None,
    /// This many seconds after the range end (negative values count as 0).
    Seconds(f64),
}

/// What to render. Times are beats.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderRequest {
    /// The first beat (negative values count as 0).
    pub start_beat: f64,
    /// The end beat; `None` for the project end.
    pub end_beat: Option<f64>,
    /// What follows the range.
    pub tail: Tail,
    /// `None` for the full mix, or a track or bus rendered alone through its output path (like
    /// soloing it); the master renders the full mix.
    pub target: Option<String>,
    /// Skips the master's inserts and fader (stems without master processing).
    pub bypass_master: bool,
    /// Audio processed before `start_beat` and discarded, so reverbs and delays carry in when a
    /// render starts in the middle of a song (negative values count as 0).
    pub preroll_seconds: f64,
    /// Frames per block.
    pub block_size: NonZeroUsize,
    /// The longest range allowed, in seconds.
    pub max_seconds: f64,
}

impl Default for RenderRequest {
    fn default() -> Self {
        Self {
            start_beat: 0.0,
            end_beat: None,
            tail: Tail::Auto,
            target: None,
            bypass_master: false,
            preroll_seconds: 0.0,
            block_size: DEFAULT_BLOCK,
            max_seconds: MAX_RENDER_SECONDS,
        }
    }
}

/// What a render produced besides its audio.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// Frames rendered: the range plus the tail.
    pub frames: usize,
    /// The graph's latency in frames (compensated in the output).
    pub latency: usize,
    /// Frames rendered after the range end.
    pub tail_frames: usize,
    /// The graph's warnings ([`RenderGraph::warnings`]).
    pub warnings: Vec<String>,
    /// Assets whose media was missing ([`RenderGraph::missing_media`]).
    pub missing_media: Vec<String>,
}

/// A finished render.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedAudio {
    /// Stereo frames.
    pub data: Vec<[f64; 2]>,
    /// The project's sample rate.
    pub sample_rate: u32,
    /// The timeline frame of the first frame.
    pub start_frame: i64,
    /// Frame counts, latency and warnings.
    pub stats: RenderStats,
}

impl RenderedAudio {
    /// The length in seconds.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        frames_to_f64(self.data.len()) / f64::from(self.sample_rate)
    }
}

/// The timeline frames `[start, end)` that `request` renders from `project`
/// (`OfflineRenderer.range_frames`).
///
/// # Errors
///
/// [`EngineError::Domain`] with `INVALID_ARGUMENT` for an empty range (details `start_beat` and
/// `end_beat`), and `LIMIT_EXCEEDED` for a range longer than `request.max_seconds`.
pub fn range_frames(project: &Project, request: &RenderRequest) -> Result<(i64, i64), EngineError> {
    let start = project.beat_to_frame(py_max_zero(request.start_beat));
    let end_beat = request.end_beat.unwrap_or_else(|| project.end_beat());
    let end = project.beat_to_frame(end_beat);
    if end <= start {
        return Err(DomainError::invalid(
            "nothing to render: the range is empty (add content or set an end position)",
        )
        .with_detail("start_beat", request.start_beat)
        .with_detail("end_beat", end_beat)
        .into());
    }
    let seconds = frames_to_f64(usize::try_from(end - start).unwrap_or(usize::MAX))
        / f64::from(project.settings.sample_rate);
    if seconds > request.max_seconds {
        return Err(DomainError::new(
            ErrorCode::LimitExceeded,
            format!(
                "render length {seconds:.0}s exceeds the limit of {:.0}s",
                request.max_seconds
            ),
        )
        .into());
    }
    Ok((start, end))
}

/// Renders projects offline with the built-in plugins and a project's media.
#[derive(Debug, Clone, Copy)]
pub struct OfflineRenderer<'a> {
    registry: &'a PluginRegistry,
    media: &'a MediaProvider,
}

impl<'a> OfflineRenderer<'a> {
    /// A renderer over `registry` and `media`.
    #[must_use]
    pub fn new(registry: &'a PluginRegistry, media: &'a MediaProvider) -> Self {
        Self { registry, media }
    }

    /// Starts rendering `request` from `project`: builds and resets the graph and returns the
    /// stream of output blocks (`render_iter`), latency-compensated but not trimmed.
    ///
    /// # Errors
    ///
    /// The errors of [`range_frames`]; [`EngineError::Domain`] with `NOT_FOUND` for an unknown
    /// target track; and the errors of [`RenderGraph::new`].
    pub fn stream(
        &self,
        project: &Project,
        request: &RenderRequest,
    ) -> Result<RenderStream, EngineError> {
        let sample_rate = project.settings.sample_rate;
        let (start, end) = range_frames(project, request)?;
        if let Some(target) = &request.target {
            project.track(target)?;
        }
        let options = GraphOptions {
            isolate: request.target.clone().filter(|target| target != MASTER_ID),
            bypass_master: request.bypass_master,
            // The schedules are the only notes of an offline render: no live input.
            playable_notes: PlayableNotes::Scheduled,
        };
        let mut graph = RenderGraph::new(
            project,
            self.registry,
            self.media,
            request.block_size,
            &options,
        )?;
        graph.reset();
        let tail_limit = match request.tail {
            Tail::None => 0,
            Tail::Auto => {
                // Python's `min(30, tail + 0.1)`.
                let tail = graph.tail_seconds() + 0.1;
                let tail = if tail < MAX_AUTO_TAIL_SECONDS {
                    tail
                } else {
                    MAX_AUTO_TAIL_SECONDS
                };
                seconds_to_frames(tail, sample_rate)
            }
            Tail::Seconds(seconds) => seconds_to_frames(py_max_zero(seconds), sample_rate),
        };
        let preroll = seconds_to_frames(py_max_zero(request.preroll_seconds), sample_rate);
        let lead = usize::try_from(start).unwrap_or(0).min(preroll);
        let stats = RenderStats {
            latency: graph.latency(),
            warnings: graph.warnings().to_vec(),
            missing_media: graph.missing_media().to_vec(),
            ..RenderStats::default()
        };
        Ok(RenderStream {
            skip: graph.latency().saturating_add(lead),
            source: start - i64::try_from(lead).unwrap_or(0),
            graph,
            auto_tail: request.tail == Tail::Auto,
            tail_limit,
            total: usize::try_from(end - start).unwrap_or(0),
            emitted: 0,
            silent_run: 0,
            hold: seconds_to_frames(SILENCE_HOLD_SECONDS, sample_rate),
            pending: None,
            finished: false,
            start_frame: start,
            stats,
        })
    }

    /// Renders `request` from `project` into memory, trimming an auto tail after its last
    /// sample above [`SILENCE`] plus 10 ms. `cancelled` is polled before every block.
    ///
    /// # Errors
    ///
    /// The errors of [`OfflineRenderer::stream`] and [`RenderStream::next_block`].
    pub fn render(
        &self,
        project: &Project,
        request: &RenderRequest,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<RenderedAudio, EngineError> {
        let mut stream = self.stream(project, request)?;
        let mut data: Vec<[f64; 2]> = Vec::new();
        while let Some(block) = stream.next_block(cancelled)? {
            data.extend_from_slice(block);
        }
        let start_frame = stream.start_frame();
        let mut stats = stream.into_stats();
        if request.tail == Tail::Auto && stats.tail_frames > 0 {
            let main = data.len().saturating_sub(stats.tail_frames);
            let last_loud = data
                .get(main..)
                .and_then(|tail| tail.iter().rposition(is_loud));
            let keep = main
                .saturating_add(last_loud.map_or(0, |k| k.saturating_add(1)))
                .saturating_add(seconds_to_frames(
                    TRIM_KEEP_SECONDS,
                    project.settings.sample_rate,
                ))
                .min(data.len());
            stats.tail_frames = keep - main;
            data.truncate(keep);
            stats.frames = data.len();
        }
        Ok(RenderedAudio {
            data,
            sample_rate: project.settings.sample_rate,
            start_frame,
            stats,
        })
    }
}

/// The output of a render, block by block (`render_iter`): latency-compensated, starting at the
/// range start, with the tail rule applied but not trimmed.
#[derive(Debug)]
pub struct RenderStream {
    graph: RenderGraph,
    auto_tail: bool,
    tail_limit: usize,
    /// Output frames still to drop (latency and pre-roll).
    skip: usize,
    /// The source frame of the next block.
    source: i64,
    /// Frames of the range.
    total: usize,
    /// Range frames emitted.
    emitted: usize,
    /// Silent frames at the end of the tail so far.
    silent_run: usize,
    hold: usize,
    /// The part of the current block after its range part: the start of the tail.
    pending: Option<Range<usize>>,
    finished: bool,
    start_frame: i64,
    stats: RenderStats,
}

impl RenderStream {
    /// The next block of output, `None` once the render is complete. Blocks are at most one
    /// graph block long; a block that crosses the range end comes as two (the range part, then
    /// the tail part). `cancelled` is polled before every graph block.
    ///
    /// # Errors
    ///
    /// [`EngineError::Domain`] with `CANCELLED` (`render cancelled`) when `cancelled` returns
    /// true, and [`EngineError::Media`] when a clip cannot be read.
    pub fn next_block(
        &mut self,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Option<&[[f64; 2]]>, EngineError> {
        let range = loop {
            if self.finished {
                return Ok(None);
            }
            if let Some(rest) = self.pending.take() {
                if let Some(range) = self.tail_part(rest) {
                    break range;
                }
                continue;
            }
            if cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(DomainError::new(ErrorCode::Cancelled, "render cancelled").into());
            }
            self.graph.read_media(self.source)?;
            let block = self.graph.process(self.source, true).len();
            self.source = self
                .source
                .saturating_add(i64::try_from(block).unwrap_or(i64::MAX));
            if self.skip >= block {
                self.skip -= block;
                continue;
            }
            let start = std::mem::take(&mut self.skip);
            let remaining = self.total - self.emitted;
            if remaining > 0 {
                let take = remaining.min(block - start);
                self.emitted += take;
                if start + take < block {
                    self.pending = Some(start + take..block);
                }
                break start..start + take;
            }
            if let Some(range) = self.tail_part(start..block) {
                break range;
            }
        };
        Ok(self.graph.output().get(range))
    }

    /// The tail section of a block (`rest`, frames after the range end): the frames the tail
    /// rule takes, or `None` when the render is complete.
    fn tail_part(&mut self, rest: Range<usize>) -> Option<Range<usize>> {
        if rest.is_empty() {
            return None;
        }
        if self.stats.tail_frames >= self.tail_limit {
            self.finish();
            return None;
        }
        let take = rest.len().min(self.tail_limit - self.stats.tail_frames);
        let range = rest.start..rest.start + take;
        let frames = self.graph.output().get(range.clone()).unwrap_or_default();
        match frames.iter().rposition(is_loud) {
            Some(last) => self.silent_run = take - 1 - last,
            None => self.silent_run += take,
        }
        self.stats.tail_frames += take;
        if self.auto_tail && self.silent_run >= self.hold {
            self.finish();
        }
        Some(range)
    }

    fn finish(&mut self) {
        self.finished = true;
        self.stats.frames = self.emitted + self.stats.tail_frames;
    }

    /// The share of the range rendered so far, from 0 to 1.
    #[must_use]
    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        (frames_to_f64(self.emitted) / frames_to_f64(self.total)).min(1.0)
    }

    /// The timeline frame of the first output frame.
    #[must_use]
    pub fn start_frame(&self) -> i64 {
        self.start_frame
    }

    /// The project's sample rate.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.graph.sample_rate()
    }

    /// The statistics so far; `frames` is set once the stream is complete.
    #[must_use]
    pub fn stats(&self) -> &RenderStats {
        &self.stats
    }

    /// The statistics, ending the stream.
    #[must_use]
    pub fn into_stats(self) -> RenderStats {
        self.stats
    }
}

/// Whether a frame is above [`SILENCE`] on either channel (`max(|x|, axis=1) > SILENCE`).
fn is_loud(frame: &[f64; 2]) -> bool {
    frame[0].abs().max(frame[1].abs()) > SILENCE
}

/// Python's `max(0.0, x)`: `x` only when it is greater.
fn py_max_zero(x: f64) -> f64 {
    if x > 0.0 { x } else { 0.0 }
}

/// Python's `int(seconds * sample_rate)` for a non-negative duration: truncation.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a non-negative duration truncated as Python's int() does"
)]
fn seconds_to_frames(seconds: f64, sample_rate: u32) -> usize {
    (seconds * f64::from(sample_rate)) as usize
}

/// A frame count as a float (exact below 2^53).
#[expect(
    clippy::cast_precision_loss,
    reason = "frame counts stay far below 2^53, where the conversion is exact"
)]
fn frames_to_f64(frames: usize) -> f64 {
    frames as f64
}

#[cfg(test)]
mod tests {
    use super::{RenderStream, py_max_zero, seconds_to_frames};
    use crate::RenderGraph;

    #[test]
    fn renders_can_run_on_worker_threads() {
        fn movable<T: Send>() {}
        movable::<RenderGraph>();
        movable::<RenderStream>();
    }

    #[test]
    fn durations_truncate_like_python_int() {
        assert_eq!(seconds_to_frames(0.25, 48_000), 12_000);
        assert_eq!(seconds_to_frames(0.01, 44_100), 441);
        assert_eq!(seconds_to_frames(2.000_009, 48_000), 96_000);
        assert_eq!(py_max_zero(-0.5), 0.0);
        assert_eq!(py_max_zero(f64::NAN), 0.0);
    }
}
