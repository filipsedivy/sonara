//! The system's MIDI layer through midir: the API name the golden
//! transcripts record, and port listings through a real client.
//!
//! Listing needs CoreMIDI (macOS) or an ALSA sequencer (Linux). The development container has
//! no `/dev/snd/seq`: there, and only there, the listing test reports the missing
//! sequencer and skips.

use sonara_io::MidiError;
use sonara_io::midi;

#[test]
fn the_api_name_is_rtmidis() {
    let expected = if cfg!(target_os = "macos") {
        "core"
    } else if cfg!(target_os = "windows") {
        "winmm"
    } else {
        "alsa"
    };
    assert_eq!(midi::api_name(), expected);
}

#[test]
fn the_system_lists_its_ports() {
    match (midi::input_ports(), midi::output_ports()) {
        (Ok(inputs), Ok(outputs)) => {
            // The probe client creates no port of its own.
            assert!(!inputs.iter().any(|name| name == midi::PROBE_CLIENT_NAME));
            assert!(!outputs.iter().any(|name| name == midi::PROBE_CLIENT_NAME));
        }
        (Err(MidiError::Init), _) | (_, Err(MidiError::Init))
            if cfg!(target_os = "linux") && !std::path::Path::new("/dev/snd/seq").exists() =>
        {
            eprintln!("skipped: no ALSA sequencer (/dev/snd/seq)");
        }
        (inputs, outputs) => panic!("listing failed: {inputs:?} {outputs:?}"),
    }
}
