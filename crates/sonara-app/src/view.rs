//! The project view of an interface: the open project as the desktop's
//! panels read it, in one serializable snapshot with the session version it shows.
//!
//! The prototype's panels read the immutable project snapshot directly and refreshed on every
//! session event (`panels.py`). The desktop's page lives in another process, so it asks for
//! this view after each `session-changed` event instead. It carries what the panels show and
//! nothing they would have to compute themselves (DES-01):
//!
//! - the tracks in order, then the master, each with its clips (notes included), its
//!   instrument and effects with their parameters resolved against the plugin descriptors, its
//!   sends, its input, its folder depth and its routing targets (the outputs and send targets
//!   the mixer and the inspector offer);
//! - the descriptors of every plugin the project uses;
//! - the tempo map, the time signatures, the markers, the sections, the transport settings,
//!   the metadata and the media assets;
//! - the undo and redo labels, the dirty state, the path, and the [`meter layout`] that the
//!   live frames of [`crate::live`] follow.
//!
//! Building it reads one snapshot under the session lock and does the rest outside it.
//!
//! **The tracks view** ([`tracks_view`]) is the refresh of an edit that changed only some
//! tracks' mixer settings: a fader, a knob or a send moved, M or S toggled. It carries those
//! tracks' views with the version, the dirty state, the history and the meter layout, and the
//! page puts them into the view it shows. A fader drag on 100 tracks × 40 clips commits about 60
//! edits a second, and a full view of that project is 1.9 MB of JSON: in the meter benchmark's
//! drag (PERF-01) the page applies a tracks view in 0.4 ms instead of 2.2 ms, and the
//! meters drop no frame where whole views dropped 1.1 %.
//!
//! **The clips view** ([`clips_view`]) is the refresh of an edit that changed only some
//! MIDI clips' notes: a piano-roll drag, a nudge, a velocity, a quantize, or an agent's
//! `add_midi_notes`. It carries those clips' views with their tracks, the version, the dirty
//! state, the history and the meter layout, and the page puts them into the view it shows. A note
//! drag commits an edit per display frame; on the piano roll benchmark's project (100 tracks × 40
//! clips and a 2,000-note and a 10,000-note clip, 24,000 notes) a whole view per edit cost the
//! drag up to four long frames per gesture and a worst pointer-to-pixel of 70 ms
//! (PERF-01).
//!
//! **The devices view** ([`devices_view`]) is the refresh of an edit that changed only
//! some devices' state: a knob, a toggle, a choice, a preset or On/Off on a device card. It carries
//! those devices' views with their tracks, the version, the dirty state, the history and the meter
//! layout, and the page puts them into their tracks. Such an edit carries the flag `devices`, which
//! the prototype's panels count as structural, but it adds, removes or moves nothing. A knob drag
//! commits an edit per display frame; in the meter benchmark's knob drag on 100 tracks × 40 clips
//! (PERF-01) the page applies a devices view in 1 ms instead of 2 ms for the 1.9 MB view,
//! and whole views dropped up to 3 % of the frames when they did not run first.
//!
//! [`meter layout`]: OpenProject::meter_layout

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};
use sonara_core::ids;
use sonara_core::model::clip::{Clip, Fade};
use sonara_core::model::device::Device;
use sonara_core::model::media::MediaAsset;
use sonara_core::model::project::Project;
use sonara_core::model::track::{Track, TrackKind};
use sonara_core::plugins::{ParamScale, ParamSpec, PluginDescriptor, PluginRegistry};

use crate::session::SharedSession;

/// The deepest folder nesting the track headers indent (the prototype's `MAX_DEPTH`).
pub const MAX_FOLDER_DEPTH: u32 = 6;

/// The project view: the session version, and the open project when there is one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectView {
    /// The session version the view shows; it grows with every event.
    pub version: u64,
    /// The open project, `None` when none is open.
    pub project: Option<OpenProject>,
}

/// The open project as the panels show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenProject {
    /// The project ID.
    pub id: String,
    /// The project name (the window title).
    pub name: String,
    /// The `.sonara` path; `None` until the first save ("not saved" in the title).
    pub path: Option<String>,
    /// Whether the project differs from the saved one (the title's dirty marker).
    pub dirty: bool,
    /// The project sample rate.
    pub sample_rate: u32,
    /// The explicit song end in beats; `None` means the end of the content.
    pub length_beats: Option<f64>,
    /// The song end in beats: the explicit end, else the end of the last clip.
    pub end_beat: f64,
    /// The tempo map; the first point is the base tempo.
    pub tempo: Vec<TempoView>,
    /// The time signatures, from bar 0 on; the snap grid's "Bar" and the ruler read them.
    pub time_signatures: Vec<TimeSignatureView>,
    /// Loop, punch, metronome and count-in.
    pub transport: TransportView,
    /// The song metadata.
    pub metadata: MetadataView,
    /// The tracks in order, without the master.
    pub tracks: Vec<TrackView>,
    /// The master track, pinned right in the mixer.
    pub master: TrackView,
    /// The markers, in order.
    pub markers: Vec<MarkerView>,
    /// The sections, in order.
    pub sections: Vec<SectionView>,
    /// The media assets.
    pub assets: Vec<AssetView>,
    /// The descriptor of every plugin a device of the project uses, by plugin ID.
    pub plugins: BTreeMap<String, PluginView>,
    /// The undo and redo labels.
    pub history: HistoryView,
    /// The identity of the order of the meters and gain-reduction values in a live frame:
    /// [`meter_layout`] of this project. A frame with the same value lines up with
    /// [`Self::tracks`], then the master, and their devices.
    pub meter_layout: u32,
}

/// A tempo point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TempoView {
    /// Where it starts, in beats.
    pub beat: f64,
    /// The tempo.
    pub bpm: f64,
    /// Whether the tempo ramps to the next point.
    pub ramp: bool,
}

/// A time signature, from its bar on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TimeSignatureView {
    /// The 0-based bar it starts at.
    pub bar: i64,
    /// The beat it starts at.
    pub beat: f64,
    /// Beats per bar.
    pub numerator: u32,
    /// The beat's note value.
    pub denominator: u32,
    /// The length of a bar in quarter-note beats.
    pub beats_per_bar: f64,
}

/// The transport settings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TransportView {
    /// Whether playback loops.
    pub loop_enabled: bool,
    /// The loop start in beats.
    pub loop_start: f64,
    /// The loop end in beats.
    pub loop_end: f64,
    /// Whether recording punches in and out.
    pub punch_enabled: bool,
    /// The punch-in in beats.
    pub punch_in: f64,
    /// The punch-out in beats.
    pub punch_out: f64,
    /// Whether the metronome clicks.
    pub metronome: bool,
    /// The count-in before recording, in bars.
    pub count_in_bars: u32,
}

/// The song metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetadataView {
    /// Title.
    pub title: String,
    /// Artist.
    pub artist: String,
    /// Album.
    pub album: String,
    /// Genre.
    pub genre: String,
    /// Composer (set through the API only).
    pub composer: String,
    /// Comment (set through the API only).
    pub comment: String,
}

/// A track or the master.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackView {
    /// The track ID (`master` for the master).
    pub id: String,
    /// The name.
    pub name: String,
    /// `audio`, `instrument`, `bus`, `folder` or `master`.
    pub kind: &'static str,
    /// `#rrggbb`, or `None` for the automatic colour.
    pub color: Option<String>,
    /// The folder the track belongs to.
    pub parent_id: Option<String>,
    /// The folder nesting, 0 for top-level tracks, at most [`MAX_FOLDER_DEPTH`].
    pub depth: u32,
    /// Fader, pan and switches.
    pub mixer: MixerView,
    /// The bus or master the track feeds; `None` for the master.
    pub output_id: Option<String>,
    /// The outputs it may feed: the master, then the buses other than itself.
    pub output_targets: Vec<TargetView>,
    /// The sends.
    pub sends: Vec<SendView>,
    /// The buses a new send may target: neither the track itself nor already targeted.
    pub send_targets: Vec<TargetView>,
    /// The input settings.
    pub input: InputView,
    /// The instrument of an instrument track.
    pub instrument: Option<DeviceView>,
    /// The effects, in order.
    pub inserts: Vec<DeviceView>,
    /// The clips, in order.
    pub clips: Vec<ClipView>,
}

/// Fader, pan and switches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MixerView {
    /// The fader level in dB.
    pub volume_db: f64,
    /// The pan, −1 to 1.
    pub pan: f64,
    /// Muted.
    pub mute: bool,
    /// Soloed.
    pub solo: bool,
    /// Kept audible while another track is soloed.
    pub solo_safe: bool,
}

/// A routing target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetView {
    /// The track ID (`master` for the master).
    pub id: String,
    /// The name shown.
    pub name: String,
}

/// A send.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SendView {
    /// The send ID.
    pub id: String,
    /// The bus it feeds.
    pub target_id: String,
    /// The level in dB.
    pub level_db: f64,
    /// Whether it taps before the fader.
    pub pre_fader: bool,
    /// Whether it sounds (drawn dimmed when off).
    pub enabled: bool,
}

/// The input settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InputView {
    /// The input channels, 0-based.
    pub channels: Vec<u32>,
    /// Armed for recording.
    pub armed: bool,
    /// `off`, `auto` or `on`.
    pub monitor: &'static str,
}

/// An instrument or effect.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeviceView {
    /// The device ID.
    pub id: String,
    /// The plugin.
    pub plugin_id: String,
    /// The display name.
    pub name: String,
    /// On, or bypassed.
    pub enabled: bool,
    /// The preset last applied.
    pub preset: Option<String>,
    /// The sidechain source track.
    pub sidechain_id: Option<String>,
    /// Whether the plugin is installed; an unknown plugin's device is bypassed.
    pub available: bool,
    /// Every parameter's value: the stored values resolved against the descriptor's
    /// defaults, in descriptor order; the stored values as they are for an unknown plugin.
    pub params: Map<String, Value>,
}

/// A clip.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ClipView {
    /// An audio clip.
    Audio(AudioClipView),
    /// A MIDI clip.
    Midi(MidiClipView),
}

/// An audio clip.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioClipView {
    /// The clip ID.
    pub id: String,
    /// The name.
    pub name: String,
    /// The start in beats.
    pub start: f64,
    /// The length in beats.
    pub length: f64,
    /// Muted.
    pub muted: bool,
    /// `#rrggbb`, or `None` for the track's colour.
    pub color: Option<String>,
    /// The media asset.
    pub asset_id: String,
    /// The first source frame played, at the asset's rate.
    pub source_offset: i64,
    /// The clip gain in dB.
    pub gain_db: f64,
    /// The fade-in.
    pub fade_in: FadeView,
    /// The fade-out.
    pub fade_out: FadeView,
}

/// A fade.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FadeView {
    /// Its length in seconds.
    pub seconds: f64,
    /// `linear`, `equal_power`, `exponential` or `s_curve`.
    pub curve: &'static str,
}

/// A MIDI clip.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MidiClipView {
    /// The clip ID.
    pub id: String,
    /// The name.
    pub name: String,
    /// The start in beats.
    pub start: f64,
    /// The length in beats.
    pub length: f64,
    /// Muted.
    pub muted: bool,
    /// `#rrggbb`, or `None` for the track's colour.
    pub color: Option<String>,
    /// The content beat at the clip's start.
    pub content_offset: f64,
    /// The loop length of looped content, in beats.
    pub loop_length: Option<f64>,
    /// The notes, in content beats.
    pub notes: Vec<NoteView>,
}

/// A MIDI note.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteView {
    /// The note ID.
    pub id: String,
    /// The pitch, 0–127.
    pub pitch: u8,
    /// The start in content beats.
    pub start: f64,
    /// The length in beats.
    pub length: f64,
    /// The velocity, 1–127.
    pub velocity: u8,
    /// The MIDI channel, 0–15.
    pub channel: u8,
}

/// A marker.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarkerView {
    /// The marker ID.
    pub id: String,
    /// Where it is, in beats.
    pub beat: f64,
    /// The name.
    pub name: String,
    /// `#rrggbb`, or `None`.
    pub color: Option<String>,
}

/// A section.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SectionView {
    /// The section ID.
    pub id: String,
    /// The name.
    pub name: String,
    /// The start in beats.
    pub start: f64,
    /// The length in beats.
    pub length: f64,
    /// `#rrggbb`, or `None`.
    pub color: Option<String>,
}

/// A media asset.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssetView {
    /// The asset ID (the `peaks:` scheme's key).
    pub id: String,
    /// `audio`, `midi` or `impulse`.
    pub kind: &'static str,
    /// The display name.
    pub name: String,
    /// `embedded` or `linked`.
    pub location: &'static str,
    /// The file's sample rate.
    pub sample_rate: u32,
    /// The channel count.
    pub channels: u32,
    /// The frame count at the file's rate.
    pub frames: u64,
    /// The duration in seconds.
    pub duration_seconds: f64,
}

/// A plugin's descriptor.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PluginView {
    /// The plugin ID.
    pub id: String,
    /// The display name.
    pub name: String,
    /// `effect` or `instrument`.
    pub kind: &'static str,
    /// The category (`eq`, `dynamics`, …).
    pub category: String,
    /// The help text.
    pub description: String,
    /// Whether it takes a sidechain.
    pub sidechain: bool,
    /// The preset names.
    pub presets: Vec<String>,
    /// The parameters, in descriptor order.
    pub params: Vec<ParamView>,
}

/// A plugin parameter.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParamView {
    /// The parameter ID.
    pub id: String,
    /// The display name.
    pub name: String,
    /// `float`, `int`, `bool` or `enum`.
    pub kind: &'static str,
    /// The default value.
    pub default: Value,
    /// The minimum of a numeric parameter.
    pub min: Option<f64>,
    /// The maximum of a numeric parameter.
    pub max: Option<f64>,
    /// The unit (`Hz`, `dB`, …), empty for none.
    pub unit: String,
    /// The choices of an enum.
    pub choices: Vec<String>,
    /// Whether the control maps the range logarithmically.
    pub log_scale: bool,
    /// Whether it can be automated.
    pub automatable: bool,
    /// The help text.
    pub description: String,
}

/// The undo and redo labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryView {
    /// The label of the step Undo would undo.
    pub undo: Option<String>,
    /// The label of the step Redo would redo.
    pub redo: Option<String>,
}

/// Some tracks of the open project, for a refresh that changed only them ([`tracks_view`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TracksView {
    /// The session version the tracks show.
    pub version: u64,
    /// The tracks, `None` when no project is open.
    pub project: Option<OpenTracks>,
}

/// The tracks of [`TracksView`] and what every edit changes besides them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenTracks {
    /// The project ID: tracks of another project never land in a view.
    pub id: String,
    /// Whether the project differs from the saved one.
    pub dirty: bool,
    /// The undo and redo labels.
    pub history: HistoryView,
    /// The project's [`meter_layout`]; a view with another one needs a full refresh.
    pub meter_layout: u32,
    /// The asked tracks that exist, in the project's order, the master last.
    pub tracks: Vec<TrackView>,
}

/// Some MIDI clips of the open project, for a refresh that changed only their notes
/// ([`clips_view`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipsView {
    /// The session version the clips show.
    pub version: u64,
    /// The clips, `None` when no project is open.
    pub project: Option<OpenClips>,
}

/// The clips of [`ClipsView`] and what every edit changes besides them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenClips {
    /// The project ID: clips of another project never land in a view.
    pub id: String,
    /// Whether the project differs from the saved one.
    pub dirty: bool,
    /// The undo and redo labels.
    pub history: HistoryView,
    /// The project's [`meter_layout`]; a view with another one needs a full refresh.
    pub meter_layout: u32,
    /// The asked clips that exist, in the project's order, each with its track.
    pub clips: Vec<PlacedClipView>,
}

/// A clip and the track it is on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlacedClipView {
    /// The track's ID.
    pub track_id: String,
    /// The clip.
    pub clip: ClipView,
}

/// Some devices of the open project, for a refresh that changed only their state
/// ([`devices_view`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DevicesView {
    /// The session version the devices show.
    pub version: u64,
    /// The devices, `None` when no project is open.
    pub project: Option<OpenDevices>,
}

/// The devices of [`DevicesView`] and what every edit changes besides them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenDevices {
    /// The project ID: devices of another project never land in a view.
    pub id: String,
    /// Whether the project differs from the saved one.
    pub dirty: bool,
    /// The undo and redo labels.
    pub history: HistoryView,
    /// The project's [`meter_layout`]; a view with another one needs a full refresh.
    pub meter_layout: u32,
    /// The asked devices that exist, in the project's order (each track's instrument first, the
    /// master's last), each with its track.
    pub devices: Vec<PlacedDeviceView>,
}

/// A device and the track it is on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlacedDeviceView {
    /// The track's ID.
    pub track_id: String,
    /// The device, its parameters resolved as in the full view.
    pub device: DeviceView,
}

/// The views of the devices `device_ids` of `session` now; unknown IDs are left out. Each device
/// is built under the session lock, as [`tracks_view`] builds its tracks; the project is not
/// copied.
#[must_use]
pub fn devices_view(
    session: &SharedSession,
    plugins: &PluginRegistry,
    device_ids: &[String],
) -> DevicesView {
    let session = session.lock();
    let project = session.project().map(|project| {
        let history = session.history();
        OpenDevices {
            id: project.id.clone(),
            dirty: session.is_dirty(),
            history: HistoryView {
                undo: history.undo_label().map(str::to_owned),
                redo: history.redo_label().map(str::to_owned),
            },
            meter_layout: meter_layout(project),
            devices: project
                .all_tracks()
                .flat_map(|track| {
                    track
                        .devices()
                        .filter(|device| device_ids.iter().any(|id| *id == device.id))
                        .map(|device| PlacedDeviceView {
                            track_id: track.id.clone(),
                            device: DeviceView::of(device, plugins),
                        })
                })
                .collect(),
        }
    });
    DevicesView {
        version: session.version(),
        project,
    }
}

/// The views of the clips `clip_ids` of `session` now; unknown IDs are left out. Each clip is
/// built under the session lock, as [`tracks_view`] builds its tracks; the project is not copied.
#[must_use]
pub fn clips_view(session: &SharedSession, clip_ids: &[String]) -> ClipsView {
    let session = session.lock();
    let project = session.project().map(|project| {
        let history = session.history();
        OpenClips {
            id: project.id.clone(),
            dirty: session.is_dirty(),
            history: HistoryView {
                undo: history.undo_label().map(str::to_owned),
                redo: history.redo_label().map(str::to_owned),
            },
            meter_layout: meter_layout(project),
            clips: project
                .all_tracks()
                .flat_map(|track| {
                    track
                        .clips
                        .iter()
                        .filter(|clip| clip_ids.iter().any(|id| id == clip.id()))
                        .map(|clip| PlacedClipView {
                            track_id: track.id.clone(),
                            clip: ClipView::of(clip),
                        })
                })
                .collect(),
        }
    });
    ClipsView {
        version: session.version(),
        project,
    }
}

/// The views of the tracks `track_ids` of `session` now (the master by its ID); unknown IDs are
/// left out. Each track is built under the session lock, which a few tracks keep for
/// microseconds; the project is not copied.
#[must_use]
pub fn tracks_view(
    session: &SharedSession,
    plugins: &PluginRegistry,
    track_ids: &[String],
) -> TracksView {
    let session = session.lock();
    let project = session.project().map(|project| {
        let history = session.history();
        OpenTracks {
            id: project.id.clone(),
            dirty: session.is_dirty(),
            history: HistoryView {
                undo: history.undo_label().map(str::to_owned),
                redo: history.redo_label().map(str::to_owned),
            },
            meter_layout: meter_layout(project),
            tracks: project
                .all_tracks()
                .filter(|track| track_ids.iter().any(|id| *id == track.id))
                .map(|track| TrackView::of(project, track, plugins))
                .collect(),
        }
    });
    TracksView {
        version: session.version(),
        project,
    }
}

/// The view of `session` now; the session is locked only to copy its snapshot.
#[must_use]
pub fn project_view(session: &SharedSession, plugins: &PluginRegistry) -> ProjectView {
    let (version, open) = {
        let session = session.lock();
        let open = session.project().map(|project| {
            let location = session.location();
            let history = session.history();
            (
                project.clone(),
                location.and_then(|l| l.path.as_ref().map(|p| p.display().to_string())),
                session.is_dirty(),
                HistoryView {
                    undo: history.undo_label().map(str::to_owned),
                    redo: history.redo_label().map(str::to_owned),
                },
            )
        });
        (session.version(), open)
    };
    ProjectView {
        version,
        project: open.map(|(project, path, dirty, history)| {
            OpenProject::of(&project, plugins, path, dirty, history)
        }),
    }
}

impl OpenProject {
    fn of(
        project: &Project,
        plugins: &PluginRegistry,
        path: Option<String>,
        dirty: bool,
        history: HistoryView,
    ) -> Self {
        let mut used = BTreeMap::new();
        for device in project.all_tracks().flat_map(Track::devices) {
            if let Some(descriptor) = plugins.find(&device.plugin_id) {
                used.entry(descriptor.id.clone())
                    .or_insert_with(|| PluginView::of(descriptor));
            }
        }
        let transport = &project.transport;
        let metadata = &project.metadata;
        Self {
            id: project.id.clone(),
            name: project.name.clone(),
            path,
            dirty,
            sample_rate: project.settings.sample_rate,
            length_beats: project.settings.length_beats,
            end_beat: project.end_beat(),
            tempo: project
                .tempo_map
                .points()
                .iter()
                .map(|p| TempoView {
                    beat: p.beat,
                    bpm: p.bpm,
                    ramp: p.ramp,
                })
                .collect(),
            time_signatures: project
                .time_signatures
                .points()
                .iter()
                .map(|p| TimeSignatureView {
                    bar: p.bar,
                    beat: project.time_signatures.bar_to_beat(bar_f64(p.bar)),
                    numerator: p.numerator,
                    denominator: p.denominator,
                    beats_per_bar: p.beats_per_bar(),
                })
                .collect(),
            transport: TransportView {
                loop_enabled: transport.loop_enabled,
                loop_start: transport.loop_start,
                loop_end: transport.loop_end,
                punch_enabled: transport.punch_enabled,
                punch_in: transport.punch_in,
                punch_out: transport.punch_out,
                metronome: transport.metronome,
                count_in_bars: transport.count_in_bars,
            },
            metadata: MetadataView {
                title: metadata.title.clone(),
                artist: metadata.artist.clone(),
                album: metadata.album.clone(),
                genre: metadata.genre.clone(),
                composer: metadata.composer.clone(),
                comment: metadata.comment.clone(),
            },
            tracks: project
                .tracks
                .iter()
                .map(|t| TrackView::of(project, t, plugins))
                .collect(),
            master: TrackView::of(project, &project.master, plugins),
            markers: project
                .markers
                .iter()
                .map(|m| MarkerView {
                    id: m.id.clone(),
                    beat: m.beat,
                    name: m.name.clone(),
                    color: m.color.clone(),
                })
                .collect(),
            sections: project
                .sections
                .iter()
                .map(|s| SectionView {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    start: s.start,
                    length: s.length,
                    color: s.color.clone(),
                })
                .collect(),
            assets: project.assets.iter().map(AssetView::of).collect(),
            plugins: used,
            history,
            meter_layout: meter_layout(project),
        }
    }
}

/// A bar number as the time-signature map takes it.
#[expect(
    clippy::cast_precision_loss,
    reason = "bar numbers stay far below 2^53"
)]
fn bar_f64(bar: i64) -> f64 {
    bar as f64
}

impl TrackView {
    fn of(project: &Project, track: &Track, plugins: &PluginRegistry) -> Self {
        let is_master = track.kind == TrackKind::Master;
        let buses = || project.buses().filter(|bus| bus.id != track.id);
        let output_targets = if is_master {
            Vec::new()
        } else {
            std::iter::once(TargetView {
                id: ids::MASTER_ID.to_owned(),
                name: "Master".to_owned(),
            })
            .chain(buses().map(TargetView::of))
            .collect()
        };
        let send_targets = if is_master {
            Vec::new()
        } else {
            buses()
                .filter(|bus| !track.sends.iter().any(|send| send.target_id == bus.id))
                .map(TargetView::of)
                .collect()
        };
        Self {
            id: track.id.clone(),
            name: track.name.clone(),
            kind: track.kind.as_str(),
            color: track.color.clone(),
            parent_id: track.parent_id.clone(),
            depth: folder_depth(project, track),
            mixer: MixerView {
                volume_db: track.mixer.volume_db,
                pan: track.mixer.pan,
                mute: track.mixer.mute,
                solo: track.mixer.solo,
                solo_safe: track.mixer.solo_safe,
            },
            output_id: track.output_id.clone(),
            output_targets,
            sends: track
                .sends
                .iter()
                .map(|send| SendView {
                    id: send.id.clone(),
                    target_id: send.target_id.clone(),
                    level_db: send.level_db,
                    pre_fader: send.pre_fader,
                    enabled: send.enabled,
                })
                .collect(),
            send_targets,
            input: InputView {
                channels: track.input.channels.clone(),
                armed: track.input.armed,
                monitor: track.input.monitor.as_str(),
            },
            instrument: track
                .instrument
                .as_ref()
                .map(|device| DeviceView::of(device, plugins)),
            inserts: track
                .inserts
                .iter()
                .map(|device| DeviceView::of(device, plugins))
                .collect(),
            clips: track.clips.iter().map(ClipView::of).collect(),
        }
    }
}

/// How deep `track` is nested in folders, capped at [`MAX_FOLDER_DEPTH`] (which also ends a
/// cycle, which the model never stores).
fn folder_depth(project: &Project, track: &Track) -> u32 {
    let mut depth = 0;
    let mut parent = track.parent_id.as_deref();
    while let Some(id) = parent
        && depth < MAX_FOLDER_DEPTH
    {
        depth += 1;
        parent = project.find_track(id).and_then(|t| t.parent_id.as_deref());
    }
    depth
}

impl TargetView {
    fn of(track: &Track) -> Self {
        Self {
            id: track.id.clone(),
            name: track.name.clone(),
        }
    }
}

impl DeviceView {
    fn of(device: &Device, plugins: &PluginRegistry) -> Self {
        let descriptor = plugins.find(&device.plugin_id);
        let params = match descriptor {
            Some(descriptor) => descriptor.resolve(&device.params),
            None => device.params.clone(),
        };
        Self {
            id: device.id.clone(),
            plugin_id: device.plugin_id.clone(),
            name: device.name.clone(),
            enabled: device.enabled,
            preset: device.preset.clone(),
            sidechain_id: device.sidechain_id.clone().filter(|id| !id.is_empty()),
            available: descriptor.is_some(),
            params: params
                .iter()
                .map(|(name, value)| (name.clone(), Value::from(value)))
                .collect(),
        }
    }
}

impl FadeView {
    fn of(fade: &Fade) -> Self {
        Self {
            seconds: fade.seconds,
            curve: fade.curve.as_str(),
        }
    }
}

impl ClipView {
    fn of(clip: &Clip) -> Self {
        match clip {
            Clip::Audio(audio) => Self::Audio(AudioClipView {
                id: audio.id.clone(),
                name: audio.name.clone(),
                start: audio.start,
                length: audio.length,
                muted: audio.muted,
                color: audio.color.clone(),
                asset_id: audio.asset_id.clone(),
                source_offset: audio.source_offset,
                gain_db: audio.gain_db,
                fade_in: FadeView::of(&audio.fade_in),
                fade_out: FadeView::of(&audio.fade_out),
            }),
            Clip::Midi(midi) => Self::Midi(MidiClipView {
                id: midi.id.clone(),
                name: midi.name.clone(),
                start: midi.start,
                length: midi.length,
                muted: midi.muted,
                color: midi.color.clone(),
                content_offset: midi.content_offset,
                loop_length: midi.loop_length,
                notes: midi
                    .notes
                    .iter()
                    .map(|note| NoteView {
                        id: note.id.clone(),
                        pitch: note.pitch,
                        start: note.start,
                        length: note.length,
                        velocity: note.velocity,
                        channel: note.channel,
                    })
                    .collect(),
            }),
        }
    }
}

impl AssetView {
    fn of(asset: &MediaAsset) -> Self {
        Self {
            id: asset.id.clone(),
            kind: asset.kind.as_str(),
            name: asset.name.clone(),
            location: asset.location.as_str(),
            sample_rate: asset.sample_rate,
            channels: asset.channels,
            frames: asset.frames,
            duration_seconds: asset.duration_seconds(),
        }
    }
}

impl PluginView {
    fn of(descriptor: &PluginDescriptor) -> Self {
        Self {
            id: descriptor.id.clone(),
            name: descriptor.name.clone(),
            kind: descriptor.kind.as_str(),
            category: descriptor.category.clone(),
            description: descriptor.description.clone(),
            sidechain: descriptor.sidechain,
            presets: descriptor.presets.iter().map(|p| p.name.clone()).collect(),
            params: descriptor.params.iter().map(ParamView::of).collect(),
        }
    }
}

impl ParamView {
    fn of(spec: &ParamSpec) -> Self {
        Self {
            id: spec.id.clone(),
            name: spec.name.clone(),
            kind: spec.kind.as_str(),
            default: Value::from(&spec.default),
            min: spec.minimum,
            max: spec.maximum,
            unit: spec.unit.clone(),
            choices: spec.choices.clone(),
            log_scale: spec.scale == ParamScale::Log,
            automatable: spec.automatable,
            description: spec.description.clone(),
        }
    }
}

/// The identity of a project's meter order: a 32-bit FNV-1a hash of the track IDs in order,
/// then the master's, each followed by its device IDs (instrument first). A live frame
/// carries the layout it was packed in, and the page uses its values only when the layout
/// equals its view's, so the meters never land on the wrong strip after a track or device was
/// added, removed or moved. Edits that keep the order (a fader move) keep the layout, so the
/// meters keep flowing while the page fetches the next view.
#[must_use]
pub fn meter_layout(project: &Project) -> u32 {
    const OFFSET: u32 = 0x811c_9dc5;
    const PRIME: u32 = 0x0100_0193;
    let mut hash = OFFSET;
    let mut feed = |text: &str| {
        // A separator byte that IDs never contain keeps "ab"+"c" apart from "a"+"bc".
        for byte in text.bytes().chain(std::iter::once(0xff)) {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    };
    for track in project.all_tracks() {
        feed(&track.id);
        for device in track.devices() {
            feed(&device.id);
        }
        feed("");
    }
    hash
}
