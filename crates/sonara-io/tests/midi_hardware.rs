//! [M] MIDI through midir on the host: virtual ports, a round trip through
//! them, and the port names compared with the prototype's RtMidi names.
//!
//! These tests open CoreMIDI or ALSA ports, which the development container cannot (it has no
//! `/dev/snd/seq`), so they are ignored and run locally (HW-01):
//!
//! ```text
//! cargo test -p sonara-io --test midi_hardware -- --ignored --nocapture
//! ```
//!
//! **Port names.** `port_names_match_the_prototype` prints midir's port names as JSON. With
//! `SONARA_PROTOTYPE_MIDI=<file>`, it compares them with a listing in the same shape
//! (`{"inputs": [...], "outputs": [...]}`), as the prototype's `devices.input_ports()` and
//! `devices.output_ports()` printed it for the same devices. With
//! `SONARA_MIDI_HOLD_SECONDS=<n>` it first creates the virtual ports of [`HOLD_SOURCES`] and
//! [`HOLD_DESTINATIONS`] and keeps them open for `n` seconds, so that another program can list
//! them meanwhile. Run it alone (`port_names_match_the_prototype`), so that the other tests'
//! ports do not appear.
//!
//! RtMidi trims spaces from both
//! ends of a CoreMIDI endpoint name (`CFStringTrim` in `CreateEndpointName`), and midir keeps
//! them (`kMIDIPropertyDisplayName`), so `sonara-io` trims them as RtMidi does
//! (`midi::rtmidi_name`). The composed names of external devices are compared with a real
//! keyboard by hand. Plain and non-ASCII names are equal in both directions. (RtMidi creates virtual ports with ASCII-encoded names, so a non-ASCII name that
//! the prototype creates reads back garbled on both sides: a prototype quirk, not a midir
//! difference.)

use std::time::{Duration, Instant};

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use serde_json::json;
use sonara_io::midi::{self, MidiInputPort, MidiOutputPort};
use sonara_rt::{MidiEvent, MidiKind, MidiReceiver};

/// RT-02: the render thread's side of the MIDI input rings (`pop`) runs inside
/// `assert_no_alloc`, which aborts on any allocation or free. The MIDI thread's callback is
/// checked by RealtimeSanitizer (`rust-rtsan`), which sees every thread.
#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

/// Waits until `ready` holds or `timeout` passes; virtual ports appear asynchronously.
fn wait_for(timeout: Duration, mut ready: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if ready() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn drain(receiver: &mut MidiReceiver, into: &mut Vec<MidiEvent>) {
    while let Some(event) = assert_no_alloc(|| receiver.pop()) {
        into.push(event);
    }
}

/// The prototype's `test_virtual_port_round_trip`: a virtual output "Sonara Test Loop", an
/// input opened on it by name, a note on and off received in order with their exact bytes, and
/// times that never run backwards.
#[test]
#[ignore = "opens CoreMIDI or ALSA ports; run locally (HW-01)"]
fn virtual_port_round_trip() {
    let mut out = MidiOutputPort::open_virtual("Sonara Test Loop").expect("a virtual output");
    assert!(
        wait_for(Duration::from_secs(2), || midi::input_ports()
            .expect("the input ports")
            .iter()
            .any(|name| name == "Sonara Test Loop")),
        "the virtual output appears as an input port"
    );
    let (input, mut receiver) = MidiInputPort::open("Sonara Test Loop", 7).expect("the input");
    assert_eq!(input.name(), "Sonara Test Loop");
    std::thread::sleep(Duration::from_millis(50));
    out.send(&[0x90, 64, 90]).expect("note on");
    out.send(&[0x80, 64, 0]).expect("note off");
    let mut events = Vec::new();
    assert!(
        wait_for(Duration::from_secs(2), || {
            drain(&mut receiver, &mut events);
            events.len() >= 2
        }),
        "both messages arrive: {events:?}"
    );
    std::thread::sleep(Duration::from_millis(100));
    drain(&mut receiver, &mut events);
    let bytes: Vec<&[u8]> = events.iter().map(MidiEvent::bytes).collect();
    assert_eq!(bytes, [&[0x90, 64, 90][..], &[0x80, 64, 0][..]]);
    assert_eq!(
        events.iter().map(MidiEvent::kind).collect::<Vec<_>>(),
        [MidiKind::NoteOn, MidiKind::NoteOff]
    );
    assert!(events.iter().all(|event| event.port() == 7));
    assert!(events[0].time_us() <= events[1].time_us());
    assert_eq!(input.overflows().get(), 0);
    drop(input);
}

/// The other direction: our virtual input receives from an output that another client opens
/// on it by name.
#[test]
#[ignore = "opens CoreMIDI or ALSA ports; run locally (HW-01)"]
fn a_virtual_input_receives_from_an_output() {
    let (input, mut receiver) =
        MidiInputPort::open_virtual("Sonara Test Sink", 2).expect("a virtual input");
    assert!(
        wait_for(Duration::from_secs(2), || midi::output_ports()
            .expect("the output ports")
            .iter()
            .any(|name| name == "Sonara Test Sink")),
        "the virtual input appears as an output port"
    );
    let mut out = MidiOutputPort::open("Sonara Test Sink").expect("the output");
    out.send(&[0xB0, 7, 100]).expect("control change");
    // System exclusive is dropped but arrives as a marker.
    out.send(&[0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7])
        .expect("system exclusive");
    out.send(&[0xE0, 0x00, 0x40]).expect("pitch bend");
    let mut events = Vec::new();
    assert!(
        wait_for(Duration::from_secs(2), || {
            drain(&mut receiver, &mut events);
            events.len() >= 3
        }),
        "three messages arrive: {events:?}"
    );
    assert_eq!(events[0].kind(), MidiKind::Cc);
    assert_eq!((events[0].note(), events[0].value()), (7, 100));
    assert!(events[1].is_sysex());
    assert_eq!(events[2].kind(), MidiKind::PitchBend);
    assert_eq!(events[2].value(), 0);
    assert!(events.iter().all(|event| event.port() == 2));
    drop(input);
}

/// An unknown name fails with the names there are.
#[test]
#[ignore = "opens CoreMIDI or ALSA clients; run locally (HW-01)"]
fn an_unknown_input_is_not_found() {
    let Err(err) = MidiInputPort::open("No Such Sonara Port", 0) else {
        panic!("an unknown port opened");
    };
    match err {
        sonara_io::MidiError::InputNotFound { name, available } => {
            assert_eq!(name, "No Such Sonara Port");
            assert!(!available.iter().any(|port| port == "No Such Sonara Port"));
        }
        other => panic!("unexpected error: {other}"),
    }
}

/// The virtual sources (input ports for other clients) of the port-name comparison: a plain
/// name, a leading space (RtMidi trims it from endpoint names), and non-ASCII characters.
const HOLD_SOURCES: [&str; 3] = [
    "Sonara Test Source",
    " Sonara Leading Space",
    "Sonara Křivka ✓",
];

/// The virtual destinations (output ports for other clients) of the port-name comparison.
const HOLD_DESTINATIONS: [&str; 2] = ["Sonara Test Destination", " Sonara Leading Destination"];

/// Midir's port names, optionally with virtual ports held open and compared with the
/// prototype's listing (see the module documentation).
#[test]
#[ignore = "lists the host's MIDI ports; run locally (HW-01)"]
fn port_names_match_the_prototype() {
    let hold = std::env::var("SONARA_MIDI_HOLD_SECONDS")
        .ok()
        .map(|seconds| Duration::from_secs(seconds.parse().expect("seconds")));
    let virtual_ports = hold.map(|_| {
        let sources: Vec<MidiOutputPort> = HOLD_SOURCES
            .iter()
            .map(|name| MidiOutputPort::open_virtual(name).expect("a virtual output"))
            .collect();
        let destinations: Vec<(MidiInputPort, MidiReceiver)> = HOLD_DESTINATIONS
            .iter()
            .map(|name| MidiInputPort::open_virtual(name, 0).expect("a virtual input"))
            .collect();
        (sources, destinations)
    });
    if virtual_ports.is_some() {
        std::thread::sleep(Duration::from_millis(200));
    }
    let listing = json!({
        "api": midi::api_name(),
        "inputs": midi::input_ports().expect("the input ports"),
        "outputs": midi::output_ports().expect("the output ports"),
    });
    println!("midir: {listing}");
    if let Some(path) = std::env::var_os("SONARA_PROTOTYPE_MIDI") {
        let text = std::fs::read_to_string(path).expect("the prototype's listing");
        let prototype: serde_json::Value = serde_json::from_str(&text).expect("JSON");
        assert_eq!(listing["inputs"], prototype["inputs"], "input names");
        assert_eq!(listing["outputs"], prototype["outputs"], "output names");
    }
    if let Some(hold) = hold {
        println!("holding the virtual ports for {hold:?}");
        std::thread::sleep(hold);
    }
    drop(virtual_ports);
}
