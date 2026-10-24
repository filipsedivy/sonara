//! Inspection: the `sf.info` equivalent and the importer's `inspect_audio`
//! (`import_export/audio_import.py`).
//!
//! The container comes from the file's first bytes and, where symphonia demuxes it, from the
//! demuxer; the encoding from the codec. Names, frame counts and strings follow libsndfile 1.2.2,
//! the prototype's decoder (golden `media/info.json`).

use std::fs::File;
use std::path::Path;

use serde_json::{Value, json};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::JsonMap;
use symphonia::core::codecs::audio::AudioCodecParameters;
use symphonia::core::codecs::audio::well_known as codec;
use symphonia::core::formats::well_known as fmt;
use symphonia::core::meta::RawValue;

use crate::container::{self, Family};
use crate::error::MediaError;
use crate::ffmpeg::Ffmpeg;
use crate::format::{MajorFormat, Subtype, bit_depth, is_lossy};
use crate::stream::{AudioStream, Opened};

/// The string fields the importer reads (`audio_import.METADATA_FIELDS`), in its order.
pub const METADATA_FIELDS: [&str; 10] = [
    "title",
    "copyright",
    "software",
    "artist",
    "comment",
    "date",
    "album",
    "license",
    "tracknumber",
    "genre",
];

/// What `sf.info` reports about an audio file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundInfo {
    /// The container.
    pub format: MajorFormat,
    /// The sample encoding.
    pub subtype: Subtype,
    /// Frames per second.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u32,
    /// Frames, without encoder delay and padding.
    pub frames: u64,
}

/// Reads the container, encoding, rate, channels and frame count of `path`.
///
/// # Errors
///
/// [`MediaError::NotRecognised`] for a file no reader knows, [`MediaError::Rejected`] for a CAF
/// or Ogg Opus file libsndfile refuses (with its reason), [`MediaError::Unsupported`] for a
/// container or encoding libsndfile reads but this crate cannot decode, and the I/O and decode
/// errors of reading the header (or the whole stream, when the container states no frame
/// count).
pub fn sound_info(path: &Path) -> Result<SoundInfo, MediaError> {
    let Opened {
        mut stream,
        family,
        wave,
        ..
    } = AudioStream::open(path)?;
    // Only what can be decoded is reported.
    stream.check_decodable()?;
    let format = match family {
        Family::Riff => {
            if wave.as_ref().is_some_and(|w| w.format.extensible) {
                MajorFormat::Wavex
            } else {
                MajorFormat::Wav
            }
        }
        Family::Rf64 => MajorFormat::Rf64,
        Family::W64 => MajorFormat::W64,
        Family::Aiff => MajorFormat::Aiff,
        Family::Caf => MajorFormat::Caf,
        Family::OggOpus => MajorFormat::Ogg,
        Family::Other => {
            let id = stream
                .format_reader()
                .map(|reader| reader.format_info().format);
            match id {
                Some(fmt::FORMAT_ID_FLAC) => MajorFormat::Flac,
                Some(fmt::FORMAT_ID_OGG) => MajorFormat::Ogg,
                Some(fmt::FORMAT_ID_MP1 | fmt::FORMAT_ID_MP2 | fmt::FORMAT_ID_MP3) => {
                    MajorFormat::Mp3
                }
                _ => {
                    return Err(MediaError::NotRecognised {
                        path: path.to_owned(),
                    });
                }
            }
        }
    };
    let subtype = match (stream.own_subtype(), stream.codec_params()) {
        (Some(subtype), _) => Some(subtype),
        (None, Some(params)) => codec_subtype(params),
        (None, None) => None,
    }
    .ok_or_else(|| MediaError::unsupported(path, "sample encoding"))?;
    let frames = match stream.frames() {
        Some(frames) => frames,
        None => count_frames(&mut stream)?,
    };
    Ok(SoundInfo {
        format,
        subtype,
        sample_rate: stream.sample_rate(),
        channels: u32::try_from(stream.channels()).unwrap_or(u32::MAX),
        frames,
    })
}

/// libsndfile's subtype for a codec symphonia decodes.
fn codec_subtype(params: &AudioCodecParameters) -> Option<Subtype> {
    let by_bits = |s8, s16, s24, s32| match params.bits_per_sample {
        Some(8) => s8,
        Some(16) => s16,
        Some(24) => s24,
        Some(32) => s32,
        _ => None,
    };
    match params.codec {
        codec::CODEC_ID_PCM_S8 => Some(Subtype::PcmS8),
        codec::CODEC_ID_PCM_U8 => Some(Subtype::PcmU8),
        codec::CODEC_ID_PCM_S16LE | codec::CODEC_ID_PCM_S16BE => Some(Subtype::Pcm16),
        codec::CODEC_ID_PCM_S24LE | codec::CODEC_ID_PCM_S24BE => Some(Subtype::Pcm24),
        codec::CODEC_ID_PCM_S32LE | codec::CODEC_ID_PCM_S32BE => Some(Subtype::Pcm32),
        codec::CODEC_ID_PCM_F32LE | codec::CODEC_ID_PCM_F32BE => Some(Subtype::Float),
        codec::CODEC_ID_PCM_F64LE | codec::CODEC_ID_PCM_F64BE => Some(Subtype::Double),
        codec::CODEC_ID_PCM_ALAW => Some(Subtype::Alaw),
        codec::CODEC_ID_PCM_MULAW => Some(Subtype::Ulaw),
        codec::CODEC_ID_FLAC => by_bits(
            Some(Subtype::PcmS8),
            Some(Subtype::Pcm16),
            Some(Subtype::Pcm24),
            Some(Subtype::Pcm32),
        ),
        codec::CODEC_ID_ALAC => by_bits(
            None,
            Some(Subtype::Alac16),
            Some(Subtype::Alac24),
            Some(Subtype::Alac32),
        )
        .or((params.bits_per_sample == Some(20)).then_some(Subtype::Alac20)),
        codec::CODEC_ID_VORBIS => Some(Subtype::Vorbis),
        codec::CODEC_ID_OPUS => Some(Subtype::Opus),
        codec::CODEC_ID_MP1 => Some(Subtype::MpegLayerI),
        codec::CODEC_ID_MP2 => Some(Subtype::MpegLayerII),
        codec::CODEC_ID_MP3 => Some(Subtype::MpegLayerIII),
        _ => None,
    }
}

/// The frame count of a stream whose container does not state it: decode it all.
fn count_frames(stream: &mut AudioStream) -> Result<u64, MediaError> {
    let mut buffer = vec![0.0; 65_536 * stream.channels()];
    let mut frames = 0u64;
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Ok(frames);
        }
        frames += read as u64;
    }
}

/// The metadata strings libsndfile reports for `path`, for the fields of [`METADATA_FIELDS`]
/// in that order; empty values are left out (`inspect_audio`'s `meta`).
///
/// Sources: RIFF `INFO` (WAV, WAVEX, RF64), AIFF text chunks, the CAF `info` chunk (the last
/// value of a key wins), Vorbis comments (FLAC, Ogg Vorbis, field names compared without case,
/// the first entry wins), Ogg Opus's `OpusTags` (`ENCODER` is the software, the last value wins)
/// and ID3 (MP3). Wave64 has none. Elsewhere, where a field occurs twice, the first occurrence
/// wins, as in libsndfile.
///
/// # Errors
///
/// The errors of opening the file ([`sound_info`]).
pub fn metadata_strings(path: &Path) -> Result<Vec<(&'static str, String)>, MediaError> {
    let Opened {
        mut stream,
        family,
        wave,
        strings,
    } = AudioStream::open(path)?;
    let found = match family {
        Family::Riff | Family::Rf64 => wave.map(|w| w.strings).unwrap_or_default(),
        Family::W64 => Vec::new(),
        Family::Caf | Family::OggOpus => strings.unwrap_or_default(),
        Family::Aiff => {
            let mut file = File::open(path).map_err(|e| MediaError::io("open", path, e))?;
            let len = file
                .metadata()
                .map_err(|e| MediaError::io("inspect", path, e))?
                .len();
            container::read_aiff_strings(&mut file, len)
                .map_err(|e| MediaError::malformed(path, e.to_string()))?
        }
        Family::Other => stream.format_reader().map(tag_strings).unwrap_or_default(),
    };
    Ok(METADATA_FIELDS
        .iter()
        .filter_map(|field| {
            found
                .iter()
                .find(|(name, _)| name == field)
                .filter(|(_, value)| !value.is_empty())
                .map(|(name, value)| (*name, value.clone()))
        })
        .collect())
}

/// Vorbis comment (FLAC, Ogg) or ID3 (MP3) strings from symphonia's metadata, over every
/// revision the demuxer logged (an MP3 can carry `ID3v2` at the start and `ID3v1` at the end).
fn tag_strings(
    reader: &mut dyn symphonia::core::formats::FormatReader,
) -> Vec<(&'static str, String)> {
    let id3 = matches!(
        reader.format_info().format,
        fmt::FORMAT_ID_MP1 | fmt::FORMAT_ID_MP2 | fmt::FORMAT_ID_MP3
    );
    let mut metadata = reader.metadata();
    let mut revisions = Vec::new();
    while let Some(revision) = metadata.pop() {
        revisions.push(revision);
    }
    revisions.extend(metadata.current().cloned());
    let mut primary = Vec::new();
    let mut fallback = Vec::new();
    for tag in revisions.iter().flat_map(|r| &r.media.tags) {
        let key = tag.raw.key.as_str();
        let Some(text) = raw_text(&tag.raw.value) else {
            continue;
        };
        if !id3 {
            if let Some(field) = METADATA_FIELDS
                .iter()
                .find(|field| field.eq_ignore_ascii_case(key))
            {
                primary.push((*field, text));
            }
        } else if let Some(field) = id3v2_field(key) {
            primary.push((field, text));
        } else if let Some(field) = id3v1_field(key) {
            // symphonia decodes ID3v1 as ISO-8859-1; libsndfile passes the bytes on, which
            // soundfile decodes as UTF-8.
            let bytes: Option<Vec<u8>> = text.chars().map(|c| u8::try_from(c).ok()).collect();
            let text = bytes.map_or(text, |b| String::from_utf8_lossy(&b).into_owned());
            fallback.push((field, text));
        }
    }
    primary.extend(fallback);
    primary
}

fn raw_text(value: &RawValue) -> Option<String> {
    match value {
        RawValue::String(text) => Some(text.as_str().to_owned()),
        RawValue::StringList(list) => list.first().cloned(),
        RawValue::UnsignedInt(n) => Some(n.to_string()),
        RawValue::SignedInt(n) => Some(n.to_string()),
        _ => None,
    }
}

/// `ID3v2` frames for libsndfile's fields (MP3 through mpg123).
fn id3v2_field(key: &str) -> Option<&'static str> {
    Some(match key {
        "TIT2" => "title",
        "TCOP" => "copyright",
        "TSSE" => "software",
        "TPE1" => "artist",
        "COMM" => "comment",
        "TDRC" | "TYER" => "date",
        "TALB" => "album",
        "TRCK" => "tracknumber",
        "TCON" => "genre",
        _ => return None,
    })
}

/// symphonia's `ID3v1` keys for libsndfile's fields.
fn id3v1_field(key: &str) -> Option<&'static str> {
    Some(match key {
        "TITLE" => "title",
        "ARTIST" => "artist",
        "ALBUM" => "album",
        "YEAR" => "date",
        "TRACK" => "tracknumber",
        "COMMENT" => "comment",
        "GENRE" => "genre",
        _ => return None,
    })
}

/// The importer's description of an audio file (`audio_import.AudioFileInfo`).
#[derive(Debug, Clone, PartialEq)]
pub struct AudioFileInfo {
    /// The path as given.
    pub path: String,
    /// libsndfile's container name.
    pub format: String,
    /// libsndfile's encoding name.
    pub subtype: String,
    /// Frames per second.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u32,
    /// Frames.
    pub frames: u64,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Bits per sample of the encoding, if it has one.
    pub bit_depth: Option<u32>,
    /// Whether the importer warns about a lossy encoding.
    pub lossy: bool,
    /// The decoder the prototype names, as tool results carry it (CTR-01): `"libsndfile"` for
    /// every file this crate reads, `"ffmpeg"` for one the `FFmpeg` fallback inspected.
    pub decoder: String,
    /// The metadata strings, in [`METADATA_FIELDS`] order.
    pub metadata: Vec<(String, String)>,
}

impl AudioFileInfo {
    /// Duration in seconds (0 without a sample rate).
    #[must_use]
    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            #[expect(
                clippy::cast_precision_loss,
                reason = "Python's frames / sample_rate rounds the same way"
            )]
            let frames = self.frames as f64;
            frames / f64::from(self.sample_rate)
        }
    }

    /// The JSON form (`AudioFileInfo.to_json`), with the duration rounded to six decimals.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let metadata: JsonMap = self
            .metadata
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        json!({
            "path": self.path,
            "format": self.format,
            "subtype": self.subtype,
            "sample_rate": self.sample_rate,
            "channels": self.channels,
            "frames": self.frames,
            "duration_seconds": py_round6(self.duration_seconds()),
            "size_bytes": self.size_bytes,
            "bit_depth": self.bit_depth,
            "lossy": self.lossy,
            "decoder": self.decoder,
            "metadata": metadata,
        })
    }
}

/// Python's `round(x, 6)`: the decimal string correctly rounded from the exact binary value,
/// parsed back (as `sonara-midi`'s `num::py_round6`).
fn py_round6(value: f64) -> f64 {
    format!("{value:.6}").parse().unwrap_or(value)
}

/// Inspects an audio file for import (`audio_import.inspect_audio`) without the `FFmpeg`
/// fallback: [`inspect_audio_with`] and no `FFmpeg`. A file this crate cannot read is
/// `UNSUPPORTED`, worded as the prototype words it when `FFmpeg` is not installed.
///
/// # Errors
///
/// A [`DomainError`] with `NOT_FOUND` when `path` is not a file, and `UNSUPPORTED` when it
/// cannot be read as audio.
pub fn inspect_audio(path: &Path) -> Result<AudioFileInfo, DomainError> {
    inspect_audio_with(path, None)
}

/// Inspects an audio file for import (`audio_import.inspect_audio`). A file this crate cannot
/// read goes to `ffmpeg`'s `ffprobe` when `ffmpeg` is given ([`Ffmpeg::inspect`]), as the
/// prototype's goes when `FFmpeg` is installed.
///
/// # Errors
///
/// A [`DomainError`] with `NOT_FOUND` when `path` is not a file; `UNSUPPORTED` when it cannot be
/// read as audio and there is no `ffmpeg`; else the errors of [`Ffmpeg::inspect`].
pub fn inspect_audio_with(
    path: &Path,
    ffmpeg: Option<&Ffmpeg>,
) -> Result<AudioFileInfo, DomainError> {
    let display = path.display().to_string();
    if !path.is_file() {
        return Err(DomainError::new(
            ErrorCode::NotFound,
            format!("audio file {display} does not exist"),
        ));
    }
    let size_bytes = path.metadata().map_or(0, |m| m.len());
    let info = match sound_info(path) {
        Ok(info) => info,
        Err(err) => {
            if let Some(ffmpeg) = ffmpeg {
                return ffmpeg.inspect(path, size_bytes);
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            return Err(DomainError::new(
                ErrorCode::Unsupported,
                format!(
                    "{name}: format not supported by libsndfile and FFmpeg is not installed ({err})"
                ),
            ));
        }
    };
    let metadata = metadata_strings(path)
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
    let subtype = info.subtype.name();
    Ok(AudioFileInfo {
        path: display,
        format: info.format.name().to_owned(),
        subtype: subtype.to_owned(),
        sample_rate: info.sample_rate,
        channels: info.channels,
        frames: info.frames,
        size_bytes,
        bit_depth: bit_depth(subtype),
        lossy: is_lossy(subtype),
        decoder: "libsndfile".to_owned(),
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::py_round6;

    #[test]
    fn durations_round_like_python() {
        assert_eq!(py_round6(1.5), 1.5);
        assert_eq!(py_round6(2.0 / 3.0), 0.666_667);
        assert_eq!(py_round6(66_150.0 / 44_100.0), 1.5);
    }
}
