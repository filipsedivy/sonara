//! Waveform peak pyramids for drawing audio clips (the prototype's `ui/timeline/waveforms.py`).
//!
//! **The pyramid.** Level 0 holds the minimum and maximum of every [`BIN_FRAMES`] frames, mixed
//! over the channels; each further level groups [`FACTOR`] bins of the one below, until a level
//! has at most [`MIN_LEVEL_BINS`] bins. The frames are read through the open workspace's media
//! provider at the project rate, so the peaks show the media exactly as the renderer hears it
//! (decoded and resampled). [`compute_level0`] and [`build_levels`] are the prototype's
//! functions; their golden values come from the prototype (`tests/fixtures/peaks_level0.json`).
//!
//! **The cache** ([`PeakCache`]) computes pyramids on two worker threads of its own
//! (`sonara-peaks-<n>`, the prototype's `sonara-peaks` pool), never on the caller's thread: a
//! request answers at once with the pyramid, "pending" or "failed", and a [`PeaksReady`] notice
//! follows when a pending one is done. Level 0 is persisted under
//! `<workspace>/cache/peaks/<asset>-<sha12>-<rate>.peaks` in this module's own format
//! ([`encode_level0`]); the coarser levels are derived on load. The cache folder is regenerable
//! and never packaged, so the prototype's `.npy` files are no contract and are ignored. A pyramid
//! of missing media is all zeros, flagged `missing`, and never persisted.
//!
//! The workers are not the job manager's: a pyramid per audio asset would count against its 16
//! pending jobs, show in the status bar and in the agent's `list_operations`, none of which the
//! prototype's pool did (CTR-01).
//!
//! **Tiles** ([`PeakCache::tile`]) cut a level into pieces of [`TILE_BINS`] bins for the
//! desktop's `peaks:` URI scheme ([`encode_tile`]).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crossbeam_channel::Receiver as TaskReceiver;
use serde::Serialize;
use sonara_core::error::DomainError;
use sonara_core::model::media::MediaAsset;
use sonara_media::{MediaError, MediaProvider, MediaReader};

use crate::media::MediaService;
use crate::session::SharedSession;
use crate::sync::lock;

/// Frames per bin of level 0 (`LEVEL0_BIN`).
pub const BIN_FRAMES: u64 = 256;
/// How many bins of a level make one bin of the next (`FACTOR`).
pub const FACTOR: usize = 4;
/// A level with at most this many bins is the last (`build_levels`).
pub const MIN_LEVEL_BINS: usize = 64;
/// Bins per tile of the `peaks:` scheme.
pub const TILE_BINS: usize = 4096;
/// Frames read per lock of the shared media reader: a multiple of [`BIN_FRAMES`], so the bins
/// equal the prototype's 512 Ki-frame reads, and short enough that the player's prefetch thread,
/// which shares the reader, never waits long.
const READ_FRAMES: usize = 64 * 1024;
/// The worker threads (the prototype's pool of 2).
const WORKERS: usize = 2;

/// The magic number of a persisted level 0.
const FILE_MAGIC: [u8; 4] = *b"SNPK";
/// The format version of a persisted level 0.
const FILE_VERSION: u16 = 1;
/// The header of a persisted level 0: magic, version, reserved, bin frames, sample rate,
/// frames, bins.
const FILE_HEADER: usize = 4 + 2 + 2 + 4 + 4 + 8 + 8;
/// The format version of a tile.
pub const TILE_VERSION: u16 = 1;
/// The header of a tile (see [`encode_tile`]).
pub const TILE_HEADER: usize = 32;

/// One bin: the minimum and the maximum.
pub type Bin = [f32; 2];

/// A waveform peak pyramid (`PeakPyramid`).
#[derive(Debug, Clone, PartialEq)]
pub struct PeakPyramid {
    /// The rate the media was read at (the project's).
    pub sample_rate: u32,
    /// The media's frames at that rate.
    pub frames: u64,
    /// The levels, finest first; level `k` has one bin per `BIN_FRAMES · FACTOR^k` frames.
    pub levels: Vec<Vec<Bin>>,
    /// Whether the media is missing or unreadable (the pyramid is silence).
    pub missing: bool,
}

impl PeakPyramid {
    /// The frames per bin of `level` (`bin_size`).
    #[must_use]
    pub fn bin_frames(level: usize) -> u64 {
        let factor = u32::try_from(FACTOR).unwrap_or(4);
        let level = u32::try_from(level).unwrap_or(u32::MAX);
        BIN_FRAMES.saturating_mul(u64::from(factor).saturating_pow(level))
    }
}

/// `compute_level0`: the minimum and maximum per [`BIN_FRAMES`] frames over all channels of
/// the first `frames` frames of `reader`, which is locked for one read of at most 64 Ki frames
/// at a time. A partial last bin covers only the frames there are. `cancelled` is asked between
/// reads.
///
/// # Errors
///
/// The reader's I/O and decode errors; [`MediaError::Cancelled`] when `cancelled` says so.
pub fn compute_level0(
    reader: &Mutex<MediaReader>,
    frames: u64,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Bin>, MediaError> {
    let channels = lock(reader).channels();
    let bins = usize::try_from(frames.div_ceil(BIN_FRAMES)).unwrap_or(usize::MAX);
    let mut out = Vec::with_capacity(bins);
    if channels == 0 {
        out.resize(bins, [0.0; 2]);
        return Ok(out);
    }
    let bin = usize::try_from(BIN_FRAMES).unwrap_or(256);
    let mut buffer = vec![0.0; READ_FRAMES * channels];
    let mut pos: u64 = 0;
    while pos < frames {
        if cancelled() {
            return Err(MediaError::Cancelled);
        }
        let n = usize::try_from(frames - pos)
            .unwrap_or(usize::MAX)
            .min(READ_FRAMES);
        let window = &mut buffer[..n * channels];
        {
            let start = i64::try_from(pos).unwrap_or(i64::MAX);
            lock(reader).read_into(start, window)?;
        }
        for chunk in window.chunks(bin * channels) {
            let (low, high) = chunk.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(low, high), &sample| (low.min(sample), high.max(sample)),
            );
            // NumPy computes in f64 and stores into a float32 array.
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the prototype's float32 bins"
            )]
            out.push([low as f32, high as f32]);
        }
        pos += n as u64;
    }
    Ok(out)
}

/// `build_levels`: level 0, then one level per [`FACTOR`] bins of the one below (the minimum of
/// the minima and the maximum of the maxima) while the level above has more than
/// [`MIN_LEVEL_BINS`] bins. A partial last group repeats the level's last bin.
#[must_use]
pub fn build_levels(finest: Vec<Bin>) -> Vec<Vec<Bin>> {
    let mut levels = vec![finest];
    while let Some(current) = levels.last()
        && current.len() > MIN_LEVEL_BINS
    {
        let next: Vec<Bin> = current
            .chunks(FACTOR)
            .map(|group| {
                group
                    .iter()
                    .fold([f32::INFINITY, f32::NEG_INFINITY], |[low, high], b| {
                        [low.min(b[0]), high.max(b[1])]
                    })
            })
            .collect();
        levels.push(next);
    }
    levels
}

/// A persisted level 0: `SNPK`, the format version (u16), 0 (u16), [`BIN_FRAMES`] (u32), the
/// sample rate (u32), the frames (u64), the bin count (u64), then each bin's minimum and
/// maximum (f32); all little-endian.
#[must_use]
pub fn encode_level0(sample_rate: u32, frames: u64, level0: &[Bin]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FILE_HEADER + level0.len() * 8);
    out.extend_from_slice(&FILE_MAGIC);
    out.extend_from_slice(&FILE_VERSION.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&u32::try_from(BIN_FRAMES).unwrap_or(256).to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&frames.to_le_bytes());
    out.extend_from_slice(&(level0.len() as u64).to_le_bytes());
    for [low, high] in level0 {
        out.extend_from_slice(&low.to_le_bytes());
        out.extend_from_slice(&high.to_le_bytes());
    }
    out
}

/// The level 0 of `bytes` ([`encode_level0`]) when it holds `frames` frames at `sample_rate`;
/// `None` for another format, version, rate or length, or a damaged file.
#[must_use]
pub fn decode_level0(bytes: &[u8], sample_rate: u32, frames: u64) -> Option<Vec<Bin>> {
    let (header, body) = bytes.split_at_checked(FILE_HEADER)?;
    let field = |range: std::ops::Range<usize>| header.get(range);
    let u16_at = |at: usize| Some(u16::from_le_bytes(field(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_le_bytes(field(at..at + 4)?.try_into().ok()?));
    let u64_at = |at: usize| Some(u64::from_le_bytes(field(at..at + 8)?.try_into().ok()?));
    let valid = field(0..4)? == FILE_MAGIC
        && u16_at(4)? == FILE_VERSION
        && u64::from(u32_at(8)?) == BIN_FRAMES
        && u32_at(12)? == sample_rate
        && u64_at(16)? == frames
        && u64_at(24)? == frames.div_ceil(BIN_FRAMES)
        && body.len() as u64 == u64_at(24)?.checked_mul(8)?;
    if !valid {
        return None;
    }
    let (bins, _) = body.as_chunks::<8>();
    Some(
        bins.iter()
            .map(|bin| {
                let (low, high) = bin.split_at(4);
                [
                    f32::from_le_bytes(low.try_into().unwrap_or_default()),
                    f32::from_le_bytes(high.try_into().unwrap_or_default()),
                ]
            })
            .collect(),
    )
}

/// A tile of the `peaks:` scheme: a header of [`TILE_HEADER`] bytes, then the bins.
///
/// | Offset | Type | Field |
/// |---|---|---|
/// | 0 | u16 | [`TILE_VERSION`] |
/// | 2 | u16 | the level |
/// | 4 | u32 | the tile's index in the level |
/// | 8 | u32 | the bins in this tile |
/// | 12 | u32 | the bins in the level |
/// | 16 | u32 | the frames per bin of the level |
/// | 20 | u32 | the sample rate |
/// | 24 | u64 | the media's frames |
///
/// Then each bin's minimum and maximum as f32; all little-endian. `None` past the last tile.
#[must_use]
pub fn encode_tile(pyramid: &PeakPyramid, level: usize, index: usize) -> Option<Vec<u8>> {
    let bins = pyramid.levels.get(level)?;
    let first = index.checked_mul(TILE_BINS)?;
    if first >= bins.len() && !(index == 0 && bins.is_empty()) {
        return None;
    }
    let tile = bins.get(first..(first + TILE_BINS).min(bins.len()))?;
    let as_u32 = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(TILE_HEADER + tile.len() * 8);
    out.extend_from_slice(&TILE_VERSION.to_le_bytes());
    out.extend_from_slice(&u16::try_from(level).unwrap_or(u16::MAX).to_le_bytes());
    out.extend_from_slice(&as_u32(index).to_le_bytes());
    out.extend_from_slice(&as_u32(tile.len()).to_le_bytes());
    out.extend_from_slice(&as_u32(bins.len()).to_le_bytes());
    let bin_frames = u32::try_from(PeakPyramid::bin_frames(level)).unwrap_or(u32::MAX);
    out.extend_from_slice(&bin_frames.to_le_bytes());
    out.extend_from_slice(&pyramid.sample_rate.to_le_bytes());
    out.extend_from_slice(&pyramid.frames.to_le_bytes());
    for [low, high] in tile {
        out.extend_from_slice(&low.to_le_bytes());
        out.extend_from_slice(&high.to_le_bytes());
    }
    Some(out)
}

/// A tile request of the `peaks:` scheme: `/tile/<asset>/<sample rate>/<level>/<index>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileRequest {
    /// The asset.
    pub asset_id: String,
    /// The rate the media was read at.
    pub sample_rate: u32,
    /// The level, 0 for the finest.
    pub level: usize,
    /// The tile's index in the level.
    pub index: usize,
}

impl TileRequest {
    /// The request of the URL path `path`; `None` for any other path.
    #[must_use]
    pub fn parse(path: &str) -> Option<Self> {
        let mut parts = path.strip_prefix("/tile/")?.split('/');
        let asset_id = parts.next().filter(|id| !id.is_empty())?.to_owned();
        let sample_rate = parts.next()?.parse().ok()?;
        let level = parts.next()?.parse().ok()?;
        let index = parts.next()?.parse().ok()?;
        parts.next().is_none().then_some(Self {
            asset_id,
            sample_rate,
            level,
            index,
        })
    }
}

/// What a pyramid request answers.
#[derive(Debug, Clone, PartialEq)]
pub enum PeaksState {
    /// The pyramid.
    Ready(Arc<PeakPyramid>),
    /// It is being computed; a [`PeaksReady`] notice follows.
    Pending,
    /// It could not be computed (the error was logged).
    Failed,
}

/// What the page needs to fetch a ready pyramid's tiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PeaksInfo {
    /// `ready`, `pending` or `failed`.
    pub state: &'static str,
    /// The rate the media was read at.
    pub sample_rate: u32,
    /// The media's frames at that rate (0 unless ready).
    pub frames: u64,
    /// Whether the media is missing ("media missing" on the clip).
    pub missing: bool,
    /// The bins of each level, finest first (empty unless ready).
    pub level_bins: Vec<u64>,
    /// The frames per bin of level 0; each level multiplies it by [`FACTOR`].
    pub bin_frames: u64,
    /// The bins per tile.
    pub tile_bins: u64,
}

impl PeaksInfo {
    /// The info of `state` for media read at `sample_rate`.
    #[must_use]
    pub fn of(state: &PeaksState, sample_rate: u32) -> Self {
        let mut info = Self {
            state: "pending",
            sample_rate,
            frames: 0,
            missing: false,
            level_bins: Vec::new(),
            bin_frames: BIN_FRAMES,
            tile_bins: TILE_BINS as u64,
        };
        match state {
            PeaksState::Ready(pyramid) => {
                info.state = "ready";
                info.frames = pyramid.frames;
                info.missing = pyramid.missing;
                info.level_bins = pyramid.levels.iter().map(|l| l.len() as u64).collect();
            }
            PeaksState::Pending => {}
            PeaksState::Failed => info.state = "failed",
        }
        info
    }
}

/// A pyramid became ready, or failed (the prototype's `ready` signal, plus failures).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PeaksReady {
    /// The asset.
    pub asset_id: String,
    /// The rate it was read at.
    pub sample_rate: u32,
    /// `ready` or `failed`.
    pub state: &'static str,
}

/// A pyramid's key: the asset, its content hash and the rate.
type Key = (String, Option<String>, u32);

/// A computation for a worker.
struct Task {
    key: Key,
    asset: MediaAsset,
    workspace: Option<PathBuf>,
}

/// The cache's state, shared with its workers.
#[derive(Default)]
struct State {
    ready: HashMap<Key, Arc<PeakPyramid>>,
    pending: HashSet<Key>,
    failed: HashSet<Key>,
    closed: bool,
}

struct Shared {
    media: Arc<MediaService>,
    state: Mutex<State>,
    listeners: Mutex<Vec<Sender<PeaksReady>>>,
}

/// The running workers.
struct Workers {
    tasks: crossbeam_channel::Sender<Task>,
    threads: Vec<JoinHandle<()>>,
}

/// The asynchronous, persistent pyramid cache (`WaveformCache`; see the module docs).
pub struct PeakCache {
    shared: Arc<Shared>,
    session: SharedSession,
    workers: Mutex<Option<Workers>>,
}

impl std::fmt::Debug for PeakCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeakCache").finish_non_exhaustive()
    }
}

impl PeakCache {
    /// A cache reading media through `media`, persisting into the workspace of `session`'s
    /// project. Its workers start with the first request.
    #[must_use]
    pub fn new(media: Arc<MediaService>, session: SharedSession) -> Self {
        Self {
            shared: Arc::new(Shared {
                media,
                state: Mutex::new(State::default()),
                listeners: Mutex::new(Vec::new()),
            }),
            session,
            workers: Mutex::new(None),
        }
    }

    /// A channel receiving a [`PeaksReady`] for every computation that finishes from now on.
    /// Dropping the receiver unsubscribes.
    pub fn subscribe(&self) -> Receiver<PeaksReady> {
        let (sender, receiver) = channel();
        lock(&self.shared.listeners).push(sender);
        receiver
    }

    /// The pyramid of `asset` at `sample_rate` (`get`): ready, or "pending" after scheduling
    /// it on a worker (never computed on this thread), or "failed" for one that failed before.
    /// After [`Self::close`] nothing is scheduled.
    pub fn request(&self, asset: &MediaAsset, sample_rate: u32) -> PeaksState {
        let key: Key = (asset.id.clone(), asset.sha256.clone(), sample_rate);
        {
            let mut state = lock(&self.shared.state);
            if let Some(found) = state.ready.get(&key) {
                return PeaksState::Ready(Arc::clone(found));
            }
            if state.failed.contains(&key) {
                return PeaksState::Failed;
            }
            if state.pending.contains(&key) || state.closed {
                return PeaksState::Pending;
            }
            state.pending.insert(key.clone());
        }
        let workspace = self
            .session
            .lock()
            .location()
            .map(|location| location.workspace.clone());
        let task = Task {
            key: key.clone(),
            asset: asset.clone(),
            workspace,
        };
        if let Err(error) = self.schedule(task) {
            log::error!("cannot compute the peaks of {}: {error}", asset.id);
            let mut state = lock(&self.shared.state);
            state.pending.remove(&key);
            state.failed.insert(key);
            return PeaksState::Failed;
        }
        PeaksState::Pending
    }

    /// The pyramid of the open project's asset `asset_id` at the project rate, requested as
    /// [`Self::request`] does, as the page's [`PeaksInfo`].
    ///
    /// # Errors
    ///
    /// `NO_PROJECT` without a project; `NOT_FOUND` for an unknown asset.
    pub fn info(&self, asset_id: &str) -> Result<PeaksInfo, DomainError> {
        let (asset, sample_rate) = {
            let session = self.session.lock();
            let project = session.require_project()?;
            (
                project.asset(asset_id)?.clone(),
                project.settings.sample_rate,
            )
        };
        Ok(PeaksInfo::of(
            &self.request(&asset, sample_rate),
            sample_rate,
        ))
    }

    /// The ready pyramid of the asset `asset_id` at `sample_rate`, whatever its content hash.
    #[must_use]
    pub fn ready(&self, asset_id: &str, sample_rate: u32) -> Option<Arc<PeakPyramid>> {
        lock(&self.shared.state)
            .ready
            .iter()
            .find(|((id, _, rate), _)| id == asset_id && *rate == sample_rate)
            .map(|(_, pyramid)| Arc::clone(pyramid))
    }

    /// Tile `index` of `level` of the ready pyramid of `asset_id` at `sample_rate`
    /// ([`encode_tile`]); `None` when it is not ready or the tile does not exist.
    #[must_use]
    pub fn tile(
        &self,
        asset_id: &str,
        sample_rate: u32,
        level: usize,
        index: usize,
    ) -> Option<Vec<u8>> {
        encode_tile(self.ready(asset_id, sample_rate)?.as_ref(), level, index)
    }

    /// Whether no computation is pending.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        lock(&self.shared.state).pending.is_empty()
    }

    /// Forgets every pyramid and failure (`clear`); running computations still finish.
    pub fn clear(&self) {
        let mut state = lock(&self.shared.state);
        state.ready.clear();
        state.failed.clear();
    }

    /// Stops scheduling, asks running computations to stop and waits for the workers.
    pub fn close(&self) {
        lock(&self.shared.state).closed = true;
        let workers = lock(&self.workers).take();
        if let Some(Workers { tasks, threads }) = workers {
            drop(tasks);
            for thread in threads {
                // ERR-06: a worker that panicked has reported it on its thread; nothing is left
                // to stop.
                let _ = thread.join();
            }
        }
    }

    fn schedule(&self, task: Task) -> io::Result<()> {
        let mut workers = lock(&self.workers);
        if workers.is_none() {
            *workers = Some(spawn_workers(&self.shared)?);
        }
        match workers.as_ref() {
            Some(running) => running
                .tasks
                .send(task)
                .map_err(|_| io::Error::other("the peak workers have stopped")),
            None => Err(io::Error::other("the peak workers did not start")),
        }
    }
}

impl Drop for PeakCache {
    fn drop(&mut self) {
        self.close();
    }
}

fn spawn_workers(shared: &Arc<Shared>) -> io::Result<Workers> {
    let (tasks, receiver) = crossbeam_channel::unbounded::<Task>();
    let mut threads = Vec::with_capacity(WORKERS);
    for index in 0..WORKERS {
        let shared = Arc::clone(shared);
        let receiver: TaskReceiver<Task> = receiver.clone();
        threads.push(
            thread::Builder::new()
                .name(format!("sonara-peaks-{index}"))
                .spawn(move || {
                    for task in receiver {
                        run(&shared, task);
                    }
                })?,
        );
    }
    Ok(Workers { tasks, threads })
}

/// Computes one pyramid and announces it (`_compute`).
fn run(shared: &Shared, task: Task) {
    let closed = || lock(&shared.state).closed;
    let provider = shared.media.provider();
    let outcome = compute(&provider, &task, &closed);
    let pyramid = match outcome {
        Ok(pyramid) => Some(Arc::new(pyramid)),
        Err(MediaError::Cancelled) => None,
        Err(error) => {
            log::warn!("peak computation failed for {}: {error}", task.asset.id);
            None
        }
    };
    let (asset_id, _, sample_rate) = task.key.clone();
    let notify = {
        let mut state = lock(&shared.state);
        state.pending.remove(&task.key);
        match &pyramid {
            Some(pyramid) => {
                state.ready.insert(task.key, Arc::clone(pyramid));
            }
            None => {
                state.failed.insert(task.key);
            }
        }
        !state.closed
    };
    if notify {
        let ready = PeaksReady {
            asset_id,
            sample_rate,
            state: if pyramid.is_some() { "ready" } else { "failed" },
        };
        lock(&shared.listeners).retain(|listener| listener.send(ready.clone()).is_ok());
    }
}

/// The pyramid of `task`: level 0 from the cache file when it is valid, else computed and
/// persisted (unless the media is missing).
fn compute(
    provider: &MediaProvider,
    task: &Task,
    closed: &dyn Fn() -> bool,
) -> Result<PeakPyramid, MediaError> {
    let (_, _, sample_rate) = task.key;
    let reader = provider.reader(&task.asset, sample_rate);
    let (missing, frames) = {
        let reader = lock(&reader);
        (reader.is_missing(), reader.frames())
    };
    let file = task
        .workspace
        .as_deref()
        .map(|workspace| cache_file(workspace, &task.asset, sample_rate));
    let cached = match (&file, missing) {
        (Some(path), false) => fs::read(path)
            .ok()
            .and_then(|bytes| decode_level0(&bytes, sample_rate, frames)),
        _ => None,
    };
    let level0 = if let Some(level0) = cached {
        level0
    } else {
        let level0 = compute_level0(&reader, frames, closed)?;
        if let (Some(path), false) = (&file, missing)
            && let Err(error) = persist(path, &encode_level0(sample_rate, frames, &level0))
        {
            log::warn!("cannot keep the peaks in {}: {error}", path.display());
        }
        level0
    };
    Ok(PeakPyramid {
        sample_rate,
        frames,
        levels: build_levels(level0),
        missing,
    })
}

/// `<workspace>/cache/peaks/<asset>-<sha12>-<rate>.peaks` (`_cache_file`).
#[must_use]
pub fn cache_file(workspace: &Path, asset: &MediaAsset, sample_rate: u32) -> PathBuf {
    let sha: String = asset
        .sha256
        .as_deref()
        .unwrap_or("nohash")
        .chars()
        .take(12)
        .collect();
    workspace
        .join("cache")
        .join("peaks")
        .join(format!("{}-{sha}-{sample_rate}.peaks", asset.id))
}

/// Writes `bytes` to `path` through a temporary file, so a reader never sees half a file.
fn persist(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("peaks.tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::{
        BIN_FRAMES, PeakPyramid, TILE_BINS, TILE_HEADER, build_levels, decode_level0,
        encode_level0, encode_tile,
    };

    fn ramp(bins: usize) -> Vec<[f32; 2]> {
        let count = f32::from(u16::try_from(bins).unwrap());
        (0..bins)
            .map(|i| {
                let v = f32::from(u16::try_from(i).unwrap()) / count;
                [-v, v]
            })
            .collect()
    }

    #[test]
    fn tile_paths_parse() {
        use super::TileRequest;
        assert_eq!(
            TileRequest::parse("/tile/med_1/48000/2/3"),
            Some(TileRequest {
                asset_id: "med_1".to_owned(),
                sample_rate: 48_000,
                level: 2,
                index: 3,
            })
        );
        for path in [
            "/tile/med_1/48000/2",
            "/tile/med_1/48000/2/3/4",
            "/tile//48000/2/3",
            "/tile/med_1/x/2/3",
            "/tiles/med_1/48000/2/3",
            "/tile/med_1/48000/-1/3",
        ] {
            assert_eq!(TileRequest::parse(path), None, "{path}");
        }
    }

    #[test]
    fn levels_shrink_by_four_until_64_bins() {
        let levels = build_levels(ramp(4097));
        let sizes: Vec<usize> = levels.iter().map(Vec::len).collect();
        // 4097 → 1025 (the last group is the one leftover bin) → 257 → 65 → 17.
        assert_eq!(sizes, [4097, 1025, 257, 65, 17]);
        let second = levels.get(1).unwrap();
        assert_eq!(second.first(), Some(&[-3.0 / 4097.0, 3.0 / 4097.0]));
        assert_eq!(second.last(), Some(&[-4096.0 / 4097.0, 4096.0 / 4097.0]));
        assert_eq!(build_levels(ramp(64)).len(), 1);
        assert_eq!(build_levels(Vec::new()).len(), 1);
    }

    #[test]
    fn a_persisted_level0_round_trips_and_rejects_another_media() {
        let level0 = ramp(10);
        let frames = 9 * BIN_FRAMES + 1;
        let bytes = encode_level0(48_000, frames, &level0);
        assert_eq!(bytes.len(), 32 + 10 * 8);
        assert_eq!(decode_level0(&bytes, 48_000, frames), Some(level0));
        assert_eq!(decode_level0(&bytes, 44_100, frames), None);
        assert_eq!(decode_level0(&bytes, 48_000, frames + BIN_FRAMES), None);
        assert_eq!(
            decode_level0(&bytes[..bytes.len() - 1], 48_000, frames),
            None
        );
        let mut other = bytes.clone();
        other[0] = b'X';
        assert_eq!(decode_level0(&other, 48_000, frames), None);
    }

    #[test]
    fn tiles_cut_a_level_with_a_header() {
        let pyramid = PeakPyramid {
            sample_rate: 48_000,
            frames: 5000 * BIN_FRAMES,
            levels: build_levels(ramp(5000)),
            missing: false,
        };
        let first = encode_tile(&pyramid, 0, 0).unwrap();
        assert_eq!(first.len(), TILE_HEADER + TILE_BINS * 8);
        let last = encode_tile(&pyramid, 0, 1).unwrap();
        assert_eq!(last.len(), TILE_HEADER + (5000 - TILE_BINS) * 8);
        let header =
            |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(u16::from_le_bytes([last[0], last[1]]), 1);
        assert_eq!(header(&last, 4), 1);
        assert_eq!(header(&last, 8), 5000 - 4096);
        assert_eq!(header(&last, 12), 5000);
        assert_eq!(header(&last, 16), 256);
        assert_eq!(header(&last, 20), 48_000);
        let first_bin =
            f32::from_le_bytes(last[TILE_HEADER + 4..TILE_HEADER + 8].try_into().unwrap());
        assert_eq!(first_bin, 4096.0 / 5000.0);
        assert_eq!(encode_tile(&pyramid, 0, 2), None);
        let level1 = encode_tile(&pyramid, 1, 0).unwrap();
        assert_eq!(header(&level1, 16), 1024);
        assert_eq!(encode_tile(&pyramid, 9, 0), None);
    }
}
