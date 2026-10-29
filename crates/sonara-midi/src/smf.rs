//! Standard MIDI File import and export (`sonara.midi.smf`).
//!
//! Export writes a Type 1 file at 960 PPQ: track 0 carries the tempo map, the time signatures
//! and the markers; every instrument track becomes a MIDI track with its notes, controllers and
//! pitch bend (looped clips are expanded). Drum-kit tracks use channel 10. Beat positions are
//! rounded to ticks, and the largest rounding error is reported. The bytes are identical to the
//! prototype's (mido) output for the same project.
//!
//! # Not exported
//!
//! Audio tracks, folders and buses, muted clips, and everything that is not note, controller,
//! pitch bend, tempo, time signature or marker data: track volume, pan, mute and solo, effects,
//! instrument choice and parameters, and automation. Nothing is dropped silently within a
//! supported kind: failures are errors (`INVALID_ARGUMENT`, `LIMIT_EXCEEDED`, `IO_ERROR`).
//!
//! # Deviations from the prototype
//!
//! * **Text encoding.** mido writes names as Latin-1 and
//!   crashes on other characters. Names and markers are written as UTF-8. Import decodes a name
//!   as UTF-8 when it is valid UTF-8 and falls back to Latin-1 (the prototype's reading, and the
//!   usual encoding of old files) otherwise, so files written by either version read back.
//! * **Malformed events.** The container is checked like mido does (header, track chunks,
//!   truncation). Inside a track, `midly` ends the track at its first malformed event instead of
//!   failing with a `FORMAT_ERROR`, and it rejects system common and real-time messages, which
//!   are illegal in files anyway.
//! * **Division.** A negative (SMPTE) time division is `UNSUPPORTED`, and a division of 0 is a
//!   `FORMAT_ERROR`; the prototype divides by it, or produces negative beats.
//! * **Tempo 0.** A `set_tempo` of 0 microseconds per beat reads as the fastest tempo
//!   (999 BPM); the prototype raises `ZeroDivisionError`.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;

use midly::num::{u4, u7, u15, u24, u28};
use midly::{
    Format, Header, MetaMessage, MidiMessage, PitchBend, Smf, Timing, TrackEvent, TrackEventKind,
};
use sonara_core::error::{DomainError, ErrorCode, Result};
use sonara_core::ids::IdGenerator;
use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote};
use sonara_core::model::clip::MidiClip;
use sonara_core::model::project::Project;
use sonara_core::model::track::{Track, TrackKind};
use sonara_core::time::{
    TempoMap, TempoPoint, TimeSignatureMap, TimeSignaturePoint, VALID_DENOMINATORS, python_round,
};

use crate::num::{py_floordiv, py_max, py_min, py_strip, to_f64};

/// Ticks per quarter note of exported files.
pub const PPQ: u16 = 960;
/// Plugin ID of the drum kit, whose tracks use MIDI channel 10.
pub const DRUM_PLUGIN: &str = "sonara.drums";
/// Largest file the importer reads.
pub const MAX_SMF_BYTES: u64 = 64 * 1024 * 1024;

/// The 0-based MIDI channel of drums (channel 10).
const DRUM_CHANNEL: u8 = 9;
/// Delta times are 28-bit values.
const MAX_DELTA_TICKS: i64 = 0x0FFF_FFFF;
/// Time signature meta event: MIDI clocks per metronome click.
const CLOCKS_PER_CLICK: u8 = 24;
/// Time signature meta event: notated 32nd notes per quarter note.
const THIRTY_SECONDS_PER_QUARTER: u8 = 8;

// -- export -------------------------------------------------------------------------------

/// What an export wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportedMidi {
    /// The file path, as given.
    pub path: String,
    /// Number of MIDI tracks in the file, including the meta track.
    pub tracks: usize,
    /// Number of notes written.
    pub notes: usize,
    /// Largest distance between a position and its tick, in beats.
    pub max_rounding_error_beats: f64,
}

/// An exported file held in memory.
#[derive(Debug, Clone, PartialEq)]
pub struct EncodedMidi {
    /// The bytes of the file.
    pub bytes: Vec<u8>,
    /// Number of MIDI tracks in the file, including the meta track.
    pub tracks: usize,
    /// Number of notes written.
    pub notes: usize,
    /// Largest distance between a position and its tick, in beats.
    pub max_rounding_error_beats: f64,
}

/// An event of the export, before delta times are computed.
enum Payload {
    Name(String),
    Tempo(u32),
    TimeSignature {
        numerator: u8,
        denominator_power: u8,
    },
    Marker(String),
    NoteOn {
        channel: u8,
        key: u8,
        velocity: u8,
    },
    NoteOff {
        channel: u8,
        key: u8,
    },
    Control {
        channel: u8,
        kind: ControlKind,
        number: u8,
        value: i64,
    },
}

/// An event at an absolute tick. Events at the same tick sort by `priority` (controllers and meta
/// events, then note-offs and markers, then note-ons), and otherwise keep their order.
struct Event {
    tick: i64,
    priority: u8,
    payload: Payload,
}

/// Collects the events of one track and the rounding error.
struct Ticks {
    max_error: f64,
}

impl Ticks {
    /// Rounds a position in beats to ticks and records the rounding error.
    fn tick(&mut self, beat: f64) -> i64 {
        let ticks = python_round(beat * f64::from(PPQ));
        self.max_error = py_max(
            self.max_error,
            (to_f64(ticks) / f64::from(PPQ) - beat).abs(),
        );
        ticks
    }
}

/// Python's `bpm2tempo`: microseconds per quarter note, rounded half to even.
fn bpm_to_tempo(bpm: f64) -> Result<u32> {
    let micros = python_round(60.0 * 1e6 / bpm * 4.0 / 4.0);
    u32::try_from(micros)
        .ok()
        .filter(|tempo| *tempo <= 0x00FF_FFFF)
        .ok_or_else(|| DomainError::invalid("tempo does not fit a MIDI file"))
}

/// The tempo events of the tempo map. SMF has no ramps, so a ramp becomes a tempo change every
/// sixteenth note; each step uses the step's average tempo, so step boundaries keep their exact
/// ramp times.
fn tempo_events(project: &Project, ticks: &mut Ticks, events: &mut Vec<Event>) -> Result<()> {
    let tempo_map = &project.tempo_map;
    let points = tempo_map.points();
    for point in points {
        let next = points.iter().find(|q| q.beat > point.beat);
        let mut push = |tick: i64, bpm: f64| -> Result<()> {
            events.push(Event {
                tick,
                priority: 0,
                payload: Payload::Tempo(bpm_to_tempo(bpm)?),
            });
            Ok(())
        };
        match next {
            Some(next) if point.ramp => {
                let mut b = point.beat;
                while b < next.beat - 1e-9 {
                    let e = py_min(b + 0.25, next.beat);
                    let bpm = 60.0 * (e - b)
                        / (tempo_map.beat_to_seconds(e) - tempo_map.beat_to_seconds(b));
                    push(ticks.tick(b), bpm)?;
                    b = e;
                }
            }
            _ => push(ticks.tick(point.beat), point.bpm)?,
        }
    }
    Ok(())
}

/// The events of the meta track: name, tempo map, time signatures, markers and sections.
fn meta_events(project: &Project, ticks: &mut Ticks) -> Result<Vec<Event>> {
    let mut events = vec![Event {
        tick: 0,
        priority: 0,
        payload: Payload::Name(project.name.clone()),
    }];
    tempo_events(project, ticks, &mut events)?;
    for point in project.time_signatures.points() {
        let numerator = u8::try_from(point.numerator).map_err(|_| {
            DomainError::invalid("time signature numerator does not fit a MIDI file")
        })?;
        let power = VALID_DENOMINATORS
            .iter()
            .position(|d| *d == point.denominator)
            .and_then(|i| u8::try_from(i).ok())
            .ok_or_else(|| {
                DomainError::invalid("time signature denominator is not a power of 2")
            })?;
        let beat = project.time_signatures.bar_to_beat(to_f64(point.bar));
        events.push(Event {
            tick: ticks.tick(beat),
            priority: 0,
            payload: Payload::TimeSignature {
                numerator,
                denominator_power: power,
            },
        });
    }
    for marker in &project.markers {
        events.push(Event {
            tick: ticks.tick(marker.beat),
            priority: 1,
            payload: Payload::Marker(marker.name.clone()),
        });
    }
    for section in &project.sections {
        events.push(Event {
            tick: ticks.tick(section.start),
            priority: 1,
            payload: Payload::Marker(section.name.clone()),
        });
    }
    Ok(events)
}

/// The channel of the next instrument track: drums use channel 10, the others count up and skip
/// it.
fn next_channel(is_drums: bool, cursor: &mut usize) -> u8 {
    if is_drums {
        return DRUM_CHANNEL;
    }
    let mut channel = *cursor % 16;
    if channel == usize::from(DRUM_CHANNEL) {
        *cursor += 1;
        channel = *cursor % 16;
    }
    *cursor += 1;
    u8::try_from(channel).unwrap_or(0)
}

/// The events of one instrument track; returns the number of notes.
fn instrument_events(track: &Track, channel: u8, ticks: &mut Ticks) -> (Vec<Event>, usize) {
    let mut events = vec![Event {
        tick: 0,
        priority: 0,
        payload: Payload::Name(track.name.clone()),
    }];
    let mut notes = 0;
    for clip in track.midi_clips() {
        if clip.muted {
            continue;
        }
        let clip_end = clip.start + clip.length;
        for (t, note) in clip.playback_notes() {
            let end = py_min(t + note.length, clip_end);
            let on = ticks.tick(t);
            let off = (ticks.tick(end)).max(ticks.tick(t) + 1);
            events.push(Event {
                tick: on,
                priority: 2,
                payload: Payload::NoteOn {
                    channel,
                    key: note.pitch,
                    velocity: note.velocity,
                },
            });
            events.push(Event {
                tick: off,
                priority: 1,
                payload: Payload::NoteOff {
                    channel,
                    key: note.pitch,
                },
            });
            notes += 1;
        }
        if clip.loop_length.is_none() {
            for control in clip.controls.iter() {
                let beat = clip.start + control.beat - clip.content_offset;
                if clip.start <= beat && beat < clip_end {
                    events.push(Event {
                        tick: ticks.tick(beat),
                        priority: 0,
                        payload: Payload::Control {
                            channel,
                            kind: control.kind,
                            number: u8::try_from(control.number).unwrap_or(0),
                            value: control.value,
                        },
                    });
                }
            }
        }
    }
    (events, notes)
}

/// Computes the delta times of the (sorted) `events` and closes the track.
fn finish_track(events: &[Event]) -> Result<Vec<TrackEvent<'_>>> {
    let mut out = Vec::with_capacity(events.len() + 1);
    let mut now = 0_i64;
    for event in events {
        let delta = event.tick - now;
        now = event.tick;
        let delta = u32::try_from(delta)
            .ok()
            .filter(|d| i64::from(*d) <= MAX_DELTA_TICKS)
            .and_then(u28::try_from)
            .ok_or_else(|| {
                DomainError::new(
                    ErrorCode::LimitExceeded,
                    "a position is outside the range of a MIDI file",
                )
            })?;
        let kind = match &event.payload {
            Payload::Name(text) => TrackEventKind::Meta(MetaMessage::TrackName(text.as_bytes())),
            Payload::Marker(text) => TrackEventKind::Meta(MetaMessage::Marker(text.as_bytes())),
            Payload::Tempo(tempo) => TrackEventKind::Meta(MetaMessage::Tempo(u24::new(*tempo))),
            Payload::TimeSignature {
                numerator,
                denominator_power,
            } => TrackEventKind::Meta(MetaMessage::TimeSignature(
                *numerator,
                *denominator_power,
                CLOCKS_PER_CLICK,
                THIRTY_SECONDS_PER_QUARTER,
            )),
            Payload::NoteOn {
                channel,
                key,
                velocity,
            } => TrackEventKind::Midi {
                channel: u4::new(*channel),
                message: MidiMessage::NoteOn {
                    key: u7::new(*key),
                    vel: u7::new(*velocity),
                },
            },
            Payload::NoteOff { channel, key } => TrackEventKind::Midi {
                channel: u4::new(*channel),
                message: MidiMessage::NoteOff {
                    key: u7::new(*key),
                    vel: u7::new(0),
                },
            },
            Payload::Control {
                channel,
                kind,
                number,
                value,
            } => TrackEventKind::Midi {
                channel: u4::new(*channel),
                message: control_message(*kind, *number, *value),
            },
        };
        out.push(TrackEvent { delta, kind });
    }
    out.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    Ok(out)
}

/// The channel message of a controller event.
fn control_message(kind: ControlKind, number: u8, value: i64) -> MidiMessage {
    let byte = |v: i64| u7::new(u8::try_from(v.clamp(0, 127)).unwrap_or(0));
    match kind {
        ControlKind::PitchBend => MidiMessage::PitchBend {
            bend: PitchBend::from_int(i16::try_from(value).unwrap_or(0)),
        },
        ControlKind::ChannelPressure => MidiMessage::ChannelAftertouch { vel: byte(value) },
        ControlKind::Cc => MidiMessage::Controller {
            controller: u7::new(number),
            value: byte(value),
        },
    }
}

/// Encodes `project` as a Type 1 Standard MIDI File. `track_ids` restricts the export to those
/// instrument tracks (`None` exports all of them).
///
/// # Errors
///
/// `INVALID_ARGUMENT` when a tempo or time signature does not fit the file format;
/// `LIMIT_EXCEEDED` when a position does not fit a 28-bit delta time.
pub fn encode_smf(project: &Project, track_ids: Option<&[&str]>) -> Result<EncodedMidi> {
    let mut ticks = Ticks { max_error: 0.0 };
    let mut all_events = vec![meta_events(project, &mut ticks)?];
    let mut total_notes = 0;
    let mut channel_cursor = 0;
    for track in &project.tracks {
        if track.kind != TrackKind::Instrument
            || track_ids.is_some_and(|ids| !ids.contains(&track.id.as_str()))
        {
            continue;
        }
        let is_drums = track
            .instrument
            .as_ref()
            .is_some_and(|device| device.plugin_id == DRUM_PLUGIN);
        let channel = next_channel(is_drums, &mut channel_cursor);
        let (events, notes) = instrument_events(track, channel, &mut ticks);
        total_notes += notes;
        all_events.push(events);
    }
    // A stable sort by tick and priority, like the prototype's `events.sort(key=...)`.
    for events in &mut all_events {
        events.sort_by_key(|event| (event.tick, event.priority));
    }
    let tracks = all_events
        .iter()
        .map(|events| finish_track(events))
        .collect::<Result<Vec<_>>>()?;
    let smf = Smf {
        header: Header::new(Format::Parallel, Timing::Metrical(u15::new(PPQ))),
        tracks,
    };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes).map_err(|err| {
        DomainError::new(
            ErrorCode::LimitExceeded,
            format!("cannot encode the MIDI file: {err}"),
        )
    })?;
    Ok(EncodedMidi {
        bytes,
        tracks: smf.tracks.len(),
        notes: total_notes,
        max_rounding_error_beats: ticks.max_error,
    })
}

/// Exports `project` to a Type 1 Standard MIDI File at `path`, creating missing parent
/// directories. `track_ids` restricts the export to those instrument tracks.
///
/// # Errors
///
/// The errors of [`encode_smf`], and `IO_ERROR` when the file cannot be written.
pub fn export_smf(
    project: &Project,
    path: &Path,
    track_ids: Option<&[&str]>,
) -> Result<ExportedMidi> {
    let encoded = encode_smf(project, track_ids)?;
    let io_error = |err: std::io::Error| {
        DomainError::new(
            ErrorCode::IoError,
            format!("cannot write {}: {err}", path.display()),
        )
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(io_error)?;
    }
    std::fs::write(path, &encoded.bytes).map_err(io_error)?;
    Ok(ExportedMidi {
        path: path.display().to_string(),
        tracks: encoded.tracks,
        notes: encoded.notes,
        max_rounding_error_beats: encoded.max_rounding_error_beats,
    })
}

// -- import -------------------------------------------------------------------------------

/// A MIDI track of an imported file: one MIDI channel of one file track.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedTrack {
    /// The track name.
    pub name: String,
    /// The 0-based MIDI channel.
    pub channel: u8,
    /// The notes, in the order their note-offs appear.
    pub notes: Vec<MidiNote>,
    /// Controller, pitch bend and pressure events.
    pub controls: Vec<MidiControlEvent>,
    /// The last program change, if any.
    pub program: Option<u8>,
}

impl ImportedTrack {
    /// Whether the track is on the drum channel (channel 10).
    #[must_use]
    pub fn is_drums(&self) -> bool {
        self.channel == DRUM_CHANNEL
    }

    /// The end of the last note in beats (0 without notes).
    #[must_use]
    pub fn end_beat(&self) -> f64 {
        self.notes.iter().map(MidiNote::end).fold(0.0, py_max)
    }
}

/// What the importer read from a file.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedMidi {
    /// The tempo map.
    pub tempo_map: TempoMap,
    /// The time signatures.
    pub time_signatures: TimeSignatureMap,
    /// Tracks that have notes or controllers.
    pub tracks: Vec<ImportedTrack>,
    /// Markers and cue points as `(beat, text)`.
    pub markers: Vec<(f64, String)>,
    /// Things that were skipped.
    pub warnings: Vec<String>,
}

/// The chunk structure of a file the way mido reads it: the header and `ntracks` track chunks.
/// Returns the message of mido's exception for a file it cannot read.
fn check_container(bytes: &[u8]) -> std::result::Result<(i16, i16, i16), String> {
    // EOFError has no message.
    let eof = String::new;
    let header = bytes.get(..8).ok_or_else(eof)?;
    if &header[..4] != b"MThd" {
        return Err("MThd not found. Probably not a MIDI file".to_owned());
    }
    let size = usize::try_from(u32::from_be_bytes([
        header[4], header[5], header[6], header[7],
    ]))
    .unwrap_or(usize::MAX);
    let data = bytes
        .get(8..)
        .map_or(&[][..], |rest| &rest[..size.min(rest.len())]);
    if data.len() < 6 {
        return Err(eof());
    }
    let word = |i: usize| i16::from_be_bytes([data[i], data[i + 1]]);
    let (format, ntracks, division) = (word(0), word(2), word(4));
    let mut pos = 8 + data.len();
    for _ in 0..ntracks.max(0) {
        let chunk = bytes.get(pos..pos + 8).ok_or_else(eof)?;
        if &chunk[..4] != b"MTrk" {
            return Err("no MTrk header at start of track".to_owned());
        }
        let size = usize::try_from(u32::from_be_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]))
            .unwrap_or(usize::MAX);
        pos = pos
            .checked_add(8)
            .and_then(|p| p.checked_add(size))
            .filter(|end| *end <= bytes.len())
            .ok_or_else(eof)?;
    }
    Ok((format, ntracks, division))
}

/// Decodes a name or marker text: UTF-8 when valid, else Latin-1.
fn decode_text(raw: &[u8]) -> String {
    std::str::from_utf8(raw).map_or_else(
        |_| raw.iter().map(|byte| char::from(*byte)).collect(),
        str::to_owned,
    )
}

/// 2 to the power `exponent` in decimal (the prototype prints the denominator of a time
/// signature as an integer, which can have up to 77 digits).
fn pow2_decimal(exponent: u32) -> String {
    let mut digits = vec![1_u8];
    for _ in 0..exponent {
        let mut carry = 0;
        for digit in &mut digits {
            let doubled = *digit * 2 + carry;
            *digit = doubled % 10;
            carry = doubled / 10;
        }
        if carry > 0 {
            digits.push(carry);
        }
    }
    digits.iter().rev().map(|d| char::from(b'0' + d)).collect()
}

/// An imported track with the file track it came from.
struct Channel {
    file_track: usize,
    track: ImportedTrack,
}

/// Reads a Standard MIDI File. Note IDs come from `ids`, in the order the note-offs are read.
///
/// # Errors
///
/// - `NOT_FOUND`: `MIDI file <path> does not exist`.
/// - `LIMIT_EXCEEDED`: `MIDI file is too large` (over [`MAX_SMF_BYTES`]).
/// - `FORMAT_ERROR`: `<name> is not a valid Standard MIDI File: <reason>`.
/// - `UNSUPPORTED`: Type 2 (sequential) files and SMPTE time division.
/// - `INVALID_ARGUMENT`: tempo and time signature data the project model rejects.
pub fn import_smf(ids: &mut dyn IdGenerator, path: &Path) -> Result<ImportedMidi> {
    if !path.is_file() {
        return Err(DomainError::new(
            ErrorCode::NotFound,
            format!("MIDI file {} does not exist", path.display()),
        ));
    }
    let file_name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let invalid = |reason: &str| {
        DomainError::new(
            ErrorCode::FormatError,
            format!("{file_name} is not a valid Standard MIDI File: {reason}"),
        )
    };
    let size = std::fs::metadata(path)
        .map_err(|err| invalid(&err.to_string()))?
        .len();
    if size > MAX_SMF_BYTES {
        return Err(DomainError::new(
            ErrorCode::LimitExceeded,
            "MIDI file is too large",
        ));
    }
    let bytes = std::fs::read(path).map_err(|err| invalid(&err.to_string()))?;
    parse_smf(ids, &bytes, &file_name)
}

/// Reads a Standard MIDI File from memory; `file_name` is used in error messages.
///
/// # Errors
///
/// As [`import_smf`], except for the file system errors.
pub fn parse_smf(ids: &mut dyn IdGenerator, bytes: &[u8], file_name: &str) -> Result<ImportedMidi> {
    let invalid = |reason: &str| {
        DomainError::new(
            ErrorCode::FormatError,
            format!("{file_name} is not a valid Standard MIDI File: {reason}"),
        )
    };
    let (format, ntracks, division) = check_container(bytes).map_err(|reason| invalid(&reason))?;
    if format == 2 {
        return Err(DomainError::new(
            ErrorCode::Unsupported,
            "Type 2 (sequential) MIDI files are not supported",
        ));
    }
    if division < 0 {
        return Err(DomainError::new(
            ErrorCode::Unsupported,
            "SMPTE time division is not supported",
        ));
    }
    if division == 0 {
        return Err(invalid("ticks per beat must be positive"));
    }
    let smf = Smf::parse(bytes).map_err(|err| invalid(&err.to_string()))?;
    let ppq = f64::from(division);
    let tracks = &smf.tracks[..smf
        .tracks
        .len()
        .min(usize::try_from(ntracks.max(0)).unwrap_or(0))];
    read_tracks(ids, tracks, ppq)
}

/// What the events of all tracks add up to, before the project model validates it.
#[derive(Default)]
struct Collected {
    tempos: Vec<TempoPoint>,
    signatures: Vec<(f64, u8, u8)>,
    markers: Vec<(f64, String)>,
    channels: Vec<Channel>,
    warnings: Vec<String>,
}

fn read_tracks(
    ids: &mut dyn IdGenerator,
    tracks: &[Vec<TrackEvent<'_>>],
    ppq: f64,
) -> Result<ImportedMidi> {
    let mut collected = Collected::default();
    for (file_track, events) in tracks.iter().enumerate() {
        read_track(ids, &mut collected, file_track, events, ppq)?;
    }
    let Collected {
        mut tempos,
        signatures,
        markers,
        mut channels,
        mut warnings,
    } = collected;
    let single_file_track = channels
        .first()
        .is_some_and(|first| channels.iter().all(|c| c.file_track == first.file_track));
    if single_file_track {
        for channel in &mut channels {
            if channel.track.name.starts_with("Track ") {
                channel.track.name = format!("Channel {}", channel.track.channel + 1);
            }
        }
    }
    tempos.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    if tempos.first().is_none_or(|first| first.beat > 0.0) {
        let bpm = tempos.first().map_or(120.0, |first| first.bpm);
        tempos.insert(0, TempoPoint::new(0.0, bpm, false)?);
    }
    let tempo_map = TempoMap::new(tempos)?;
    let time_signatures = build_time_signatures(signatures, &mut warnings)?;
    Ok(ImportedMidi {
        tempo_map,
        time_signatures,
        tracks: channels
            .into_iter()
            .map(|c| c.track)
            .filter(|t| !t.notes.is_empty() || !t.controls.is_empty())
            .collect(),
        markers,
        warnings,
    })
}

/// The time signature map of the signatures read, ignoring unsupported denominators.
fn build_time_signatures(
    mut signatures: Vec<(f64, u8, u8)>,
    warnings: &mut Vec<String>,
) -> Result<TimeSignatureMap> {
    let mut points = vec![TimeSignaturePoint::new(0, 4, 4)?];
    let mut cursor = TimeSignatureMap::default();
    signatures.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    for (beat, numerator, power) in signatures {
        let denominator = 1_u64.checked_shl(u32::from(power)).unwrap_or(0);
        let supported = u32::try_from(denominator)
            .ok()
            .filter(|d| VALID_DENOMINATORS.contains(d));
        let Some(denominator) = supported else {
            let shown = if power < 64 {
                denominator.to_string()
            } else {
                pow2_decimal(u32::from(power))
            };
            warnings.push(format!(
                "time signature {numerator}/{shown} not supported; ignored"
            ));
            continue;
        };
        let bar = python_round(cursor.beat_to_bar(beat));
        points.retain(|p| p.bar != bar);
        points.push(TimeSignaturePoint::new(
            bar,
            u32::from(numerator),
            denominator,
        )?);
        points.sort_by_key(|p| p.bar);
        cursor = TimeSignatureMap::new(points.clone())?;
    }
    Ok(cursor)
}

/// Reads the events of one file track.
fn read_track(
    ids: &mut dyn IdGenerator,
    out: &mut Collected,
    file_track: usize,
    events: &[TrackEvent<'_>],
    ppq: f64,
) -> Result<()> {
    let mut ticks = 0_i64;
    let mut name = format!("Track {}", file_track + 1);
    let mut open: HashMap<(u8, u8), VecDeque<(f64, u8)>> = HashMap::new();
    for event in events {
        ticks += i64::from(event.delta.as_int());
        let beat = to_f64(ticks) / ppq;
        match event.kind {
            TrackEventKind::Meta(meta) => read_meta(out, &mut name, beat, meta),
            TrackEventKind::Midi { channel, message } => {
                let channel = channel.as_int();
                let index = channel_track(out, file_track, channel, &name);
                let track = &mut out.channels[index].track;
                match message {
                    MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                        open.entry((channel, key.as_int()))
                            .or_default()
                            .push_back((beat, vel.as_int()));
                    }
                    MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                        let started = open
                            .get_mut(&(channel, key.as_int()))
                            .and_then(VecDeque::pop_front);
                        if let Some((start, velocity)) = started {
                            let length = py_max(beat - start, 1.0 / ppq);
                            track.notes.push(MidiNote::create(
                                ids,
                                i64::from(key.as_int()),
                                start,
                                length,
                                i64::from(velocity.max(1)),
                                0,
                            )?);
                        }
                    }
                    MidiMessage::Controller { controller, value } => {
                        track.controls.push(MidiControlEvent::new(
                            beat,
                            ControlKind::Cc,
                            i64::from(value.as_int()),
                            i64::from(controller.as_int()),
                            0,
                        )?);
                    }
                    MidiMessage::PitchBend { bend } => {
                        track.controls.push(MidiControlEvent::new(
                            beat,
                            ControlKind::PitchBend,
                            i64::from(bend.as_int()),
                            0,
                            0,
                        )?);
                    }
                    MidiMessage::ChannelAftertouch { vel } => {
                        track.controls.push(MidiControlEvent::new(
                            beat,
                            ControlKind::ChannelPressure,
                            i64::from(vel.as_int()),
                            0,
                            0,
                        )?);
                    }
                    MidiMessage::ProgramChange { program } => {
                        track.program = Some(program.as_int());
                    }
                    // Polyphonic key pressure is not imported.
                    MidiMessage::Aftertouch { .. } => {}
                }
            }
            // System exclusive messages have no channel and are skipped.
            TrackEventKind::SysEx(_) | TrackEventKind::Escape(_) => {}
        }
    }
    let dangling: usize = open.values().map(VecDeque::len).sum();
    if dangling > 0 {
        out.warnings.push(format!(
            "{name}: {dangling} note(s) without note-off were dropped"
        ));
    }
    Ok(())
}

/// Handles a meta event: tempo, time signature, track name and markers.
fn read_meta(out: &mut Collected, name: &mut String, beat: f64, meta: MetaMessage<'_>) {
    match meta {
        MetaMessage::Tempo(tempo) => {
            // Python's `tempo2bpm`; a tempo of 0 gives infinity, which the clamp turns into 999.
            let bpm = py_min(
                py_max(60.0 * 1e6 / f64::from(tempo.as_int()) * 4.0 / 4.0, 10.0),
                999.0,
            );
            out.tempos.retain(|p| (p.beat - beat).abs() > 1e-9);
            out.tempos.push(TempoPoint {
                beat,
                bpm,
                ramp: false,
            });
        }
        MetaMessage::TimeSignature(numerator, power, _, _) => {
            out.signatures.push((beat, numerator, power));
        }
        MetaMessage::TrackName(raw) => {
            let text = decode_text(raw);
            let text = py_strip(&text);
            if !text.is_empty() {
                text.clone_into(name);
            }
        }
        MetaMessage::Marker(raw) | MetaMessage::CuePoint(raw) => {
            let text = decode_text(raw);
            let text = py_strip(&text);
            if !text.is_empty() {
                out.markers.push((beat, text.to_owned()));
            }
        }
        _ => {}
    }
}

/// The index of the imported track of `channel` in `file_track`, created on first use. A drum
/// channel track gets the suffix ` (Drums)` unless its name already says drum.
fn channel_track(out: &mut Collected, file_track: usize, channel: u8, name: &str) -> usize {
    if let Some(index) = out
        .channels
        .iter()
        .position(|c| c.file_track == file_track && c.track.channel == channel)
    {
        return index;
    }
    let name = if channel != DRUM_CHANNEL || name.to_lowercase().contains("drum") {
        name.to_owned()
    } else {
        format!("{name} (Drums)")
    };
    out.channels.push(Channel {
        file_track,
        track: ImportedTrack {
            name,
            channel,
            notes: Vec::new(),
            controls: Vec::new(),
            program: None,
        },
    });
    out.channels.len() - 1
}

/// A MIDI clip for an imported track, placed at `start`. The length defaults to the content
/// rounded up to whole 4-beat bars (at least one bar), and the name to the track's name.
///
/// # Errors
///
/// The validation errors of [`MidiClip`].
pub fn clip_from_imported(
    ids: &mut dyn IdGenerator,
    track: &ImportedTrack,
    start: f64,
    name: Option<&str>,
    length: Option<f64>,
) -> Result<MidiClip> {
    let last_control = track.controls.iter().map(|c| c.beat).fold(0.0, py_max);
    let mut end = py_max(track.end_beat(), last_control);
    if end == 0.0 {
        end = 4.0;
    }
    let bars = py_max(1.0, -py_floordiv(-end, 4.0));
    let length = length.filter(|l| *l != 0.0).unwrap_or(bars * 4.0);
    let name = name.filter(|n| !n.is_empty()).unwrap_or(&track.name);
    let clip = MidiClip::create(ids, name, start, length, track.notes.clone())?;
    clip.edit(|data| data.controls = Arc::new(track.controls.clone()))
}
