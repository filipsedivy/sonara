//! The reference analysis (SON-P15) on synthetic signals, whose tempo and energy are known by
//! construction. No recording is used, so there is nothing to license.

use sonara_media::reference::{EnergyLevel, ReferenceAnalyzer, ReferenceError};

const RATE: u32 = 22_050;

/// A click (a decaying 1 kHz burst of 20 ms) every `60 / bpm` seconds, at `gain`, for `seconds`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "test signal: small positive sample counts"
)]
fn clicks(bpm: f64, seconds: f64, gain: f64) -> Vec<f64> {
    let rate = f64::from(RATE);
    let frames = (seconds * rate) as usize;
    let period = 60.0 / bpm * rate;
    let burst = (0.02 * rate) as usize;
    let mut out = vec![0.0; frames];
    let mut beat = 0_u32;
    loop {
        let at = (f64::from(beat) * period) as usize;
        if at >= frames {
            break;
        }
        for i in 0..burst {
            let t = i as f64 / rate;
            let sample =
                gain * (-t * 150.0).exp() * (2.0 * std::f64::consts::PI * 1000.0 * t).sin();
            if let Some(slot) = out.get_mut(at + i) {
                *slot += sample;
            }
        }
        beat += 1;
    }
    out
}

fn analyze(
    samples: &[f64],
    channels: usize,
) -> Result<sonara_media::reference::ReferenceReport, ReferenceError> {
    let mut analyzer = ReferenceAnalyzer::new(RATE, channels)?;
    analyzer.feed(samples);
    analyzer.finish()
}

#[test]
fn a_click_track_reads_as_its_tempo_with_a_confidence() {
    for bpm in [90.0, 120.0, 150.0] {
        let report = analyze(&clicks(bpm, 24.0, 0.5), 1).expect("a report");
        let tempo = report.tempo.expect("a tempo");
        assert!(
            (tempo.bpm - bpm).abs() <= 1.5,
            "{bpm} BPM read as {}",
            tempo.bpm
        );
        assert!(
            tempo.confidence > 0.5,
            "{bpm}: confidence {}",
            tempo.confidence
        );
        assert!(
            tempo
                .alternatives_bpm
                .iter()
                .any(|a| (a - bpm * 2.0).abs() <= 3.0 || (a - bpm / 2.0).abs() <= 3.0)
        );
        assert!(
            report
                .limitations
                .iter()
                .any(|l| l.contains("constant pulse"))
        );
    }
}

#[test]
fn a_quiet_part_then_a_loud_part_makes_two_segments() {
    let mut samples = clicks(120.0, 10.0, 0.05);
    samples.extend(clicks(120.0, 10.0, 0.8));
    let report = analyze(&samples, 1).expect("a report");
    assert_eq!(report.segments.len(), 2, "{:?}", report.segments);
    let (quiet, loud) = (&report.segments[0], &report.segments[1]);
    assert!(
        (quiet.end_seconds - 10.0).abs() <= 1.0,
        "boundary at {}",
        quiet.end_seconds
    );
    assert_eq!(quiet.end_seconds, loud.start_seconds);
    assert_eq!(
        (quiet.energy, loud.energy),
        (EnergyLevel::Low, EnergyLevel::High)
    );
    assert!(loud.mean_rms_dbfs > quiet.mean_rms_dbfs + 20.0);
    assert_eq!(report.segments[1].end_seconds, report.duration_seconds);
}

#[test]
fn a_steady_signal_is_one_medium_segment() {
    let report = analyze(&clicks(100.0, 20.0, 0.5), 1).expect("a report");
    assert_eq!(report.segments.len(), 1);
    assert_eq!(report.segments[0].energy, EnergyLevel::Medium);
}

#[test]
fn silence_has_no_tempo_and_says_why() {
    let report = analyze(&vec![0.0; RATE as usize * 10], 1).expect("a report");
    assert!(report.tempo.is_none());
    assert_eq!(report.mean_rms_dbfs, -120.0);
    assert!(
        report
            .limitations
            .iter()
            .any(|l| l.contains("no regular pulse"))
    );
}

#[test]
fn a_short_recording_has_energy_but_no_tempo() {
    let report = analyze(&clicks(120.0, 4.0, 0.5), 1).expect("a report");
    assert!(report.tempo.is_none());
    assert!(
        report
            .limitations
            .iter()
            .any(|l| l.contains("shorter than"))
    );
}

#[test]
fn the_result_does_not_depend_on_the_block_size_or_the_channel_layout() {
    let mono = clicks(110.0, 12.0, 0.4);
    let whole = analyze(&mono, 1).expect("a report").to_json();
    let mut analyzer = ReferenceAnalyzer::new(RATE, 1).expect("analyzer");
    for block in mono.chunks(997) {
        analyzer.feed(block);
    }
    assert_eq!(analyzer.finish().expect("a report").to_json(), whole);
    // Two identical channels mix down to the same mono signal.
    let stereo: Vec<f64> = mono.iter().flat_map(|&s| [s, s]).collect();
    assert_eq!(analyze(&stereo, 2).expect("a report").to_json(), whole);
}

#[test]
fn bad_input_is_an_explicit_error() {
    assert_eq!(
        ReferenceAnalyzer::new(100, 1).unwrap_err(),
        ReferenceError::InvalidSampleRate(100)
    );
    assert_eq!(
        ReferenceAnalyzer::new(RATE, 0).unwrap_err(),
        ReferenceError::InvalidChannels(0)
    );
    assert_eq!(analyze(&[], 1).unwrap_err(), ReferenceError::Empty);
    assert!(
        matches!(analyze(&clicks(120.0, 1.0, 0.5), 1), Err(ReferenceError::TooShort(s)) if (s - 1.0).abs() < 0.02)
    );
    let mut samples = clicks(120.0, 8.0, 0.5);
    samples[10] = f64::NAN;
    samples[11] = f64::INFINITY;
    let report = analyze(&samples, 1).expect("a report");
    assert_eq!(report.non_finite_samples, 2);
    assert!(report.limitations.iter().any(|l| l.contains("2 NaN")));
}

#[test]
fn the_json_report_states_its_limits() {
    let json = analyze(&clicks(120.0, 12.0, 0.5), 1)
        .expect("a report")
        .to_json();
    assert!(json["tempo"]["confidence"].is_number());
    assert!(json["segments"][0]["energy"].is_string());
    assert!(
        json["limitations"][0]
            .as_str()
            .expect("text")
            .contains("no melody")
    );
}

/// A 16-bit mono PCM WAV of `samples` at `RATE`.
fn wav(samples: &[f64]) -> Vec<u8> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "samples are within [-1, 1]"
    )]
    let pcm: Vec<u8> = samples
        .iter()
        .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes())
        .collect();
    let len = u32::try_from(pcm.len()).expect("a small file");
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&pcm);
    out
}

#[test]
fn a_decoded_file_is_analyzed_and_bad_files_are_explicit_errors() {
    use sonara_media::export::{ExportError, analyze_reference_file};

    let dir = tempfile::tempdir().expect("a directory");
    let song = dir.path().join("click.wav");
    std::fs::write(&song, wav(&clicks(120.0, 12.0, 0.5))).expect("write");
    let report = analyze_reference_file(&song).expect("a report");
    let tempo = report.tempo.expect("a tempo");
    assert!((tempo.bpm - 120.0).abs() <= 1.5, "{}", tempo.bpm);
    assert_eq!(report.sample_rate, RATE);

    let short = dir.path().join("short.wav");
    std::fs::write(&short, wav(&clicks(120.0, 1.0, 0.5))).expect("write");
    assert!(
        matches!(analyze_reference_file(&short), Err(ExportError::InvalidArgument(m)) if m.contains("at least"))
    );

    let junk = dir.path().join("junk.wav");
    std::fs::write(&junk, b"not audio").expect("write");
    assert!(analyze_reference_file(&junk).is_err());
    assert!(analyze_reference_file(&dir.path().join("missing.wav")).is_err());
}
