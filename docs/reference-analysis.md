# Reference audio analysis (SON-P15)

`sonara_media::reference` describes a reference recording: how fast it is, how its energy moves
and where that energy changes. It extracts no melody, harmony or arrangement, and nothing in
the report can reproduce them. It is a library API; it adds no agent tool, CLI command or schema
field (CTR-01) and does not depend on the composition code.

## Use

- `ReferenceAnalyzer::new(sample_rate, channels)`, `feed(interleaved_f64)` in blocks of any size,
  `finish()` for the `ReferenceReport`. Only a 100 Hz RMS envelope of the mono mix is kept, so a
  long file costs about 800 bytes per second, and the report does not depend on the block size.
- `sonara_media::export::analyze_reference_file(path)` decodes a file in blocks and analyzes it.
  It reads what the media decoder reads: WAV, AIFF, FLAC, MP3, Ogg Vorbis and Opus, RF64, Wave64
  and CAF (the `ffmpeg` fallback of the importer is not used here).
- `ReferenceReport::to_json()` gives the report as JSON.

## The report

| Field | Meaning |
|---|---|
| `tempo.bpm`, `tempo.confidence` | The tempo (60 to 200 BPM) and the normalized autocorrelation at it, 0 to 1. It says how regular the pulse is, not how likely the tempo is musically right. |
| `tempo.alternatives_bpm` | Half and double the estimate (30 to 300 BPM): a pulse has no single correct octave. |
| `segments` | Stretches of similar energy covering the recording: start, end, mean RMS in dBFS and a low, medium or high level relative to the rest. |
| `energy_dbfs` | The RMS of every 0.5 s window. |
| `mean_rms_dbfs`, `duration_seconds`, `sample_rate` | Whole-recording values; the length is whole 10 ms frames. |
| `non_finite_samples` | NaN and infinite samples, counted as 0. |
| `limitations` | What the analysis does not do, and why a tempo is missing. |

## Methods

- **Tempo:** autocorrelation of the half-wave-rectified change of the envelope, refined by a
  parabola; of the peaks within 10 % of the strongest the shortest lag wins. No tempo below a
  confidence of 0.05 or under 6 s of audio. A constant tempo is assumed.
- **Segments:** a boundary where the next 2 s differ from the previous 2 s by at least 4 dB (the
  largest change within 2 s wins); segments last at least 2 s. A recording whose segments span
  under 3 dB is one medium level throughout. Segments follow energy, not musical form.

## Errors

`ReferenceError`: `InvalidSampleRate` (outside 8 kHz to 384 kHz), `InvalidChannels` (0 or more
than 64), `Empty` (no complete 10 ms frame) and `TooShort` (under 2 s). File analysis maps these
to `ExportError::InvalidArgument` (`INVALID_ARGUMENT`), and an unreadable or undecodable file to
`ExportError::Media`.

## Tests

`crates/sonara-media/tests/reference_analysis.rs` uses synthetic signals whose tempo and energy
are known by construction (click tracks, a quiet then loud part, silence), so no recording is
needed and there is nothing to license.
