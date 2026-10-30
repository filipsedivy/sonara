//! The General MIDI Level 1 voice of an instrument: the program or the channel-10 keys a GM1
//! device plays for it, with an exact voice kept apart from a substitute; and the orchestral
//! percussion map, the GM1 keys that name an orchestral percussion instrument.
//!
//! GM1 numbers its 128 programs 1–128 and sends them as 0–127; its percussion key map assigns
//! the keys 35–81 of channel 10 (the MIDI Association's GM 1 Sound Set). A GM1 program or key is
//! named here by its number followed by its GM1 name, such as 41 Violin or 81 Open Triangle.
//!
//! The orchestral map sits beside the drum kit's map ([`crate::gm::GM_MAP`]), which it does not
//! change: the drum kit's sounds are recorded golden data (GLD-01), and the GM keys here are GM1
//! facts, not drum kit capabilities.

use super::data::INSTRUMENTS;
use super::id::InstrumentType;
use super::instrument::{Instrument, Provenance};

/// A General MIDI Level 1 program, numbered 1–128 as GM1 lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GmProgram(u8);

impl GmProgram {
    /// The lowest program number.
    pub const FIRST: u8 = 1;
    /// The highest program number.
    pub const LAST: u8 = 128;

    /// The program numbered `number`, or `None` outside 1–128.
    #[must_use]
    pub const fn new(number: u8) -> Option<Self> {
        if number < Self::FIRST || number > Self::LAST {
            return None;
        }
        Some(Self(number))
    }

    /// The program number as GM1 lists it, 1–128.
    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }

    /// The value a program change message carries, 0–127: the number minus one.
    #[must_use]
    pub const fn program_change(self) -> u8 {
        // `new` admits 1–128 only, so this cannot underflow.
        self.0 - 1
    }
}

/// A key of the General MIDI Level 1 percussion key map: a note on channel 10, 35–81.
///
/// Keys outside the map have no value of this type, so a key that only a later or vendor
/// standard defines, such as the Roland GS castanets on 85, cannot be stored as a GM1 key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GmNote(u8);

impl GmNote {
    /// The lowest key of the GM1 percussion key map (Acoustic Bass Drum).
    pub const LOWEST: u8 = 35;
    /// The highest key of the GM1 percussion key map (Open Triangle).
    pub const HIGHEST: u8 = 81;

    /// The key `key`, or `None` outside the GM1 percussion key map, 35–81.
    #[must_use]
    pub const fn new(key: u8) -> Option<Self> {
        if key < Self::LOWEST || key > Self::HIGHEST {
            return None;
        }
        Some(Self(key))
    }

    /// The MIDI note number of the key on channel 10.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// How close a substitute comes to the instrument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SubstituteFit {
    /// A near relative or another size of the instrument, such as the flute for the alto flute:
    /// the colour is close, the register may differ.
    Approximate,
    /// A different instrument that shares only part of the sound, such as the French horn for a
    /// Wagner tuba: a last resort before a built-in sound or silence.
    Weak,
}

/// What a substitute plays: a program, or channel-10 keys for an unpitched instrument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GmTarget {
    /// A GM1 program, played at sounding pitch.
    Program(GmProgram),
    /// GM1 channel-10 keys, the preferred one first.
    Notes(&'static [GmNote]),
}

/// The General MIDI Level 1 voice of an instrument.
///
/// An exact voice and a substitute are separate variants, so a caller cannot mistake an
/// approximation for the instrument: [`GmVoice::Program`] and [`GmVoice::Percussion`] name the
/// instrument, [`GmVoice::Substitute`] only stands in for it. Whether a substitute is ever played
/// is the caller's decision. A pitched instrument's voice is a program and an unpitched one's is
/// channel-10 keys; [`Instrument::try_new`] rejects any other pairing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GmVoice {
    /// The GM1 program whose name names the instrument or its class at its register, such as
    /// 41 Violin or 72 Clarinet for the clarinet in A.
    Program(GmProgram),
    /// The GM1 channel-10 keys whose names name the instrument, the preferred one first, such
    /// as 81 Open Triangle and 80 Mute Triangle.
    Percussion(&'static [GmNote]),
    /// GM1 has no voice for the instrument; `target` is the nearest it has.
    Substitute {
        /// The program or keys to play instead.
        target: GmTarget,
        /// How close the substitute comes.
        fit: SubstituteFit,
        /// Why the target was chosen.
        provenance: Provenance,
    },
    /// GM1 has nothing that would not misrepresent the instrument, such as the tam-tam.
    NoEquivalent,
}

impl GmVoice {
    /// The channel-10 keys of the voice: those of [`GmVoice::Percussion`] or of a substitute
    /// that plays keys; empty for a program and for [`GmVoice::NoEquivalent`].
    #[must_use]
    pub const fn keys(&self) -> &'static [GmNote] {
        match self {
            Self::Percussion(keys)
            | Self::Substitute {
                target: GmTarget::Notes(keys),
                ..
            } => keys,
            Self::Program(_)
            | Self::Substitute {
                target: GmTarget::Program(_),
                ..
            }
            | Self::NoEquivalent => &[],
        }
    }
}

/// The number of entries of [`ORCHESTRAL_PERCUSSION_MAP`]: every exact channel-10 key of every
/// record.
const MAP_LEN: usize = {
    let mut len = 0;
    let mut i = 0;
    while i < INSTRUMENTS.len() {
        len += exact_keys(&INSTRUMENTS[i]).len();
        i += 1;
    }
    len
};

/// The orchestral percussion map: every GM1 channel-10 key whose GM1 name names an orchestral
/// percussion instrument, with that instrument, in key order. 35 Acoustic Bass Drum is the bass
/// drum and 80 Mute Triangle the triangle.
///
/// It holds exact keys only, those of the records' [`GmVoice::Percussion`]: a key that only stands
/// in for an instrument, such as a kit crash for the clash cymbals, names no orchestral instrument
/// and has no entry. It is built from the records at compile time, and a compile-time check proves
/// that no key names two instruments. It sits beside the drum kit's [`crate::gm::GM_MAP`] and does
/// not change it.
pub static ORCHESTRAL_PERCUSSION_MAP: [(GmNote, InstrumentType); MAP_LEN] = orchestral_map();

/// The orchestral percussion instrument the GM1 channel-10 key `key` names, from
/// [`ORCHESTRAL_PERCUSSION_MAP`]; `None` for a key that names none, such as 42 Closed Hi Hat.
#[must_use]
pub fn percussion_instrument(key: GmNote) -> Option<InstrumentType> {
    ORCHESTRAL_PERCUSSION_MAP
        .iter()
        .find(|(entry, _)| *entry == key)
        .map(|(_, instrument_type)| *instrument_type)
}

/// The exact channel-10 keys of `record`: those of a [`GmVoice::Percussion`], else none.
const fn exact_keys(record: &Instrument) -> &'static [GmNote] {
    match record.gm() {
        GmVoice::Percussion(keys) => keys,
        GmVoice::Program(_) | GmVoice::Substitute { .. } | GmVoice::NoEquivalent => &[],
    }
}

/// Builds [`ORCHESTRAL_PERCUSSION_MAP`]: for each key in ascending order, every record that lists
/// it as an exact key.
const fn orchestral_map() -> [(GmNote, InstrumentType); MAP_LEN] {
    let mut map = [(GmNote(GmNote::LOWEST), InstrumentType::ALL[0]); MAP_LEN];
    let mut filled = 0;
    let mut key = GmNote::LOWEST;
    while key <= GmNote::HIGHEST {
        let mut i = 0;
        while i < INSTRUMENTS.len() {
            let keys = exact_keys(&INSTRUMENTS[i]);
            let mut j = 0;
            while j < keys.len() {
                if keys[j].0 == key {
                    map[filled] = (keys[j], INSTRUMENTS[i].instrument_type());
                    filled += 1;
                }
                j += 1;
            }
            i += 1;
        }
        key += 1;
    }
    // Every key of a record lies in 35–81 (`GmNote`), so the loop above found all `MAP_LEN`.
    assert!(
        filled == MAP_LEN,
        "every exact key of the records is in the map"
    );
    map
}

// Compile-time check: the keys of the map ascend strictly, so no key names two instruments and the
// map is a function from key to instrument.
const _: () = {
    let mut i = 1;
    while i < ORCHESTRAL_PERCUSSION_MAP.len() {
        assert!(
            ORCHESTRAL_PERCUSSION_MAP[i - 1].0.0 < ORCHESTRAL_PERCUSSION_MAP[i].0.0,
            "a GM1 channel-10 key names at most one orchestral instrument"
        );
        i += 1;
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_are_numbered_one_to_128_and_sent_one_lower() {
        assert_eq!(GmProgram::new(0), None);
        assert_eq!(GmProgram::new(129), None);
        assert_eq!(GmProgram::new(u8::MAX), None);
        for number in 1..=128 {
            let program = GmProgram::new(number).expect("1-128 is a GM1 program");
            assert_eq!(program.number(), number);
            assert_eq!(program.program_change(), number - 1);
        }
        let first = GmProgram::new(1).expect("first program");
        let last = GmProgram::new(128).expect("last program");
        assert_eq!((first.program_change(), last.program_change()), (0, 127));
    }

    #[test]
    fn percussion_keys_are_those_of_the_gm1_key_map() {
        for key in (0..35).chain(82..=u8::MAX) {
            assert_eq!(GmNote::new(key), None, "{key}");
        }
        // The Roland GS keys 82 Shaker, 83 Jingle Bell and 85 Castanets are not GM1.
        for key in [82, 83, 85] {
            assert_eq!(GmNote::new(key), None, "{key}");
        }
        let keys: Vec<u8> = (0..=u8::MAX)
            .filter_map(GmNote::new)
            .map(GmNote::get)
            .collect();
        assert_eq!(keys, (35..=81).collect::<Vec<u8>>());
    }

    #[test]
    fn a_voice_lists_the_keys_it_plays() {
        const KEYS: &[GmNote] = &[GmNote(81), GmNote(80)];
        const PROGRAM: GmProgram = GmProgram(48);
        let estimate = Provenance::Estimate { reason: "test" };
        assert_eq!(GmVoice::Percussion(KEYS).keys(), KEYS);
        let substitute = GmVoice::Substitute {
            target: GmTarget::Notes(KEYS),
            fit: SubstituteFit::Approximate,
            provenance: estimate,
        };
        assert_eq!(substitute.keys(), KEYS);
        let by_program = GmVoice::Substitute {
            target: GmTarget::Program(PROGRAM),
            fit: SubstituteFit::Weak,
            provenance: estimate,
        };
        for voice in [GmVoice::Program(PROGRAM), by_program, GmVoice::NoEquivalent] {
            assert_eq!(voice.keys(), [], "{voice:?}");
        }
    }
}
