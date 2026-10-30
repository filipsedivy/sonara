//! Tracks, buses and the master channel (`sonara.domain.track`).

use std::fmt;

use super::automation::AutomationLane;
use super::clip::{AudioClip, Clip, MidiClip};
use super::device::Device;
use super::part::Part;
use super::{JsonMap, Valid, Validate, cmp_f64, has_text, validate_color};
use crate::error::{DomainError, Result, require, require_finite};
use crate::ids::{self, IdGenerator};

/// Lowest fader level in dB; treated as silence.
pub const MIN_VOLUME_DB: f64 = -144.0;
/// Highest fader level in dB.
pub const MAX_VOLUME_DB: f64 = 12.0;
/// Lowest send level in dB.
pub const MIN_SEND_DB: f64 = -144.0;
/// Highest send level in dB.
pub const MAX_SEND_DB: f64 = 12.0;
/// Longest track name, in characters.
pub const MAX_NAME_CHARS: usize = 200;

/// The kind of a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackKind {
    /// Plays audio clips.
    Audio,
    /// Plays MIDI clips through an instrument.
    Instrument,
    /// A mix bus fed by outputs and sends.
    Bus,
    /// Groups tracks; produces no audio.
    Folder,
    /// The master channel.
    Master,
}

impl TrackKind {
    /// The kind as stored in project files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Instrument => "instrument",
            Self::Bus => "bus",
            Self::Folder => "folder",
            Self::Master => "master",
        }
    }

    /// Parses the stored form.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "audio" => Some(Self::Audio),
            "instrument" => Some(Self::Instrument),
            "bus" => Some(Self::Bus),
            "folder" => Some(Self::Folder),
            "master" => Some(Self::Master),
            _ => None,
        }
    }

    /// Whether tracks of this kind hold audio clips.
    #[must_use]
    pub const fn holds_audio_clips(self) -> bool {
        matches!(self, Self::Audio)
    }

    /// Whether tracks of this kind hold MIDI clips.
    #[must_use]
    pub const fn holds_midi_clips(self) -> bool {
        matches!(self, Self::Instrument)
    }

    /// Whether outputs and sends may target tracks of this kind.
    #[must_use]
    pub const fn is_routing_target(self) -> bool {
        matches!(self, Self::Bus | Self::Master)
    }

    /// Whether tracks of this kind produce audio.
    #[must_use]
    pub const fn produces_audio(self) -> bool {
        !matches!(self, Self::Folder)
    }
}

impl fmt::Display for TrackKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Input monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MonitorMode {
    /// Never monitor the input.
    Off,
    /// Monitor while armed.
    #[default]
    Auto,
    /// Always monitor the input.
    On,
}

impl MonitorMode {
    /// The mode as stored in project files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Auto => "auto",
            Self::On => "on",
        }
    }

    /// Parses the stored form.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "off" => Some(Self::Off),
            "auto" => Some(Self::Auto),
            "on" => Some(Self::On),
            _ => None,
        }
    }
}

/// Fader, pan and switches of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MixerSettings {
    /// Fader level in dB, −144..+12.
    pub volume_db: f64,
    /// Pan, −1 (left) to +1 (right).
    pub pan: f64,
    /// Muted.
    pub mute: bool,
    /// Soloed.
    pub solo: bool,
    /// Not muted by other tracks' solo.
    pub solo_safe: bool,
}

impl MixerSettings {
    /// Validated settings.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] for a level or pan out of range.
    pub fn new(volume_db: f64, pan: f64, mute: bool, solo: bool, solo_safe: bool) -> Result<Self> {
        let settings = Self {
            volume_db,
            pan,
            mute,
            solo,
            solo_safe,
        };
        settings.validate()?;
        Ok(settings)
    }

    /// Checks the level and pan ranges.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] with the prototype's messages.
    pub fn validate(&self) -> Result<()> {
        require_finite(self.volume_db, "volume")?;
        require_finite(self.pan, "pan")?;
        require(
            (MIN_VOLUME_DB..=MAX_VOLUME_DB).contains(&self.volume_db),
            || "volume must be within -144..+12 dB".to_owned(),
        )?;
        require((-1.0..=1.0).contains(&self.pan), || {
            "pan must be within -1 (left) .. +1 (right)".to_owned()
        })
    }

    /// The fader gain as a linear factor.
    #[must_use]
    pub fn gain(&self) -> f64 {
        db_to_gain(self.volume_db)
    }
}

/// A send from a track to a bus.
#[derive(Debug, Clone, PartialEq)]
pub struct Send {
    /// Stable ID (`snd_…`).
    pub id: String,
    /// The receiving bus.
    pub target_id: String,
    /// Level in dB, −144..+12.
    pub level_db: f64,
    /// Taps the signal before the fader.
    pub pre_fader: bool,
    /// Whether the send is active.
    pub enabled: bool,
}

impl Send {
    /// A validated send.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] for a level out of range.
    pub fn new(
        id: impl Into<String>,
        target_id: impl Into<String>,
        level_db: f64,
        pre_fader: bool,
        enabled: bool,
    ) -> Result<Self> {
        require_finite(level_db, "send level")?;
        require((MIN_SEND_DB..=MAX_SEND_DB).contains(&level_db), || {
            "send level must be within -144..+12 dB".to_owned()
        })?;
        Ok(Self {
            id: id.into(),
            target_id: target_id.into(),
            level_db,
            pre_fader,
            enabled,
        })
    }

    /// A new enabled send with a fresh ID.
    ///
    /// # Errors
    ///
    /// The validation errors of [`Send::new`].
    pub fn create(
        ids: &mut dyn IdGenerator,
        target_id: impl Into<String>,
        level_db: f64,
        pre_fader: bool,
    ) -> Result<Self> {
        Self::new(ids.new_id(ids::SEND), target_id, level_db, pre_fader, true)
    }
}

/// Recording input: device input channels (0-based), arm and monitoring.
#[derive(Debug, Clone, PartialEq)]
pub struct InputSettings {
    /// One or two device input channels.
    pub channels: Vec<u32>,
    /// Armed for recording.
    pub armed: bool,
    /// Input monitoring.
    pub monitor: MonitorMode,
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            channels: vec![0],
            armed: false,
            monitor: MonitorMode::Auto,
        }
    }
}

impl InputSettings {
    /// Validated input settings.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] for no or more than two channels, or a
    /// negative channel.
    pub fn new(channels: &[i64], armed: bool, monitor: MonitorMode) -> Result<Self> {
        require((1..=2).contains(&channels.len()), || {
            "track input uses 1 or 2 device channels".to_owned()
        })?;
        let channels = channels
            .iter()
            .map(|c| u32::try_from(*c).ok())
            .collect::<Option<Vec<u32>>>()
            .ok_or_else(|| DomainError::invalid("input channels must be >= 0"))?;
        Ok(Self {
            channels,
            armed,
            monitor,
        })
    }
}

/// The data of a [`Track`].
#[derive(Debug, Clone, PartialEq)]
pub struct TrackData {
    /// Stable ID (`trk_…`, or `master`).
    pub id: String,
    /// Display name (1–200 characters, not only whitespace).
    pub name: String,
    /// Kind.
    pub kind: TrackKind,
    /// Clips, sorted by start and ID on validation.
    pub clips: Vec<Clip>,
    /// The instrument of an instrument track.
    pub instrument: Option<Device>,
    /// Insert effects in signal order.
    pub inserts: Vec<Device>,
    /// Fader, pan and switches.
    pub mixer: MixerSettings,
    /// The bus or master the track feeds; `None` for the master.
    pub output_id: Option<String>,
    /// Sends to buses.
    pub sends: Vec<Send>,
    /// Automation lanes.
    pub automation: Vec<AutomationLane>,
    /// Recording input.
    pub input: InputSettings,
    /// The folder track this track belongs to.
    pub parent_id: Option<String>,
    /// `#rrggbb`, or `None` for automatic.
    pub color: Option<String>,
    /// The part an instrument track plays (project schema version 2).
    pub part: Option<Part>,
    /// Unknown keys preserved from newer project files.
    pub extra: JsonMap,
}

impl TrackData {
    /// A track's data with the prototype's defaults: no content, unity mixer, output to master.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: TrackKind) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            clips: Vec::new(),
            instrument: None,
            inserts: Vec::new(),
            mixer: MixerSettings::default(),
            output_id: Some(ids::MASTER_ID.to_owned()),
            sends: Vec::new(),
            automation: Vec::new(),
            input: InputSettings::default(),
            parent_id: None,
            color: None,
            part: None,
            extra: JsonMap::new(),
        }
    }
}

impl Validate for TrackData {
    fn validate(&mut self) -> Result<()> {
        // The prototype checks the mixer when it is built, before the track.
        self.mixer.validate()?;
        require(has_text(&self.name), || {
            "track name must not be empty".to_owned()
        })?;
        require(self.name.chars().count() <= MAX_NAME_CHARS, || {
            "track name is too long".to_owned()
        })?;
        validate_color(self.color.as_deref())?;
        for clip in &self.clips {
            match clip {
                Clip::Audio(_) => require(self.kind.holds_audio_clips(), || {
                    format!("{} track cannot hold audio clips", self.kind)
                })?,
                Clip::Midi(_) => require(self.kind.holds_midi_clips(), || {
                    format!("{} track cannot hold MIDI clips", self.kind)
                })?,
            }
        }
        if self.instrument.is_some() {
            require(self.kind == TrackKind::Instrument, || {
                "only instrument tracks have an instrument".to_owned()
            })?;
        }
        if let Some(part) = &self.part {
            require(self.kind == TrackKind::Instrument, || {
                "only instrument tracks have a part".to_owned()
            })?;
            part.validate()?;
        }
        if self.kind == TrackKind::Master {
            require(self.output_id.is_none(), || {
                "the master track has no output target".to_owned()
            })?;
            require(self.sends.is_empty(), || {
                "the master track cannot have sends".to_owned()
            })?;
        }
        if self.kind == TrackKind::Folder {
            require(self.inserts.is_empty() && self.sends.is_empty(), || {
                "folder tracks do not process audio".to_owned()
            })?;
        }
        self.clips
            .sort_by(|a, b| cmp_f64(a.start(), b.start()).then_with(|| a.id().cmp(b.id())));
        Ok(())
    }
}

/// A track, bus, folder or the master channel.
pub type Track = Valid<TrackData>;

impl Track {
    /// A new track with a fresh ID and the defaults of [`TrackData::new`].
    ///
    /// # Errors
    ///
    /// The validation errors of [`TrackData`].
    pub fn create(
        ids: &mut dyn IdGenerator,
        name: impl Into<String>,
        kind: TrackKind,
        change: impl FnOnce(&mut TrackData),
    ) -> Result<Self> {
        let mut data = TrackData::new(ids.new_id(ids::TRACK), name, kind);
        change(&mut data);
        Self::new(data)
    }

    /// The master channel of a new project.
    ///
    /// # Panics
    ///
    /// Never: the default master track is valid.
    #[must_use]
    pub fn master() -> Self {
        let mut data = TrackData::new(ids::MASTER_ID, "Master", TrackKind::Master);
        data.output_id = None;
        Self::new(data).expect("the default master track is valid")
    }

    // -- lookups --------------------------------------------------------------------------

    /// The clip with `clip_id`.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::NotFound`]: `clip '…' not found`.
    pub fn clip(&self, clip_id: &str) -> Result<&Clip> {
        self.find_clip(clip_id)
            .ok_or_else(|| DomainError::not_found("clip", clip_id))
    }

    /// The clip with `clip_id`, if any.
    #[must_use]
    pub fn find_clip(&self, clip_id: &str) -> Option<&Clip> {
        self.clips.iter().find(|c| c.id() == clip_id)
    }

    /// The instrument (if any) followed by the inserts, in signal order.
    pub fn devices(&self) -> impl Iterator<Item = &Device> {
        self.instrument.iter().chain(self.inserts.iter())
    }

    /// The device with `device_id`, if any.
    #[must_use]
    pub fn find_device(&self, device_id: &str) -> Option<&Device> {
        self.devices().find(|d| d.id == device_id)
    }

    /// The send with `send_id`.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::NotFound`]: `send '…' not found`.
    pub fn send(&self, send_id: &str) -> Result<&Send> {
        self.sends
            .iter()
            .find(|s| s.id == send_id)
            .ok_or_else(|| DomainError::not_found("send", send_id))
    }

    /// The lane automating `param` of `device_id` (`None`: the mixer).
    #[must_use]
    pub fn lane_for(&self, device_id: Option<&str>, param: &str) -> Option<&AutomationLane> {
        self.automation.iter().find(|lane| {
            lane.target.device_id.as_deref() == device_id && lane.target.param == param
        })
    }

    /// The end of the last clip in beats (0 without clips).
    #[must_use]
    pub fn content_end(&self) -> f64 {
        self.clips
            .iter()
            .map(Clip::end)
            .fold(None, |acc: Option<f64>, end| match acc {
                Some(best) if end <= best => Some(best),
                _ => Some(end),
            })
            .unwrap_or(0.0)
    }

    /// The MIDI clips.
    pub fn midi_clips(&self) -> impl Iterator<Item = &MidiClip> {
        self.clips.iter().filter_map(Clip::as_midi)
    }

    /// The audio clips.
    pub fn audio_clips(&self) -> impl Iterator<Item = &AudioClip> {
        self.clips.iter().filter_map(Clip::as_audio)
    }

    // -- copy-on-write edits --------------------------------------------------------------

    /// The track with the clip of the same ID replaced, or the clip added.
    ///
    /// # Errors
    ///
    /// The validation errors of [`TrackData`] (for example a MIDI clip on an audio track).
    pub fn with_clip(&self, clip: Clip) -> Result<Self> {
        self.edit(|d| {
            d.clips.retain(|c| c.id() != clip.id());
            d.clips.push(clip);
        })
    }

    /// The track without the clip.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::NotFound`] when the track has no such clip.
    pub fn without_clip(&self, clip_id: &str) -> Result<Self> {
        self.clip(clip_id)?;
        self.edit(|d| d.clips.retain(|c| c.id() != clip_id))
    }

    /// The track with the device of the same ID (instrument or insert) replaced.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::NotFound`] when the track has no such device.
    pub fn with_device(&self, device: Device) -> Result<Self> {
        if self.instrument.as_ref().is_some_and(|i| i.id == device.id) {
            return self.edit(|d| d.instrument = Some(device));
        }
        let Some(index) = self.inserts.iter().position(|d| d.id == device.id) else {
            return Err(DomainError::not_found("device", &device.id));
        };
        self.edit(|d| {
            if let Some(slot) = d.inserts.get_mut(index) {
                *slot = device;
            }
        })
    }

    /// The track with an insert effect added at `index` (clamped), or at the end.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::InvalidArgument`] for a folder track.
    pub fn with_insert(&self, device: Device, index: Option<i64>) -> Result<Self> {
        if self.kind == TrackKind::Folder {
            return Err(DomainError::invalid("folder tracks cannot hold effects"));
        }
        self.edit(|d| {
            let at = clamp_index(index, d.inserts.len());
            d.inserts.insert(at, device);
        })
    }

    /// The track without the device and the lanes automating it.
    ///
    /// # Errors
    ///
    /// [`crate::error::ErrorCode::NotFound`] when the track has no such device.
    pub fn without_device(&self, device_id: &str) -> Result<Self> {
        let is_instrument = self.instrument.as_ref().is_some_and(|i| i.id == device_id);
        if !is_instrument && self.inserts.iter().all(|d| d.id != device_id) {
            return Err(DomainError::not_found("device", device_id));
        }
        self.edit(|d| {
            if is_instrument {
                d.instrument = None;
            } else {
                d.inserts.retain(|i| i.id != device_id);
            }
            d.automation
                .retain(|lane| lane.target.device_id.as_deref() != Some(device_id));
        })
    }

    /// The track with the lane of the same ID replaced (moved to the end), or the lane added.
    ///
    /// # Errors
    ///
    /// The validation errors of [`TrackData`].
    pub fn with_lane(&self, lane: AutomationLane) -> Result<Self> {
        self.edit(|d| {
            d.automation.retain(|a| a.id != lane.id);
            d.automation.push(lane);
        })
    }
}

/// Python's `list.insert` position: `None` appends, other values are clamped to `0..=len`.
#[must_use]
pub fn clamp_index(index: Option<i64>, len: usize) -> usize {
    index.map_or(len, |i| {
        usize::try_from(i.max(0)).map_or(len, |i| i.min(len))
    })
}

/// A dB level as a linear gain; levels at or below [`MIN_VOLUME_DB`] are silence.
#[must_use]
pub fn db_to_gain(db: f64) -> f64 {
    if db <= MIN_VOLUME_DB {
        0.0
    } else {
        10.0_f64.powf(db / 20.0)
    }
}

/// A linear gain in dB; zero or negative gain is −∞.
#[must_use]
pub fn gain_to_db(gain: f64) -> f64 {
    if gain <= 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * gain.log10()
    }
}

#[cfg(test)]
mod tests {
    use super::{InputSettings, MonitorMode, Track, TrackKind, clamp_index};
    use crate::error::ErrorCode;
    use crate::ids::SequentialIds;
    use crate::model::clip::{AudioClip, Clip, MidiClip};
    use crate::model::part::Part;

    #[test]
    fn only_instrument_tracks_play_a_valid_part() {
        let mut ids = SequentialIds::new(0);
        let violins = Part::new("violin").expect("valid");
        let track = Track::create(&mut ids, "Violins I", TrackKind::Instrument, |t| {
            t.part = Some(violins.clone());
        })
        .expect("valid");
        assert_eq!(track.part.as_ref(), Some(&violins));
        // Edits keep the part.
        let renamed = track.edit(|t| t.name = "Vln. I".into()).expect("valid");
        assert_eq!(renamed.part, track.part);
        for kind in [
            TrackKind::Audio,
            TrackKind::Bus,
            TrackKind::Folder,
            TrackKind::Master,
        ] {
            let err = Track::create(&mut ids, "T", kind, |t| {
                t.output_id = None;
                t.part = Some(violins.clone());
            })
            .expect_err("no part");
            assert_eq!(err.code, ErrorCode::InvalidArgument, "{kind}");
            assert_eq!(err.message, "only instrument tracks have a part", "{kind}");
        }
        let err = track
            .edit(|t| {
                if let Some(part) = &mut t.part {
                    part.players = Some(0);
                }
            })
            .expect_err("invalid part");
        assert_eq!(err.message, "part players must be 1-128, got 0");
    }

    #[test]
    fn tracks_hold_only_their_clip_kind() {
        let mut ids = SequentialIds::new(0);
        let track = Track::create(&mut ids, "Keys", TrackKind::Audio, |_| {}).expect("valid");
        let midi = MidiClip::create(&mut ids, "M", 0.0, 4.0, Vec::new()).expect("valid");
        let err = track.with_clip(Clip::Midi(midi)).expect_err("wrong kind");
        assert_eq!(err.message, "audio track cannot hold MIDI clips");
    }

    #[test]
    fn clips_are_sorted_by_start_then_id() {
        let mut ids = SequentialIds::new(0);
        let late = AudioClip::create(&mut ids, "B", "med_1", 8.0, 4.0, 0).expect("valid");
        let early = AudioClip::create(&mut ids, "A", "med_1", 0.0, 4.0, 0).expect("valid");
        let track = Track::create(&mut ids, "Audio", TrackKind::Audio, |d| {
            d.clips = vec![late.into(), early.into()];
        })
        .expect("valid");
        let starts: Vec<f64> = track.clips.iter().map(Clip::start).collect();
        assert_eq!(starts, [0.0, 8.0]);
        assert!((track.content_end() - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn master_and_input_rules_follow_the_prototype() {
        let master = Track::master();
        assert_eq!(master.id, "master");
        let err = master
            .edit(|d| d.output_id = Some("trk_1".into()))
            .expect_err("master has no output");
        assert_eq!(err.message, "the master track has no output target");
        let err = InputSettings::new(&[0, 1, 2], false, MonitorMode::Auto).expect_err("3 inputs");
        assert_eq!(err.message, "track input uses 1 or 2 device channels");
        assert_eq!(clamp_index(Some(-3), 2), 0);
        assert_eq!(clamp_index(Some(9), 2), 2);
        assert_eq!(clamp_index(None, 2), 2);
    }
}
