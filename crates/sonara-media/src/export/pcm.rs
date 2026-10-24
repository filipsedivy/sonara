//! An own writer for the uncompressed containers, byte for byte as libsndfile 1.2.2 writes them
//! for the prototype's exports (`wav.c`, `wavlike.c`, `rf64.c`, `w64.c`, `aiff.c`, `caf.c`;
//! checked against the golden files and the recorded export cases).
//!
//! **Why not `hound`** (QG-13): `hound` writes no `LIST`/`INFO` strings, no `fact` and no
//! `PEAK` chunk, uses `WAVE_FORMAT_EXTENSIBLE` for 24- and 32-bit files where libsndfile writes
//! a plain tag-1 `fmt ` chunk, and writes none of RF64, Wave64, AIFF or CAF. The header bytes
//! are part of the export contract (WAV and BWF headers byte-identical, CTR-01/GLD-01), so the
//! layout is written here.
//!
//! What libsndfile writes, per container (`frames`, `data` = frames × channels × bytes):
//!
//! - **WAV**: `RIFF` (file length − 8) `WAVE`; `fmt ` of 16 bytes with tag 1 (PCM) or 3
//!   (float); for float a `fact` chunk with the frame count; the `LIST`/`INFO` strings in the
//!   order they were set; for float a `PEAK` chunk (version 1, the wall-clock time, per channel
//!   the largest absolute value and its frame); `data`; a pad byte after odd data, counted in
//!   the RIFF size but not in the data size.
//! - **RF64**: `RF64` 0xFFFFFFFF `WAVE`; `ds64` (RIFF size, data size, frames as 64-bit, no
//!   table); `fmt ` of 40 bytes (`WAVE_FORMAT_EXTENSIBLE`, channel mask, sub-format GUID);
//!   strings; `data` 0xFFFFFFFF; no `PEAK`.
//! - **Wave64**: GUID chunks with 64-bit sizes that include their 24-byte headers, the RIFF
//!   size being the file length; a 16-byte `fmt `; `fact` for float; no strings (libsndfile
//!   refuses them) and no pad byte.
//! - **AIFF**: big-endian; `FORM` (file length − 8) `AIFF`, or `AIFC` with an `FVER` chunk for
//!   float (`COMM` with `FL32`/`FL64`); the rate as an 80-bit float; `NAME`, `AUTH`, `ANNO`,
//!   `APPL` (`m3ga` + software) and `(c) ` strings; `PEAK` for float; `SSND` whose size counts
//!   the pad byte.
//! - **CAF**: big-endian; `desc`; an `info` chunk with every string as key/value pairs; `peak`
//!   for float (an edit count, no time); a `free` chunk that puts the audio at a multiple of
//!   4096 bytes; `data` with an edit count; a pad byte after odd data.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::strings::{StrKind, Strings};
use super::{Container, ExportClock, ExportError};

/// `WAVE_FORMAT_PCM`.
const FORMAT_PCM: u16 = 1;
/// `WAVE_FORMAT_IEEE_FLOAT`.
const FORMAT_FLOAT: u16 = 3;
/// `WAVE_FORMAT_EXTENSIBLE`.
const FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// The tail of the `KSDATAFORMAT_SUBTYPE_*` GUIDs after the format tag.
const GUID_TAIL: [u8; 14] = [
    0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];
/// The tail of Wave64's chunk GUIDs after the four-character code.
const W64_TAIL: [u8; 12] = [
    0xF3, 0xAC, 0xD3, 0x11, 0x8C, 0xD1, 0x00, 0xC0, 0x4F, 0x8E, 0xDB, 0x8A,
];
/// Wave64's `riff` GUID.
const W64_RIFF: [u8; 16] = [
    b'r', b'i', b'f', b'f', 0x2E, 0x91, 0xCF, 0x11, 0xA5, 0xD6, 0x28, 0xDB, 0x04, 0xC1, 0x00, 0x00,
];
/// AIFC version 1 (`FVER`).
const AIFC_VERSION: u32 = 0xA280_5140;
/// libsndfile's limit of the CAF `info` buffer (`struct put_buffer`).
const CAF_INFO_MAX: usize = 16 * 1024;

/// An uncompressed container the writer produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PcmContainer {
    /// RIFF WAVE (also the base of BWF).
    Wav,
    /// RF64 (EBU Tech 3306).
    Rf64,
    /// Sony Wave64.
    W64,
    /// AIFF, or AIFF-C for float samples.
    Aiff,
    /// Apple Core Audio Format.
    Caf,
}

impl PcmContainer {
    /// The export container this layout belongs to (BWF writes [`PcmContainer::Wav`]).
    pub(crate) fn of(container: Container) -> Option<Self> {
        Some(match container {
            Container::Wav | Container::Bwf => Self::Wav,
            Container::Rf64 => Self::Rf64,
            Container::W64 => Self::W64,
            Container::Aiff => Self::Aiff,
            Container::Caf => Self::Caf,
            _ => return None,
        })
    }

    /// The export container whose metadata rules apply.
    fn metadata_container(self) -> Container {
        match self {
            Self::Wav => Container::Wav,
            Self::Rf64 => Container::Rf64,
            Self::W64 => Container::W64,
            Self::Aiff => Container::Aiff,
            Self::Caf => Container::Caf,
        }
    }

    const fn big_endian(self) -> bool {
        matches!(self, Self::Aiff | Self::Caf)
    }
}

/// A sample encoding of the uncompressed containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleFormat {
    /// Signed 16-bit integers.
    Pcm16,
    /// Signed 24-bit integers.
    Pcm24,
    /// Signed 32-bit integers.
    Pcm32,
    /// 32-bit IEEE floats.
    Float,
    /// 64-bit IEEE floats.
    Double,
}

impl SampleFormat {
    /// Bytes per sample.
    #[must_use]
    pub const fn bytes(self) -> usize {
        match self {
            Self::Pcm16 => 2,
            Self::Pcm24 => 3,
            Self::Pcm32 | Self::Float => 4,
            Self::Double => 8,
        }
    }

    const fn is_float(self) -> bool {
        matches!(self, Self::Float | Self::Double)
    }

    fn bits(self) -> u16 {
        u16::try_from(self.bytes() * 8).unwrap_or(u16::MAX)
    }
}

/// The largest absolute value of one channel and its frame (libsndfile's `PEAK_POS`).
#[derive(Debug, Clone, Copy, Default)]
struct Peak {
    value: f64,
    position: u64,
}

/// Writes one uncompressed audio file. Create it, write interleaved samples in the encoding's
/// type, then [`finish`](Self::finish) it; the header is written first with no frames and
/// rewritten with the final sizes, as libsndfile does.
pub struct PcmFileWriter {
    file: BufWriter<File>,
    path: PathBuf,
    container: PcmContainer,
    format: SampleFormat,
    channels: u16,
    sample_rate: u32,
    strings: Strings,
    written: Vec<String>,
    peaks: Option<Vec<Peak>>,
    frames: u64,
    header_len: u64,
    clock: ExportClock,
    bytes: Vec<u8>,
}

impl std::fmt::Debug for PcmFileWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcmFileWriter")
            .field("path", &self.path)
            .field("container", &self.container)
            .field("format", &self.format)
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}

impl PcmFileWriter {
    /// Creates (or truncates) `path` and writes the provisional header. `metadata` is offered
    /// to the container as the prototype sets it (see [`PcmFileWriter::metadata_written`]);
    /// `clock` stamps the `PEAK` chunk of float WAV and AIFF files.
    ///
    /// # Errors
    ///
    /// [`ExportError::InvalidArgument`] for a channel count other than 1 to 65535 or a zero
    /// rate, and [`ExportError::Io`] when the file cannot be created or written.
    pub fn create(
        path: &Path,
        container: PcmContainer,
        format: SampleFormat,
        channels: u16,
        sample_rate: u32,
        metadata: &[(String, String)],
        clock: ExportClock,
    ) -> Result<Self, ExportError> {
        if channels == 0 || sample_rate == 0 {
            return Err(ExportError::InvalidArgument(format!(
                "cannot write {channels} channels at {sample_rate} Hz"
            )));
        }
        let (strings, written) = Strings::set(container.metadata_container(), metadata);
        let file = File::create(path).map_err(|e| ExportError::io("create", path, e))?;
        // libsndfile adds a PEAK chunk to float WAV, AIFF and CAF files, not to RF64 or Wave64.
        let peaks = (format.is_float()
            && matches!(
                container,
                PcmContainer::Wav | PcmContainer::Aiff | PcmContainer::Caf
            ))
        .then(|| vec![Peak::default(); usize::from(channels)]);
        let mut writer = Self {
            file: BufWriter::with_capacity(1 << 20, file),
            path: path.to_owned(),
            container,
            format,
            channels,
            sample_rate,
            strings,
            written,
            peaks,
            frames: 0,
            header_len: 0,
            clock,
            bytes: Vec::new(),
        };
        let header = writer.header(0, 0);
        writer.header_len = len_u64(header.len());
        writer
            .file
            .write_all(&header)
            .map_err(|e| ExportError::io("write", path, e))?;
        Ok(writer)
    }

    /// The metadata keys the container accepted, in order (the prototype's
    /// `metadata_written`).
    #[must_use]
    pub fn metadata_written(&self) -> &[String] {
        &self.written
    }

    /// Frames written so far.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Writes interleaved integer samples of a PCM encoding, each in the encoding's range
    /// (a 24-bit sample is in `-2^23..2^23`).
    ///
    /// # Errors
    ///
    /// [`ExportError::InvalidArgument`] for a float encoding or a partial frame, and
    /// [`ExportError::Io`] when writing fails.
    pub fn write_pcm(&mut self, samples: &[i32]) -> Result<(), ExportError> {
        self.check_frames(samples.len())?;
        let be = self.container.big_endian();
        self.bytes.clear();
        match self.format {
            SampleFormat::Pcm16 => {
                for &s in samples {
                    let v = i16::try_from(s.clamp(-32768, 32767)).unwrap_or_default();
                    self.bytes.extend_from_slice(&if be {
                        v.to_be_bytes()
                    } else {
                        v.to_le_bytes()
                    });
                }
            }
            SampleFormat::Pcm24 => {
                for &s in samples {
                    let b = s.to_le_bytes();
                    if be {
                        self.bytes.extend_from_slice(&[b[2], b[1], b[0]]);
                    } else {
                        self.bytes.extend_from_slice(&[b[0], b[1], b[2]]);
                    }
                }
            }
            SampleFormat::Pcm32 => {
                for &s in samples {
                    self.bytes.extend_from_slice(&if be {
                        s.to_be_bytes()
                    } else {
                        s.to_le_bytes()
                    });
                }
            }
            SampleFormat::Float | SampleFormat::Double => {
                return Err(ExportError::InvalidArgument(
                    "integer samples for a float encoding".to_owned(),
                ));
            }
        }
        self.write_bytes(samples.len())
    }

    /// Writes interleaved 32-bit float samples to a [`SampleFormat::Float`] file (one
    /// `sf_write_float` call: the `PEAK` values are updated over the whole block).
    ///
    /// # Errors
    ///
    /// [`ExportError::InvalidArgument`] for another encoding or a partial frame, and
    /// [`ExportError::Io`] when writing fails.
    pub fn write_f32(&mut self, samples: &[f32]) -> Result<(), ExportError> {
        self.check_frames(samples.len())?;
        if self.format != SampleFormat::Float {
            return Err(ExportError::InvalidArgument(
                "float samples for a non-float encoding".to_owned(),
            ));
        }
        self.update_peaks(samples.iter().map(|&v| f64::from(v.abs())));
        let be = self.container.big_endian();
        self.bytes.clear();
        for &s in samples {
            self.bytes
                .extend_from_slice(&if be { s.to_be_bytes() } else { s.to_le_bytes() });
        }
        self.write_bytes(samples.len())
    }

    /// Writes interleaved 64-bit float samples to a [`SampleFormat::Double`] file.
    ///
    /// # Errors
    ///
    /// [`ExportError::InvalidArgument`] for another encoding or a partial frame, and
    /// [`ExportError::Io`] when writing fails.
    pub fn write_f64(&mut self, samples: &[f64]) -> Result<(), ExportError> {
        self.check_frames(samples.len())?;
        if self.format != SampleFormat::Double {
            return Err(ExportError::InvalidArgument(
                "double samples for a non-double encoding".to_owned(),
            ));
        }
        self.update_peaks(samples.iter().map(|v| v.abs()));
        let be = self.container.big_endian();
        self.bytes.clear();
        for &s in samples {
            self.bytes
                .extend_from_slice(&if be { s.to_be_bytes() } else { s.to_le_bytes() });
        }
        self.write_bytes(samples.len())
    }

    /// Writes the trailing pad byte and the final header, and closes the file.
    ///
    /// # Errors
    ///
    /// [`ExportError::Io`] when writing fails.
    pub fn finish(mut self) -> Result<(), ExportError> {
        let data_bytes = self.data_bytes();
        let pad = u64::from(self.container != PcmContainer::W64 && data_bytes % 2 == 1);
        if pad == 1 {
            self.file
                .write_all(&[0])
                .map_err(|e| ExportError::io("write", &self.path, e))?;
        }
        let file_len = self.header_len + data_bytes + pad;
        let header = self.header(self.frames, file_len);
        if len_u64(header.len()) != self.header_len {
            return Err(ExportError::Format(format!(
                "{}: header length changed",
                self.path.display()
            )));
        }
        let path = self.path.clone();
        let io = |e| ExportError::io("write", &path, e);
        self.file.seek(SeekFrom::Start(0)).map_err(io)?;
        self.file.write_all(&header).map_err(io)?;
        self.file.flush().map_err(io)?;
        Ok(())
    }

    fn check_frames(&self, samples: usize) -> Result<(), ExportError> {
        if samples.is_multiple_of(usize::from(self.channels)) {
            Ok(())
        } else {
            Err(ExportError::InvalidArgument(
                "a block ends with a partial frame".to_owned(),
            ))
        }
    }

    fn write_bytes(&mut self, samples: usize) -> Result<(), ExportError> {
        self.file
            .write_all(&self.bytes)
            .map_err(|e| ExportError::io("write", &self.path, e))?;
        self.frames += len_u64(samples / usize::from(self.channels));
        Ok(())
    }

    /// libsndfile's `float32_peak_update` and `double64_peak_update` over one write call: per
    /// channel the first largest absolute value of the block, kept when it beats the stored
    /// one (strictly), at the frame `write_current + index`.
    fn update_peaks(&mut self, values: impl Iterator<Item = f64>) {
        let Some(peaks) = self.peaks.as_mut() else {
            return;
        };
        let channels = peaks.len();
        let mut block: Vec<Option<(f64, u64)>> = vec![None; channels];
        for (k, v) in values.enumerate() {
            let channel = k % channels;
            let frame = len_u64(k / channels);
            if let Some(slot) = block.get_mut(channel) {
                match slot {
                    None => *slot = Some((v, frame)),
                    Some((max, position)) => {
                        if *max < v {
                            *max = v;
                            *position = frame;
                        }
                    }
                }
            }
        }
        for (peak, found) in peaks.iter_mut().zip(block) {
            if let Some((max, position)) = found
                && max > peak.value
            {
                peak.value = max;
                peak.position = self.frames + position;
            }
        }
    }

    fn data_bytes(&self) -> u64 {
        self.frames * len_u64(self.format.bytes()) * u64::from(self.channels)
    }

    /// The header for `frames` frames in a file of `file_len` bytes (0 while writing).
    fn header(&self, frames: u64, file_len: u64) -> Vec<u8> {
        let data_bytes = frames * len_u64(self.format.bytes()) * u64::from(self.channels);
        match self.container {
            PcmContainer::Wav => self.wav_header(frames, data_bytes, file_len),
            PcmContainer::Rf64 => self.rf64_header(frames, data_bytes, file_len),
            PcmContainer::W64 => self.w64_header(frames, data_bytes, file_len),
            PcmContainer::Aiff => self.aiff_header(frames, file_len),
            PcmContainer::Caf => self.caf_header(data_bytes),
        }
    }

    fn block_align(&self) -> u16 {
        self.format.bits() / 8 * self.channels
    }

    fn byte_rate(&self) -> u32 {
        self.sample_rate.wrapping_mul(u32::from(self.block_align()))
    }

    fn wav_header(&self, frames: u64, data_bytes: u64, file_len: u64) -> Vec<u8> {
        let mut h = Vec::with_capacity(512);
        h.extend_from_slice(b"RIFF");
        put_u32_le(&mut h, clamp_u32(file_len.max(8) - 8));
        h.extend_from_slice(b"WAVE");
        h.extend_from_slice(b"fmt ");
        put_u32_le(&mut h, 16);
        let tag = if self.format.is_float() {
            FORMAT_FLOAT
        } else {
            FORMAT_PCM
        };
        self.fmt_body(&mut h, tag);
        if self.format.is_float() {
            h.extend_from_slice(b"fact");
            put_u32_le(&mut h, 4);
            put_u32_le(&mut h, low_u32(frames));
        }
        self.list_info(&mut h);
        self.peak_chunk(&mut h, false);
        h.extend_from_slice(b"data");
        put_u32_le(&mut h, clamp_u32(data_bytes));
        h
    }

    fn rf64_header(&self, frames: u64, data_bytes: u64, file_len: u64) -> Vec<u8> {
        let mut h = Vec::with_capacity(512);
        h.extend_from_slice(b"RF64");
        put_u32_le(&mut h, u32::MAX);
        h.extend_from_slice(b"WAVE");
        h.extend_from_slice(b"ds64");
        put_u32_le(&mut h, 28);
        h.extend_from_slice(&file_len.wrapping_sub(8).to_le_bytes());
        h.extend_from_slice(&data_bytes.to_le_bytes());
        h.extend_from_slice(&frames.to_le_bytes());
        put_u32_le(&mut h, 0);
        h.extend_from_slice(b"fmt ");
        put_u32_le(&mut h, 40);
        self.fmt_body(&mut h, FORMAT_EXTENSIBLE);
        h.extend_from_slice(&22u16.to_le_bytes());
        h.extend_from_slice(&self.format.bits().to_le_bytes());
        let mask: u32 = match self.channels {
            1 => 0x4,
            2 => 0x3,
            4 => 0x33,
            6 => 0x3F,
            8 => 0xFF,
            _ => 0,
        };
        put_u32_le(&mut h, mask);
        let sub = if self.format.is_float() {
            FORMAT_FLOAT
        } else {
            FORMAT_PCM
        };
        h.extend_from_slice(&sub.to_le_bytes());
        h.extend_from_slice(&GUID_TAIL);
        self.list_info(&mut h);
        h.extend_from_slice(b"data");
        put_u32_le(&mut h, u32::MAX);
        h
    }

    fn w64_header(&self, frames: u64, data_bytes: u64, file_len: u64) -> Vec<u8> {
        let mut h = Vec::with_capacity(160);
        h.extend_from_slice(&W64_RIFF);
        h.extend_from_slice(&file_len.to_le_bytes());
        w64_guid(&mut h, *b"wave");
        w64_guid(&mut h, *b"fmt ");
        h.extend_from_slice(&40u64.to_le_bytes());
        let tag = if self.format.is_float() {
            FORMAT_FLOAT
        } else {
            FORMAT_PCM
        };
        self.fmt_body(&mut h, tag);
        if self.format.is_float() {
            w64_guid(&mut h, *b"fact");
            h.extend_from_slice(&32u64.to_le_bytes());
            h.extend_from_slice(&frames.to_le_bytes());
        }
        w64_guid(&mut h, *b"data");
        h.extend_from_slice(&(data_bytes + 24).to_le_bytes());
        h
    }

    /// The common 16 bytes of a `fmt ` chunk: tag, channels, rate, byte rate, block align,
    /// bits per sample.
    fn fmt_body(&self, h: &mut Vec<u8>, tag: u16) {
        h.extend_from_slice(&tag.to_le_bytes());
        h.extend_from_slice(&self.channels.to_le_bytes());
        put_u32_le(h, self.sample_rate);
        put_u32_le(h, self.byte_rate());
        h.extend_from_slice(&self.block_align().to_le_bytes());
        h.extend_from_slice(&self.format.bits().to_le_bytes());
    }

    /// `wavlike_write_strings`: a `LIST`/`INFO` chunk when any string is set, with one
    /// NUL-terminated, even-padded sub-chunk per field the format names (the licence has no
    /// INFO field and is skipped).
    fn list_info(&self, h: &mut Vec<u8>) {
        if self.strings.is_empty() {
            return;
        }
        let start = h.len();
        h.extend_from_slice(b"LIST");
        put_u32_le(h, 0);
        h.extend_from_slice(b"INFO");
        for (kind, text) in self.strings.iter() {
            let id: &[u8; 4] = match kind {
                StrKind::Software => b"ISFT",
                StrKind::Title => b"INAM",
                StrKind::Copyright => b"ICOP",
                StrKind::Artist => b"IART",
                StrKind::Comment => b"ICMT",
                StrKind::Date => b"ICRD",
                StrKind::Genre => b"IGNR",
                StrKind::Album => b"IPRD",
                StrKind::TrackNumber => b"ITRK",
                StrKind::License => continue,
            };
            h.extend_from_slice(id);
            let size = text.len() + 1;
            let padded = size + (size & 1);
            put_u32_le(h, low_u32(len_u64(padded)));
            h.extend_from_slice(text);
            h.resize(h.len() + (padded - text.len()), 0);
        }
        let size = low_u32(len_u64(h.len() - start - 8));
        if let Some(slot) = h.get_mut(start + 4..start + 8) {
            slot.copy_from_slice(&size.to_le_bytes());
        }
    }

    /// The `PEAK` chunk of float WAV (little-endian) and AIFF (big-endian) files: version 1,
    /// the clock's seconds, then per channel the value as a float and the frame as 32 bits.
    fn peak_chunk(&self, h: &mut Vec<u8>, big_endian: bool) {
        let Some(peaks) = &self.peaks else {
            return;
        };
        let put = |h: &mut Vec<u8>, v: u32| {
            h.extend_from_slice(&if big_endian {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            });
        };
        h.extend_from_slice(b"PEAK");
        put(h, low_u32(8 + 8 * len_u64(peaks.len())));
        put(h, 1);
        put(h, clock_seconds(&self.clock));
        for peak in peaks {
            put(h, to_f32(peak.value).to_bits());
            put(h, low_u32(peak.position));
        }
    }

    fn aiff_header(&self, frames: u64, file_len: u64) -> Vec<u8> {
        let mut h = Vec::with_capacity(512);
        let aifc = self.format.is_float();
        h.extend_from_slice(b"FORM");
        put_u32_be(&mut h, low_u32(file_len.wrapping_sub(8)));
        if aifc {
            h.extend_from_slice(b"AIFC");
            h.extend_from_slice(b"FVER");
            put_u32_be(&mut h, 4);
            put_u32_be(&mut h, AIFC_VERSION);
        } else {
            h.extend_from_slice(b"AIFF");
        }
        h.extend_from_slice(b"COMM");
        put_u32_be(&mut h, if aifc { 24 } else { 18 });
        h.extend_from_slice(&self.channels.to_be_bytes());
        // `frames` counts the pad byte's share: libsndfile derives it from the SSND length.
        let data_len = file_len.saturating_sub(self.header_len);
        let stored_frames = if file_len == 0 {
            frames
        } else {
            data_len / (len_u64(self.format.bytes()) * u64::from(self.channels))
        };
        put_u32_be(&mut h, clamp_u32(stored_frames));
        h.extend_from_slice(&self.format.bits().to_be_bytes());
        h.extend_from_slice(&ten_byte_float(self.sample_rate));
        if aifc {
            h.extend_from_slice(if self.format == SampleFormat::Double {
                b"FL64"
            } else {
                b"FL32"
            });
            h.extend_from_slice(&[0, 0]);
        }
        for (kind, text) in self.strings.iter() {
            match kind {
                StrKind::Software => {
                    h.extend_from_slice(b"APPL");
                    put_u32_be(&mut h, low_u32(len_u64(text.len() + 4)));
                    h.extend_from_slice(b"m3ga");
                    h.extend_from_slice(text);
                    if text.len() % 2 == 1 {
                        h.push(0);
                    }
                }
                StrKind::Title | StrKind::Copyright | StrKind::Artist | StrKind::Comment => {
                    let id: &[u8; 4] = match kind {
                        StrKind::Title => b"NAME",
                        StrKind::Copyright => b"(c) ",
                        StrKind::Artist => b"AUTH",
                        _ => b"ANNO",
                    };
                    h.extend_from_slice(id);
                    put_u32_be(&mut h, low_u32(len_u64(text.len())));
                    h.extend_from_slice(text);
                    if text.len() % 2 == 1 {
                        h.push(0);
                    }
                }
                _ => {}
            }
        }
        self.peak_chunk(&mut h, true);
        h.extend_from_slice(b"SSND");
        put_u32_be(&mut h, clamp_u32(data_len + 8));
        put_u32_be(&mut h, 0);
        put_u32_be(&mut h, 0);
        h
    }

    fn caf_header(&self, data_bytes: u64) -> Vec<u8> {
        let mut h = Vec::with_capacity(4096);
        h.extend_from_slice(b"caff");
        h.extend_from_slice(&1u16.to_be_bytes());
        h.extend_from_slice(&0u16.to_be_bytes());
        h.extend_from_slice(b"desc");
        h.extend_from_slice(&32u64.to_be_bytes());
        h.extend_from_slice(&f64::from(self.sample_rate).to_be_bytes());
        h.extend_from_slice(b"lpcm");
        put_u32_be(&mut h, u32::from(self.format.is_float()));
        put_u32_be(&mut h, u32::from(self.block_align()));
        put_u32_be(&mut h, 1);
        put_u32_be(&mut h, u32::from(self.channels));
        put_u32_be(&mut h, u32::from(self.format.bits()));
        let mut info = Vec::new();
        let mut count = 0u32;
        for (kind, text) in self.strings.iter() {
            let key = kind.key().as_bytes();
            if info.len() + key.len() + text.len() + 2 > CAF_INFO_MAX {
                continue;
            }
            info.extend_from_slice(key);
            info.push(0);
            info.extend_from_slice(text);
            info.push(0);
            count += 1;
        }
        if count > 0 {
            h.extend_from_slice(b"info");
            h.extend_from_slice(&(len_u64(info.len()) + 4).to_be_bytes());
            put_u32_be(&mut h, count);
            h.extend_from_slice(&info);
        }
        if let Some(peaks) = &self.peaks {
            h.extend_from_slice(b"peak");
            h.extend_from_slice(&(4 + 12 * len_u64(peaks.len())).to_be_bytes());
            put_u32_be(&mut h, 0);
            for peak in peaks {
                h.extend_from_slice(&to_f32(peak.value).to_be_bytes());
                h.extend_from_slice(&peak.position.to_be_bytes());
            }
        }
        // A `free` chunk puts the audio at a multiple of 4096 bytes.
        let mut free = 0x1000_i64 - i64::try_from(h.len()).unwrap_or(0) - 16 - 12;
        while free < 0 {
            free += 0x1000;
        }
        h.extend_from_slice(b"free");
        h.extend_from_slice(&free.to_be_bytes());
        h.resize(h.len() + usize::try_from(free).unwrap_or(0), 0);
        h.extend_from_slice(b"data");
        h.extend_from_slice(&(data_bytes + 4).to_be_bytes());
        put_u32_be(&mut h, 0);
        h
    }
}

/// The clock's seconds since 1970 as libsndfile stores `time (NULL)` in a `PEAK` chunk.
fn clock_seconds(clock: &ExportClock) -> u32 {
    let now: SystemTime = clock();
    let secs = now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    low_u32(secs)
}

/// libsndfile's `uint2tenbytefloat`: an unsigned integer as an 80-bit extended float.
fn ten_byte_float(num: u32) -> [u8; 10] {
    let mut bytes = [0u8; 10];
    if num <= 1 {
        bytes[0] = 0x3F;
        bytes[1] = 0xFF;
        bytes[2] = 0x80;
        return bytes;
    }
    bytes[0] = 0x40;
    let mut mask: u32 = 0x4000_0000;
    if num >= mask {
        bytes[1] = 0x1D;
        return bytes;
    }
    let mut count = 0u32;
    while count < 32 {
        if num & mask != 0 {
            break;
        }
        mask >>= 1;
        count += 1;
    }
    let shifted = if count < 31 { num << (count + 1) } else { 0 };
    bytes[1] = u8::try_from(29 - count).unwrap_or(0);
    bytes[2..6].copy_from_slice(&shifted.to_be_bytes());
    bytes
}

fn w64_guid(h: &mut Vec<u8>, code: [u8; 4]) {
    h.extend_from_slice(&code);
    h.extend_from_slice(&W64_TAIL);
}

fn put_u32_le(h: &mut Vec<u8>, v: u32) {
    h.extend_from_slice(&v.to_le_bytes());
}

fn put_u32_be(h: &mut Vec<u8>, v: u32) {
    h.extend_from_slice(&v.to_be_bytes());
}

/// `SF_MIN (value, UINT32_MAX)`.
fn clamp_u32(v: u64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

/// The low 32 bits, as libsndfile's 32-bit header fields store a 64-bit value.
#[expect(
    clippy::cast_possible_truncation,
    reason = "libsndfile writes the low 32 bits of these 64-bit values"
)]
fn low_u32(v: u64) -> u32 {
    v as u32
}

/// A peak value as the `float` libsndfile writes.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the C code converts the double peak to float"
)]
fn to_f32(v: f64) -> f32 {
    v as f32
}

fn len_u64(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::ten_byte_float;

    #[test]
    fn sample_rates_as_80_bit_floats_match_libsndfile() {
        assert_eq!(
            ten_byte_float(48_000),
            [0x40, 0x0E, 0xBB, 0x80, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            ten_byte_float(44_100),
            [0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(ten_byte_float(1), [0x3F, 0xFF, 0x80, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(ten_byte_float(1 << 30)[..2], [0x40, 0x1D]);
    }
}
