//! The MIDI hub and the `list_midi_devices` JSON against the prototype's
//! `MidiInputHub` and the golden `transport/controls` and `coverage` answers, with injected MIDI
//! devices (TST-04, TST-08): no test here needs CoreMIDI or ALSA.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::midi::{
    MidiDevices, MidiHub, OpenInput, RESCAN_INTERVAL, SystemMidiDevices, devices_json,
    idle_status_json, not_found, status_json,
};
use sonara_app::settings::Settings;
use sonara_io::MidiError;
use sonara_rt::{MidiEvent, MidiKind, MidiSender, midi_channel};
use sonara_testkit::Golden;

/// Counts its live connections; dropping one closes the port.
struct Connection(Arc<AtomicUsize>);

impl Drop for Connection {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// MIDI devices whose ports the test sets, with rings the test feeds.
#[derive(Default)]
struct FakeDevices {
    inputs: Mutex<Vec<String>>,
    outputs: Vec<String>,
    listing_fails: Mutex<bool>,
    /// Names whose open fails; `true` fails as "not found" (the port vanished).
    failing: Mutex<Vec<(String, bool)>>,
    /// Every open: the name and port index.
    opened: Mutex<Vec<(String, u8)>>,
    live: Arc<AtomicUsize>,
    senders: Mutex<Vec<(String, MidiSender)>>,
}

impl FakeDevices {
    fn with_inputs(names: &[&str]) -> Arc<Self> {
        let fake = Self::default();
        fake.set_inputs(names);
        Arc::new(fake)
    }

    fn set_inputs(&self, names: &[&str]) {
        *self.inputs.lock().expect("an unpoisoned lock") =
            names.iter().map(|&name| name.to_owned()).collect();
    }

    fn live(&self) -> usize {
        self.live.load(Ordering::SeqCst)
    }

    fn opened(&self) -> Vec<(String, u8)> {
        self.opened.lock().expect("an unpoisoned lock").clone()
    }

    /// Plays `bytes` into the ring of the latest input opened as `name`.
    fn send(&self, name: &str, time_us: u64, bytes: &[u8]) {
        let mut senders = self.senders.lock().expect("an unpoisoned lock");
        let (_, sender) = senders
            .iter_mut()
            .rev()
            .find(|(open, _)| open == name)
            .expect("an open input");
        assert!(sender.push(MidiEvent::new(time_us, 0, bytes).expect("a short message")));
    }

    fn connect(&self, name: &str, port: u8) -> OpenInput {
        let (sender, events) = midi_channel(16);
        let overflows = sender.overflows();
        self.senders
            .lock()
            .expect("an unpoisoned lock")
            .push((name.to_owned(), sender));
        self.opened
            .lock()
            .expect("an unpoisoned lock")
            .push((name.to_owned(), port));
        self.live.fetch_add(1, Ordering::SeqCst);
        OpenInput {
            events,
            overflows,
            connection: Box::new(Connection(Arc::clone(&self.live))),
        }
    }
}

impl MidiDevices for FakeDevices {
    fn api_name(&self) -> &'static str {
        "core"
    }

    fn input_ports(&self) -> Result<Vec<String>, MidiError> {
        if *self.listing_fails.lock().expect("an unpoisoned lock") {
            return Err(MidiError::Init);
        }
        Ok(self.inputs.lock().expect("an unpoisoned lock").clone())
    }

    fn output_ports(&self) -> Result<Vec<String>, MidiError> {
        Ok(self.outputs.clone())
    }

    fn open_input(&self, name: &str, port: u8) -> Result<OpenInput, MidiError> {
        let inputs = self.inputs.lock().expect("an unpoisoned lock").clone();
        let failing = self.failing.lock().expect("an unpoisoned lock").clone();
        match failing.iter().find(|(failing, _)| failing == name) {
            Some((_, true)) => Err(MidiError::InputNotFound {
                name: name.to_owned(),
                available: inputs,
            }),
            Some((_, false)) => Err(MidiError::Connect {
                name: name.to_owned(),
                reason: "error connecting MIDI input port".to_owned(),
            }),
            None if !inputs.iter().any(|input| input == name) => Err(MidiError::InputNotFound {
                name: name.to_owned(),
                available: inputs,
            }),
            None => Ok(self.connect(name, port)),
        }
    }

    fn open_virtual_input(&self, name: &str, port: u8) -> Result<OpenInput, MidiError> {
        Ok(self.connect(name, port))
    }
}

fn hub(fake: &Arc<FakeDevices>, wanted: Option<&[&str]>) -> MidiHub {
    let devices: Arc<dyn MidiDevices> = Arc::clone(fake) as Arc<dyn MidiDevices>;
    MidiHub::new(
        devices,
        wanted.map(|names| names.iter().map(|&name| name.to_owned()).collect()),
    )
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|&name| name.to_owned()).collect()
}

/// The result of golden step `index` of the `transport/controls` scenario, a call of `call`.
fn controls_step(index: usize, call: &str) -> Value {
    let transcript: Value = Golden::open()
        .expect("golden data")
        .json("transport/controls/transcript.json")
        .expect("the controls transcript");
    let step = &transcript["steps"][index];
    assert_eq!(step["call"], call, "controls step {index}");
    step["result"].clone()
}

/// The golden `list_midi_devices` answer of the coverage batch.
fn coverage_step_116() -> Value {
    let transcript: Value = Golden::open()
        .expect("golden data")
        .json("coverage/transcript.json")
        .expect("the coverage transcript");
    let step = &transcript["steps"][116];
    assert_eq!(step["tool"], "list_midi_devices");
    step["result"].clone()
}

#[test]
fn the_device_list_and_the_idle_status_match_the_golden_answers() {
    // The recording host had no MIDI ports (the recorder pinned them).
    let fake = FakeDevices::with_inputs(&[]);
    let recorded = coverage_step_116();
    assert_eq!(devices_json(fake.as_ref()).unwrap(), recorded["devices"]);
    assert_eq!(idle_status_json(), recorded["status"]);
    // Before the player exists.
    let controls = controls_step(4, "list_midi_devices");
    assert_eq!(devices_json(fake.as_ref()).unwrap(), controls["devices"]);
    assert_eq!(idle_status_json(), controls["status"]);
}

#[test]
fn the_device_list_names_every_port() {
    let fake = Arc::new(FakeDevices {
        outputs: names(&["IAC Driver Bus 1"]),
        ..FakeDevices::default()
    });
    fake.set_inputs(&["Keystep", "IAC Driver Bus 1"]);
    assert_eq!(
        devices_json(fake.as_ref()).unwrap(),
        json!({
            "available": true,
            "api": "core",
            "inputs": ["Keystep", "IAC Driver Bus 1"],
            "outputs": ["IAC Driver Bus 1"],
        })
    );
    *fake.listing_fails.lock().expect("an unpoisoned lock") = true;
    assert!(matches!(devices_json(fake.as_ref()), Err(MidiError::Init)));
}

/// `set_midi_input{enabled: true}` and the status after it, then after `enabled: false`, on the
/// recording host without ports (controls steps 7–11).
#[test]
fn the_player_status_matches_the_golden_answers() {
    let fake = FakeDevices::with_inputs(&[]);
    let mut all = hub(&fake, None);
    assert_eq!(all.refresh(), [] as [String; 0]);
    let enabled = controls_step(7, "set_midi_input");
    assert_eq!(enabled["ports"], json!(all.open_ports()));
    assert_eq!(
        status_json(Some(&all), &[], 0, 25.0),
        controls_step(8, "list_midi_devices")["status"]
    );
    // A wanted port that is not there is not an error.
    let mut keys = hub(&fake, Some(&["Golden Keys"]));
    assert_eq!(keys.refresh(), [] as [String; 0]);
    assert_eq!(
        controls_step(9, "set_midi_input")["ports"],
        json!(keys.open_ports())
    );
    assert_eq!(keys.errors(), [] as [String; 0]);
    // Disabled: the player is there, the hub is not.
    assert_eq!(
        status_json(None, &[], 0, 25.0),
        controls_step(11, "list_midi_devices")["status"]
    );
}

#[test]
fn a_refresh_opens_every_port_but_sonaras_own() {
    let fake = FakeDevices::with_inputs(&["Keystep", "Sonara Test Loop", "IAC Driver Bus 1"]);
    let mut hub = hub(&fake, None);
    assert_eq!(hub.refresh(), names(&["Keystep", "IAC Driver Bus 1"]));
    assert_eq!(
        fake.opened(),
        [
            ("Keystep".to_owned(), 0),
            ("IAC Driver Bus 1".to_owned(), 1)
        ]
    );
    // Open ports are not opened again.
    assert_eq!(hub.refresh(), names(&["Keystep", "IAC Driver Bus 1"]));
    assert_eq!(fake.opened().len(), 2);
    assert_eq!(fake.live(), 2);
}

#[test]
fn a_refresh_opens_only_the_wanted_ports() {
    let fake = FakeDevices::with_inputs(&["Keystep", "IAC Driver Bus 1", "Launchpad"]);
    let mut hub = hub(&fake, Some(&["Launchpad", "Absent", "Keystep"]));
    // In the listing's order, not the wanted list's.
    assert_eq!(hub.refresh(), names(&["Keystep", "Launchpad"]));
    assert_eq!(
        hub.wanted(),
        Some(&names(&["Launchpad", "Absent", "Keystep"])[..])
    );
    assert_eq!(hub.errors(), [] as [String; 0]);
}

#[test]
fn a_vanished_port_closes_and_a_new_one_opens_on_refresh() {
    let fake = FakeDevices::with_inputs(&["Keystep", "Launchpad"]);
    let mut hub = hub(&fake, None);
    hub.refresh();
    fake.set_inputs(&["Launchpad", "Microfreak"]);
    assert_eq!(hub.refresh(), names(&["Launchpad", "Microfreak"]));
    assert_eq!(fake.live(), 2);
    // The new port takes the lowest free index, the one the vanished port had.
    assert_eq!(fake.opened().last(), Some(&("Microfreak".to_owned(), 0)));
}

#[test]
fn open_failures_are_listed_and_the_other_ports_still_open() {
    let fake = FakeDevices::with_inputs(&["Busy", "Gone", "Keystep"]);
    *fake.failing.lock().expect("an unpoisoned lock") =
        vec![("Busy".to_owned(), false), ("Gone".to_owned(), true)];
    let mut hub = hub(&fake, None);
    assert_eq!(hub.refresh(), names(&["Keystep"]));
    assert_eq!(
        hub.errors(),
        [
            "Busy: error connecting MIDI input port",
            "Gone: [NOT_FOUND] MIDI input 'Gone' not found",
        ]
    );
}

#[test]
fn a_failed_listing_is_listed_and_keeps_the_open_ports() {
    let fake = FakeDevices::with_inputs(&["Keystep"]);
    let mut hub = hub(&fake, None);
    hub.refresh();
    *fake.listing_fails.lock().expect("an unpoisoned lock") = true;
    assert_eq!(hub.refresh(), names(&["Keystep"]));
    assert_eq!(hub.errors(), ["MIDI support could not be initialized"]);
    assert_eq!(fake.live(), 1);
}

#[test]
fn the_status_reports_the_last_five_errors() {
    let fake = FakeDevices::with_inputs(&[]);
    *fake.listing_fails.lock().expect("an unpoisoned lock") = true;
    let mut hub = hub(&fake, None);
    for _ in 0..7 {
        hub.refresh();
    }
    assert_eq!(hub.errors().len(), 7);
    assert_eq!(hub.recent_errors().len(), 5);
    let status = status_json(Some(&hub), &names(&["t_1"]), 3, 25.0);
    assert_eq!(
        status,
        json!({
            "enabled": true,
            "ports": [],
            "targets": ["t_1"],
            "messages_received": 3,
            "live_lookahead_ms": 25.0,
            "errors": vec!["MIDI support could not be initialized"; 5],
        })
    );
}

#[test]
fn rescans_happen_every_two_seconds_unless_no_port_is_wanted() {
    let fake = FakeDevices::with_inputs(&["Keystep"]);
    let mut hub = hub(&fake, None);
    let start = Instant::now();
    // The first poll rescans at once.
    assert!(hub.rescan_if_due(start));
    assert_eq!(hub.open_ports(), names(&["Keystep"]));
    fake.set_inputs(&["Keystep", "Launchpad"]);
    assert!(!hub.rescan_if_due(start + Duration::from_millis(1_999)));
    assert_eq!(hub.open_ports(), names(&["Keystep"]));
    assert!(hub.rescan_if_due(start + RESCAN_INTERVAL));
    assert_eq!(hub.open_ports(), names(&["Keystep", "Launchpad"]));
    assert!(!hub.rescan_if_due(start + RESCAN_INTERVAL + Duration::from_secs(1)));

    let mut none = self::hub(&fake, Some(&[]));
    assert!(!none.rescan_if_due(start));
    assert!(!none.rescan_if_due(start + Duration::from_secs(60)));
}

#[test]
fn ports_opened_by_hand_bypass_the_skip_and_the_wanted_list() {
    let fake = FakeDevices::with_inputs(&["Sonara Test Keys"]);
    // The prototype's `add_midi_port` creates a hub that wants nothing.
    let mut hub = hub(&fake, Some(&[]));
    hub.open("Sonara Test Keys").unwrap();
    hub.open_virtual("Sonara Virtual In").unwrap();
    assert_eq!(
        hub.open_ports(),
        names(&["Sonara Test Keys", "Sonara Virtual In"])
    );
    // A hub that wants nothing never rescans, so they stay open.
    assert!(!hub.rescan_if_due(Instant::now()));
    assert_eq!(fake.live(), 2);
    // An explicit refresh closes what is not wanted, as the prototype's does.
    assert_eq!(hub.refresh(), [] as [String; 0]);
    assert_eq!(fake.live(), 0);
}

#[test]
fn opening_an_unknown_port_fails_with_the_ports_there_are() {
    let fake = FakeDevices::with_inputs(&["Keystep"]);
    let mut hub = hub(&fake, Some(&[]));
    let err = hub.open("Nope").unwrap_err();
    let MidiError::InputNotFound { name, available } = err else {
        panic!("unexpected error: {err}");
    };
    let err = not_found(&name, &available);
    assert_eq!(err.to_string(), "[NOT_FOUND] MIDI input 'Nope' not found");
    assert_eq!(err.details["available"], json!(["Keystep"]));
    // Python's repr of a name with a single quote.
    assert_eq!(
        not_found("Bob's Keys", &[]).message,
        "MIDI input \"Bob's Keys\" not found"
    );
    assert_eq!(hub.open_ports(), [] as [String; 0]);
}

#[test]
fn reopening_a_port_replaces_it_in_its_place() {
    let fake = FakeDevices::with_inputs(&["A", "B"]);
    let mut hub = hub(&fake, None);
    hub.refresh();
    hub.open("A").unwrap();
    assert_eq!(hub.open_ports(), names(&["A", "B"]));
    assert_eq!(fake.live(), 2);
    // The replacement keeps its port index.
    assert_eq!(fake.opened().last(), Some(&("A".to_owned(), 0)));
}

#[test]
fn opened_inputs_hand_out_their_rings() {
    let fake = FakeDevices::with_inputs(&["Keystep", "Launchpad"]);
    let mut hub = hub(&fake, None);
    hub.refresh();
    let mut inputs = hub.take_inputs();
    assert!(hub.take_inputs().is_empty());
    let summary: Vec<(u8, &str)> = inputs
        .iter()
        .map(|input| (input.port, input.name.as_str()))
        .collect();
    assert_eq!(summary, [(0, "Keystep"), (1, "Launchpad")]);
    fake.send("Launchpad", 42, &[0x90, 60, 100]);
    let event = inputs[1].events.pop().expect("an event");
    assert_eq!(
        (event.time_us(), event.kind(), event.note()),
        (42, MidiKind::NoteOn, 60)
    );
    assert!(inputs[0].events.pop().is_none());
    assert_eq!(hub.overflows(), 0);

    // A port that closes before its ring is taken drops the ring with it.
    fake.set_inputs(&["Keystep", "Launchpad", "Microfreak"]);
    hub.refresh();
    fake.set_inputs(&["Keystep", "Launchpad"]);
    hub.refresh();
    assert!(hub.take_inputs().is_empty());
    hub.close();
    assert_eq!(hub.open_ports(), [] as [String; 0]);
    assert_eq!(fake.live(), 0);
}

#[test]
fn the_context_uses_the_injected_devices() {
    let fake = FakeDevices::with_inputs(&["Keystep"]);
    let devices: Arc<dyn MidiDevices> = Arc::clone(&fake) as Arc<dyn MidiDevices>;
    let workspaces = tempfile::tempdir().unwrap();
    let app = AppContext::create(AppOptions {
        workspaces: Some(workspaces.path().to_path_buf()),
        settings: Some(Settings::default()),
        persist_settings: false,
        midi_devices: Some(devices),
        ..AppOptions::default()
    })
    .unwrap();
    assert_eq!(
        devices_json(app.midi_devices().as_ref()).unwrap()["inputs"],
        json!(["Keystep"])
    );
    app.close();
}

#[test]
fn the_system_devices_report_rtmidis_api_name() {
    assert_eq!(SystemMidiDevices.api_name(), sonara_io::midi::api_name());
}
