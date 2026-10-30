//! Opens readers for project assets (`media.MediaProvider`): the direct reader where the asset
//! is an uncompressed file at the project rate, the decode cache otherwise, and silence for
//! media that is missing or cannot be read. It also loads the sampler's instruments: an
//! application-supplied [`InstrumentResolver`] turns the stored value of a sampler's
//! `instrument` parameter into the patch file to load.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use sonara_core::model::media::MediaAsset;
use sonara_dsp::bank::SampleBank;

use crate::cache::{CacheInfo, decode_to_cache};
use crate::error::MediaError;
use crate::format::needs_cache;
use crate::reader::{CachedReader, DirectReader, MediaReader, SilentReader};
use crate::sfz::load_sfz;

/// A reader shared by everything that plays one asset at one rate.
pub type SharedReader = Arc<Mutex<MediaReader>>;

/// Maps an asset to its file on disk.
pub type Resolver = Box<dyn Fn(&MediaAsset) -> PathBuf + Send + Sync>;

/// What a sampler's `instrument` value plays: the installed preset it names and its patch file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentSource {
    /// The preset's ID, as the sampler's `instrument` parameter offers it among its choices.
    pub id: String,
    /// The SFZ patch file of the preset.
    pub patch: PathBuf,
}

/// Maps the stored value of a sampler's `instrument` parameter to what it plays: the value is a
/// preset ID, or the patch path an older project stored. `None` when nothing installed on this
/// machine matches. The application builds it from its instrument catalog; the engine knows
/// nothing of libraries (DES-06).
pub type InstrumentResolver = Box<dyn Fn(&str) -> Option<InstrumentSource> + Send + Sync>;

/// A whole (short) asset in memory, such as an impulse response (`load_full`).
#[derive(Debug, Clone, PartialEq)]
pub struct FullAudio {
    /// Channel count.
    pub channels: usize,
    /// Frames.
    pub frames: usize,
    /// Interleaved samples.
    pub samples: Vec<f64>,
}

type Key = (String, u32);

/// Opens and caches readers for assets (`media.MediaProvider`).
pub struct MediaProvider {
    resolve: Resolver,
    instruments: Option<InstrumentResolver>,
    cache_dir: Option<PathBuf>,
    readers: Mutex<HashMap<Key, SharedReader>>,
    full: Mutex<HashMap<Key, Arc<FullAudio>>>,
    banks: Mutex<HashMap<PathBuf, Arc<SampleBank>>>,
}

impl std::fmt::Debug for MediaProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaProvider")
            .field("cache_dir", &self.cache_dir)
            .field("resolves_instruments", &self.instruments.is_some())
            .finish_non_exhaustive()
    }
}

/// Locks a mutex, taking over its state if another thread panicked while holding it: the maps
/// only ever hold complete entries.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The frame count of an asset at `sample_rate`: `round(frames * sample_rate / asset_rate)`,
/// ties to even, 0 without an asset rate.
///
/// The product is exact; Python divides the integers with one correct rounding, which the `f64`
/// division reproduces while the product stays below 2^53 (about 1500 years of 192 kHz audio).
#[must_use]
pub fn frames_at_rate(frames: u64, asset_rate: u32, sample_rate: u32) -> u64 {
    if asset_rate == 0 {
        return 0;
    }
    let product = u128::from(frames) * u128::from(sample_rate);
    #[expect(
        clippy::cast_precision_loss,
        reason = "exact below 2^53, see the function documentation"
    )]
    let quotient = product as f64 / f64::from(asset_rate);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a non-negative, already rounded frame count"
    )]
    let rounded = quotient.round_ties_even() as u64;
    rounded
}

impl MediaProvider {
    /// A provider that finds files with `resolve` and keeps decode-cache entries in
    /// `cache_dir` (`cache/decoded/` of the project; `None` disables the cache).
    #[must_use]
    pub fn new(
        resolve: impl Fn(&MediaAsset) -> PathBuf + Send + Sync + 'static,
        cache_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            resolve: Box::new(resolve),
            instruments: None,
            cache_dir,
            readers: Mutex::new(HashMap::new()),
            full: Mutex::new(HashMap::new()),
            banks: Mutex::new(HashMap::new()),
        }
    }

    /// The same provider, resolving sampler instruments with `resolve` ([`InstrumentResolver`]).
    #[must_use]
    pub fn with_instruments(
        mut self,
        resolve: impl Fn(&str) -> Option<InstrumentSource> + Send + Sync + 'static,
    ) -> Self {
        self.instruments = Some(Box::new(resolve));
        self
    }

    /// What the stored `instrument` value of a sampler plays, through the resolver of
    /// [`MediaProvider::with_instruments`]; `None` when it names nothing installed.
    ///
    /// Without a resolver the value is the patch path itself, as before preset IDs: its ID is
    /// the value and its patch is the value read as a path. Whether the source is offered at all
    /// is the caller's check against the plugin's choices.
    #[must_use]
    pub fn resolve_instrument(&self, value: &str) -> Option<InstrumentSource> {
        match &self.instruments {
            Some(resolve) => resolve(value),
            None => Some(InstrumentSource {
                id: value.to_owned(),
                patch: PathBuf::from(value),
            }),
        }
    }

    /// The file of `asset`.
    #[must_use]
    pub fn path_for(&self, asset: &MediaAsset) -> PathBuf {
        (self.resolve)(asset)
    }

    /// The decode-cache directory.
    #[must_use]
    pub fn cache_dir(&self) -> Option<&Path> {
        self.cache_dir.as_deref()
    }

    /// Whether `asset` must go through the decode cache at `sample_rate`.
    #[must_use]
    pub fn needs_cache(asset: &MediaAsset, sample_rate: u32) -> bool {
        needs_cache(
            &asset.format,
            &asset.subtype,
            asset.sample_rate,
            sample_rate,
        )
    }

    /// Whether `asset` can be read at `sample_rate` without decoding first: it needs no cache,
    /// or its cache entry is valid ([`CacheInfo::is_ready`]).
    #[must_use]
    pub fn cache_ready(&self, asset: &MediaAsset, sample_rate: u32) -> bool {
        if !Self::needs_cache(asset, sample_rate) {
            return true;
        }
        self.cache_dir.as_deref().is_some_and(|dir| {
            CacheInfo::new(dir, &asset.id, sample_rate).is_ready(asset.sha256.as_deref())
        })
    }

    /// Builds the decode-cache entry of `asset` at `sample_rate` if it needs one and has none
    /// (call it from a worker thread).
    ///
    /// # Errors
    ///
    /// [`MediaError::NoCacheDir`], [`MediaError::Missing`] when the file does not exist, and
    /// the errors of [`decode_to_cache`].
    pub fn prepare(
        &self,
        asset: &MediaAsset,
        sample_rate: u32,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(), MediaError> {
        if self.cache_ready(asset, sample_rate) {
            return Ok(());
        }
        let dir = self.cache_dir.as_deref().ok_or(MediaError::NoCacheDir)?;
        let path = self.path_for(asset);
        if !path.exists() {
            return Err(MediaError::Missing {
                name: asset.name.clone(),
                path,
            });
        }
        decode_to_cache(
            &path,
            &CacheInfo::new(dir, &asset.id, sample_rate),
            sample_rate,
            asset.sha256.as_deref(),
            cancelled,
        )?;
        Ok(())
    }

    /// The reader of `asset` at `sample_rate`, opened on first use and shared afterwards. A
    /// missing or unreadable file gives a [`SilentReader`] of the asset's length at the rate.
    pub fn reader(&self, asset: &MediaAsset, sample_rate: u32) -> SharedReader {
        let key = (asset.id.clone(), sample_rate);
        if let Some(existing) = lock(&self.readers).get(&key) {
            return Arc::clone(existing);
        }
        // Opening may decode the whole file; the map is not locked meanwhile.
        let reader = Arc::new(Mutex::new(self.open(asset, sample_rate)));
        Arc::clone(lock(&self.readers).entry(key).or_insert(reader))
    }

    fn open(&self, asset: &MediaAsset, sample_rate: u32) -> MediaReader {
        let channels = usize::try_from(asset.channels).unwrap_or(usize::MAX);
        let frames = frames_at_rate(asset.frames, asset.sample_rate, sample_rate);
        let path = self.path_for(asset);
        if !path.exists() {
            return MediaReader::Silent(SilentReader::new(channels, frames, "missing"));
        }
        match self.open_file(asset, sample_rate, &path) {
            Ok(reader) => reader,
            Err(err) => MediaReader::Silent(SilentReader::new(
                channels,
                frames,
                format!("unreadable: {err}"),
            )),
        }
    }

    fn open_file(
        &self,
        asset: &MediaAsset,
        sample_rate: u32,
        path: &Path,
    ) -> Result<MediaReader, MediaError> {
        if !Self::needs_cache(asset, sample_rate) {
            return Ok(MediaReader::Direct(DirectReader::open(path)?));
        }
        self.prepare(asset, sample_rate, None)?;
        let dir = self.cache_dir.as_deref().ok_or(MediaError::NoCacheDir)?;
        let info = CacheInfo::new(dir, &asset.id, sample_rate);
        let (channels, frames) = info.read_layout()?;
        Ok(MediaReader::Cached(CachedReader::open(
            &info.raw, channels, frames,
        )?))
    }

    /// The first `min(frames, int(max_seconds × sample_rate))` frames of `asset` at
    /// `sample_rate` (`load_full`, for impulse responses; the prototype's default is 30 s).
    /// The result is kept per asset and rate, whatever `max_seconds` a later call passes, as in
    /// the prototype. Missing media loads as silence.
    ///
    /// # Errors
    ///
    /// The I/O and decode errors of reading the media.
    pub fn load_full(
        &self,
        asset: &MediaAsset,
        sample_rate: u32,
        max_seconds: f64,
    ) -> Result<Arc<FullAudio>, MediaError> {
        let key = (asset.id.clone(), sample_rate);
        if let Some(full) = lock(&self.full).get(&key) {
            return Ok(Arc::clone(full));
        }
        let reader = self.reader(asset, sample_rate);
        let mut reader = lock(&reader);
        // Python's `int(max_seconds * sample_rate)` truncates toward zero.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "truncation is the prototype's int(); negative limits load nothing"
        )]
        let limit = (max_seconds * f64::from(sample_rate)).trunc().max(0.0) as u64;
        let frames = usize::try_from(reader.frames().min(limit)).unwrap_or(usize::MAX);
        let samples = reader.read(0, frames)?;
        let full = Arc::new(FullAudio {
            channels: reader.channels(),
            frames,
            samples,
        });
        lock(&self.full).insert(key, Arc::clone(&full));
        Ok(full)
    }

    /// The sampler instrument of the SFZ patch `path`, loaded once and then shared: a live
    /// graph rebuild must not read the library again (`sonara.sampler`, ADR 0010). The cache is
    /// keyed by the patch file, so a preset ID and the old path that resolves to it share one
    /// load.
    ///
    /// # Errors
    ///
    /// The errors of [`load_sfz`].
    pub fn load_sample_bank(&self, path: &Path) -> Result<Arc<SampleBank>, MediaError> {
        if let Some(bank) = lock(&self.banks).get(path) {
            return Ok(Arc::clone(bank));
        }
        let bank = Arc::new(load_sfz(path)?);
        lock(&self.banks).insert(path.to_owned(), Arc::clone(&bank));
        Ok(bank)
    }

    /// Whether `asset` plays as silence at `sample_rate` because its media is missing or
    /// unreadable.
    pub fn is_missing(&self, asset: &MediaAsset, sample_rate: u32) -> bool {
        lock(&self.reader(asset, sample_rate)).is_missing()
    }

    /// Drops every reader and loaded asset.
    pub fn close(&self) {
        lock(&self.readers).clear();
        lock(&self.full).clear();
        lock(&self.banks).clear();
    }

    /// Drops the readers and loaded copies of one asset, for example after relinking it.
    pub fn forget(&self, asset_id: &str) {
        lock(&self.readers).retain(|(id, _), _| id != asset_id);
        lock(&self.full).retain(|(id, _), _| id != asset_id);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{InstrumentSource, MediaProvider, frames_at_rate};

    #[test]
    fn providers_can_be_shared_between_threads() {
        fn shareable<T: Send + Sync>() {}
        shareable::<MediaProvider>();
    }

    fn provider() -> MediaProvider {
        MediaProvider::new(|_| PathBuf::from("/nonexistent"), None)
    }

    #[test]
    fn without_a_resolver_an_instrument_value_is_its_patch_path() {
        assert_eq!(
            provider().resolve_instrument("/library/Violin.sfz"),
            Some(InstrumentSource {
                id: "/library/Violin.sfz".to_owned(),
                patch: PathBuf::from("/library/Violin.sfz"),
            })
        );
    }

    #[test]
    fn a_resolver_maps_ids_and_old_paths_and_refuses_the_rest() {
        let media = provider().with_instruments(|value| {
            let violin = || InstrumentSource {
                id: "lib/Violin".to_owned(),
                patch: PathBuf::from("/root/lib/Violin.sfz"),
            };
            match value {
                "lib/Violin" | "/old/lib/Violin.sfz" => Some(violin()),
                _ => None,
            }
        });
        let expected = Some(InstrumentSource {
            id: "lib/Violin".to_owned(),
            patch: PathBuf::from("/root/lib/Violin.sfz"),
        });
        assert_eq!(media.resolve_instrument("lib/Violin"), expected);
        assert_eq!(media.resolve_instrument("/old/lib/Violin.sfz"), expected);
        assert_eq!(media.resolve_instrument("lib/Cello"), None);
        assert_eq!(
            media.resolve_instrument("/root/lib/Cello.sfz"),
            None,
            "a resolver does not fall back to reading the value as a path"
        );
    }

    #[test]
    fn frames_at_rate_rounds_half_to_even() {
        assert_eq!(frames_at_rate(66_150, 44_100, 48_000), 72_000);
        assert_eq!(frames_at_rate(1, 44_100, 22_050), 0);
        assert_eq!(frames_at_rate(3, 44_100, 22_050), 2);
        assert_eq!(frames_at_rate(5, 44_100, 22_050), 2);
        assert_eq!(frames_at_rate(10, 0, 48_000), 0);
    }
}
