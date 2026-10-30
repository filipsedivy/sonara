//! How a keyboard instrument makes and stops its sound: the action, the keys, the pedals and the
//! MIDI controllers they send, sympathetic resonance, the noises of the mechanism, the decay and a
//! lasting change of timbre.
//!
//! The values describe the acoustic or electric instrument, not a sample library. Pedals are
//! controls, never articulations; other controls of an instrument (a Rhodes tremolo, Clavinet
//! pickups, drawbars, a Leslie, organ registration) are track or device controls and are not
//! modelled here.

use super::instrument::Provenance;
use super::pitch::PitchRange;

/// A MIDI 1.0 control change number that a pedal sends, 0–119.
///
/// MIDI 1.0 reserves the numbers 120–127 for channel mode messages, which no pedal sends, so they
/// have no value of this type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Controller(u8);

impl Controller {
    /// The highest controller number a pedal can send.
    pub const HIGHEST: u8 = 119;

    /// The controller numbered `number`, or `None` above 119.
    #[must_use]
    pub const fn new(number: u8) -> Option<Self> {
        if number > Self::HIGHEST {
            return None;
        }
        Some(Self(number))
    }

    /// The controller number a control change message carries.
    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }
}

/// What a key moves to make the sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// A grand piano's: hammers strike the horizontal strings from below and fall back by
    /// gravity; double escapement lets a key repeat after rising about a third of its travel.
    Grand,
    /// An upright piano's: hammers strike the vertical strings and return by springs; single
    /// escapement, so a key must rise almost fully before it repeats.
    Upright,
    /// Hammers strike tines, each sustained by a tone bar and read by a pickup (the Rhodes).
    TineHammer,
    /// Hammers strike steel reeds read by an electrostatic pickup (the Wurlitzer).
    ReedHammer,
    /// Felt hammers strike steel plates, each over a tuned wooden resonator (the celesta).
    PlateHammer,
    /// A rubber pad under the key presses the string onto an anvil and stays on it while the key
    /// is held (the Clavinet).
    Plunger,
    /// The key lifts a jack whose plectrum plucks the string; a damper on the jack stops it when
    /// the key rises (the harpsichord).
    Jack,
    /// A brass tangent on the key strikes the string and stays on it while the key is held,
    /// fixing the pitch (the clavichord).
    Tangent,
    /// The key opens a valve that lets wind into the pipes of the drawn stops, mechanically
    /// (tracker), pneumatically or electrically (the pipe organ).
    PipeValve,
    /// The key closes electrical contacts, one for each drawbar (the Hammond organ).
    Contacts,
    /// The key opens a pallet that lets the bellows' air through free reeds (the accordion).
    ReedValve,
    /// A baton, struck with the closed fist, pulls a clapper against a bell by a wire (the
    /// carillon).
    Baton,
}

impl Action {
    /// Whether the force of the stroke sets the loudness. It does not for a jack, a pipe valve,
    /// contacts or a reed valve: a harpsichord changes its loudness only by its choirs, an organ
    /// by its stops and its swell pedal, and an accordion by its bellows.
    #[must_use]
    pub const fn is_touch_sensitive(self) -> bool {
        match self {
            Self::Grand
            | Self::Upright
            | Self::TineHammer
            | Self::ReedHammer
            | Self::PlateHammer
            | Self::Plunger
            | Self::Tangent
            | Self::Baton => true,
            Self::Jack | Self::PipeValve | Self::Contacts | Self::ReedValve => false,
        }
    }
}

/// What a pedal does. The list is open: a pedal no variant names is added when an instrument
/// needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PedalKind {
    /// The damper (sustain) pedal: holds every damper up, so notes ring on after their keys rise.
    Sustain,
    /// Holds up only the dampers of the keys that are down when it is pressed (a grand's middle
    /// pedal).
    Sostenuto,
    /// Shifts a grand's action sideways, so each hammer strikes fewer strings with less worn
    /// felt: softer and duller.
    UnaCorda,
    /// An upright's soft (half-blow) pedal: moves the hammers closer to the strings, a little
    /// softer without a change of timbre.
    Soft,
    /// Lowers a felt strip between the hammers and the strings, which mutes the sound strongly
    /// (an upright's practice pedal).
    PracticeMuffler,
    /// A volume pedal (the Clavinet's foot swell).
    FootSwell,
    /// An organ's swell (expression) pedal, which sets the level.
    Expression,
    /// An organ's crescendo pedal, which draws stops one after another.
    Crescendo,
}

/// A pedal and the controller a MIDI keyboard or a library sends for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pedal {
    /// What the pedal does.
    pub kind: PedalKind,
    /// The controller it sends, or `None` when MIDI 1.0 defines none for it (an upright's
    /// practice muffler, an organ's crescendo pedal).
    pub controller: Option<Controller>,
}

/// Whether strings or bells that are not played ring along with those that are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Resonance {
    /// Nothing rings along: the strings rest damped while their keys are up, or the instrument
    /// has no strings, plates or bells to ring.
    Absent,
    /// Little: each note is its own tone generator with no shared soundboard, or the strings
    /// that are not played ring only faintly.
    Weak,
    /// The strings ring along, most strongly at harmonically related pitches, while the sustain
    /// pedal holds the dampers up.
    DampersRaised,
    /// Nothing is damped: every bell rings freely, and bells struck together sustain together.
    Undamped,
}

/// A sound of the mechanism besides the tone, which a library may record on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Noise {
    /// The key and the action coming back up as the key is released (a library's key-release or
    /// note-off noise).
    KeyRelease,
    /// A damper landing on its string or tine as the key rises.
    DamperReturn,
    /// The pedal mechanism going down and coming back up.
    Pedal,
    /// The hammer heard apart from the tone, as it strikes or as it falls back.
    Hammer,
    /// The click of a key's electrical contacts as a note starts and stops (a Hammond's key
    /// click).
    KeyClick,
    /// The blower and the wind supply.
    Wind,
    /// The bellows and the air moving through them.
    Bellows,
    /// The clappers and the wires from the clavier.
    Clapper,
    /// Objects on the strings rattling on after the stroke (a prepared piano).
    Rattle,
}

/// How a note fades.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Decay {
    /// Fades while the key is held, fast at first (the prompt sound) and then much more slowly
    /// (the aftersound), because the two or three strings of a note couple through the bridge.
    /// The dampers stop it when the key rises, unless a pedal holds them up.
    TwoStage,
    /// Fades in one stage while the key is held, as a struck or plucked tine, reed, plate or
    /// string does. It stops when the key rises, unless a sustain pedal holds the dampers up.
    Decaying,
    /// Holds its level without fading while the key is held and wind or air is supplied, and
    /// stops when the key rises.
    WhileHeld,
    /// Rings on after the stroke until it dies away, whether the key is held or not.
    Free,
    /// Differs from key to key: a prepared piano's objects turn some keys into short thuds and
    /// leave others ringing.
    PerKey,
}

/// A lasting change to an instrument's ordinary sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimbreState {
    /// A felt strip between the hammers and the strings: soft, dark and close, with the
    /// mechanism prominent.
    Felt,
    /// Objects on or between the strings change each prepared key's sound, from a drum-like
    /// thud to a metallic ring, so the pitch of a prepared key is unreliable.
    Prepared,
    /// The unison strings of each note are slightly detuned, so they beat instead of locking
    /// and the aftersound wavers (a honky-tonk piano).
    DetunedUnisons,
}

/// How a keyboard instrument makes and stops its sound.
///
/// [`Instrument::try_new`](super::Instrument::try_new) checks that the keys lie inside the
/// playable range, that no pedal kind, controller or noise is listed twice, and that the
/// provenance is stated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardMechanics {
    /// What a key moves to make the sound.
    pub action: Action,
    /// The sounding pitches of the lowest and the highest key; every key between them is
    /// present. An instrument with registers (the organs, the accordion) is given at its unison
    /// (8′) register, and the accordion's keys are those of its right-hand keyboard.
    pub keys: PitchRange,
    /// The pedals, the most used first, each kind at most once; empty when there are none.
    pub pedals: &'static [Pedal],
    /// Whether strings or bells that are not played ring along.
    pub resonance: Resonance,
    /// The sounds of the mechanism besides the tone, each at most once; empty when none is
    /// documented.
    pub noises: &'static [Noise],
    /// How a note fades.
    pub decay: Decay,
    /// A lasting change to the ordinary sound, or `None` for an instrument in its ordinary state.
    pub timbre: Option<TimbreState>,
    /// What the values above leave out, such as a sound that rings along but is not sympathetic
    /// resonance; may be empty.
    pub note: &'static str,
    /// Where the values come from.
    pub provenance: Provenance,
}

impl KeyboardMechanics {
    /// The number of keys, from the lowest to the highest.
    #[must_use]
    pub const fn key_count(&self) -> u8 {
        let count = self.keys.high().get() - self.keys.low().get() + 1;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a range of MIDI notes holds 1 to 128 pitches, which fits a u8"
        )]
        let count = count as u8;
        count
    }

    /// The controller the pedal of `kind` sends; `None` when the instrument has no such pedal or
    /// MIDI 1.0 defines no controller for it.
    #[must_use]
    pub const fn controller(&self, kind: PedalKind) -> Option<Controller> {
        let mut i = 0;
        while i < self.pedals.len() {
            if self.pedals[i].kind as u8 == kind as u8 {
                return self.pedals[i].controller;
            }
            i += 1;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controllers_are_the_numbers_a_pedal_can_send() {
        for number in 0..=119 {
            let controller = Controller::new(number).expect("0-119 is a controller");
            assert_eq!(controller.number(), number);
        }
        // MIDI 1.0 channel mode messages and numbers outside seven bits.
        for number in 120..=u8::MAX {
            assert_eq!(Controller::new(number), None, "{number}");
        }
    }

    #[test]
    fn the_keys_are_counted_from_the_lowest_to_the_highest() {
        let mechanics = |low, high| KeyboardMechanics {
            action: Action::Grand,
            keys: PitchRange::new(low, high),
            pedals: &[],
            resonance: Resonance::DampersRaised,
            noises: &[],
            decay: Decay::TwoStage,
            timbre: None,
            note: "",
            provenance: Provenance::Estimate { reason: "test" },
        };
        // TST-10: an 88-key piano from A0 to C8, a 61-key manual from C2 to C7, one key, and
        // every MIDI note.
        assert_eq!(mechanics(21, 108).key_count(), 88);
        assert_eq!(mechanics(36, 96).key_count(), 61);
        assert_eq!(mechanics(60, 60).key_count(), 1);
        assert_eq!(mechanics(0, 127).key_count(), 128);
    }

    #[test]
    fn a_pedal_is_found_by_its_kind() {
        const PEDALS: &[Pedal] = &[
            Pedal {
                kind: PedalKind::Sustain,
                controller: Some(Controller(64)),
            },
            Pedal {
                kind: PedalKind::PracticeMuffler,
                controller: None,
            },
        ];
        let mechanics = KeyboardMechanics {
            action: Action::Upright,
            keys: PitchRange::new(21, 108),
            pedals: PEDALS,
            resonance: Resonance::DampersRaised,
            noises: &[],
            decay: Decay::TwoStage,
            timbre: None,
            note: "",
            provenance: Provenance::Estimate { reason: "test" },
        };
        assert_eq!(
            mechanics.controller(PedalKind::Sustain),
            Some(Controller(64))
        );
        // A pedal without a controller and a pedal the instrument lacks both answer `None`.
        assert_eq!(mechanics.controller(PedalKind::PracticeMuffler), None);
        assert_eq!(mechanics.controller(PedalKind::Sostenuto), None);
    }
}
