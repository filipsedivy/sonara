//! The chain's two tables that name libraries and plugins, which the orchestration metadata does
//! not know: the order of preference among the presets of the libraries Sonara has profiles for
//! (step 1), and the built-in sound that approximates each instrument type (step 6).
//!
//! Both are timbre and practice judgements, not facts read from a source.

use sonara_midi::orchestration::InstrumentType;
use sonara_midi::orchestration::InstrumentType::{
    Accordion, AltoFlute, BassClarinet, BassDrum, BassFlute, Bassoon, Carillon, Castanets, Celesta,
    Cello, ChoirAlto, ChoirBass, ChoirSoprano, ChoirTenor, Cimbasso, ClarinetA, ClarinetBb,
    ClarinetEb, ClashCymbals, Clavichord, Clavinet, Contrabassoon, Cornet, Crotales, DoubleBass,
    ElectricPianoRhodes, ElectricPianoWurlitzer, EnglishHorn, Euphonium, Flugelhorn, Flute,
    Glockenspiel, GuitarClassical, Harp, Harpsichord, HornF, Lute, Mandolin, Maracas, Marimba,
    Oboe, OboeDamore, OrganHammond, OrganPipe, PianoConcertGrand, PianoFelt, PianoHonkyTonk,
    PianoPrepared, PianoStudioGrand, PianoUpright, Piccolo, SaxophoneAlto, SaxophoneBaritone,
    SaxophoneSoprano, SaxophoneTenor, Shaker, SleighBells, SnareDrum, SuspendedCymbal, TamTam,
    Tambourine, TempleBlocks, Timpani, Triangle, TromboneBass, TromboneTenor, TrumpetBb, TrumpetC,
    TrumpetPiccolo, Tuba, TubularBells, Vibraphone, Viola, Violin, VoiceAlto, VoiceBaritone,
    VoiceBass, VoiceMezzoSoprano, VoiceSoprano, VoiceTenor, WagnerTubaBass, WagnerTubaTenor, Whip,
    WoodBlock, Xylophone,
};

/// The plugin ID of the Poly Synth.
pub(super) const POLY_SYNTH: &str = "sonara.synth.poly";
/// The plugin ID of the Drum Kit.
pub(super) const DRUM_KIT: &str = "sonara.drums";
/// The Drum Kit's preset for orchestral percussion: big low toms and long cymbals.
const ORCHESTRAL_KIT: &str = "Cinematic Toms";

/// A built-in instrument's preset that stands in for an instrument no installed sample plays
/// (step 6 of the chain).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    /// A preset of the Poly Synth, playing the part's notes.
    Synth {
        /// The preset's name.
        preset: &'static str,
    },
    /// A preset of the Poly Synth playing every stroke of an unpitched part on one key.
    SynthKey {
        /// The preset's name.
        preset: &'static str,
        /// The key, a MIDI note.
        key: u8,
    },
    /// A preset of the Drum Kit playing the part on its General MIDI percussion keys; an
    /// unpitched part plays on the first, and a pitched one loses its pitches.
    Drums {
        /// The preset's name.
        preset: &'static str,
        /// The keys, the first for an ordinary stroke.
        keys: &'static [u8],
    },
}

impl Builtin {
    /// The ID of the plugin that plays it.
    #[must_use]
    pub const fn plugin_id(self) -> &'static str {
        match self {
            Self::Synth { .. } | Self::SynthKey { .. } => POLY_SYNTH,
            Self::Drums { .. } => DRUM_KIT,
        }
    }

    /// The name of the plugin's preset.
    #[must_use]
    pub const fn preset(self) -> &'static str {
        match self {
            Self::Synth { preset } | Self::SynthKey { preset, .. } | Self::Drums { preset, .. } => {
                preset
            }
        }
    }

    /// The plugin's short name, for messages.
    pub(super) const fn plugin_name(self) -> &'static str {
        match self {
            Self::Synth { .. } | Self::SynthKey { .. } => "Poly Synth",
            Self::Drums { .. } => "Drum Kit",
        }
    }
}

/// The built-in sound of one instrument type.
#[derive(Debug, Clone, Copy)]
pub(super) struct BuiltinRow {
    /// The instrument type.
    pub instrument: InstrumentType,
    /// The sound that approximates it.
    pub sound: Builtin,
    /// Whether the user confirms it first, because it changes the part's character.
    pub asks_first: bool,
}

const fn synth(instrument: InstrumentType, preset: &'static str) -> BuiltinRow {
    BuiltinRow {
        instrument,
        sound: Builtin::Synth { preset },
        asks_first: false,
    }
}

const fn drums(instrument: InstrumentType, keys: &'static [u8]) -> BuiltinRow {
    BuiltinRow {
        instrument,
        sound: Builtin::Drums {
            preset: ORCHESTRAL_KIT,
            keys,
        },
        asks_first: false,
    }
}

const fn asking(row: BuiltinRow) -> BuiltinRow {
    BuiltinRow {
        asks_first: true,
        ..row
    }
}

/// The toms of the Drum Kit's General MIDI map, low to high; the hi-hats on 42, 44 and 46 and the
/// crash on 49 lie between them.
const TOMS: &[u8] = &[41, 43, 45, 47, 48, 50];

/// The built-in sound of every instrument type, at the type's place in score order (the order of
/// [`InstrumentType::ALL`]). Long woodwinds and flutes take the soft pad, reeds the square lead,
/// brass, strings and voices the presets named after them, plucked strings, harpsichords and
/// mallets the pluck, bells the glass bells, pianos the electric keys and organs the pad.
/// Percussion takes the Drum Kit's keys of the same General MIDI sounds, with the toms for the
/// timpani, whose pitches they lose. The triangle, which the Drum Kit lacks, rings on a high key
/// of the glass bells. A whip played as a clap and a prepared piano played as a pluck change
/// the part's character, so the user confirms them first.
pub(super) static BUILTINS: [BuiltinRow; InstrumentType::ALL.len()] = [
    synth(Piccolo, "Square Lead"),
    synth(Flute, "Warm Pad"),
    synth(AltoFlute, "Warm Pad"),
    synth(BassFlute, "Warm Pad"),
    synth(Oboe, "Square Lead"),
    synth(OboeDamore, "Square Lead"),
    synth(EnglishHorn, "Square Lead"),
    synth(ClarinetEb, "Square Lead"),
    synth(ClarinetBb, "Square Lead"),
    synth(ClarinetA, "Square Lead"),
    synth(BassClarinet, "Square Lead"),
    synth(SaxophoneSoprano, "Square Lead"),
    synth(SaxophoneAlto, "Square Lead"),
    synth(SaxophoneTenor, "Square Lead"),
    synth(SaxophoneBaritone, "Square Lead"),
    synth(Bassoon, "Square Lead"),
    synth(Contrabassoon, "Warm Pad"),
    synth(HornF, "Brass"),
    synth(WagnerTubaTenor, "Brass"),
    synth(WagnerTubaBass, "Brass"),
    synth(TrumpetPiccolo, "Brass"),
    synth(TrumpetC, "Brass"),
    synth(TrumpetBb, "Brass"),
    synth(Cornet, "Brass"),
    synth(Flugelhorn, "Brass"),
    synth(TromboneTenor, "Brass"),
    synth(TromboneBass, "Brass"),
    synth(Euphonium, "Brass"),
    synth(Tuba, "Brass"),
    synth(Cimbasso, "Brass"),
    drums(Timpani, TOMS),
    drums(BassDrum, &[36]),
    drums(SnareDrum, &[38]),
    drums(ClashCymbals, &[49]),
    // The ride for strokes, the crash for swells.
    drums(SuspendedCymbal, &[51, 49]),
    // The crash, with the long cymbal decay of the orchestral kit.
    drums(TamTam, &[49]),
    BuiltinRow {
        instrument: Triangle,
        sound: Builtin::SynthKey {
            preset: "Glass Bells",
            key: 96,
        },
        asks_first: false,
    },
    drums(Tambourine, &[54]),
    // The claves: the kit has no castanets.
    drums(Castanets, &[75]),
    drums(WoodBlock, &[76, 77]),
    drums(TempleBlocks, &[76, 77]),
    // The tambourine's jingles: the kit has no sleigh bells.
    drums(SleighBells, &[54]),
    drums(Maracas, &[70]),
    drums(Shaker, &[70]),
    // The clap: the kit has no whip.
    asking(drums(Whip, &[39])),
    synth(Glockenspiel, "Glass Bells"),
    synth(Xylophone, "Pluck"),
    synth(Vibraphone, "Glass Bells"),
    synth(Marimba, "Pluck"),
    synth(TubularBells, "Glass Bells"),
    synth(Crotales, "Glass Bells"),
    synth(Carillon, "Glass Bells"),
    synth(Harp, "Pluck"),
    synth(GuitarClassical, "Pluck"),
    synth(Lute, "Pluck"),
    synth(Mandolin, "Pluck"),
    synth(Celesta, "Glass Bells"),
    synth(PianoConcertGrand, "Electric Keys"),
    synth(PianoStudioGrand, "Electric Keys"),
    synth(PianoUpright, "Electric Keys"),
    synth(PianoFelt, "Electric Keys"),
    asking(synth(PianoPrepared, "Pluck")),
    synth(PianoHonkyTonk, "Electric Keys"),
    synth(Harpsichord, "Pluck"),
    synth(Clavichord, "Pluck"),
    synth(ElectricPianoRhodes, "Electric Keys"),
    synth(ElectricPianoWurlitzer, "Electric Keys"),
    synth(Clavinet, "Pluck"),
    synth(OrganPipe, "Warm Pad"),
    synth(OrganHammond, "Warm Pad"),
    synth(Accordion, "Warm Pad"),
    synth(VoiceSoprano, "Choir Pad"),
    synth(VoiceMezzoSoprano, "Choir Pad"),
    synth(VoiceAlto, "Choir Pad"),
    synth(VoiceTenor, "Choir Pad"),
    synth(VoiceBaritone, "Choir Pad"),
    synth(VoiceBass, "Choir Pad"),
    synth(ChoirSoprano, "Choir Pad"),
    synth(ChoirAlto, "Choir Pad"),
    synth(ChoirTenor, "Choir Pad"),
    synth(ChoirBass, "Choir Pad"),
    synth(Violin, "Strings"),
    synth(Viola, "Strings"),
    synth(Cello, "Strings"),
    synth(DoubleBass, "Strings"),
];

// Compile-time check: the row at each place of `BUILTINS` is that of the type at the same place
// of `InstrumentType::ALL`, so a lookup by place cannot return another type's sound (PRI-04).
const _: () = {
    let mut i = 0;
    while i < BUILTINS.len() {
        assert!(
            BUILTINS[i].instrument as usize == i,
            "BUILTINS holds every instrument type once, in score order"
        );
        i += 1;
    }
};

/// The built-in sound of `instrument`.
pub(super) fn builtin(instrument: InstrumentType) -> Option<&'static BuiltinRow> {
    BUILTINS
        .get(instrument as usize)
        .filter(|row| row.instrument == instrument)
}

/// The presets of VSCO 2 Community Edition and VCSL in order of preference, per instrument type
/// with several of them, by preset ID. The order only ranks presets that the catalog finds
/// playing the type and that tie on everything the chain weighs first; it adds none, so a
/// manifest that says a listed preset plays another type takes it out of its row.
///
/// The judgement behind it: a patch that switches the instrument's articulations by key comes
/// first, then the set of its single-articulation patches, then those patches with the ordinary
/// sound first; a library's own recording before a General MIDI style kit; the louder and newer
/// recordings before those a library marks quiet or legacy; and where both libraries record an
/// instrument, VSCO 2 CE's orchestral recording first.
pub(super) static PREFERRED: &[(InstrumentType, &[&str])] = &[
    (
        Piccolo,
        &[
            "vsco2ce/@piccolo",
            "vsco2ce/PiccoloSus",
            "vsco2ce/PiccoloStac",
        ],
    ),
    (
        Flute,
        &[
            "vsco2ce/Flute-KS",
            "vsco2ce/@flute",
            "vsco2ce/FluteSusVib",
            "vsco2ce/FluteSusNV",
            "vsco2ce/FluteExpVib",
            "vsco2ce/FluteStac",
        ],
    ),
    (
        Oboe,
        &[
            "vsco2ce/@oboe",
            "vsco2ce/OboeSusVib",
            "vsco2ce/OboeSusNV",
            "vsco2ce/OboeStac",
        ],
    ),
    (
        ClarinetBb,
        &[
            "vsco2ce/Clarinet-KS",
            "vsco2ce/@clarinet",
            "vsco2ce/ClarinetSus",
            "vsco2ce/ClarinetStac",
        ],
    ),
    (
        SaxophoneSoprano,
        &[
            "vcsl/Aerophones/Reed Aerophones/Saxello - Keyswitch",
            "vcsl/@saxello",
            "vcsl/Aerophones/Reed Aerophones/Saxello - Vibrato",
            "vcsl/Aerophones/Reed Aerophones/Saxello - Non-Vibrato",
            "vcsl/Aerophones/Reed Aerophones/Saxello - Staccato",
        ],
    ),
    (
        SaxophoneTenor,
        &[
            "vcsl/Aerophones/Reed Aerophones/Tenor Saxophone - Keyswitch",
            "vcsl/@tenor-saxophone",
            "vcsl/Aerophones/Reed Aerophones/Tenor Saxophone - Vibrato",
            "vcsl/Aerophones/Reed Aerophones/Tenor Saxophone - Non-Vibrato",
            "vcsl/Aerophones/Reed Aerophones/Tenor Saxophone - Staccato",
        ],
    ),
    (
        Bassoon,
        &[
            "vsco2ce/@bassoon",
            "vsco2ce/BassoonSus",
            "vsco2ce/BassoonVib",
            "vsco2ce/BassoonStac",
        ],
    ),
    (
        HornF,
        &[
            "vsco2ce/@horn",
            "vsco2ce/FHornSus",
            "vsco2ce/FHornStac",
            "vsco2ce/FHornMute",
        ],
    ),
    (
        TrumpetBb,
        &[
            "vsco2ce/@trumpet",
            "vsco2ce/TrumpetSus",
            "vsco2ce/TrumpetSusVib",
            "vsco2ce/TrumpetStac",
            "vsco2ce/TrumpetStraightMuteSus",
            "vsco2ce/TrumpetHarmonMuteSus",
        ],
    ),
    (
        TromboneTenor,
        &[
            "vsco2ce/@trombone",
            "vsco2ce/TromboneSus",
            "vsco2ce/TromboneVib",
            "vsco2ce/TromboneStac",
        ],
    ),
    (
        Tuba,
        &[
            "vsco2ce/Tuba-KS",
            "vsco2ce/@tuba",
            "vsco2ce/TubaSus",
            "vsco2ce/TubaStac",
        ],
    ),
    (
        Timpani,
        &[
            "vsco2ce/@timpani",
            "vsco2ce/Timpani",
            "vsco2ce/TimpaniRolls",
            "vcsl/@timpani-1",
            "vcsl/Membranophones/Struck Membranophones/Timpani 1 - Hit",
            "vcsl/Membranophones/Struck Membranophones/Timpani 1 - Roll",
            "vcsl/Membranophones/Struck Membranophones/Timpani 2 - Scale",
        ],
    ),
    (
        BassDrum,
        &[
            "vcsl/Membranophones/Struck Membranophones/Bass Drum 2",
            "vcsl/Membranophones/Struck Membranophones/Bass Drum 1",
            "vcsl/Membranophones/Struck Membranophones/Bass Drum 3 - Legacy",
            "vsco2ce/GM-StylePerc",
        ],
    ),
    (
        SnareDrum,
        &[
            "vcsl/Membranophones/Struck Membranophones/Snare Drum, Modern 1",
            "vcsl/Membranophones/Struck Membranophones/Snare Drum, Modern 2",
            "vcsl/Membranophones/Struck Membranophones/Snare Drum, Modern 3",
            "vcsl/Membranophones/Struck Membranophones/Snare Drum, Rope Tension",
            "vsco2ce/GM-StylePerc",
        ],
    ),
    (
        ClashCymbals,
        &[
            "vcsl/Idiophones/Struck Idiophones/Clash Cymbals 1",
            "vcsl/Idiophones/Struck Idiophones/Clash Cymbals 2",
            "vsco2ce/GM-StylePerc",
        ],
    ),
    (
        SuspendedCymbal,
        &[
            "vcsl/Idiophones/Struck Idiophones/Suspended Cymbal 1",
            "vcsl/Idiophones/Struck Idiophones/Suspended Cymbal 2",
            "vsco2ce/GM-StylePerc",
        ],
    ),
    (
        TamTam,
        &[
            "vcsl/Idiophones/Struck Idiophones/Gong 1",
            "vcsl/Idiophones/Struck Idiophones/Gong 2",
            "vsco2ce/GM-StylePerc",
        ],
    ),
    (
        Triangle,
        &[
            "vcsl/Idiophones/Struck Idiophones/Triangles",
            "vsco2ce/GM-StylePerc",
        ],
    ),
    (
        Tambourine,
        &[
            "vcsl/Idiophones/Struck Idiophones/Tambourine 1",
            "vcsl/Idiophones/Struck Idiophones/Tambourine 2",
            "vsco2ce/GM-StylePerc",
            "vcsl/Idiophones/Struck Idiophones/Tambourine 4 - Legacy",
            "vcsl/Idiophones/Struck Idiophones/Tambourine 5 - Legacy",
            "vcsl/Idiophones/Struck Idiophones/Tambourine 3 - Legacy",
        ],
    ),
    (
        SleighBells,
        &[
            "vcsl/Idiophones/Struck Idiophones/Sleigh Bells",
            "vsco2ce/GM-StylePerc",
        ],
    ),
    (
        Shaker,
        &[
            "vcsl/Idiophones/Struck Idiophones/Shaker, Small",
            "vcsl/Idiophones/Struck Idiophones/Shaker, Large",
            "vcsl/Idiophones/Struck Idiophones/Shaker - Legacy",
        ],
    ),
    (
        Glockenspiel,
        &[
            "vsco2ce/Glockenspiel",
            "vcsl/Idiophones/Struck Idiophones/Glockenspiel",
        ],
    ),
    (
        Xylophone,
        &[
            "vsco2ce/Xylophone",
            "vcsl/Idiophones/Struck Idiophones/Xylophone - Medium Mallets",
            "vcsl/Idiophones/Struck Idiophones/Xylophone - Hard Mallets",
            "vcsl/Idiophones/Struck Idiophones/Xylophone - Soft Mallets",
        ],
    ),
    (
        Vibraphone,
        &[
            "vcsl/Idiophones/Struck Idiophones/Vibraphone - Keyswitch",
            "vcsl/@vibraphone",
            "vcsl/Idiophones/Struck Idiophones/Vibraphone - Soft Mallets",
            "vcsl/Idiophones/Struck Idiophones/Vibraphone - Hard Mallets",
            "vcsl/Idiophones/Struck Idiophones/Vibraphone - Bowed",
        ],
    ),
    (
        Marimba,
        &[
            "vsco2ce/Marimba",
            "vcsl/Idiophones/Struck Idiophones/Marimba",
        ],
    ),
    (
        TubularBells,
        &[
            "vsco2ce/TubularBells",
            "vcsl/Idiophones/Struck Idiophones/Tubular Bells 1",
            "vcsl/Idiophones/Struck Idiophones/Tubular Bells 2",
            "vcsl/Idiophones/Struck Idiophones/Tubular Bells 3 - Legacy",
        ],
    ),
    (
        Harp,
        &[
            "vsco2ce/Harp",
            "vcsl/Chordophones/Composite Chordophones/Concert Harp",
        ],
    ),
    (
        PianoStudioGrand,
        &[
            "vcsl/Chordophones/Zithers/Grand Piano, Steinway B",
            "vcsl/Chordophones/Zithers/Grand Piano, Kawai",
            "vcsl/Chordophones/Zithers/Grand Piano, Kawai - Legacy",
        ],
    ),
    (
        PianoUpright,
        &[
            "vsco2ce/UprightPiano",
            "vsco2ce/VSUpright1",
            "vcsl/Chordophones/Zithers/Upright Piano, Yamaha",
            "vcsl/Chordophones/Zithers/Upright Piano, Knight",
        ],
    ),
    (
        Harpsichord,
        &[
            "vcsl/Chordophones/Zithers/Harpsichord, English - Keyswitch",
            "vcsl/Chordophones/Zithers/Harpsichord, English - Normal",
            "vcsl/Chordophones/Zithers/Harpsichord, French",
            "vcsl/Chordophones/Zithers/Harpsichord, Italian",
            "vcsl/Chordophones/Zithers/Harpsichord, Flemish - 4'",
        ],
    ),
    (
        OrganPipe,
        &[
            "vsco2ce/OrganLoud",
            "vsco2ce/OrganQuiet",
            "vsco2ce/OrganLoudPedal",
            "vsco2ce/OrganQuietPedal",
            "vcsl/Aerophones/Edge-blown Aerophones/Pipe Organ - Keyswitch",
            "vcsl/Aerophones/Edge-blown Aerophones/Pipe Organ - Loud",
            "vcsl/Aerophones/Edge-blown Aerophones/Pipe Organ - Quiet",
        ],
    ),
    (
        Violin,
        &[
            "vsco2ce/SViolin-KS",
            "vsco2ce/ViolinEns-KS",
            "vsco2ce/@violin-solo",
            "vsco2ce/@violin-section",
            "vsco2ce/SViolinVib",
            "vsco2ce/ViolinEnsSusVib",
            "vsco2ce/SViolinVib-Quiet",
            "vsco2ce/ViolinEnsSusVib-Quiet",
        ],
    ),
    (
        Viola,
        &[
            "vsco2ce/ViolaEns-KS",
            "vsco2ce/@viola-section",
            "vsco2ce/ViolaEnsSusVib",
            "vsco2ce/ViolaEnsSusVib-Quiet",
        ],
    ),
    (
        Cello,
        &[
            "vsco2ce/CelloEns-KS",
            "vsco2ce/@cello-section",
            "vsco2ce/CelloEnsSusVib",
            "vsco2ce/CelloEnsSusVib-Quiet",
        ],
    ),
    (
        DoubleBass,
        &[
            "vsco2ce/Contrabass-KS",
            "vsco2ce/@double-bass",
            "vsco2ce/ContrabassSusVB",
            "vsco2ce/ContrabassSusNV",
            "vsco2ce/ContrabassSusVB-Quiet",
        ],
    ),
];

/// The place of the preset `id` in the order of preference of `instrument`, or `None` when the
/// order does not list it.
pub(super) fn preference(instrument: InstrumentType, id: &str) -> Option<usize> {
    PREFERRED
        .iter()
        .find(|(listed, _)| *listed == instrument)
        .and_then(|(_, ids)| ids.iter().position(|listed| *listed == id))
}
