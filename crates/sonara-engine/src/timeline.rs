//! Real-time: the queries run on the audio render thread (see docs/audio-engine.md);
//! [`Timeline::new`] is setup.
//!
//! The tempo and time-signature snapshot a graph carries for the player:
//! the metronome places its clicks with it and the MIDI takes map frames to beats with it, as
//! the prototype's player does with `project.tempo_map` and `project.time_signatures`
//! (`_add_clicks`, `_midi_takes`). It is taken when the graph is built, so it changes with the
//! graph that plays, never under the render thread.
//!
//! The queries are those of [`TempoMap`] and [`TimeSignatureMap`]: binary and linear searches
//! over their points, which allocate nothing.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use sonara_core::model::project::Project;
use sonara_core::time::{TempoMap, TimeSignatureMap};

/// The tempo and time-signature maps of a project at a sample rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    tempo: TempoMap,
    signatures: TimeSignatureMap,
    sample_rate: u32,
}

impl Timeline {
    /// The maps of `project` at `sample_rate`. Setup: copies them.
    pub(crate) fn new(project: &Project, sample_rate: u32) -> Self {
        Self {
            tempo: project.tempo_map.clone(),
            signatures: project.time_signatures.clone(),
            sample_rate,
        }
    }

    /// The sample rate the conversions use.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The tempo map.
    #[must_use]
    pub fn tempo_map(&self) -> &TempoMap {
        &self.tempo
    }

    /// The time-signature map.
    #[must_use]
    pub fn time_signatures(&self) -> &TimeSignatureMap {
        &self.signatures
    }

    /// The nearest frame of `beat` (`tempo_map.beat_to_sample(beat, sr)`).
    #[must_use]
    pub fn beat_to_frame(&self, beat: f64) -> i64 {
        self.tempo.beat_to_sample(beat, self.sample_rate)
    }

    /// The beat at `frame` (`tempo_map.sample_to_beat(frame, sr)`).
    #[must_use]
    pub fn frame_to_beat(&self, frame: i64) -> f64 {
        self.tempo
            .sample_to_beat(frame_to_f64(frame), self.sample_rate)
    }
}

/// A frame as Python converts an `int` for the division by the sample rate.
#[expect(
    clippy::cast_precision_loss,
    reason = "timeline frames stay far below 2^53, where the conversion is exact"
)]
fn frame_to_f64(frame: i64) -> f64 {
    frame as f64
}

#[cfg(test)]
mod tests {
    use sonara_core::model::project::{Project, ProjectData};
    use sonara_core::time::{TempoMap, TimeSignatureMap, TimeSignaturePoint};

    use super::Timeline;

    #[test]
    fn conversions_use_the_snapshot_and_its_rate() {
        let mut data = ProjectData::new("prj_1", "Timeline");
        data.tempo_map = TempoMap::constant(90.0).unwrap();
        data.time_signatures =
            TimeSignatureMap::new([TimeSignaturePoint::new(0, 3, 4).unwrap()].to_vec()).unwrap();
        let timeline = Timeline::new(&Project::new(data).unwrap(), 48_000);
        // At 90 BPM a beat is 2/3 s, 32 000 frames.
        assert_eq!(timeline.beat_to_frame(1.0), 32_000);
        assert_eq!(timeline.frame_to_beat(48_000), 1.5);
        assert_eq!(timeline.time_signatures().at_beat(4.0).numerator, 3);
        assert_eq!(timeline.sample_rate(), 48_000);
        assert_eq!(timeline.tempo_map().bpm_at(0.0), 90.0);
    }
}
