//! A decoded audio file as a seekable stream of interleaved `f64` frames: symphonia for every
//! container it demuxes, [`crate::pcm`] for RF64 and Wave64, [`crate::caf`]'s packets with
//! symphonia's decoders for CAF, and [`crate::opus`] for Ogg Opus.
//!
//! symphonia converts to `f64` exactly as libsndfile does for every lossless encoding (checked
//! sample for sample against the golden decodes); decoders that work in `f32` (MP3, Vorbis) are
//! widened exactly. Casting a frame back to `f32` therefore gives libsndfile's `float` read.

use std::fs::File;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use symphonia::core::audio::Channels;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::well_known as codec;
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::packet::Packet;
use symphonia::core::units::{Duration, Timestamp};

use crate::caf::{self, AlacQuirks, CafEncoding, CafError, CafHeader, CafPackets};
use crate::container::{self, Family, WaveHeader};
use crate::error::{MediaError, Rejection};
use crate::format::Subtype;
use crate::opus::OpusStream;
use crate::pcm::PcmStream;

/// Bytes read to classify a file: enough for the first Ogg page's segment table and the start
/// of its first packet.
const SNIFF_BYTES: u64 = 300;

/// What [`AudioStream::open`] learned about the file besides its audio.
pub(crate) struct Opened {
    pub(crate) stream: AudioStream,
    /// The container family from the file's first bytes.
    pub(crate) family: Family,
    /// The RIFF-family header, for RIFF, RF64 and Wave64 files.
    pub(crate) wave: Option<WaveHeader>,
    /// The metadata strings of a CAF or Ogg Opus file, which this crate's own readers parse.
    pub(crate) strings: Option<Vec<(&'static str, String)>>,
}

/// A decoded audio stream.
pub(crate) struct AudioStream {
    inner: Inner,
    path: PathBuf,
    channels: usize,
    sample_rate: u32,
    frames: Option<u64>,
    /// The frame index of the next frame [`AudioStream::read`] returns.
    position: u64,
}

enum Inner {
    Symphonia(Box<SymphoniaStream>),
    Pcm(PcmStream),
    Opus(Box<OpusStream>),
}

/// Where a symphonia decoder's packets come from.
enum Source {
    /// symphonia's demuxer, and the audio track's ID.
    Demuxer {
        format: Box<dyn FormatReader>,
        track_id: u32,
    },
    /// This crate's CAF reader.
    Caf(CafPackets),
}

struct SymphoniaStream {
    source: Source,
    /// libsndfile's ALAC decoding, where it differs from symphonia's (CAF ALAC only).
    quirks: Option<AlacQuirks>,
    /// Made on the first decode: inspection needs no decoder, so a container symphonia
    /// demuxes but cannot decode still inspects.
    decoder: Option<Box<dyn AudioDecoder>>,
    params: AudioCodecParameters,
    /// Decoded frames not yet returned, interleaved.
    pending: Vec<f64>,
    /// Index of the first unreturned sample in `pending`.
    pending_pos: usize,
    /// The container family, for reopening.
    family: Family,
}

impl AudioStream {
    /// Opens `path` and reads its header.
    ///
    /// # Errors
    ///
    /// [`MediaError::Io`] when the file cannot be read, [`MediaError::NotRecognised`] when no
    /// reader knows the format, and [`MediaError::Unsupported`] or [`MediaError::Malformed`]
    /// for files whose codec or header cannot be used.
    pub(crate) fn open(path: &Path) -> Result<Opened, MediaError> {
        let mut file = File::open(path).map_err(|e| MediaError::io("open", path, e))?;
        let file_len = file
            .metadata()
            .map_err(|e| MediaError::io("inspect", path, e))?
            .len();
        let mut head = Vec::with_capacity(16);
        (&mut file)
            .take(SNIFF_BYTES)
            .read_to_end(&mut head)
            .map_err(|e| MediaError::io("read", path, e))?;
        let family = container::sniff(&head);
        match family {
            Family::Caf => return Self::open_caf(file, path, file_len),
            Family::OggOpus => {
                drop(file);
                let opus = OpusStream::open(path)?;
                let strings = Some(opus.strings().to_vec());
                return Ok(Opened {
                    stream: Self {
                        channels: opus.channels(),
                        sample_rate: opus.sample_rate(),
                        frames: Some(opus.frames()),
                        inner: Inner::Opus(Box::new(opus)),
                        path: path.to_owned(),
                        position: 0,
                    },
                    family,
                    wave: None,
                    strings,
                });
            }
            _ => {}
        }
        let wave = match family {
            Family::Riff | Family::Rf64 => Some(container::read_riff(
                &mut file,
                file_len,
                family == Family::Rf64,
            )),
            Family::W64 => Some(container::read_w64(&mut file, file_len)),
            _ => None,
        }
        .transpose()
        .map_err(|e| MediaError::malformed(path, e.to_string()))?;
        let pcm_header = wave
            .as_ref()
            .filter(|_| matches!(family, Family::Rf64 | Family::W64));
        let stream = if let Some(header) = pcm_header {
            let pcm = PcmStream::new(file, path, header)?;
            Self {
                channels: pcm.channels(),
                sample_rate: header.format.sample_rate,
                frames: Some(pcm.frames()),
                inner: Inner::Pcm(pcm),
                path: path.to_owned(),
                position: 0,
            }
        } else {
            let mut stream = Self::open_symphonia(file, path, family)?;
            // libsndfile limits a RIFF data chunk to the file; symphonia trusts the header.
            if let Some(header) = &wave {
                stream.frames = Some(header.frames());
            }
            stream
        };
        Ok(Opened {
            stream,
            family,
            wave,
            strings: None,
        })
    }

    /// Opens a CAF file with this crate's reader and symphonia's decoder for its encoding.
    fn open_caf(mut file: File, path: &Path, file_len: u64) -> Result<Opened, MediaError> {
        let header = caf::read_header(&mut file, file_len).map_err(|err| match err {
            CafError::Rejected(reason) => MediaError::rejected(path, reason),
            CafError::Io(e) => MediaError::io("read", path, e),
        })?;
        let channels = usize::try_from(header.channels).unwrap_or(0);
        if channels == 0 {
            // libsndfile's `validate_sfinfo`: no channels.
            return Err(MediaError::rejected(path, Rejection::Internal));
        }
        let quirks = match header.encoding {
            CafEncoding::Alac { bits } => {
                Some(AlacQuirks::new(bits, channels, header.frames_per_packet))
            }
            _ => None,
        };
        let mut symphonia = SymphoniaStream {
            source: Source::Caf(CafPackets::new(file, &header)),
            quirks,
            decoder: None,
            params: header.codec_params(),
            pending: Vec::new(),
            pending_pos: 0,
            family: Family::Caf,
        };
        let frames = match header.encoding {
            CafEncoding::Alac { .. } => symphonia.alac_frames(&header, path)?,
            _ => header.pcm_frames(),
        };
        Ok(Opened {
            stream: Self {
                inner: Inner::Symphonia(Box::new(symphonia)),
                path: path.to_owned(),
                channels,
                sample_rate: header.sample_rate,
                frames: Some(frames),
                position: 0,
            },
            family: Family::Caf,
            wave: None,
            strings: Some(header.strings),
        })
    }

    fn open_symphonia(mut file: File, path: &Path, family: Family) -> Result<Self, MediaError> {
        // The header readers moved the cursor; symphonia reads from the current position.
        file.seek(SeekFrom::Start(0))
            .map_err(|e| MediaError::io("seek in", path, e))?;
        let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|err| match err {
                // Nothing claimed the file: libsndfile's "Format not recognised".
                SymphoniaError::Unsupported(_) if family == Family::Other => {
                    MediaError::NotRecognised {
                        path: path.to_owned(),
                    }
                }
                SymphoniaError::Unsupported(what) => MediaError::unsupported(path, what),
                SymphoniaError::IoError(e) if e.kind() != ErrorKind::UnexpectedEof => {
                    MediaError::io("read", path, e)
                }
                other => MediaError::Decode {
                    path: path.to_owned(),
                    source: other,
                },
            })?;
        let track = format
            .first_track(TrackType::Audio)
            .ok_or_else(|| MediaError::malformed(path, "no audio track"))?;
        let Some(CodecParameters::Audio(params)) = track.codec_params.clone() else {
            return Err(MediaError::malformed(path, "no audio codec parameters"));
        };
        let track_id = track.id;
        let frames = track.num_frames;
        let channels = params
            .channels
            .as_ref()
            .map(symphonia::core::audio::Channels::count)
            .filter(|&n| n > 0)
            .ok_or_else(|| MediaError::malformed(path, "no channel count"))?;
        let sample_rate = params
            .sample_rate
            .filter(|&r| r > 0)
            .ok_or_else(|| MediaError::malformed(path, "no sample rate"))?;
        Ok(Self {
            inner: Inner::Symphonia(Box::new(SymphoniaStream {
                source: Source::Demuxer { format, track_id },
                quirks: None,
                decoder: None,
                params,
                pending: Vec::new(),
                pending_pos: 0,
                family,
            })),
            path: path.to_owned(),
            channels,
            sample_rate,
            frames,
            position: 0,
        })
    }

    /// Fails unless the stream's codec can be decoded (the decoder is made now).
    ///
    /// # Errors
    ///
    /// [`MediaError::Unsupported`] for a codec without a decoder.
    pub(crate) fn check_decodable(&mut self) -> Result<(), MediaError> {
        match &mut self.inner {
            Inner::Symphonia(s) => decoder(&mut s.decoder, &s.params, &self.path).map(|_| ()),
            Inner::Pcm(_) | Inner::Opus(_) => Ok(()),
        }
    }

    /// Channel count.
    pub(crate) fn channels(&self) -> usize {
        self.channels
    }

    /// Frames per second.
    pub(crate) fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The frame count from the header, if the container states it.
    pub(crate) fn frames(&self) -> Option<u64> {
        self.frames
    }

    /// The codec parameters of a symphonia decoder (`None` for RF64, Wave64 and Ogg Opus).
    pub(crate) fn codec_params(&self) -> Option<&AudioCodecParameters> {
        match &self.inner {
            Inner::Symphonia(s) => Some(&s.params),
            Inner::Pcm(_) | Inner::Opus(_) => None,
        }
    }

    /// The encoding of a stream this crate decodes itself: RF64, Wave64 and Ogg Opus.
    pub(crate) fn own_subtype(&self) -> Option<Subtype> {
        match &self.inner {
            Inner::Symphonia(_) => None,
            Inner::Pcm(pcm) => Some(pcm.encoding().subtype()),
            Inner::Opus(_) => Some(Subtype::Opus),
        }
    }

    /// The symphonia demuxer, for its metadata (`None` where this crate reads the container).
    pub(crate) fn format_reader(&mut self) -> Option<&mut dyn FormatReader> {
        match &mut self.inner {
            Inner::Symphonia(s) => match &mut s.source {
                Source::Demuxer { format, .. } => Some(format.as_mut()),
                Source::Caf(_) => None,
            },
            Inner::Pcm(_) | Inner::Opus(_) => None,
        }
    }

    /// Reads up to `out.len() / channels` frames as interleaved `f64`. Returns the frames read;
    /// fewer only at the end of the stream (or of the frame count the header states).
    ///
    /// # Errors
    ///
    /// [`MediaError::Io`] or [`MediaError::Decode`] when the data cannot be read or decoded.
    pub(crate) fn read(&mut self, out: &mut [f64]) -> Result<usize, MediaError> {
        let channels = self.channels;
        let mut want = out.len() / channels;
        if let Some(frames) = self.frames {
            let left = frames.saturating_sub(self.position);
            want = want.min(usize::try_from(left).unwrap_or(usize::MAX));
        }
        let read = match &mut self.inner {
            Inner::Pcm(pcm) => pcm.read(&mut out[..want * channels])?,
            Inner::Symphonia(s) => s.read(&mut out[..want * channels], channels, &self.path)?,
            Inner::Opus(opus) => opus.read(&mut out[..want * channels]),
        };
        self.position += read as u64;
        Ok(read)
    }

    /// Skips `frames` frames.
    fn skip(&mut self, mut frames: u64) -> Result<(), MediaError> {
        let mut scratch = vec![0.0; 4096 * self.channels];
        while frames > 0 {
            let chunk = usize::try_from(frames.min(4096)).unwrap_or(4096);
            let read = self.read(&mut scratch[..chunk * self.channels])?;
            if read == 0 {
                break;
            }
            frames -= read as u64;
        }
        Ok(())
    }

    /// Moves to `frame`, so that the next [`AudioStream::read`] starts there.
    ///
    /// Symphonia seeks to a packet boundary at or before the frame, and the frames up to it are
    /// decoded and dropped. A stream that cannot seek accurately is reopened and decoded from
    /// the start.
    ///
    /// # Errors
    ///
    /// The errors of [`AudioStream::open`] and [`AudioStream::read`].
    pub(crate) fn seek(&mut self, frame: u64) -> Result<(), MediaError> {
        if frame == self.position {
            return Ok(());
        }
        match &mut self.inner {
            Inner::Pcm(pcm) => {
                pcm.seek(frame);
                self.position = frame.min(pcm.frames());
                Ok(())
            }
            Inner::Opus(_) => {
                let reopened = OpusStream::open(&self.path)?;
                self.inner = Inner::Opus(Box::new(reopened));
                self.position = 0;
                self.skip(frame)
            }
            Inner::Symphonia(s) => {
                if let Source::Caf(packets) = &mut s.source {
                    // Every CAF packet decodes on its own: start at the one holding `frame`.
                    let actual = packets.seek(frame);
                    if let Some(decoder) = &mut s.decoder {
                        decoder.reset();
                    }
                    s.pending.clear();
                    s.pending_pos = 0;
                    self.position = actual;
                    return self.skip(frame - actual);
                }
                let landed = match &mut s.source {
                    Source::Demuxer { format, track_id } => {
                        i64::try_from(frame).ok().and_then(|ts| {
                            let to = SeekTo::Timestamp {
                                ts: Timestamp::new(ts),
                                track_id: *track_id,
                            };
                            let seeked = format.seek(SeekMode::Accurate, to).ok()?;
                            u64::try_from(seeked.actual_ts.get())
                                .ok()
                                .filter(|&actual| actual <= frame)
                        })
                    }
                    Source::Caf(_) => None,
                };
                if let Some(actual) = landed {
                    if let Some(decoder) = &mut s.decoder {
                        decoder.reset();
                    }
                    s.pending.clear();
                    s.pending_pos = 0;
                    self.position = actual;
                    return self.skip(frame - actual);
                }
                let file =
                    File::open(&self.path).map_err(|e| MediaError::io("open", &self.path, e))?;
                let family = s.family;
                let reopened = Self::open_symphonia(file, &self.path, family)?;
                let frames = self.frames;
                *self = Self { frames, ..reopened };
                self.skip(frame)
            }
        }
    }
}

impl SymphoniaStream {
    fn read(&mut self, out: &mut [f64], channels: usize, path: &Path) -> Result<usize, MediaError> {
        let want = out.len() / channels;
        let mut filled = 0;
        while filled < want {
            let available = (self.pending.len() - self.pending_pos) / channels;
            if available > 0 {
                let n = available.min(want - filled);
                let src = &self.pending[self.pending_pos..self.pending_pos + n * channels];
                out[filled * channels..(filled + n) * channels].copy_from_slice(src);
                self.pending_pos += n * channels;
                filled += n;
                continue;
            }
            if !self.decode_next(channels, path)? {
                break;
            }
        }
        Ok(filled)
    }

    /// The next packet of the audio track; `None` at the end of the stream.
    fn next_packet(&mut self, path: &Path) -> Result<Option<Packet>, MediaError> {
        match &mut self.source {
            Source::Demuxer { format, track_id } => loop {
                let packet = match format.next_packet() {
                    Ok(Some(packet)) => packet,
                    Ok(None) => return Ok(None),
                    Err(SymphoniaError::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => {
                        return Ok(None);
                    }
                    Err(source) => {
                        return Err(MediaError::Decode {
                            path: path.to_owned(),
                            source,
                        });
                    }
                };
                if packet.track_id == *track_id {
                    return Ok(Some(packet));
                }
            },
            Source::Caf(packets) => Ok(packets
                .next_packet()
                .map_err(|e| MediaError::io("read", path, e))?
                .map(|data| Packet::new(0, Timestamp::new(0), Duration::new(0), data))),
        }
    }

    /// Decodes the next packet of the track into `pending`; `false` at the end of the stream.
    fn decode_next(&mut self, channels: usize, path: &Path) -> Result<bool, MediaError> {
        loop {
            let Some(packet) = self.next_packet(path)? else {
                return Ok(false);
            };
            let buffer = decoder(&mut self.decoder, &self.params, path)?
                .decode(&packet)
                .map_err(|source| MediaError::Decode {
                    path: path.to_owned(),
                    source,
                })?;
            if buffer.spec().channels().count() != channels {
                return Err(MediaError::malformed(path, "channel count changed"));
            }
            self.pending.resize(buffer.frames() * channels, 0.0);
            buffer.copy_to_slice_interleaved::<f64, _>(&mut self.pending);
            if let Some(quirks) = &mut self.quirks {
                quirks.apply(&packet.data, &mut self.pending);
            }
            self.pending_pos = 0;
            if !self.pending.is_empty() {
                return Ok(true);
            }
        }
    }

    /// libsndfile's ALAC frame count (`alac_reader_calc_frames`): 4096 frames for every packet
    /// but the last, plus the frames the last decodes to when the file holds it in full. The
    /// stream is then back at its first packet.
    fn alac_frames(&mut self, header: &CafHeader, path: &Path) -> Result<u64, MediaError> {
        let Source::Caf(packets) = &mut self.source else {
            return Ok(0);
        };
        let blocks = packets.alac_packets();
        if blocks == 0 {
            return Ok(0);
        }
        let per_packet = u64::from(header.frames_per_packet);
        let full = per_packet * (blocks - 1);
        packets.seek(full);
        let last = packets
            .next_packet()
            .map_err(|e| MediaError::io("read", path, e))?;
        packets.seek(0);
        // libsndfile's decoder must open with the file's channel count (`alac_reader_init`).
        let made = decoder(&mut self.decoder, &self.params, path)
            .map_err(|_| MediaError::rejected(path, Rejection::Internal))?;
        if made.codec_params().channels.as_ref().map(Channels::count)
            != usize::try_from(header.channels).ok()
        {
            return Err(MediaError::rejected(path, Rejection::Internal));
        }
        let frames = last.map_or(0, |data| {
            let packet = Packet::new(0, Timestamp::new(0), Duration::new(0), data);
            made.decode(&packet)
                .map_or(0, |buffer| buffer.frames() as u64)
        });
        made.reset();
        Ok(full + frames)
    }
}

/// The decoder in `slot`, made on first use.
fn decoder<'a>(
    slot: &'a mut Option<Box<dyn AudioDecoder>>,
    params: &AudioCodecParameters,
    path: &Path,
) -> Result<&'a mut dyn AudioDecoder, MediaError> {
    if slot.is_none() {
        let made = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|err| match err {
                SymphoniaError::Unsupported(_) => {
                    MediaError::unsupported(path, format!("codec {}", codec_name(params)))
                }
                other => MediaError::Decode {
                    path: path.to_owned(),
                    source: other,
                },
            })?;
        *slot = Some(made);
    }
    match slot {
        Some(made) => Ok(made.as_mut()),
        None => Err(MediaError::malformed(path, "no decoder")),
    }
}

/// A readable name for a codec symphonia demuxes but cannot decode.
fn codec_name(params: &AudioCodecParameters) -> String {
    match params.codec {
        codec::CODEC_ID_OPUS => "Opus".to_owned(),
        codec::CODEC_ID_ALAC => "ALAC".to_owned(),
        codec::CODEC_ID_AAC => "AAC".to_owned(),
        codec::CODEC_ID_ADPCM_MS | codec::CODEC_ID_ADPCM_IMA_WAV | codec::CODEC_ID_ADPCM_IMA_QT => {
            "ADPCM".to_owned()
        }
        codec::CODEC_ID_MP1 => "MPEG layer I".to_owned(),
        codec::CODEC_ID_MP2 => "MPEG layer II".to_owned(),
        other => format!("{other:?}"),
    }
}
