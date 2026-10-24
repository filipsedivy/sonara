# ADR 0006: Audio codec backends

- **Status:** Accepted (2026-10-20)
- **Update (2026-10-24):** superseded in part by the Rust application, which keeps the formats
  and the encoders' output: decoding through symphonia and Sonara's own readers, FLAC export
  through libFLAC and MP3 export through LAME (both loaded at run time), Ogg Vorbis and Opus
  through `vorbis_rs` and libopus, resampling through libsoxr. See
  [dependencies.md](../dependencies.md).

## Context

The PRD requires reading and writing WAV/BWF, AIFF, FLAC, MP3, Ogg Vorbis and Opus. MP3
export must use a verified encoder. Codec binaries have redistribution obligations.

## Findings (verified on the development machine, 2026-10-20)

- `soundfile` 0.14 wheels bundle **libsndfile 1.2.2**. This build reads and writes WAV
  (PCM 16/24/32 and float), RF64, W64, AIFF, CAF, FLAC, Ogg Vorbis, Ogg Opus and MP3
  (MPEG Layer III encoding through LAME, decoding through mpg123).
- System FFmpeg 9.0 on the development machine has libmp3lame and libopus, but **not**
  libvorbis. Its native Vorbis encoder is marked experimental. FFmpeg is not present on
  every user machine.
- An Opus round-trip of a 0.5-peak 1 kHz sine came back with a 0.61 sample peak. Lossy
  codecs must therefore be verified by alignment-tolerant correlation and SNR, not by peak
  values.

## Decision

- **libsndfile (via soundfile) is the primary codec backend** for every format listed above.
  All required codec paths depend only on Python wheels.
- **FFmpeg is an optional external backend**, found on `PATH` or configured, used for
  decode-only formats that libsndfile lacks (for example AAC/M4A). Sonara does not bundle
  FFmpeg.
- Lossy exports are always rendered from the floating-point mix and never re-encoded from
  another lossy file. Imported sources are stored in their original form, and decoded caches
  are kept separate.
- Licensing: libsndfile (LGPL-2.1+), LAME (LGPL-2.0+) and mpg123 (LGPL-2.1) are dynamically
  linked inside the soundfile wheel. Redistribution obligations are listed in
  `docs/dependencies.md`.

## Consequences

The available formats are discovered at runtime (`sf.available_formats()`) and reported by
the export dialog and the agent tools. Sonara never assumes a codec exists.
