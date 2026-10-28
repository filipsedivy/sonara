//! Media handling: decoding and resampling imported audio, the decoded-media cache, and the export
//! encoders with dither, normalization and BWF metadata.
//!
//! Step 7 ports the reading side of the prototype (`audio/engine/media.py` and the inspection
//! part of `import_export/audio_import.py`):
//!
//! - **Inspection** ([`sound_info`], [`metadata_strings`], [`inspect_audio`]): libsndfile's
//!   container and encoding names, frame counts and strings, as project assets store them.
//! - **Readers** ([`MediaProvider`], [`MediaReader`]): the direct reader for uncompressed files
//!   at the project rate, the decode cache for everything else, silence for missing media.
//! - **The decode cache** ([`decode_to_cache`], [`CacheInfo`]) in the prototype's on-disk
//!   format, resampled with libsoxr VHQ ([`ResampleStream`]).
//! - **The importer's naming rules** ([`safe_stem`], [`import_path`]).
//!
//! Step 11 ports the writing side (`import_export/audio_export.py`) in [`export`]: the
//! two-pass exporter with quantization, dither, normalization, resampling, BWF metadata and
//! verification, an own writer for WAV, BWF, RF64, Wave64, AIFF and CAF that reproduces
//! libsndfile's bytes, FLAC through libFLAC and MP3 through LAME, both loaded at run time (owner
//! decisions of 2026-10-21 and 2026-10-22), and Ogg Vorbis and Opus through `vorbis_rs` and
//! libopus.
//!
//! Decoding uses symphonia (MPL-2.0, unmodified) for WAV, AIFF, FLAC, MP3 and Ogg Vorbis;
//! an own reader for RF64 and Wave64, which symphonia does not demux; an own CAF reader whose
//! packets go to symphonia's PCM, G.711 and ALAC decoders
//! (`caf`); and an own Ogg Opus reader over libopus's multistream decoder
//! (`opus`). libsoxr, libmp3lame and libFLAC are loaded at run time in
//! [`codec_ffi`](self) (RT-10, LIC-01), which also drives the statically linked libopus.
//!
//! A file none of these reads goes to the `FFmpeg` fallback when `ffmpeg` is installed
//! ([`Ffmpeg`], [`inspect_audio_with`]): `ffprobe` and `ffmpeg` run as external processes,
//! never linked.

mod cache;
mod caf;
#[expect(
    unsafe_code,
    reason = "RT-10: the codec FFI module loads the codec libraries (libsoxr, libmp3lame, \
              libFLAC) at run time and drives libopus's multistream decoder; it is the third \
              module allowed to use unsafe"
)]
mod codec_ffi;
mod container;
mod error;
pub mod export;
mod ffmpeg;
mod format;
mod inspect;
mod names;
mod opus;
mod pcm;
mod provider;
mod reader;
mod resample;
pub mod sfz;
mod stream;

pub use cache::{CACHE_VERSION, CacheInfo, DECODE_CHUNK, decode_to_cache};
pub use codec_ffi::{CodecError, SoxrSample};
pub use error::{MediaError, Rejection};
pub use ffmpeg::{Ffmpeg, which};
pub use format::{
    DIRECT_FORMATS, DIRECT_SUBTYPES, MajorFormat, Subtype, bit_depth, is_lossy, needs_cache,
};
pub use inspect::{
    AudioFileInfo, METADATA_FIELDS, SoundInfo, inspect_audio, inspect_audio_with, metadata_strings,
    sound_info,
};
pub use names::{
    MAX_IMPORT_BYTES, check_import_size, import_path, import_warnings, path_stem, path_suffix,
    recording_path, safe_stem, transcode_paths, transcode_warning,
};
pub use provider::{FullAudio, MediaProvider, Resolver, SharedReader, frames_at_rate};
pub use reader::{CachedReader, DirectReader, MediaReader, SilentReader};
pub use resample::{ResampleStream, ResampleStream64, SoxrStream};
