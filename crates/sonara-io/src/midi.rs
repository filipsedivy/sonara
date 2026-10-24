//! MIDI device I/O through midir: the prototype's `native/src/midi.cpp`
//! (RtMidi) and the port classes of `src/sonara/midi/devices.py`. Not real-time: everything
//! here runs on a control thread; only the input callback (`midi_callback`) runs on
//! the MIDI input thread.
//!
//! - **Listing.** [`input_ports`] and [`output_ports`] list the port names through a probe
//!   client named "Sonara probe", as the prototype does. On macOS the names are midir's
//!   (CoreMIDI's display name), trimmed of spaces at both ends as RtMidi trims them.
//! - **Input.** [`MidiInputPort::open`] opens the first input port with a name, through a client
//!   named "Sonara" and a connection named "Sonara In". It ignores MIDI clock, time code and
//!   active sensing and keeps system exclusive messages, as RtMidi's
//!   `ignoreTypes(false, true, true)` does (`Ignore::TimeAndActiveSense`). Its events go to a
//!   [`MidiReceiver`] of [`INPUT_QUEUE_CAPACITY`] events, the size of the prototype's RtMidi
//!   queue.
//! - **Output.** [`MidiOutputPort`] sends raw messages; tests use it to play into an input.
//! - **Virtual ports** exist on macOS and Linux (CoreMIDI, ALSA), not on Windows.
//! - **The API name** is RtMidi's: [`api_name`].

use midir::{Ignore, MidiIO, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use sonara_rt::{MidiOverflows, MidiReceiver, midi_channel};

use crate::error::MidiError;
use crate::midi_callback::{self, InputCallback};

/// The client name of open ports (the prototype's `MidiIn("Sonara")`).
pub const CLIENT_NAME: &str = "Sonara";

/// The client name of the port listings (the prototype's `MidiIn("Sonara probe")`).
pub const PROBE_CLIENT_NAME: &str = "Sonara probe";

/// The connection name of an input opened by name (`MidiIn.open`'s default).
pub const INPUT_CONNECTION_NAME: &str = "Sonara In";

/// The connection name of an output opened by name (`MidiOut.open`'s default).
pub const OUTPUT_CONNECTION_NAME: &str = "Sonara Out";

/// Events an input's ring holds: the prototype's RtMidi queue size (`kQueueSize`).
pub const INPUT_QUEUE_CAPACITY: usize = 4096;

/// The MIDI API's name as RtMidi reports it (`RtMidi::getApiName`): `"core"` on macOS,
/// `"alsa"` on Linux and `"winmm"` on Windows. The golden transcripts record `"core"`.
#[must_use]
pub const fn api_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "core"
    } else if cfg!(target_os = "windows") {
        "winmm"
    } else {
        "alsa"
    }
}

/// The names of the MIDI input ports, in the system's order.
///
/// A port whose name cannot be read, because it vanished during the listing, is left out.
///
/// # Errors
///
/// [`MidiError::Init`] when the MIDI system cannot be used, for example on Linux without an
/// ALSA sequencer (`/dev/snd/seq`).
pub fn input_ports() -> Result<Vec<String>, MidiError> {
    let input = MidiInput::new(PROBE_CLIENT_NAME).map_err(|_| MidiError::Init)?;
    Ok(port_names(&input))
}

/// The names of the MIDI output ports, in the system's order. See [`input_ports`].
///
/// # Errors
///
/// [`MidiError::Init`] when the MIDI system cannot be used.
pub fn output_ports() -> Result<Vec<String>, MidiError> {
    let output = MidiOutput::new(PROBE_CLIENT_NAME).map_err(|_| MidiError::Init)?;
    Ok(port_names(&output))
}

fn port_names<T: MidiIO>(io: &T) -> Vec<String> {
    io.ports()
        .iter()
        .filter_map(|port| io.port_name(port).ok().map(rtmidi_name))
        .collect()
}

/// A port name as RtMidi reports it, because the settings save the prototype's names (owner
/// decision of 2026-10-22). On CoreMIDI, RtMidi trims spaces from both ends of an
/// endpoint name (`CFStringTrim(result, CFSTR(" "))` in `CreateEndpointName`), and midir keeps
/// them. RtMidi's composed names of external devices are compared with a real keyboard by
/// hand.
fn rtmidi_name(name: String) -> String {
    if cfg!(target_os = "macos") && (name.starts_with(' ') || name.ends_with(' ')) {
        name.trim_matches(' ').to_owned()
    } else {
        name
    }
}

/// The first port of `io` named `name`, and every port's name for the error.
fn find_port<T: MidiIO>(io: &T, name: &str) -> Result<T::Port, Vec<String>> {
    let ports = io.ports();
    let mut names = Vec::with_capacity(ports.len());
    for port in ports {
        match io.port_name(&port).map(rtmidi_name) {
            Ok(port_name) if port_name == name => return Ok(port),
            Ok(port_name) => names.push(port_name),
            Err(_) => {}
        }
    }
    Err(names)
}

/// An open MIDI input. Its callback pushes every message into the [`MidiReceiver`] that
/// [`open`](Self::open) returned, stamped with the input's port index. Dropping it closes the
/// port; the receiver then gets no more events.
pub struct MidiInputPort {
    name: String,
    overflows: MidiOverflows,
    /// Dropped, never closed through midir's `close`: on CoreMIDI, `close` takes the callback's
    /// state out before the port is disposed, so a message arriving in between would make
    /// midir's handler panic on CoreMIDI's thread. Dropping disposes the client and the port
    /// while the state is still there.
    _connection: MidiInputConnection<InputCallback>,
}

impl MidiInputPort {
    /// Opens the first input port named `name`; its events carry the port index `port`.
    ///
    /// # Errors
    ///
    /// - [`MidiError::Init`] when the MIDI system cannot be used;
    /// - [`MidiError::InputNotFound`] when no input port has that name, with the names of the
    ///   ports there are;
    /// - [`MidiError::Connect`] when the port cannot be connected.
    pub fn open(name: &str, port: u8) -> Result<(Self, MidiReceiver), MidiError> {
        let input = new_input()?;
        let source = find_port(&input, name).map_err(|available| MidiError::InputNotFound {
            name: name.to_owned(),
            available,
        })?;
        let (sender, receiver) = midi_channel(INPUT_QUEUE_CAPACITY);
        let overflows = sender.overflows();
        let connection = input
            .connect(
                &source,
                INPUT_CONNECTION_NAME,
                midi_callback::receive,
                InputCallback::new(sender, port),
            )
            .map_err(|err| MidiError::Connect {
                name: name.to_owned(),
                reason: err.kind().to_string(),
            })?;
        Ok((
            Self {
                name: name.to_owned(),
                overflows,
                _connection: connection,
            },
            receiver,
        ))
    }

    /// Creates a virtual input port named `name`, which other clients can send to (the
    /// prototype's `MidiInputPort(name, virtual=True)`); its events carry the port index `port`.
    ///
    /// # Errors
    ///
    /// - [`MidiError::Init`] when the MIDI system cannot be used;
    /// - [`MidiError::Connect`] when the port cannot be created;
    /// - [`MidiError::VirtualPortsUnsupported`] on Windows.
    pub fn open_virtual(name: &str, port: u8) -> Result<(Self, MidiReceiver), MidiError> {
        #[cfg(unix)]
        {
            use midir::os::unix::VirtualInput;

            let input = new_input()?;
            let (sender, receiver) = midi_channel(INPUT_QUEUE_CAPACITY);
            let overflows = sender.overflows();
            let connection = input
                .create_virtual(
                    name,
                    midi_callback::receive,
                    InputCallback::new(sender, port),
                )
                .map_err(|err| MidiError::Connect {
                    name: name.to_owned(),
                    reason: err.kind().to_string(),
                })?;
            Ok((
                Self {
                    name: name.to_owned(),
                    overflows,
                    _connection: connection,
                },
                receiver,
            ))
        }
        #[cfg(not(unix))]
        {
            let _ = (name, port);
            Err(MidiError::VirtualPortsUnsupported)
        }
    }

    /// The port's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The count of events dropped because the receiver's ring was full (RT-08), readable from
    /// any thread.
    #[must_use]
    pub fn overflows(&self) -> MidiOverflows {
        self.overflows.clone()
    }
}

/// The input client with the prototype's message filter.
fn new_input() -> Result<MidiInput, MidiError> {
    let mut input = MidiInput::new(CLIENT_NAME).map_err(|_| MidiError::Init)?;
    // RtMidi's `ignoreTypes(sysex=false, timing=true, active_sensing=true)`: midir's flags are
    // RtMidi's bits (sysex 0x01, time 0x02, active sensing 0x04), and both parse CoreMIDI
    // packets and ALSA events the same way (`native/third_party/rtmidi/RtMidi.cpp`).
    input.ignore(Ignore::TimeAndActiveSense);
    Ok(input)
}

/// An open MIDI output (the prototype's `MidiOutputPort`). Dropping it closes the port.
pub struct MidiOutputPort {
    name: String,
    connection: MidiOutputConnection,
}

impl MidiOutputPort {
    /// Opens the first output port named `name`.
    ///
    /// # Errors
    ///
    /// - [`MidiError::Init`] when the MIDI system cannot be used;
    /// - [`MidiError::OutputNotFound`] when no output port has that name;
    /// - [`MidiError::Connect`] when the port cannot be connected.
    pub fn open(name: &str) -> Result<Self, MidiError> {
        let output = MidiOutput::new(CLIENT_NAME).map_err(|_| MidiError::Init)?;
        let destination =
            find_port(&output, name).map_err(|available| MidiError::OutputNotFound {
                name: name.to_owned(),
                available,
            })?;
        let connection = output
            .connect(&destination, OUTPUT_CONNECTION_NAME)
            .map_err(|err| MidiError::Connect {
                name: name.to_owned(),
                reason: err.kind().to_string(),
            })?;
        Ok(Self {
            name: name.to_owned(),
            connection,
        })
    }

    /// Creates a virtual output port named `name`, which other clients can receive from.
    ///
    /// # Errors
    ///
    /// - [`MidiError::Init`] when the MIDI system cannot be used;
    /// - [`MidiError::Connect`] when the port cannot be created;
    /// - [`MidiError::VirtualPortsUnsupported`] on Windows.
    pub fn open_virtual(name: &str) -> Result<Self, MidiError> {
        #[cfg(unix)]
        {
            use midir::os::unix::VirtualOutput;

            let output = MidiOutput::new(CLIENT_NAME).map_err(|_| MidiError::Init)?;
            let connection = output
                .create_virtual(name)
                .map_err(|err| MidiError::Connect {
                    name: name.to_owned(),
                    reason: err.kind().to_string(),
                })?;
            Ok(Self {
                name: name.to_owned(),
                connection,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = name;
            Err(MidiError::VirtualPortsUnsupported)
        }
    }

    /// The port's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sends one message.
    ///
    /// # Errors
    ///
    /// [`MidiError::Send`] when midir rejects the bytes or the system cannot send them.
    pub fn send(&mut self, bytes: &[u8]) -> Result<(), MidiError> {
        self.connection.send(bytes).map_err(MidiError::Send)
    }
}

#[cfg(test)]
mod tests {
    use super::rtmidi_name;

    #[test]
    fn port_names_are_trimmed_as_rtmidi_trims_them_on_coremidi() {
        let trimmed = rtmidi_name("  Keys Out ".to_owned());
        if cfg!(target_os = "macos") {
            assert_eq!(trimmed, "Keys Out");
        } else {
            assert_eq!(trimmed, "  Keys Out ");
        }
        // Only spaces: RtMidi's `CFStringTrim(result, CFSTR(" "))` keeps tabs.
        assert_eq!(rtmidi_name("\tKeys".to_owned()), "\tKeys");
        assert_eq!(rtmidi_name("Křivka ✓".to_owned()), "Křivka ✓");
    }
}
