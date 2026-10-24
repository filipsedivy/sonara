//! Reading written files back: the prototype's `_verify` (`sf.info` and, for lossy formats,
//! `sf.read` plus `analyze`) and `_analyze_file` (the BWF loudness fields).
//!
//! Every container is read with this crate's readers, as libsndfile reads it (see
//! [`crate::sound_info`]): symphonia, the own RF64 and Wave64 reader, the CAF reader and the Ogg
//! Opus reader.

use std::path::Path;

use sonara_dsp::analysis::{AnalysisReport, Analyzer};

use super::{CHUNK, ExportError};
use crate::stream::AudioStream;

/// What `sf.info` reports for the verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileShape {
    pub(crate) sample_rate: u32,
    pub(crate) channels: usize,
    pub(crate) frames: u64,
}

fn analyzer(sample_rate: u32, channels: usize) -> Result<Analyzer, ExportError> {
    Analyzer::new(sample_rate, channels).map_err(|e| ExportError::InvalidArgument(e.to_string()))
}

/// The rate, channels and frames of a written file.
pub(crate) fn shape(path: &Path) -> Result<FileShape, ExportError> {
    let info = crate::sound_info(path)?;
    Ok(FileShape {
        sample_rate: info.sample_rate,
        channels: usize::try_from(info.channels).unwrap_or(0),
        frames: info.frames,
    })
}

/// `_analyze_file`: the written file read back in 65536-frame blocks as `float64`, with
/// libsndfile's integer scaling.
pub(crate) fn analyze_stream(path: &Path) -> Result<AnalysisReport, ExportError> {
    let mut stream = AudioStream::open(path)?.stream;
    let channels = stream.channels();
    let mut a = analyzer(stream.sample_rate(), channels)?;
    let mut block = vec![0.0; CHUNK * channels];
    loop {
        let frames = stream.read(&mut block)?;
        if frames == 0 {
            break;
        }
        a.feed(block.get(..frames * channels).unwrap_or_default());
        if frames < CHUNK {
            break;
        }
    }
    Ok(a.finish())
}
