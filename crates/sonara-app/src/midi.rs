//! Live MIDI input: the MIDI devices and the input hub (the prototype's
//! `MidiInputHub` and the MIDI part of `TransportService`, `src/sonara/midi/devices.py` and
//! `src/sonara/audio/transport/service.py`). Not real-time: everything here runs on a control
//! thread.
//!
//! - **[`MidiDevices`]** lists the ports and opens inputs. [`SystemMidiDevices`] does it through
//!   midir (`sonara-io`); tests and the golden replay inject their own (DES-06).
//! - **[`MidiHub`]** keeps a set of inputs open: all ports, or the wanted ones, rescanned every
//!   [`RESCAN_INTERVAL`] so that hot-plugged devices appear. It skips ports whose names start
//!   with "Sonara" (Sonara's own virtual ports) and collects open failures in its error list.
//!   Each open input has its own ring ([`MidiInput`]); the hub hands the rings out
//!   ([`take_inputs`](MidiHub::take_inputs)) instead of reading them, because the render thread
//!   drains them. The port indices of the inputs it closed since
//!   ([`take_closed`](MidiHub::take_closed)) tell the player which rings to drop.
//! - **The JSON** of `list_midi_devices`: [`devices_json`] and [`status_json`], in the
//!   prototype's shapes.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::pyfmt::str_repr;
pub use sonara_io::MidiError;
use sonara_io::midi::{self as io, MidiInputPort};
use sonara_rt::{MidiOverflows, MidiReceiver};

/// How often a hub rescans the ports while it wants any (`_poll_midi`'s 2 s).
pub const RESCAN_INTERVAL: Duration = Duration::from_secs(2);

/// The prefix of the names a hub never opens by itself: Sonara's own virtual ports.
pub const OWN_PORT_PREFIX: &str = "Sonara";

/// How many of the most recent errors [`status_json`] reports.
const STATUS_ERRORS: usize = 5;

/// The MIDI system as the application sees it: the API name, the port listings and opening
/// inputs. [`SystemMidiDevices`] is the real one; the golden replay injects the recording
/// host's (no ports), and tests inject ports with rings they feed themselves.
pub trait MidiDevices: Send + Sync {
    /// RtMidi's name of the MIDI API, for example `"core"` on macOS.
    fn api_name(&self) -> &'static str;

    /// The names of the input ports.
    ///
    /// # Errors
    ///
    /// The MIDI system cannot be used ([`MidiError::Init`]).
    fn input_ports(&self) -> Result<Vec<String>, MidiError>;

    /// The names of the output ports.
    ///
    /// # Errors
    ///
    /// The MIDI system cannot be used ([`MidiError::Init`]).
    fn output_ports(&self) -> Result<Vec<String>, MidiError>;

    /// Opens the first input port named `name`; its events carry the port index `port`.
    ///
    /// # Errors
    ///
    /// No port has that name ([`MidiError::InputNotFound`]), or it cannot be opened.
    fn open_input(&self, name: &str, port: u8) -> Result<OpenInput, MidiError>;

    /// Creates a virtual input port named `name` that other clients can send to.
    ///
    /// # Errors
    ///
    /// The port cannot be created, or the platform has no virtual ports (Windows).
    fn open_virtual_input(&self, name: &str, port: u8) -> Result<OpenInput, MidiError>;
}

/// An open input: the ring of its events, its overflow count, and the connection that keeps it
/// open until dropped.
pub struct OpenInput {
    /// The input's events, for the render thread.
    pub events: MidiReceiver,
    /// The events the ring dropped because it was full.
    pub overflows: MidiOverflows,
    /// Closes the input when dropped.
    pub connection: Box<dyn Send>,
}

impl fmt::Debug for OpenInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenInput")
            .field("events", &self.events)
            .field("overflows", &self.overflows)
            .finish_non_exhaustive()
    }
}

/// The system's MIDI devices, through midir (CoreMIDI on macOS, ALSA on Linux).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemMidiDevices;

impl MidiDevices for SystemMidiDevices {
    fn api_name(&self) -> &'static str {
        io::api_name()
    }

    fn input_ports(&self) -> Result<Vec<String>, MidiError> {
        io::input_ports()
    }

    fn output_ports(&self) -> Result<Vec<String>, MidiError> {
        io::output_ports()
    }

    fn open_input(&self, name: &str, port: u8) -> Result<OpenInput, MidiError> {
        MidiInputPort::open(name, port).map(open_input)
    }

    fn open_virtual_input(&self, name: &str, port: u8) -> Result<OpenInput, MidiError> {
        MidiInputPort::open_virtual(name, port).map(open_input)
    }
}

fn open_input((port, events): (MidiInputPort, MidiReceiver)) -> OpenInput {
    OpenInput {
        events,
        overflows: port.overflows(),
        connection: Box::new(port),
    }
}

/// The `devices` part of `list_midi_devices` (`TransportService.midi_devices`): the API name and
/// the port names. This build always has MIDI support, so `available` is always true.
///
/// # Errors
///
/// A listing failed, for example without an ALSA sequencer. The prototype raised RtMidi's
/// error, which the agent reports as `INTERNAL`.
pub fn devices_json(devices: &dyn MidiDevices) -> Result<Value, MidiError> {
    let inputs = devices.input_ports()?;
    let outputs = devices.output_ports()?;
    Ok(json!({
        "available": true,
        "api": devices.api_name(),
        "inputs": inputs,
        "outputs": outputs,
    }))
}

/// The `status` part of `list_midi_devices` before a player exists
/// (`TransportService.midi_status` without a player).
#[must_use]
pub fn idle_status_json() -> Value {
    let ports: [&str; 0] = [];
    json!({"enabled": false, "ports": ports})
}

/// The live-input status of a player (`RealtimePlayer.midi_status`): whether a hub is enabled,
/// its open ports, the target tracks, the messages received while there were targets, the live
/// lookahead, and the hub's last five errors.
#[must_use]
pub fn status_json(
    hub: Option<&MidiHub>,
    targets: &[String],
    messages_received: u64,
    live_lookahead_ms: f64,
) -> Value {
    let (ports, errors) = hub.map_or_else(
        || (Vec::new(), Vec::new()),
        |hub| (hub.open_ports(), hub.recent_errors().to_vec()),
    );
    json!({
        "enabled": hub.is_some(),
        "ports": ports,
        "targets": targets,
        "messages_received": messages_received,
        "live_lookahead_ms": live_lookahead_ms,
        "errors": errors,
    })
}

/// How a failed open reads in a hub's error list: the prototype's `str(exc)`. A missing port is
/// its `SonaraError` (`"[NOT_FOUND] MIDI input 'x' not found"`); any other failure is RtMidi's
/// message there and midir's here.
fn describe(err: &MidiError) -> String {
    match err {
        MidiError::InputNotFound { name, available } => not_found(name, available).to_string(),
        other => other.to_string(),
    }
}

/// The prototype's `NOT_FOUND` error of `MidiInputPort(name)`, with the ports there are.
#[must_use]
pub fn not_found(name: &str, available: &[String]) -> DomainError {
    DomainError::new(
        ErrorCode::NotFound,
        format!("MIDI input {} not found", str_repr(name)),
    )
    .with_detail("available", available.to_vec())
}

/// An opened input's ring, handed out by [`MidiHub::take_inputs`].
#[derive(Debug)]
pub struct MidiInput {
    /// The port index its events carry.
    pub port: u8,
    /// The port's name.
    pub name: String,
    /// The input's events.
    pub events: MidiReceiver,
}

/// One open port of a hub.
struct HubPort {
    name: String,
    index: u8,
    overflows: MidiOverflows,
    _connection: Box<dyn Send>,
}

/// A set of open MIDI inputs (the prototype's `MidiInputHub`). `wanted` names the ports to
/// open; `None` means every port there is, so hot-plugged devices appear on a rescan.
///
/// Each port is opened at most once, under its name; ports keep the order they were opened in.
/// Each gets a port index (the lowest free one), which its events carry.
pub struct MidiHub {
    devices: Arc<dyn MidiDevices>,
    wanted: Option<Vec<String>>,
    ports: Vec<HubPort>,
    taken: Vec<MidiInput>,
    /// The port indices of inputs closed (or replaced) since the last `take_closed`.
    closed: Vec<u8>,
    errors: Vec<String>,
    next_rescan: Option<Instant>,
}

impl fmt::Debug for MidiHub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MidiHub")
            .field("wanted", &self.wanted)
            .field("ports", &self.open_ports())
            .field("errors", &self.errors)
            .finish_non_exhaustive()
    }
}

impl MidiHub {
    /// A hub with no open ports yet; [`refresh`](Self::refresh) opens them.
    #[must_use]
    pub fn new(devices: Arc<dyn MidiDevices>, wanted: Option<Vec<String>>) -> Self {
        Self {
            devices,
            wanted,
            ports: Vec::new(),
            taken: Vec::new(),
            closed: Vec::new(),
            errors: Vec::new(),
            next_rescan: None,
        }
    }

    /// The ports the hub wants; `None` for all.
    #[must_use]
    pub fn wanted(&self) -> Option<&[String]> {
        self.wanted.as_deref()
    }

    /// Opens the ports the hub wants and closes the open ones that are no longer there or no
    /// longer wanted; returns the names of the open ports (`MidiInputHub.refresh`).
    ///
    /// - A wanted name that is not in the listing is not an error.
    /// - A name starting with "Sonara" is never opened here.
    /// - An open failure is added to the errors as `"<name>: <error>"`, and the other ports
    ///   still open.
    /// - A failed listing is added to the errors, and the open ports stay as they are.
    ///
    /// Like the prototype, a refresh also closes a port opened with [`open`](Self::open) or
    /// [`open_virtual`](Self::open_virtual) when its name is not in the listing or not wanted.
    pub fn refresh(&mut self) -> Vec<String> {
        let names = match self.devices.input_ports() {
            Ok(names) => names,
            Err(err) => {
                self.errors.push(err.to_string());
                return self.open_ports();
            }
        };
        let targets: Vec<String> = names
            .into_iter()
            .filter(|name| {
                self.wanted
                    .as_ref()
                    .is_none_or(|wanted| wanted.contains(name))
            })
            .collect();
        let closing: Vec<String> = self
            .ports
            .iter()
            .filter(|port| !targets.contains(&port.name))
            .map(|port| port.name.clone())
            .collect();
        for name in &closing {
            self.remove(name);
        }
        for name in &targets {
            if self.is_open(name) || name.starts_with(OWN_PORT_PREFIX) {
                continue;
            }
            if let Err(err) = self.open(name) {
                self.errors.push(format!("{name}: {}", describe(&err)));
            }
        }
        self.open_ports()
    }

    /// Rescans the ports when a rescan is due: at the first call, then every
    /// [`RESCAN_INTERVAL`], unless the hub wants no ports at all (`_poll_midi`). Returns
    /// whether it rescanned.
    pub fn rescan_if_due(&mut self, now: Instant) -> bool {
        let wants_none = matches!(self.wanted.as_deref(), Some([]));
        let due = self.next_rescan.is_none_or(|at| now >= at);
        if wants_none || !due {
            return false;
        }
        self.refresh();
        self.next_rescan = Some(now + RESCAN_INTERVAL);
        true
    }

    /// Opens the input port `name`, whatever its name and the wanted list (the prototype's
    /// `add(MidiInputPort(name))`). An open port of the same name is replaced.
    ///
    /// # Errors
    ///
    /// The port is not there ([`MidiError::InputNotFound`]), cannot be opened, or the hub
    /// already has an input on every port index.
    pub fn open(&mut self, name: &str) -> Result<(), MidiError> {
        let index = self.free_index(name)?;
        let input = self.devices.open_input(name, index)?;
        self.insert(name, index, input);
        Ok(())
    }

    /// Creates a virtual input port `name` and adds it to the hub (the prototype's
    /// `add(MidiInputPort(name, virtual=True))`). An open port of the same name is replaced.
    ///
    /// # Errors
    ///
    /// The port cannot be created, or the hub already has an input on every port index.
    pub fn open_virtual(&mut self, name: &str) -> Result<(), MidiError> {
        let index = self.free_index(name)?;
        let input = self.devices.open_virtual_input(name, index)?;
        self.insert(name, index, input);
        Ok(())
    }

    /// The names of the open ports, in the order they were opened.
    #[must_use]
    pub fn open_ports(&self) -> Vec<String> {
        self.ports.iter().map(|port| port.name.clone()).collect()
    }

    /// Every error so far, oldest first.
    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// The last five errors (`errors[-5:]`).
    #[must_use]
    pub fn recent_errors(&self) -> &[String] {
        let start = self.errors.len().saturating_sub(STATUS_ERRORS);
        self.errors.get(start..).unwrap_or_default()
    }

    /// The rings of the inputs opened since the last call, for the render thread.
    pub fn take_inputs(&mut self) -> Vec<MidiInput> {
        std::mem::take(&mut self.taken)
    }

    /// The port indices of the inputs closed or replaced since the last call whose rings were
    /// handed out: the player drops those rings before it takes new ones on the same indices.
    pub fn take_closed(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.closed)
    }

    /// Events the open inputs dropped because their rings were full (RT-08).
    #[must_use]
    pub fn overflows(&self) -> u64 {
        self.ports.iter().map(|port| port.overflows.get()).sum()
    }

    /// Closes every port.
    pub fn close(&mut self) {
        let names: Vec<String> = self.ports.iter().map(|port| port.name.clone()).collect();
        for name in &names {
            self.remove(name);
        }
    }

    fn is_open(&self, name: &str) -> bool {
        self.ports.iter().any(|port| port.name == name)
    }

    /// The lowest port index no other open port uses; a port being replaced frees its own.
    fn free_index(&self, name: &str) -> Result<u8, MidiError> {
        (0..=u8::MAX)
            .find(|index| {
                !self
                    .ports
                    .iter()
                    .any(|port| port.index == *index && port.name != name)
            })
            .ok_or_else(|| MidiError::Connect {
                name: name.to_owned(),
                reason: "too many MIDI inputs".to_owned(),
            })
    }

    /// Adds an opened port; one of the same name is replaced in its place, as assigning to a
    /// Python dict's existing key keeps the key's position.
    fn insert(&mut self, name: &str, index: u8, input: OpenInput) {
        self.forget(name);
        let port = HubPort {
            name: name.to_owned(),
            index,
            overflows: input.overflows,
            _connection: input.connection,
        };
        match self.ports.iter_mut().find(|open| open.name == name) {
            Some(open) => *open = port,
            None => self.ports.push(port),
        }
        self.taken.push(MidiInput {
            port: index,
            name: name.to_owned(),
            events: input.events,
        });
    }

    /// Closes the port `name`, and drops its ring if the render thread has not taken it.
    fn remove(&mut self, name: &str) {
        self.forget(name);
        self.ports.retain(|port| port.name != name);
    }

    /// Notes that the ring of the port `name` goes: a ring not handed out yet is dropped; one
    /// handed out is reported by [`take_closed`](Self::take_closed).
    fn forget(&mut self, name: &str) {
        let pending = self.taken.iter().any(|input| input.name == name);
        if pending {
            self.taken.retain(|input| input.name != name);
        } else if let Some(port) = self.ports.iter().find(|port| port.name == name) {
            self.closed.push(port.index);
        }
    }
}
