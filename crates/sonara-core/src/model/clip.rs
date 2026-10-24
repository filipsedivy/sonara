//! Clips: timeline regions that reference audio media or contain MIDI (`sonara.domain.clip`).
//!
//! `start` and `length` are timeline beats. Audio source offsets are integer frames at the
//! source's own sample rate. Fades are in seconds (ADR 0003).

use std::sync::Arc;

use super::{JsonMap, Valid, Validate, cmp_f64, py_mod, validate_color};
use crate::EPSILON;
use crate::error::{Result, require, require_finite};
use crate::ids::{self, IdGenerator};
use crate::midi::{MidiControlEvent, MidiNote, sort_notes};

/// Lowest clip gain in dB.
pub const MIN_GAIN_DB: f64 = -96.0;
/// Highest clip gain in dB.
pub const MAX_GAIN_DB: f64 = 24.0;
/// Upper bound on loop iterations in [`MidiClipData::playback_notes`].
const MAX_LOOP_ITERATIONS: usize = 100_000;

/// The shape of a fade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FadeCurve {
    /// Linear gain.
    #[default]
    Linear,
    /// Constant power.
    EqualPower,
    /// Exponential.
    Exponential,
    /// S-shaped.
    SCurve,
}

impl FadeCurve {
    /// The curve as stored in project files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::EqualPower => "equal_power",
            Self::Exponential => "exponential",
            Self::SCurve => "s_curve",
        }
    }

    /// Parses the stored form.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "linear" => Some(Self::Linear),
            "equal_power" => Some(Self::EqualPower),
            "exponential" => Some(Self::Exponential),
            "s_curve" => Some(Self::SCurve),
            _ => None,
        }
    }
}

/// A fade in or out.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Fade {
    /// Length in seconds (≥ 0).
    pub seconds: f64,
    /// Shape.
    pub curve: FadeCurve,
}

impl Fade {
    /// No fade.
    pub const NONE: Self = Self {
        seconds: 0.0,
        curve: FadeCurve::Linear,
    };

    /// A validated fade.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] for a non-finite or negative length.
    pub fn new(seconds: f64, curve: FadeCurve) -> Result<Self> {
        require_finite(seconds, "fade length")?;
        require(seconds >= 0.0, || "fade length must be >= 0".to_owned())?;
        Ok(Self { seconds, curve })
    }
}

fn validate_span(start: f64, length: f64) -> Result<()> {
    require(start.is_finite() && length.is_finite(), || {
        "clip start/length must be finite".to_owned()
    })?;
    require(start >= -EPSILON, || "clip start must be >= 0".to_owned())?;
    require(length > EPSILON, || "clip length must be > 0".to_owned())
}

/// The data of an [`AudioClip`].
#[derive(Debug, Clone, PartialEq)]
pub struct AudioClipData {
    /// Stable ID (`clp_…`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// The media asset played.
    pub asset_id: String,
    /// Timeline start in beats.
    pub start: f64,
    /// Length in beats.
    pub length: f64,
    /// First source frame played (≥ 0).
    pub source_offset: i64,
    /// Clip gain in dB, −96..+24.
    pub gain_db: f64,
    /// Fade at the start.
    pub fade_in: Fade,
    /// Fade at the end.
    pub fade_out: Fade,
    /// Whether the clip is silent.
    pub muted: bool,
    /// `#rrggbb`, or `None` for automatic.
    pub color: Option<String>,
    /// Unknown keys preserved from newer project files.
    pub extra: JsonMap,
}

impl Validate for AudioClipData {
    fn validate(&mut self) -> Result<()> {
        validate_span(self.start, self.length)?;
        require(self.source_offset >= 0, || {
            "source offset must be >= 0".to_owned()
        })?;
        require_finite(self.gain_db, "clip gain")?;
        require((MIN_GAIN_DB..=MAX_GAIN_DB).contains(&self.gain_db), || {
            "clip gain must be within -96..+24 dB".to_owned()
        })?;
        validate_color(self.color.as_deref())
    }
}

/// An audio region.
pub type AudioClip = Valid<AudioClipData>;

impl AudioClip {
    /// A new clip with a fresh ID, unity gain, no fades.
    ///
    /// # Errors
    ///
    /// The validation errors of [`AudioClipData`].
    pub fn create(
        ids: &mut dyn IdGenerator,
        name: impl Into<String>,
        asset_id: impl Into<String>,
        start: f64,
        length: f64,
        source_offset: i64,
    ) -> Result<Self> {
        Self::new(AudioClipData {
            id: ids.new_id(ids::CLIP),
            name: name.into(),
            asset_id: asset_id.into(),
            start,
            length,
            source_offset,
            gain_db: 0.0,
            fade_in: Fade::NONE,
            fade_out: Fade::NONE,
            muted: false,
            color: None,
            extra: JsonMap::new(),
        })
    }
}

/// The data of a [`MidiClip`].
///
/// Note and controller positions are relative to content beat 0. Timeline beat `t` inside the
/// clip plays content beat `c = content_offset + (t - start)`; with `loop_length`, `c` wraps
/// modulo `loop_length`, so the content `[0, loop_length)` repeats.
#[derive(Debug, Clone, PartialEq)]
pub struct MidiClipData {
    /// Stable ID (`clp_…`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Timeline start in beats.
    pub start: f64,
    /// Length in beats.
    pub length: f64,
    /// Notes, sorted by start, pitch and ID on validation; shared between clip versions.
    pub notes: Arc<Vec<MidiNote>>,
    /// Controller events, sorted by beat, kind and number on validation.
    pub controls: Arc<Vec<MidiControlEvent>>,
    /// Content beat played at the clip start (≥ 0).
    pub content_offset: f64,
    /// Length of the repeated content, or `None` for no looping.
    pub loop_length: Option<f64>,
    /// Whether the clip is silent.
    pub muted: bool,
    /// `#rrggbb`, or `None` for automatic.
    pub color: Option<String>,
    /// Unknown keys preserved from newer project files.
    pub extra: JsonMap,
}

impl Validate for MidiClipData {
    fn validate(&mut self) -> Result<()> {
        validate_span(self.start, self.length)?;
        require_finite(self.content_offset, "content offset")?;
        require(self.content_offset >= 0.0, || {
            "content offset must be >= 0".to_owned()
        })?;
        if let Some(loop_length) = self.loop_length {
            require_finite(loop_length, "loop length")?;
            require(loop_length > EPSILON, || {
                "loop length must be > 0".to_owned()
            })?;
        }
        validate_color(self.color.as_deref())?;
        if !self.notes.is_sorted_by(|a, b| note_order(a, b).is_le()) {
            sort_notes(Arc::make_mut(&mut self.notes).as_mut_slice());
        }
        if !self
            .controls
            .is_sorted_by(|a, b| control_order(a, b).is_le())
        {
            Arc::make_mut(&mut self.controls).sort_by(control_order);
        }
        Ok(())
    }
}

fn note_order(a: &MidiNote, b: &MidiNote) -> std::cmp::Ordering {
    cmp_f64(a.start, b.start)
        .then(a.pitch.cmp(&b.pitch))
        .then_with(|| a.id.cmp(&b.id))
}

fn control_order(a: &MidiControlEvent, b: &MidiControlEvent) -> std::cmp::Ordering {
    cmp_f64(a.beat, b.beat)
        .then(a.kind.cmp(&b.kind))
        .then(a.number.cmp(&b.number))
}

impl MidiClipData {
    /// The note with `note_id`.
    #[must_use]
    pub fn note(&self, note_id: &str) -> Option<&MidiNote> {
        self.notes.iter().find(|n| n.id == note_id)
    }

    /// The end of the last note in content beats (0 without notes).
    #[must_use]
    pub fn content_end(&self) -> f64 {
        self.notes.iter().map(MidiNote::end).fold(0.0, py_max)
    }

    /// Notes as heard on the timeline: `(timeline start beat, note)` pairs.
    ///
    /// Notes starting outside the clip span are dropped (the caller shortens a note crossing
    /// the clip end). Looping repeats the `[0, loop_length)` content.
    #[must_use]
    pub fn playback_notes(&self) -> Vec<(f64, &MidiNote)> {
        let clip_end = self.start + self.length;
        let Some(loop_length) = self.loop_length else {
            return self
                .notes
                .iter()
                .filter_map(|n| {
                    let t = self.start + (n.start - self.content_offset);
                    (n.start + EPSILON >= self.content_offset && t < clip_end - EPSILON)
                        .then_some((t, n))
                })
                .collect();
        };
        let looped: Vec<&MidiNote> = self
            .notes
            .iter()
            .filter(|n| n.start < loop_length - EPSILON)
            .collect();
        // The first iteration begins at content_offset (mod loop).
        let iteration_start = self.start - py_mod(self.content_offset, loop_length);
        let mut out = Vec::new();
        let mut k = 0_u32;
        while iteration_start + f64::from(k) * loop_length < clip_end - EPSILON {
            let base = iteration_start + f64::from(k) * loop_length;
            for n in &looped {
                let t = base + n.start;
                if t + EPSILON >= self.start && t < clip_end - EPSILON {
                    out.push((t, *n));
                }
            }
            k += 1;
            if usize::try_from(k).unwrap_or(usize::MAX) > MAX_LOOP_ITERATIONS {
                break;
            }
        }
        out
    }
}

/// Python's `max(a, b)` for the fold in `content_end`: keeps the first of equal values.
fn py_max(acc: f64, value: f64) -> f64 {
    if value > acc { value } else { acc }
}

/// A MIDI region.
pub type MidiClip = Valid<MidiClipData>;

impl MidiClip {
    /// A new clip with a fresh ID.
    ///
    /// # Errors
    ///
    /// The validation errors of [`MidiClipData`].
    pub fn create(
        ids: &mut dyn IdGenerator,
        name: impl Into<String>,
        start: f64,
        length: f64,
        notes: Vec<MidiNote>,
    ) -> Result<Self> {
        Self::new(MidiClipData {
            id: ids.new_id(ids::CLIP),
            name: name.into(),
            start,
            length,
            notes: Arc::new(notes),
            controls: Arc::new(Vec::new()),
            content_offset: 0.0,
            loop_length: None,
            muted: false,
            color: None,
            extra: JsonMap::new(),
        })
    }
}

/// A clip on a track.
#[derive(Debug, Clone, PartialEq)]
pub enum Clip {
    /// An audio region (audio tracks).
    Audio(AudioClip),
    /// A MIDI region (instrument tracks).
    Midi(MidiClip),
}

impl Clip {
    /// Stable ID.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Audio(c) => &c.id,
            Self::Midi(c) => &c.id,
        }
    }

    /// Display name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Audio(c) => &c.name,
            Self::Midi(c) => &c.name,
        }
    }

    /// Timeline start in beats.
    #[must_use]
    pub fn start(&self) -> f64 {
        match self {
            Self::Audio(c) => c.start,
            Self::Midi(c) => c.start,
        }
    }

    /// Length in beats.
    #[must_use]
    pub fn length(&self) -> f64 {
        match self {
            Self::Audio(c) => c.length,
            Self::Midi(c) => c.length,
        }
    }

    /// Timeline end in beats.
    #[must_use]
    pub fn end(&self) -> f64 {
        self.start() + self.length()
    }

    /// Whether the clip is silent.
    #[must_use]
    pub fn muted(&self) -> bool {
        match self {
            Self::Audio(c) => c.muted,
            Self::Midi(c) => c.muted,
        }
    }

    /// The clip's color.
    #[must_use]
    pub fn color(&self) -> Option<&str> {
        match self {
            Self::Audio(c) => c.color.as_deref(),
            Self::Midi(c) => c.color.as_deref(),
        }
    }

    /// The clip moved to `start`.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] for an invalid start.
    pub fn moved_to(&self, start: f64) -> Result<Self> {
        Ok(match self {
            Self::Audio(c) => Self::Audio(c.edit(|d| d.start = start)?),
            Self::Midi(c) => Self::Midi(c.edit(|d| d.start = start)?),
        })
    }

    /// The audio clip, if this is one.
    #[must_use]
    pub fn as_audio(&self) -> Option<&AudioClip> {
        match self {
            Self::Audio(c) => Some(c),
            Self::Midi(_) => None,
        }
    }

    /// The MIDI clip, if this is one.
    #[must_use]
    pub fn as_midi(&self) -> Option<&MidiClip> {
        match self {
            Self::Midi(c) => Some(c),
            Self::Audio(_) => None,
        }
    }
}

impl From<AudioClip> for Clip {
    fn from(clip: AudioClip) -> Self {
        Self::Audio(clip)
    }
}

impl From<MidiClip> for Clip {
    fn from(clip: MidiClip) -> Self {
        Self::Midi(clip)
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioClip, Clip, MidiClip};
    use crate::ids::SequentialIds;
    use crate::midi::MidiNote;
    use crate::model::py_mod;

    fn note(id: &str, pitch: i64, start: f64) -> MidiNote {
        MidiNote::new(id, pitch, start, 0.5, 100, 0).expect("valid note")
    }

    #[test]
    fn midi_clips_sort_notes_and_loop_their_content() {
        let mut ids = SequentialIds::new(0);
        let clip = MidiClip::create(
            &mut ids,
            "Loop",
            4.0,
            4.0,
            vec![note("n_2", 64, 1.0), note("n_1", 60, 0.0)],
        )
        .and_then(|c| c.edit(|d| d.loop_length = Some(2.0)))
        .expect("valid clip");
        assert_eq!(clip.notes.first().map(|n| n.id.as_str()), Some("n_1"));
        let heard: Vec<(f64, &str)> = clip
            .playback_notes()
            .into_iter()
            .map(|(t, n)| (t, n.id.as_str()))
            .collect();
        assert_eq!(
            heard,
            [(4.0, "n_1"), (5.0, "n_2"), (6.0, "n_1"), (7.0, "n_2")]
        );
        assert!((clip.content_end() - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn audio_clip_rules_follow_the_prototype() {
        let mut ids = SequentialIds::new(0);
        let err = AudioClip::create(&mut ids, "A", "med_1", 0.0, 0.0, 0).expect_err("empty");
        assert_eq!(err.message, "clip length must be > 0");
        let clip = AudioClip::create(&mut ids, "A", "med_1", 0.0, 4.0, 0).expect("valid");
        let err = clip
            .edit(|d| d.gain_db = 30.0)
            .expect_err("gain out of range");
        assert_eq!(err.message, "clip gain must be within -96..+24 dB");
        let moved = Clip::from(clip).moved_to(8.0).expect("valid");
        assert!((moved.end() - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn modulo_follows_python() {
        assert!((py_mod(5.0, 2.0) - 1.0).abs() < f64::EPSILON);
        assert!((py_mod(-1.0, 4.0) - 3.0).abs() < f64::EPSILON);
    }
}
