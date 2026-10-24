//! The take writer: not real-time code.
//!
//! The prototype wrote its takes on its render thread (`_drain_capture` in `_step`, through
//! libsndfile). Here a `sonara-takes` thread drains the capture ring into the take files, so file
//! I/O never runs on a real-time thread (RT-03): a deliberate deviation from the prototype's
//! threads, not from its files. It rests [`DRAIN_REST`] whenever the ring is empty, as the
//! prototype drained once per render step, and it measures the longest time between two drains,
//! the stall the capture ring must absorb.
//!
//! **The files** are the prototype's: WAV with the project's `record_subtype`, one per audio
//! take, with the take's input channels in the order its track names them. The samples are
//! converted as libsndfile 1.2.2 converts `float` input under soundfile, which clips: the sample
//! times 2³¹ in `float`, full scale at or beyond ±2³¹, otherwise rounded to the nearest integer
//! (ties to even) and shifted right by 16 bits for `PCM_16` and by 8 for `PCM_24`
//! (`f2les_clip_array`, `f2let_clip_array`, `f2lei_clip_array`). `FLOAT` and `DOUBLE` store the
//! value. The header is [`PcmFileWriter`]'s, byte for byte libsndfile's.
//!
//! **Takes and frames.** The writer keeps only the frames of its own take
//! ([`TakeId`]): frames an earlier take left in the ring are dropped. The take's capture start is
//! the timeline frame the device callback stamped on it.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use sonara_media::export::{
    ExportClock, ExportError, PcmContainer, PcmFileWriter, SampleFormat, system_clock,
};
use sonara_rt::{CaptureReader, TakeId};

/// Frames one read takes from the ring (the prototype's `read_input(8192)`).
const DRAIN_FRAMES: usize = 8192;

/// How long the writer rests when the ring is empty: about one render step of the prototype,
/// which drained the ring once per step.
pub const DRAIN_REST: Duration = Duration::from_millis(5);

/// 2³¹, the scale of libsndfile's clipping conversions.
const SCALE: f32 = 2_147_483_648.0;

/// An audio take to record: the track, the file and how it is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeTarget {
    /// The audio track the take belongs to.
    pub track_id: String,
    /// The WAV file to write (created, or truncated, when the recording starts).
    pub path: PathBuf,
    /// The device input channels (0-based) the take keeps, in this order.
    pub channels: Vec<usize>,
    /// The sample encoding: the project's `record_subtype`.
    pub format: SampleFormat,
}

/// An audio take after the recording: its file is finished and closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedTake {
    /// The audio track.
    pub track_id: String,
    /// The take's file.
    pub path: PathBuf,
    /// The frames written.
    pub frames: u64,
    /// Why writing the file failed, if it did; the file is then incomplete.
    pub error: Option<String>,
}

/// How the writer kept up: the capture ring must hold the longest gap between two drains.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriterStats {
    /// Drains of the ring (each reads until the ring is empty).
    pub drains: u64,
    /// The longest time from the start of one drain to the start of the next: the stall the
    /// capture ring absorbed.
    pub longest_gap: Duration,
    /// The longest single drain, file writes included.
    pub longest_drain: Duration,
    /// Frames of the take read from the ring.
    pub frames: u64,
}

/// One take's file while it is written.
struct TakeFile {
    track_id: String,
    path: PathBuf,
    channels: Vec<usize>,
    format: SampleFormat,
    writer: Option<PcmFileWriter>,
    frames: u64,
    error: Option<String>,
    ints: Vec<i32>,
    floats: Vec<f32>,
    doubles: Vec<f64>,
}

impl TakeFile {
    /// Appends the take's channels of `data`, interleaved frames of `channels` input channels.
    fn write(&mut self, data: &[f32], channels: usize) {
        let Self {
            channels: columns,
            format,
            writer,
            ints,
            floats,
            doubles,
            ..
        } = self;
        let Some(file) = writer.as_mut() else {
            return;
        };
        let format = *format;
        let samples = data.chunks_exact(channels.max(1)).flat_map(|frame| {
            columns
                .iter()
                .map(move |&c| frame.get(c).copied().unwrap_or(0.0))
        });
        let result = match format {
            SampleFormat::Pcm16 | SampleFormat::Pcm24 | SampleFormat::Pcm32 => {
                ints.clear();
                ints.extend(samples.map(|x| pcm_sample(x, format)));
                file.write_pcm(ints)
            }
            SampleFormat::Float => {
                floats.clear();
                floats.extend(samples);
                file.write_f32(floats)
            }
            SampleFormat::Double => {
                doubles.clear();
                doubles.extend(samples.map(f64::from));
                file.write_f64(doubles)
            }
        };
        match result {
            Ok(()) => self.frames = file.frames(),
            Err(error) => self.fail(&error),
        }
    }

    /// Stops writing after `error`; the file stays as far as it was written.
    fn fail(&mut self, error: &ExportError) {
        self.error = Some(error.to_string());
        self.writer = None;
    }

    /// Writes the final header and closes the file.
    fn finish(mut self) -> RecordedTake {
        if let Some(writer) = self.writer.take()
            && let Err(error) = writer.finish()
        {
            self.error = Some(error.to_string());
        }
        RecordedTake {
            track_id: self.track_id,
            path: self.path,
            frames: self.frames,
            error: self.error,
        }
    }
}

/// libsndfile's clipping conversion of one `float` sample to `format`'s integers (see the
/// module docs). NaN, which a device never delivers, becomes 0.
fn pcm_sample(x: f32, format: SampleFormat) -> i32 {
    let scaled = x * SCALE;
    let (shift, max) = match format {
        SampleFormat::Pcm16 => (16, i32::from(i16::MAX)),
        SampleFormat::Pcm24 => (8, 0x7F_FFFF),
        _ => (0, i32::MAX),
    };
    if f64::from(scaled) >= f64::from(i32::MAX) {
        return max;
    }
    if f64::from(scaled) <= f64::from(i32::MIN) {
        return -max - 1;
    }
    if scaled.is_nan() {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value is integral and inside the i32 range, checked above"
    )]
    let value = scaled.round_ties_even() as i32;
    value >> shift
}

/// The take files of a recording, created and with their provisional headers written.
pub(crate) struct OpenTakes(Vec<TakeFile>);

impl fmt::Debug for OpenTakes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(|take| &take.path))
            .finish()
    }
}

impl OpenTakes {
    /// Creates every take's file at `sample_rate` (`sf.SoundFile(path, "w", …)` in `record`).
    ///
    /// # Errors
    ///
    /// The path and the reason of the first file that cannot be created; the files created
    /// before it are removed again.
    pub(crate) fn create(targets: &[TakeTarget], sample_rate: u32) -> Result<Self, String> {
        let clock: ExportClock = system_clock();
        let mut files = Vec::with_capacity(targets.len());
        for target in targets {
            let created = u16::try_from(target.channels.len())
                .map_err(|_| ExportError::InvalidArgument("too many take channels".to_owned()))
                .and_then(|channels| {
                    PcmFileWriter::create(
                        &target.path,
                        PcmContainer::Wav,
                        target.format,
                        channels,
                        sample_rate,
                        &[],
                        Arc::clone(&clock),
                    )
                });
            match created {
                Ok(writer) => files.push(TakeFile {
                    track_id: target.track_id.clone(),
                    path: target.path.clone(),
                    channels: target.channels.clone(),
                    format: target.format,
                    writer: Some(writer),
                    frames: 0,
                    error: None,
                    ints: Vec::new(),
                    floats: Vec::new(),
                    doubles: Vec::new(),
                }),
                Err(error) => {
                    let message = format!("{}: {error}", target.path.display());
                    for file in files {
                        drop(file.writer);
                        // Best effort: the error being reported is the creation failure.
                        drop(std::fs::remove_file(&file.path));
                    }
                    return Err(message);
                }
            }
        }
        Ok(Self(files))
    }

    /// Finishes the files without a writer thread (a recording that never captured).
    pub(crate) fn finish(self) -> Vec<RecordedTake> {
        self.0.into_iter().map(TakeFile::finish).collect()
    }
}

/// What the writer thread hands back when the recording ends.
pub(crate) struct WriterResult {
    /// The stream's capture reader, for the next recording on the same stream.
    pub(crate) reader: CaptureReader,
    pub(crate) takes: Vec<RecordedTake>,
    /// The take's capture start, once its first frame was read.
    pub(crate) capture_start: Option<i64>,
    pub(crate) stats: WriterStats,
}

/// The running `sonara-takes` thread of one recording. Dropping it stops the thread and
/// finishes the files.
pub(crate) struct TakeWriter {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<WriterResult>>,
}

impl fmt::Debug for TakeWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TakeWriter").finish_non_exhaustive()
    }
}

impl TakeWriter {
    /// Starts the thread that drains `reader` into `takes`, keeping the frames of `take`.
    ///
    /// # Errors
    ///
    /// The system's error when the thread cannot be started. The closure that owned the reader
    /// and the files is dropped with it: the files keep their provisional headers, and the
    /// stream cannot capture again until it is reopened.
    pub(crate) fn spawn(
        reader: CaptureReader,
        take: TakeId,
        takes: OpenTakes,
    ) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = thread::Builder::new()
            .name("sonara-takes".to_owned())
            .spawn(move || drain(reader, take, takes, &flag))?;
        Ok(Self {
            stop,
            handle: Some(handle),
        })
    }

    /// Stops the thread after a last drain, finishes the files and returns what it wrote;
    /// `None` when the thread panicked.
    pub(crate) fn finish(mut self) -> Option<WriterResult> {
        self.join()
    }

    fn join(&mut self) -> Option<WriterResult> {
        // Release ↔ the Acquire load in `drain`.
        self.stop.store(true, Ordering::Release);
        self.handle.take().and_then(|handle| handle.join().ok())
    }
}

impl Drop for TakeWriter {
    fn drop(&mut self) {
        drop(self.join());
    }
}

/// The writer thread's loop: drains the ring until it is empty, rests, and repeats until it is
/// stopped; then drains once more and finishes the files.
fn drain(
    mut reader: CaptureReader,
    take: TakeId,
    takes: OpenTakes,
    stop: &AtomicBool,
) -> WriterResult {
    let mut files = takes.0;
    let channels = reader.channels().max(1);
    let mut buffer = vec![0.0_f32; DRAIN_FRAMES * channels];
    let mut stats = WriterStats::default();
    let mut capture_start = None;
    let mut last: Option<Instant> = None;
    loop {
        // Acquire ↔ the Release store in `TakeWriter::join`: a stop seen here is followed by
        // one more complete drain.
        let stopping = stop.load(Ordering::Acquire);
        let began = Instant::now();
        if let Some(last) = last {
            stats.longest_gap = stats.longest_gap.max(began.duration_since(last));
        }
        last = Some(began);
        loop {
            let chunk = reader.read(&mut buffer);
            if chunk.frames == 0 {
                break;
            }
            // Frames an earlier take left in the ring are not this take's.
            let Some(started) = chunk.take.filter(|t| t.id == take) else {
                continue;
            };
            capture_start.get_or_insert(started.start.get());
            let data = buffer.get(..chunk.frames * channels).unwrap_or(&[]);
            for file in &mut files {
                file.write(data, channels);
            }
            stats.frames += u64::try_from(chunk.frames).unwrap_or(u64::MAX);
        }
        stats.drains += 1;
        stats.longest_drain = stats.longest_drain.max(began.elapsed());
        if stopping {
            break;
        }
        thread::sleep(DRAIN_REST);
    }
    WriterResult {
        reader,
        takes: files.into_iter().map(TakeFile::finish).collect(),
        capture_start,
        stats,
    }
}

#[cfg(test)]
mod tests {
    use sonara_media::export::SampleFormat;

    use super::{OpenTakes, TakeTarget, pcm_sample};

    /// The probe's samples: zero, ±½, ±1.6 and 1.4 steps of 24 bits, just below full scale,
    /// ±1, ±1.5 and 2.5 steps of 32 bits.
    fn probe() -> [f32; 12] {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the f32 inputs of the probe"
        )]
        let step = |n: f64| (n / f64::from(1_u32 << 23)) as f32;
        [
            0.0,
            0.5,
            -0.5,
            step(1.6),
            step(1.4),
            step(-1.6),
            0.999_999_9,
            1.0,
            -1.0,
            1.5,
            -1.5,
            2.5 / 2_147_483_648.0 * 256.0,
        ]
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Writes `chunks` (interleaved frames of `inputs` device channels) as one take of
    /// `channels` and returns the file's bytes.
    fn take_bytes(
        format: SampleFormat,
        rate: u32,
        inputs: usize,
        channels: &[usize],
        chunks: &[&[f32]],
    ) -> Vec<u8> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("take.wav");
        let target = TakeTarget {
            track_id: "trk".to_owned(),
            path: path.clone(),
            channels: channels.to_vec(),
            format,
        };
        let mut takes = OpenTakes::create(&[target], rate).unwrap();
        for chunk in chunks {
            for file in &mut takes.0 {
                file.write(chunk, inputs);
            }
        }
        let finished = takes.finish();
        assert_eq!(finished[0].error, None);
        std::fs::read(&path).unwrap()
    }

    /// The prototype's take files byte for byte (soundfile 0.14.0 on libsndfile 1.2.2, written
    /// with `sf.SoundFile(path, "w", sr, channels, subtype, format="WAV")` on the macOS host on
    /// 2026-10-22): the 44-byte PCM header with tag 1 for every subtype, the sample conversion,
    /// a pad byte after odd data (counted in the RIFF size only), and an empty take.
    #[test]
    fn take_header_matches_prototype_subtype() {
        let probe = probe();
        let cases: [(SampleFormat, &str); 3] = [
            (
                SampleFormat::Pcm16,
                "524946463c00000057415645666d7420100000000100010080bb0000007701000200100064617461\
                 180000000000004000c000000000ffffff7fff7f0080ff7f00800000",
            ),
            (
                SampleFormat::Pcm24,
                "524946464800000057415645666d7420100000000100010080bb0000803202000300180064617461\
                 240000000000000000400000c0010000010000feffffffff7fffff7f000080ffff7f000080020000",
            ),
            (
                SampleFormat::Pcm32,
                "524946465400000057415645666d7420100000000100010080bb000000ee02000400200064617461\
                 300000000000000000000040000000c09a0100006601000066feffff00ffff7fffffff7f00000080\
                 ffffff7f0000008080020000",
            ),
        ];
        for (format, expected) in cases {
            assert_eq!(
                take_bytes(format, 48_000, 1, &[0], &[&probe]),
                hex(expected),
                "{format:?}"
            );
        }
        // Stereo at 44.1 kHz: the second channel is the first reversed and negated.
        let stereo: Vec<f32> = probe
            .iter()
            .zip(probe.iter().rev())
            .take(5)
            .flat_map(|(&l, &r)| [l, -r])
            .collect();
        assert_eq!(
            take_bytes(SampleFormat::Pcm24, 44_100, 2, &[0, 1], &[&stereo]),
            hex(
                "524946464200000057415645666d7420100000000100020044ac00009809040006001800646174611e\
                 000000000000fdffff000040ffff7f0000c0000080010000ffff7f010000000080"
            )
        );
        // Five frames in two writes: 15 bytes of data and a pad byte.
        assert_eq!(
            take_bytes(
                SampleFormat::Pcm24,
                48_000,
                1,
                &[0],
                &[&[0.0, 0.5], &[-0.5, 0.25, -0.25]]
            ),
            hex(
                "524946463400000057415645666d7420100000000100010080bb000080320200030018006461746\
                 10f0000000000000000400000c00000200000e000"
            )
        );
        assert_eq!(
            take_bytes(SampleFormat::Pcm24, 48_000, 1, &[0], &[]),
            hex(
                "524946462400000057415645666d7420100000000100010080bb0000803202000300180064617461\
                 00000000"
            )
        );
        // A take keeps its channels of the device input, in its track's order.
        let device: Vec<f32> = vec![0.5, 0.25, -0.5, -0.25];
        assert_eq!(
            take_bytes(SampleFormat::Pcm16, 48_000, 2, &[1], &[&device])[44..],
            [0x00, 0x20, 0x00, 0xE0]
        );
    }

    /// The prototype's samples (libsndfile 1.2.2 under soundfile 0.14.0, `PCM_16`, `PCM_24`
    /// and `PCM_32` WAV files written on the macOS host on 2026-10-22): clipping at full scale,
    /// and the 2³¹ scale that rounds before it shifts, so 1.6 steps of 24 bits truncate to 1
    /// and −1.6 to −2.
    #[test]
    fn samples_convert_as_libsndfile_does() {
        let inputs = probe();
        let expected: [(SampleFormat, [i32; 12]); 3] = [
            (
                SampleFormat::Pcm24,
                [
                    0, 4_194_304, -4_194_304, 1, 1, -2, 8_388_607, 8_388_607, -8_388_608,
                    8_388_607, -8_388_608, 2,
                ],
            ),
            (
                SampleFormat::Pcm16,
                [
                    0, 16_384, -16_384, 0, 0, -1, 32_767, 32_767, -32_768, 32_767, -32_768, 0,
                ],
            ),
            (
                SampleFormat::Pcm32,
                [
                    0,
                    1_073_741_824,
                    -1_073_741_824,
                    410,
                    358,
                    -410,
                    2_147_483_392,
                    2_147_483_647,
                    -2_147_483_648,
                    2_147_483_647,
                    -2_147_483_648,
                    640,
                ],
            ),
        ];
        for (format, values) in expected {
            let converted: Vec<i32> = inputs.iter().map(|&x| pcm_sample(x, format)).collect();
            assert_eq!(converted, values, "{format:?}");
        }
        assert_eq!(pcm_sample(f32::NAN, SampleFormat::Pcm24), 0);
    }
}
