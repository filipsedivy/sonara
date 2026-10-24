//! Shared by the player's integration tests: a device whose callback the test drives, the
//! golden projects, and a harness that drives a manual player step by step.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use sonara_core::model::JsonMap;
use sonara_core::model::project::Project;
use sonara_core::plugins::builtin_registry;
use sonara_engine::player::{
    AudioDevice, DeviceError, DeviceStream, Player, PlayerSettings, PlayerSetup, PrefetchWorker,
    ProjectSnapshot, RenderLoop, Step, StreamRequest,
};
use sonara_media::MediaProvider;
use sonara_project::serializer::{loads, project_from_value};
use sonara_rt::{DeviceCallback, LatencyFrames, Stream, StreamConfig, StreamControl};
use sonara_testkit::{Golden, sha256_hex};
use tempfile::TempDir;

/// The device callback of the stream a [`ManualDevice`] opened, its control handle and its
/// input channel count, for the test to drive.
pub type Callback = Arc<Mutex<Option<(DeviceCallback, StreamControl, usize)>>>;

/// A device whose callback the test calls: the stream exists, but no thread consumes it. It
/// reports the format and latency of miniaudio's null device (48 kHz, 256-frame period, 768
/// frames of output latency and, with input, 768 of input latency), as the prototype's
/// `native-null` backend obtained them.
pub struct ManualDevice {
    callback: Callback,
}

impl ManualDevice {
    pub fn new() -> (Self, Callback) {
        let callback: Callback = Arc::new(Mutex::new(None));
        (
            Self {
                callback: Arc::clone(&callback),
            },
            callback,
        )
    }
}

impl AudioDevice for ManualDevice {
    fn start(&mut self, request: &StreamRequest) -> Result<DeviceStream, DeviceError> {
        self.stop();
        let stream = Stream::new(&StreamConfig {
            output_channels: request.output_channels,
            input_channels: request.input_channels,
            block_frames: 512,
            ring_blocks: request.ring_blocks,
            capture_frames: request.capture_frames,
        })
        .map_err(|e| DeviceError(e.to_string()))?;
        let Stream {
            producer,
            callback,
            capture,
            control,
        } = stream;
        control.set_device_format(request.sample_rate, request.buffer_frames);
        let input_latency = if request.input_channels > 0 {
            request.buffer_frames * 3
        } else {
            0
        };
        control.set_latency(
            LatencyFrames::new(request.buffer_frames * 3),
            LatencyFrames::new(input_latency),
        );
        control.set_running(true);
        *self.callback.lock().expect("an unpoisoned lock") =
            Some((callback, control.clone(), request.input_channels));
        Ok(DeviceStream {
            sample_rate: request.sample_rate,
            producer,
            capture,
            control,
        })
    }

    fn stop(&mut self) {
        if let Some((_, control, _)) = self.callback.lock().expect("an unpoisoned lock").take() {
            control.set_running(false);
            control.set_playing(false);
        }
    }
}

/// A golden project from `rel`, after `edit` on its JSON.
pub fn golden_project(golden: &Golden, rel: &str, edit: impl FnOnce(&mut JsonMap)) -> Project {
    let mut data = loads(&golden.bytes(rel).expect("the project")).expect("project JSON");
    edit(&mut data);
    project_from_value(&data).expect("the project loads")
}

/// A media provider that finds nothing (projects without audio clips).
pub fn no_media() -> Arc<MediaProvider> {
    Arc::new(MediaProvider::new(
        |_| PathBuf::from("/nonexistent/sonara-media"),
        None,
    ))
}

/// The coverage batch's media, resolved as the recorder's sandbox saw them, with a decode cache
/// in a temporary directory (as `golden_render.rs` wires them).
pub fn coverage_media(golden: &Golden, project: &Project) -> (Arc<MediaProvider>, TempDir) {
    let fixtures = golden.root().join("fixtures");
    for asset in &project.assets {
        let bytes = golden
            .bytes(&format!("fixtures/{}", asset.name))
            .unwrap_or_else(|e| panic!("asset {} has no fixture: {e}", asset.id));
        if let Some(sha) = &asset.sha256 {
            assert_eq!(&sha256_hex(&bytes), sha, "fixture of asset {}", asset.id);
        }
    }
    let cache = TempDir::new().expect("a cache directory");
    let media = MediaProvider::new(
        move |asset| -> PathBuf { fixtures.join(&asset.name) },
        Some(cache.path().to_owned()),
    );
    (Arc::new(media), cache)
}

/// The tracks array of a project's JSON.
pub fn tracks(data: &mut JsonMap) -> &mut Vec<Value> {
    data.get_mut("tracks")
        .and_then(Value::as_array_mut)
        .expect("a track list")
}

/// A block the player wrote.
#[derive(Debug, Clone)]
pub struct Played {
    pub tag: i64,
    pub generation: u32,
    pub samples: Vec<f32>,
    pub parts: Vec<(i64, usize)>,
    pub clicks: Option<Vec<(i64, bool)>>,
    pub idle: bool,
}

/// A manual player and the parts of its threads, driven by hand.
pub struct Harness {
    pub player: Player,
    pub render: RenderLoop,
    pub prefetch: PrefetchWorker,
    pub callback: Callback,
    pub snapshot: ProjectSnapshot,
    /// What the player's maintenance sees as the session: [`Harness::edit`] changes it.
    pub session: Arc<Mutex<ProjectSnapshot>>,
    pub played: Vec<Played>,
    /// Whether the harness reads the clips ahead before each step (the prefetch thread).
    pub prefetching: bool,
    /// Where recordings put their take files.
    pub scratch: TempDir,
}

impl Harness {
    pub fn new(project: Project, media: Arc<MediaProvider>, settings: PlayerSettings) -> Self {
        let snapshot = ProjectSnapshot {
            project,
            version: 1,
            media,
        };
        let session = Arc::new(Mutex::new(snapshot.clone()));
        let source = {
            let session = Arc::clone(&session);
            move || Some(session.lock().expect("an unpoisoned lock").clone())
        };
        let (device, callback) = ManualDevice::new();
        let (player, render, prefetch) = Player::manual(PlayerSetup {
            registry: Arc::new(builtin_registry()),
            device: Box::new(device),
            source: Arc::new(source),
            settings,
        });
        Self {
            player,
            render,
            prefetch,
            callback,
            snapshot,
            session,
            played: Vec::new(),
            prefetching: true,
            scratch: TempDir::new().expect("a directory for takes"),
        }
    }

    /// An edit of the session's project: its JSON changed by `edit`, at the next version.
    pub fn edit(&mut self, edit: impl FnOnce(&mut JsonMap)) {
        let mut value = sonara_project::serializer::project_to_value(&self.snapshot.project);
        let data = value.as_object_mut().expect("a project object");
        edit(data);
        self.snapshot.project = project_from_value(data).expect("the edited project loads");
        self.snapshot.version += 1;
        *self.session.lock().expect("an unpoisoned lock") = self.snapshot.clone();
    }

    pub fn project(&self) -> &Project {
        &self.snapshot.project
    }

    /// The stream's control handle.
    pub fn control(&self) -> StreamControl {
        self.callback
            .lock()
            .expect("an unpoisoned lock")
            .as_ref()
            .map(|(_, control, _)| control.clone())
            .expect("a running stream")
    }

    /// One device callback of `frames` frames; returns the output. A duplex stream captures
    /// silence, as miniaudio's null device does.
    pub fn callback(&self, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        if let Some((callback, _, inputs)) =
            self.callback.lock().expect("an unpoisoned lock").as_mut()
        {
            let fill = callback.fill(&mut out);
            if *inputs > 0 {
                callback.capture(&vec![0.0; frames * *inputs], &fill);
            }
        }
        out
    }

    /// One duplex device callback: plays `input.len() / channels` frames and captures `input`
    /// (interleaved, the stream's input channels), as the device callback does after its
    /// fill. Returns the fill.
    pub fn duplex(&self, input: &[f32], channels: usize) -> sonara_rt::Fill {
        let mut out = vec![0.0; input.len() / channels * 2];
        let mut guard = self.callback.lock().expect("an unpoisoned lock");
        let (callback, _, _) = guard.as_mut().expect("a running stream");
        let fill = callback.fill(&mut out);
        callback.capture(input, &fill);
        fill
    }

    /// One render step (after the prefetch thread's work); records a written block.
    pub fn step(&mut self) -> Step {
        if self.prefetching {
            self.prefetch.fill();
        }
        // RT-02: the render step allocates and frees nothing (the binary's `AllocDisabler`).
        let step = assert_no_alloc::assert_no_alloc(|| self.render.step());
        if step == Step::Wrote {
            let block = self.render.last_block().expect("a written block");
            self.played.push(Played {
                tag: block.tag(),
                generation: block.generation().get(),
                samples: block.samples().to_vec(),
                parts: block.parts().collect(),
                clicks: block.clicks().map(Iterator::collect),
                idle: block.is_idle(),
            });
        }
        step
    }

    /// Steps until `blocks` more blocks are written; when the ring is at its target, the
    /// device plays a period and the maintenance runs, as the recorder's loop slept while the
    /// null device played (`_Run.render`).
    pub fn render(&mut self, blocks: usize) {
        let target = self.played.len() + blocks;
        let mut idle_rounds = 0;
        while self.played.len() < target {
            if self.step() == Step::Wrote {
                idle_rounds = 0;
            } else {
                self.callback(256);
                self.player.service();
                idle_rounds += 1;
                assert!(idle_rounds < 10_000, "the player stopped writing blocks");
            }
        }
    }

    /// Steps until the transport stops by itself (the end of a play range).
    pub fn render_until_stopped(&mut self) {
        let mut rounds = 0;
        while self.player.state() != sonara_engine::player::PlayerState::Stopped {
            if self.step() != Step::Wrote {
                self.callback(256);
                self.player.service();
            }
            rounds += 1;
            assert!(rounds < 100_000, "the play range never ended");
        }
    }
}

/// The samples of `played` blocks, concatenated.
pub fn concat(played: &[Played]) -> Vec<f32> {
    played
        .iter()
        .flat_map(|p| p.samples.iter().copied())
        .collect()
}

/// `x as f32` for a stream of `f64` frames (NumPy's `astype(np.float32)`).
#[expect(clippy::cast_possible_truncation, reason = "the cast under test")]
pub fn to_f32(frames: &[[f64; 2]]) -> Vec<f32> {
    frames.iter().flatten().map(|&x| x as f32).collect()
}

/// The first sample where `a` and `b` differ in their bits (or in length); `None` when they are
/// bit for bit equal.
pub fn first_difference(a: &[f32], b: &[f32]) -> Option<usize> {
    a.iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
        .or_else(|| (a.len() != b.len()).then_some(a.len().min(b.len())))
}
