//! The project aggregate root (`sonara.domain.project`).

use std::collections::BTreeSet;

use super::clip::Clip;
use super::device::Device;
use super::markers::{Marker, Section};
use super::media::MediaAsset;
use super::track::{Track, TrackKind, clamp_index};
use super::{JsonMap, Valid, Validate, cmp_f64, has_text};
use crate::EPSILON;
use crate::error::{DomainError, ErrorCode, Result, require, require_finite};
use crate::ids::{self, IdGenerator};
use crate::pyfmt::str_repr;
use crate::time::{TempoMap, TimeSignatureMap};

/// Sample rates a project may use.
pub const SUPPORTED_SAMPLE_RATES: [u32; 8] = [
    22_050, 32_000, 44_100, 48_000, 88_200, 96_000, 176_400, 192_000,
];
/// Most tracks a project may hold (the master not counted).
pub const MAX_TRACKS: usize = 512;
/// Default sample rate of new projects.
pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;
/// Default libsndfile subtype of new recordings.
pub const DEFAULT_RECORD_SUBTYPE: &str = "PCM_24";

/// Project-wide settings.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSettings {
    /// Sample rate, one of [`SUPPORTED_SAMPLE_RATES`].
    pub sample_rate: u32,
    /// Explicit song end in beats; `None` means the end of the last clip.
    pub length_beats: Option<f64>,
    /// libsndfile subtype for new recordings.
    pub record_subtype: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
            length_beats: None,
            record_subtype: DEFAULT_RECORD_SUBTYPE.to_owned(),
        }
    }
}

impl ProjectSettings {
    /// Validated settings.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] for an unsupported sample rate or a non-positive length.
    pub fn new(
        sample_rate: i64,
        length_beats: Option<f64>,
        record_subtype: impl Into<String>,
    ) -> Result<Self> {
        let rate = u32::try_from(sample_rate)
            .ok()
            .filter(|rate| SUPPORTED_SAMPLE_RATES.contains(rate))
            .ok_or_else(|| {
                let rates: Vec<String> =
                    SUPPORTED_SAMPLE_RATES.iter().map(u32::to_string).collect();
                DomainError::invalid(format!("sample rate must be one of ({})", rates.join(", ")))
            })?;
        if let Some(length) = length_beats {
            require_finite(length, "project length")?;
            require(length > 0.0, || "project length must be > 0".to_owned())?;
        }
        Ok(Self {
            sample_rate: rate,
            length_beats,
            record_subtype: record_subtype.into(),
        })
    }
}

/// Loop, punch and metronome settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransportSettings {
    /// Whether playback loops.
    pub loop_enabled: bool,
    /// Loop start in beats.
    pub loop_start: f64,
    /// Loop end in beats.
    pub loop_end: f64,
    /// Whether recording punches in and out.
    pub punch_enabled: bool,
    /// Punch-in in beats.
    pub punch_in: f64,
    /// Punch-out in beats.
    pub punch_out: f64,
    /// Whether the metronome clicks.
    pub metronome: bool,
    /// Count-in before recording, 0–4 bars.
    pub count_in_bars: u32,
}

impl Default for TransportSettings {
    fn default() -> Self {
        Self {
            loop_enabled: false,
            loop_start: 0.0,
            loop_end: 16.0,
            punch_enabled: false,
            punch_in: 0.0,
            punch_out: 16.0,
            metronome: false,
            count_in_bars: 0,
        }
    }
}

impl TransportSettings {
    /// Checks the ranges.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] with the prototype's messages.
    pub fn validate(&self) -> Result<()> {
        for (value, name) in [
            (self.loop_start, "loop_start"),
            (self.loop_end, "loop_end"),
            (self.punch_in, "punch_in"),
            (self.punch_out, "punch_out"),
        ] {
            require_finite(value, name)?;
        }
        require(
            0.0 <= self.loop_start && self.loop_start < self.loop_end,
            || "loop start must be >= 0 and before loop end".to_owned(),
        )?;
        require(
            0.0 <= self.punch_in && self.punch_in < self.punch_out,
            || "punch-in must be >= 0 and before punch-out".to_owned(),
        )?;
        require(self.count_in_bars <= 4, || {
            "count-in must be 0-4 bars".to_owned()
        })
    }

    /// Checks a count-in read from outside before it is stored.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`]: `count-in must be 0-4 bars`.
    pub fn count_in(bars: i64) -> Result<u32> {
        u32::try_from(bars)
            .ok()
            .filter(|bars| *bars <= 4)
            .ok_or_else(|| DomainError::invalid("count-in must be 0-4 bars"))
    }
}

/// Song metadata, written into exported files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectMetadata {
    /// Title.
    pub title: String,
    /// Artist.
    pub artist: String,
    /// Album.
    pub album: String,
    /// Genre.
    pub genre: String,
    /// Composer.
    pub composer: String,
    /// Free comment.
    pub comment: String,
    /// Creation time (ISO 8601).
    pub created: String,
    /// Last change (ISO 8601).
    pub modified: String,
}

impl ProjectMetadata {
    /// The field names, in file order.
    pub const FIELDS: [&'static str; 8] = [
        "title", "artist", "album", "genre", "composer", "comment", "created", "modified",
    ];

    /// The value of a field by name.
    #[must_use]
    pub fn get(&self, field: &str) -> Option<&str> {
        Some(match field {
            "title" => &self.title,
            "artist" => &self.artist,
            "album" => &self.album,
            "genre" => &self.genre,
            "composer" => &self.composer,
            "comment" => &self.comment,
            "created" => &self.created,
            "modified" => &self.modified,
            _ => return None,
        })
    }

    /// The field by name, for changing it.
    #[must_use]
    pub fn get_mut(&mut self, field: &str) -> Option<&mut String> {
        Some(match field {
            "title" => &mut self.title,
            "artist" => &mut self.artist,
            "album" => &mut self.album,
            "genre" => &mut self.genre,
            "composer" => &mut self.composer,
            "comment" => &mut self.comment,
            "created" => &mut self.created,
            "modified" => &mut self.modified,
            _ => return None,
        })
    }
}

/// The data of a [`Project`].
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectData {
    /// Stable ID (`prj_…`).
    pub id: String,
    /// Display name (not only whitespace).
    pub name: String,
    /// Sample rate, length and recording format.
    pub settings: ProjectSettings,
    /// Tempo changes.
    pub tempo_map: TempoMap,
    /// Time signature changes.
    pub time_signatures: TimeSignatureMap,
    /// Tracks in display order (the master excluded).
    pub tracks: Vec<Track>,
    /// The master channel.
    pub master: Track,
    /// Media used by the project.
    pub assets: Vec<MediaAsset>,
    /// Markers, sorted by beat and ID on validation.
    pub markers: Vec<Marker>,
    /// Sections, sorted by start and ID on validation.
    pub sections: Vec<Section>,
    /// Loop, punch and metronome.
    pub transport: TransportSettings,
    /// Song metadata.
    pub metadata: ProjectMetadata,
    /// Editor state (zoom, selection) that does not affect the sound.
    pub editor: JsonMap,
    /// Unknown keys preserved from newer project files.
    pub extra: JsonMap,
}

impl ProjectData {
    /// A project's data with the prototype's defaults: 120 BPM, 4/4, 48 kHz, no tracks.
    ///
    /// # Panics
    ///
    /// Never: the default tempo map is valid.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            settings: ProjectSettings::default(),
            tempo_map: TempoMap::constant(120.0).expect("120 BPM is a valid tempo"),
            time_signatures: TimeSignatureMap::default(),
            tracks: Vec::new(),
            master: Track::master(),
            assets: Vec::new(),
            markers: Vec::new(),
            sections: Vec::new(),
            transport: TransportSettings::default(),
            metadata: ProjectMetadata::default(),
            editor: JsonMap::new(),
            extra: JsonMap::new(),
        }
    }
}

impl Validate for ProjectData {
    fn validate(&mut self) -> Result<()> {
        // The prototype checks the transport when it is built, before the project.
        self.transport.validate()?;
        require(has_text(&self.name), || {
            "project name must not be empty".to_owned()
        })?;
        require(
            self.master.kind == TrackKind::Master && self.master.id == ids::MASTER_ID,
            || "invalid master track".to_owned(),
        )?;
        require(self.tracks.len() <= MAX_TRACKS, || {
            format!("a project can hold at most {MAX_TRACKS} tracks")
        })?;
        let mut seen = BTreeSet::from([ids::MASTER_ID]);
        for track in &self.tracks {
            require(track.kind != TrackKind::Master, || {
                "only one master track is allowed".to_owned()
            })?;
            if !seen.insert(track.id.as_str()) {
                return Err(DomainError::new(
                    ErrorCode::Conflict,
                    format!("duplicate track id {}", str_repr(&track.id)),
                ));
            }
        }
        self.markers
            .sort_by(|a, b| cmp_f64(a.beat, b.beat).then_with(|| a.id.cmp(&b.id)));
        self.sections
            .sort_by(|a, b| cmp_f64(a.start, b.start).then_with(|| a.id.cmp(&b.id)));
        Ok(())
    }
}

/// A Sonara project.
pub type Project = Valid<ProjectData>;

impl Project {
    /// A new, empty project with a fresh ID.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] for an empty name, an unsupported sample rate or an
    /// invalid tempo.
    pub fn create(
        ids: &mut dyn IdGenerator,
        name: impl Into<String>,
        sample_rate: i64,
        bpm: f64,
    ) -> Result<Self> {
        let mut data = ProjectData::new(ids.new_id(ids::PROJECT), name);
        data.settings = ProjectSettings::new(sample_rate, None, DEFAULT_RECORD_SUBTYPE)?;
        data.tempo_map = TempoMap::constant(bpm)?;
        Self::new(data)
    }

    // -- tracks ---------------------------------------------------------------------------

    /// The tracks followed by the master.
    pub fn all_tracks(&self) -> impl Iterator<Item = &Track> {
        self.tracks.iter().chain(std::iter::once(&self.master))
    }

    /// The track (or master) with `track_id`, if any.
    #[must_use]
    pub fn find_track(&self, track_id: &str) -> Option<&Track> {
        if track_id == ids::MASTER_ID {
            return Some(&self.master);
        }
        self.tracks.iter().find(|t| t.id == track_id)
    }

    /// The track (or master) with `track_id`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`]: `track '…' not found`.
    pub fn track(&self, track_id: &str) -> Result<&Track> {
        self.find_track(track_id)
            .ok_or_else(|| DomainError::not_found("track", track_id))
    }

    /// The position of a track in [`ProjectData::tracks`].
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`] for an unknown track or the master.
    pub fn track_index(&self, track_id: &str) -> Result<usize> {
        self.tracks
            .iter()
            .position(|t| t.id == track_id)
            .ok_or_else(|| DomainError::not_found("track", track_id))
    }

    /// The project with the track (or master) of the same ID replaced.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`] for an unknown track.
    pub fn with_track(&self, track: Track) -> Result<Self> {
        if track.id == ids::MASTER_ID {
            return self.edit(|d| d.master = track);
        }
        let index = self.track_index(&track.id)?;
        self.edit(|d| {
            if let Some(slot) = d.tracks.get_mut(index) {
                *slot = track;
            }
        })
    }

    /// The project with a track inserted at `index` (clamped), or appended.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::Conflict`] when a track with the same ID exists, or a project validation
    /// error (too many tracks).
    pub fn adding_track(&self, track: Track, index: Option<i64>) -> Result<Self> {
        if self.find_track(&track.id).is_some() {
            return Err(DomainError::new(
                ErrorCode::Conflict,
                format!("track {} already exists", str_repr(&track.id)),
            ));
        }
        self.edit(|d| {
            let at = clamp_index(index, d.tracks.len());
            d.tracks.insert(at, track);
        })
    }

    /// The project without the track.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] for the master, [`ErrorCode::NotFound`] for an unknown
    /// track.
    pub fn removing_track(&self, track_id: &str) -> Result<Self> {
        if track_id == ids::MASTER_ID {
            return Err(DomainError::invalid("the master track cannot be deleted"));
        }
        self.track(track_id)?;
        self.edit(|d| d.tracks.retain(|t| t.id != track_id))
    }

    /// The bus tracks.
    pub fn buses(&self) -> impl Iterator<Item = &Track> {
        self.tracks.iter().filter(|t| t.kind == TrackKind::Bus)
    }

    // -- clips and devices ----------------------------------------------------------------

    /// The track holding the clip, and the clip.
    #[must_use]
    pub fn find_clip(&self, clip_id: &str) -> Option<(&Track, &Clip)> {
        self.tracks
            .iter()
            .find_map(|t| t.find_clip(clip_id).map(|c| (t, c)))
    }

    /// The track holding the clip, and the clip.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`]: `clip '…' not found`.
    pub fn clip(&self, clip_id: &str) -> Result<(&Track, &Clip)> {
        self.find_clip(clip_id)
            .ok_or_else(|| DomainError::not_found("clip", clip_id))
    }

    /// The track holding the device (the master included), and the device.
    #[must_use]
    pub fn find_device(&self, device_id: &str) -> Option<(&Track, &Device)> {
        self.all_tracks()
            .find_map(|t| t.find_device(device_id).map(|d| (t, d)))
    }

    /// The track holding the device, and the device.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`]: `device '…' not found`.
    pub fn device(&self, device_id: &str) -> Result<(&Track, &Device)> {
        self.find_device(device_id)
            .ok_or_else(|| DomainError::not_found("device", device_id))
    }

    // -- media ----------------------------------------------------------------------------

    /// The asset with `asset_id`, if any.
    #[must_use]
    pub fn find_asset(&self, asset_id: &str) -> Option<&MediaAsset> {
        self.assets.iter().find(|a| a.id == asset_id)
    }

    /// The asset with `asset_id`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::NotFound`]: `asset '…' not found`.
    pub fn asset(&self, asset_id: &str) -> Result<&MediaAsset> {
        self.find_asset(asset_id)
            .ok_or_else(|| DomainError::not_found("asset", asset_id))
    }

    /// The project with the asset of the same ID replaced (moved to the end), or added.
    ///
    /// # Errors
    ///
    /// None in practice; the project stays valid.
    pub fn with_asset(&self, asset: MediaAsset) -> Result<Self> {
        self.edit(|d| {
            d.assets.retain(|a| a.id != asset.id);
            d.assets.push(asset);
        })
    }

    /// IDs of the assets that clips play or device state refers to.
    #[must_use]
    pub fn referenced_asset_ids(&self) -> BTreeSet<String> {
        let mut refs: BTreeSet<String> = self
            .tracks
            .iter()
            .flat_map(|t| t.audio_clips().map(|c| c.asset_id.clone()))
            .collect();
        let prefix = format!("{}_", ids::ASSET);
        for track in self.all_tracks() {
            for device in track.devices() {
                for value in device.state.values() {
                    if let Some(text) = value.as_str().filter(|s| s.starts_with(&prefix)) {
                        refs.insert(text.to_owned());
                    }
                }
            }
        }
        refs
    }

    // -- time -----------------------------------------------------------------------------

    /// The end of the content: the last clip end or section end (0 when empty).
    #[must_use]
    pub fn content_end(&self) -> f64 {
        self.tracks
            .iter()
            .map(Track::content_end)
            .chain(self.sections.iter().map(Section::end))
            .fold(None, |acc: Option<f64>, end| match acc {
                Some(best) if end <= best => Some(best),
                _ => Some(end),
            })
            .unwrap_or(0.0)
    }

    /// The song end: the explicit length if set, else the end of the content.
    #[must_use]
    pub fn end_beat(&self) -> f64 {
        self.settings
            .length_beats
            .unwrap_or_else(|| self.content_end())
    }

    /// Seconds at `beat`.
    #[must_use]
    pub fn beat_to_seconds(&self, beat: f64) -> f64 {
        self.tempo_map.beat_to_seconds(beat)
    }

    /// Beat at `seconds`.
    #[must_use]
    pub fn seconds_to_beat(&self, seconds: f64) -> f64 {
        self.tempo_map.seconds_to_beat(seconds)
    }

    /// Sample frame at `beat` at the project sample rate.
    #[must_use]
    pub fn beat_to_frame(&self, beat: f64) -> i64 {
        self.tempo_map
            .beat_to_sample(beat, self.settings.sample_rate)
    }

    /// The section with `section_id`, if any.
    #[must_use]
    pub fn find_section(&self, section_id: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.id == section_id)
    }

    /// The marker with `marker_id`, if any.
    #[must_use]
    pub fn find_marker(&self, marker_id: &str) -> Option<&Marker> {
        self.markers.iter().find(|m| m.id == marker_id)
    }
}

/// Whether two spans overlap by more than [`EPSILON`].
#[must_use]
pub fn overlaps(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> bool {
    a_start < b_end - EPSILON && b_start < a_end - EPSILON
}

#[cfg(test)]
mod tests {
    use super::{Project, ProjectSettings, TransportSettings, overlaps};
    use crate::error::ErrorCode;
    use crate::ids::SequentialIds;
    use crate::model::track::{Track, TrackKind};

    #[test]
    fn projects_follow_the_prototype_rules() {
        let mut ids = SequentialIds::new(0);
        let project = Project::create(&mut ids, "Song", 48_000, 120.0).expect("valid");
        assert_eq!(project.id, "prj_000000000001");
        let track = Track::create(&mut ids, "Bass", TrackKind::Instrument, |_| {}).expect("valid");
        let project = project.adding_track(track.clone(), None).expect("added");
        let err = project.adding_track(track, None).expect_err("duplicate");
        assert_eq!(err.code, ErrorCode::Conflict);
        assert_eq!(err.message, "track 'trk_000000000002' already exists");
        let err = project.removing_track("master").expect_err("master stays");
        assert_eq!(err.message, "the master track cannot be deleted");
        assert_eq!(project.all_tracks().count(), 2);
    }

    #[test]
    fn settings_messages_match_the_prototype() {
        let err = ProjectSettings::new(12_345, None, "PCM_24").expect_err("rate");
        assert_eq!(
            err.message,
            "sample rate must be one of (22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000)"
        );
        let transport = TransportSettings {
            loop_start: 8.0,
            loop_end: 4.0,
            ..TransportSettings::default()
        };
        assert_eq!(
            transport.validate().expect_err("inverted loop").message,
            "loop start must be >= 0 and before loop end"
        );
        assert!(overlaps(0.0, 4.0, 3.0, 5.0));
        assert!(!overlaps(0.0, 4.0, 4.0, 5.0));
    }
}
