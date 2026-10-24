//! FLAC export through libFLAC 1.4.3, loaded at run time (owner decision of 2026-10-22),
//! driven call for call as libsndfile 1.2.2 drives it for the prototype
//! (`src/flac.c`; the behaviour is reimplemented here, no code is copied), so the files are the
//! prototype's byte for byte:
//!
//! - **Open** (`flac_enc_init`): `FLAC__stream_encoder_new`; then the channel count, the sample
//!   rate, the bits per sample (16 or 24) and compression level 5
//!   (`FLAC_DEFAULT_COMPRESSION_LEVEL`: the prototype sets a compression level for the lossy
//!   formats only, and soundfile passes none). Nothing else is set: level 5 gives the block
//!   size of 4096, and there is no padding, no seek table and no verification.
//! - **First write** (`flac_write_header`): when at least one metadata string was accepted, one
//!   `VORBIS_COMMENT` block with a `name=value` entry per string in the order they were set,
//!   handed to `FLAC__stream_encoder_set_metadata`; without strings libFLAC writes an empty
//!   comment block itself. Either way libFLAC writes its own vendor string ("reference libFLAC
//!   1.4.3 20230623"). Then `FLAC__stream_encoder_init_stream` with write, seek and tell
//!   callbacks, which writes `fLaC`, `STREAMINFO` and the comment block.
//! - **Samples** (`flac_write_s2flac`, `flac_write_i2flac`): interleaved integers within the bit
//!   depth (libsndfile shifts the prototype's left-aligned 24-bit `int`s right by 8; the
//!   exporter's quantizer gives them unshifted), passed to
//!   `FLAC__stream_encoder_process_interleaved` in pieces of 2048 samples (libsndfile's
//!   8192-byte buffer of `int32_t`), each `write` call split on its own.
//! - **Close** (`flac_close`): `FLAC__stream_encoder_finish`, which encodes the last partial
//!   block and, through the seek callback, rewrites `STREAMINFO`'s frame sizes, total samples
//!   and MD5; then the encoder and the comment block are deleted.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::codec_ffi::{FlacEncodeError, FlacEncoder, FlacLibrary};

use super::ExportError;
use super::strings::Strings;

/// libsndfile's `FLAC_DEFAULT_COMPRESSION_LEVEL`.
const COMPRESSION_LEVEL: u32 = 5;

/// libsndfile's `ENC_BUFFER_SIZE` (8192) is a byte count turned into whole frames of `int32_t`
/// samples: `8192 / (4 * channels) * channels` samples per `process_interleaved` call.
const ENC_BUFFER_BYTES: usize = 8192;

/// Samples per `FLAC__stream_encoder_process_interleaved` call for `channels` (at least 1).
fn piece_samples(channels: usize) -> usize {
    let size_of_sample = std::mem::size_of::<i32>();
    (ENC_BUFFER_BYTES / (size_of_sample * channels)) * channels
}

/// The export error for a failed libFLAC call on `path`.
fn export_error(path: &Path, error: FlacEncodeError) -> ExportError {
    match error {
        FlacEncodeError::Codec(e) => ExportError::Codec(e),
        FlacEncodeError::Io(e) => ExportError::io("write", path, e),
    }
}

/// One FLAC file being written.
pub(crate) struct FlacWriter {
    encoder: FlacEncoder<BufWriter<File>>,
    path: PathBuf,
    strings: Strings,
    /// Samples per libFLAC call.
    piece: usize,
    /// Whether the stream is initialized (libsndfile's `have_written`).
    started: bool,
}

impl FlacWriter {
    /// Creates `path` for a FLAC stream at `sample_rate` with `channels` (1 or 2) of `bits`
    /// (16 or 24), carrying `strings`.
    pub(crate) fn create(
        path: &Path,
        sample_rate: u32,
        channels: usize,
        bits: u32,
        strings: Strings,
    ) -> Result<Self, ExportError> {
        let library = FlacLibrary::shared()?;
        let channel_count = u32::try_from(channels)
            .ok()
            .filter(|&c| c > 0)
            .ok_or_else(|| ExportError::InvalidArgument(format!("bad channel count {channels}")))?;
        let file = File::create(path).map_err(|e| ExportError::io("create", path, e))?;
        let mut encoder = FlacEncoder::new(library, BufWriter::with_capacity(1 << 20, file))?;
        encoder.set_channels(channel_count)?;
        encoder.set_sample_rate(sample_rate)?;
        encoder.set_bits_per_sample(bits)?;
        encoder.set_compression_level(COMPRESSION_LEVEL)?;
        Ok(Self {
            encoder,
            path: path.to_owned(),
            strings,
            piece: piece_samples(channels),
            started: false,
        })
    }

    /// `flac_write_header`, run before the first samples: the comment block, then the stream.
    fn start(&mut self) -> Result<(), ExportError> {
        if !self.strings.is_empty() {
            let fields: Vec<(&str, &[u8])> = self
                .strings
                .iter()
                .map(|(kind, text)| (kind.key(), text))
                .collect();
            self.encoder.set_vorbis_comments(&fields)?;
        }
        self.encoder
            .init_stream()
            .map_err(|e| export_error(&self.path, e))?;
        self.started = true;
        Ok(())
    }

    /// Encodes one block of interleaved samples (one `sf_writef_short` or `sf_writef_int`
    /// call), each within the bit depth.
    pub(crate) fn write(&mut self, samples: &[i32]) -> Result<(), ExportError> {
        if !self.started {
            self.start()?;
        }
        for piece in samples.chunks(self.piece) {
            self.encoder
                .process_interleaved(piece)
                .map_err(|e| export_error(&self.path, e))?;
        }
        Ok(())
    }

    /// Finishes the stream and writes the file out. A stream without samples is still
    /// initialized first, so the file is a valid empty FLAC stream (libsndfile would leave it
    /// empty; the exporter never finishes an output without samples).
    pub(crate) fn finish(mut self) -> Result<(), ExportError> {
        if !self.started {
            self.start()?;
        }
        let mut output = self
            .encoder
            .finish()
            .map_err(|e| export_error(&self.path, e))?;
        output
            .flush()
            .map_err(|e| ExportError::io("write", &self.path, e))
    }
}

#[cfg(test)]
mod tests {
    use super::piece_samples;

    #[test]
    fn libsndfile_feeds_2048_samples_per_call() {
        assert_eq!(piece_samples(1), 2048);
        assert_eq!(piece_samples(2), 2048);
    }
}
