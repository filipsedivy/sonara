//! The instrument type: a typed identifier for every acoustic instrument the metadata knows.
//!
//! The string id (`horn_f`) is the external form, used wherever an id crosses a boundary (TYP-06);
//! inside the crate, records and references use the enum, so a misspelt id does not compile
//! (PRI-04). The type names the acoustic instrument, not a sample preset of the instrument catalog
//! (TYP-07).

use super::data;
use super::instrument::{Family, Instrument};

/// An acoustic instrument type, such as the violin or the horn in F.
///
/// The variants are in score order, the order of the staves in a full score from top to bottom, so
/// the derived `Ord` is score order. [`InstrumentType::ALL`] lists them in that order and says how
/// it was chosen. Each [`Family`] forms one contiguous block of it.
///
/// The order is not a stored contract: a stored id is the string of [`InstrumentType::id`], which
/// never changes, while a later dataset may insert a type anywhere in the order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum InstrumentType {
    /// Piccolo (`piccolo`).
    Piccolo,
    /// Flute (`flute`).
    Flute,
    /// Alto flute (`alto_flute`).
    AltoFlute,
    /// Bass flute (`bass_flute`).
    BassFlute,
    /// Oboe (`oboe`).
    Oboe,
    /// Oboe d'amore (`oboe_damore`).
    OboeDamore,
    /// English horn (`english_horn`).
    EnglishHorn,
    /// E♭ clarinet (`clarinet_eb`).
    ClarinetEb,
    /// Clarinet in B♭ (`clarinet_bb`).
    ClarinetBb,
    /// Clarinet in A (`clarinet_a`).
    ClarinetA,
    /// Bass clarinet (`bass_clarinet`).
    BassClarinet,
    /// Soprano saxophone (`saxophone_soprano`).
    SaxophoneSoprano,
    /// Alto saxophone (`saxophone_alto`).
    SaxophoneAlto,
    /// Tenor saxophone (`saxophone_tenor`).
    SaxophoneTenor,
    /// Baritone saxophone (`saxophone_baritone`).
    SaxophoneBaritone,
    /// Bassoon (`bassoon`).
    Bassoon,
    /// Contrabassoon (`contrabassoon`).
    Contrabassoon,
    /// Horn in F (`horn_f`).
    HornF,
    /// Tenor Wagner tuba (`wagner_tuba_tenor`).
    WagnerTubaTenor,
    /// Bass Wagner tuba (`wagner_tuba_bass`).
    WagnerTubaBass,
    /// Piccolo trumpet (`trumpet_piccolo`).
    TrumpetPiccolo,
    /// Trumpet in C (`trumpet_c`).
    TrumpetC,
    /// Trumpet in B♭ (`trumpet_bb`).
    TrumpetBb,
    /// Cornet in B♭ (`cornet`).
    Cornet,
    /// Flugelhorn (`flugelhorn`).
    Flugelhorn,
    /// Tenor trombone (`trombone_tenor`).
    TromboneTenor,
    /// Bass trombone (`trombone_bass`).
    TromboneBass,
    /// Euphonium (`euphonium`).
    Euphonium,
    /// Tuba (`tuba`).
    Tuba,
    /// Cimbasso (`cimbasso`).
    Cimbasso,
    /// Timpani (`timpani`).
    Timpani,
    /// Bass drum (`bass_drum`).
    BassDrum,
    /// Snare drum (`snare_drum`).
    SnareDrum,
    /// Clash cymbals (`clash_cymbals`).
    ClashCymbals,
    /// Suspended cymbal (`suspended_cymbal`).
    SuspendedCymbal,
    /// Tam-tam (`tam_tam`).
    TamTam,
    /// Triangle (`triangle`).
    Triangle,
    /// Tambourine (`tambourine`).
    Tambourine,
    /// Castanets (`castanets`).
    Castanets,
    /// Wood block (`wood_block`).
    WoodBlock,
    /// Temple blocks (`temple_blocks`).
    TempleBlocks,
    /// Sleigh bells (`sleigh_bells`).
    SleighBells,
    /// Maracas (`maracas`).
    Maracas,
    /// Shaker (`shaker`).
    Shaker,
    /// Whip (`whip`).
    Whip,
    /// Glockenspiel (`glockenspiel`).
    Glockenspiel,
    /// Xylophone (`xylophone`).
    Xylophone,
    /// Vibraphone (`vibraphone`).
    Vibraphone,
    /// Marimba (`marimba`).
    Marimba,
    /// Tubular bells (`tubular_bells`).
    TubularBells,
    /// Crotales (`crotales`).
    Crotales,
    /// Carillon (`carillon`).
    Carillon,
    /// Harp (`harp`).
    Harp,
    /// Classical guitar (`guitar_classical`).
    GuitarClassical,
    /// Lute (`lute`).
    Lute,
    /// Mandolin (`mandolin`).
    Mandolin,
    /// Celesta (`celesta`).
    Celesta,
    /// Concert grand piano (`piano_concert_grand`).
    PianoConcertGrand,
    /// Studio grand piano (`piano_studio_grand`).
    PianoStudioGrand,
    /// Upright piano (`piano_upright`).
    PianoUpright,
    /// Felt piano (`piano_felt`).
    PianoFelt,
    /// Prepared piano (`piano_prepared`).
    PianoPrepared,
    /// Honky-tonk piano (`piano_honky_tonk`).
    PianoHonkyTonk,
    /// Harpsichord (`harpsichord`).
    Harpsichord,
    /// Clavichord (`clavichord`).
    Clavichord,
    /// Rhodes electric piano (`electric_piano_rhodes`).
    ElectricPianoRhodes,
    /// Wurlitzer electric piano (`electric_piano_wurlitzer`).
    ElectricPianoWurlitzer,
    /// Clavinet (`clavinet`).
    Clavinet,
    /// Pipe organ (`organ_pipe`).
    OrganPipe,
    /// Hammond organ (`organ_hammond`).
    OrganHammond,
    /// Accordion (`accordion`).
    Accordion,
    /// Soprano, solo voice (`voice_soprano`).
    VoiceSoprano,
    /// Mezzo-soprano, solo voice (`voice_mezzo_soprano`).
    VoiceMezzoSoprano,
    /// Contralto, solo voice (`voice_alto`).
    VoiceAlto,
    /// Tenor, solo voice (`voice_tenor`).
    VoiceTenor,
    /// Baritone, solo voice (`voice_baritone`).
    VoiceBaritone,
    /// Bass, solo voice (`voice_bass`).
    VoiceBass,
    /// Sopranos of a choir (`choir_soprano`).
    ChoirSoprano,
    /// Altos of a choir (`choir_alto`).
    ChoirAlto,
    /// Tenors of a choir (`choir_tenor`).
    ChoirTenor,
    /// Basses of a choir (`choir_bass`).
    ChoirBass,
    /// Violin (`violin`).
    Violin,
    /// Viola (`viola`).
    Viola,
    /// Cello (`cello`).
    Cello,
    /// Double bass (`double_bass`).
    DoubleBass,
}

impl InstrumentType {
    /// Every instrument type, in score order.
    ///
    /// The families follow the conventional full score: woodwinds, brass, percussion, then harp
    /// and the other plucked strings, keyboards, voices and the strings at the bottom (Widor,
    /// 1906; modern score-layout guides). Within a family:
    ///
    /// - woodwinds run high to low, each auxiliary beside its own family (piccolo above the
    ///   flutes, English horn below the oboes, E♭ clarinet above and bass clarinet below the
    ///   clarinets, contrabassoon at the bottom), with the saxophones after the clarinets;
    /// - the horns head the brass, the Wagner tubas directly below them, then trumpets, cornet,
    ///   flugelhorn, trombones and the tubas;
    /// - timpani come first in the percussion, then the untuned and then the tuned instruments;
    /// - the harp heads its block and the celesta comes before the pianos and the organs;
    /// - solo voices come before the choir sections;
    /// - the strings are violin, viola, cello, double bass.
    ///
    /// The order inside the untuned percussion, the keyboards and the low brass (euphonium,
    /// tuba, cimbasso) is an estimate: the sources read do not settle it.
    pub const ALL: [Self; 85] = [
        Self::Piccolo,
        Self::Flute,
        Self::AltoFlute,
        Self::BassFlute,
        Self::Oboe,
        Self::OboeDamore,
        Self::EnglishHorn,
        Self::ClarinetEb,
        Self::ClarinetBb,
        Self::ClarinetA,
        Self::BassClarinet,
        Self::SaxophoneSoprano,
        Self::SaxophoneAlto,
        Self::SaxophoneTenor,
        Self::SaxophoneBaritone,
        Self::Bassoon,
        Self::Contrabassoon,
        Self::HornF,
        Self::WagnerTubaTenor,
        Self::WagnerTubaBass,
        Self::TrumpetPiccolo,
        Self::TrumpetC,
        Self::TrumpetBb,
        Self::Cornet,
        Self::Flugelhorn,
        Self::TromboneTenor,
        Self::TromboneBass,
        Self::Euphonium,
        Self::Tuba,
        Self::Cimbasso,
        Self::Timpani,
        Self::BassDrum,
        Self::SnareDrum,
        Self::ClashCymbals,
        Self::SuspendedCymbal,
        Self::TamTam,
        Self::Triangle,
        Self::Tambourine,
        Self::Castanets,
        Self::WoodBlock,
        Self::TempleBlocks,
        Self::SleighBells,
        Self::Maracas,
        Self::Shaker,
        Self::Whip,
        Self::Glockenspiel,
        Self::Xylophone,
        Self::Vibraphone,
        Self::Marimba,
        Self::TubularBells,
        Self::Crotales,
        Self::Carillon,
        Self::Harp,
        Self::GuitarClassical,
        Self::Lute,
        Self::Mandolin,
        Self::Celesta,
        Self::PianoConcertGrand,
        Self::PianoStudioGrand,
        Self::PianoUpright,
        Self::PianoFelt,
        Self::PianoPrepared,
        Self::PianoHonkyTonk,
        Self::Harpsichord,
        Self::Clavichord,
        Self::ElectricPianoRhodes,
        Self::ElectricPianoWurlitzer,
        Self::Clavinet,
        Self::OrganPipe,
        Self::OrganHammond,
        Self::Accordion,
        Self::VoiceSoprano,
        Self::VoiceMezzoSoprano,
        Self::VoiceAlto,
        Self::VoiceTenor,
        Self::VoiceBaritone,
        Self::VoiceBass,
        Self::ChoirSoprano,
        Self::ChoirAlto,
        Self::ChoirTenor,
        Self::ChoirBass,
        Self::Violin,
        Self::Viola,
        Self::Cello,
        Self::DoubleBass,
    ];

    /// The stable identifier, for example `horn_f`: lower-case `snake_case`, never renamed.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Piccolo => "piccolo",
            Self::Flute => "flute",
            Self::AltoFlute => "alto_flute",
            Self::BassFlute => "bass_flute",
            Self::Oboe => "oboe",
            Self::OboeDamore => "oboe_damore",
            Self::EnglishHorn => "english_horn",
            Self::ClarinetEb => "clarinet_eb",
            Self::ClarinetBb => "clarinet_bb",
            Self::ClarinetA => "clarinet_a",
            Self::BassClarinet => "bass_clarinet",
            Self::SaxophoneSoprano => "saxophone_soprano",
            Self::SaxophoneAlto => "saxophone_alto",
            Self::SaxophoneTenor => "saxophone_tenor",
            Self::SaxophoneBaritone => "saxophone_baritone",
            Self::Bassoon => "bassoon",
            Self::Contrabassoon => "contrabassoon",
            Self::HornF => "horn_f",
            Self::WagnerTubaTenor => "wagner_tuba_tenor",
            Self::WagnerTubaBass => "wagner_tuba_bass",
            Self::TrumpetPiccolo => "trumpet_piccolo",
            Self::TrumpetC => "trumpet_c",
            Self::TrumpetBb => "trumpet_bb",
            Self::Cornet => "cornet",
            Self::Flugelhorn => "flugelhorn",
            Self::TromboneTenor => "trombone_tenor",
            Self::TromboneBass => "trombone_bass",
            Self::Euphonium => "euphonium",
            Self::Tuba => "tuba",
            Self::Cimbasso => "cimbasso",
            Self::Timpani => "timpani",
            Self::BassDrum => "bass_drum",
            Self::SnareDrum => "snare_drum",
            Self::ClashCymbals => "clash_cymbals",
            Self::SuspendedCymbal => "suspended_cymbal",
            Self::TamTam => "tam_tam",
            Self::Triangle => "triangle",
            Self::Tambourine => "tambourine",
            Self::Castanets => "castanets",
            Self::WoodBlock => "wood_block",
            Self::TempleBlocks => "temple_blocks",
            Self::SleighBells => "sleigh_bells",
            Self::Maracas => "maracas",
            Self::Shaker => "shaker",
            Self::Whip => "whip",
            Self::Glockenspiel => "glockenspiel",
            Self::Xylophone => "xylophone",
            Self::Vibraphone => "vibraphone",
            Self::Marimba => "marimba",
            Self::TubularBells => "tubular_bells",
            Self::Crotales => "crotales",
            Self::Carillon => "carillon",
            Self::Harp => "harp",
            Self::GuitarClassical => "guitar_classical",
            Self::Lute => "lute",
            Self::Mandolin => "mandolin",
            Self::Celesta => "celesta",
            Self::PianoConcertGrand => "piano_concert_grand",
            Self::PianoStudioGrand => "piano_studio_grand",
            Self::PianoUpright => "piano_upright",
            Self::PianoFelt => "piano_felt",
            Self::PianoPrepared => "piano_prepared",
            Self::PianoHonkyTonk => "piano_honky_tonk",
            Self::Harpsichord => "harpsichord",
            Self::Clavichord => "clavichord",
            Self::ElectricPianoRhodes => "electric_piano_rhodes",
            Self::ElectricPianoWurlitzer => "electric_piano_wurlitzer",
            Self::Clavinet => "clavinet",
            Self::OrganPipe => "organ_pipe",
            Self::OrganHammond => "organ_hammond",
            Self::Accordion => "accordion",
            Self::VoiceSoprano => "voice_soprano",
            Self::VoiceMezzoSoprano => "voice_mezzo_soprano",
            Self::VoiceAlto => "voice_alto",
            Self::VoiceTenor => "voice_tenor",
            Self::VoiceBaritone => "voice_baritone",
            Self::VoiceBass => "voice_bass",
            Self::ChoirSoprano => "choir_soprano",
            Self::ChoirAlto => "choir_alto",
            Self::ChoirTenor => "choir_tenor",
            Self::ChoirBass => "choir_bass",
            Self::Violin => "violin",
            Self::Viola => "viola",
            Self::Cello => "cello",
            Self::DoubleBass => "double_bass",
        }
    }

    /// The instrument type with identifier `id`, or `None` when no type has it. The match is
    /// exact: a display name (`Violin`) or any other spelling is not an id.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|instrument_type| instrument_type.id() == id)
    }

    /// The record of this type in the dataset, or `None` while the dataset holds no record for
    /// it. Never another type's record: the lookup table is checked at compile time.
    #[must_use]
    pub fn get(self) -> Option<&'static Instrument> {
        data::record(self)
    }

    /// The family the instrument belongs to. Voices, solo and choral, are [`Family::Choir`]; no
    /// type is [`Family::Other`].
    #[must_use]
    pub const fn family(self) -> Family {
        match self {
            Self::Piccolo
            | Self::Flute
            | Self::AltoFlute
            | Self::BassFlute
            | Self::Oboe
            | Self::OboeDamore
            | Self::EnglishHorn
            | Self::ClarinetEb
            | Self::ClarinetBb
            | Self::ClarinetA
            | Self::BassClarinet
            | Self::SaxophoneSoprano
            | Self::SaxophoneAlto
            | Self::SaxophoneTenor
            | Self::SaxophoneBaritone
            | Self::Bassoon
            | Self::Contrabassoon => Family::Woodwinds,
            Self::HornF
            | Self::WagnerTubaTenor
            | Self::WagnerTubaBass
            | Self::TrumpetPiccolo
            | Self::TrumpetC
            | Self::TrumpetBb
            | Self::Cornet
            | Self::Flugelhorn
            | Self::TromboneTenor
            | Self::TromboneBass
            | Self::Euphonium
            | Self::Tuba
            | Self::Cimbasso => Family::Brass,
            Self::Timpani
            | Self::BassDrum
            | Self::SnareDrum
            | Self::ClashCymbals
            | Self::SuspendedCymbal
            | Self::TamTam
            | Self::Triangle
            | Self::Tambourine
            | Self::Castanets
            | Self::WoodBlock
            | Self::TempleBlocks
            | Self::SleighBells
            | Self::Maracas
            | Self::Shaker
            | Self::Whip
            | Self::Glockenspiel
            | Self::Xylophone
            | Self::Vibraphone
            | Self::Marimba
            | Self::TubularBells
            | Self::Crotales
            | Self::Carillon => Family::Percussion,
            Self::Harp | Self::GuitarClassical | Self::Lute | Self::Mandolin => Family::Plucked,
            Self::Celesta
            | Self::PianoConcertGrand
            | Self::PianoStudioGrand
            | Self::PianoUpright
            | Self::PianoFelt
            | Self::PianoPrepared
            | Self::PianoHonkyTonk
            | Self::Harpsichord
            | Self::Clavichord
            | Self::ElectricPianoRhodes
            | Self::ElectricPianoWurlitzer
            | Self::Clavinet
            | Self::OrganPipe
            | Self::OrganHammond
            | Self::Accordion => Family::Keyboards,
            Self::VoiceSoprano
            | Self::VoiceMezzoSoprano
            | Self::VoiceAlto
            | Self::VoiceTenor
            | Self::VoiceBaritone
            | Self::VoiceBass
            | Self::ChoirSoprano
            | Self::ChoirAlto
            | Self::ChoirTenor
            | Self::ChoirBass => Family::Choir,
            Self::Violin | Self::Viola | Self::Cello | Self::DoubleBass => Family::Strings,
        }
    }
}

// Compile-time checks of the list above, so a broken edit does not build (PRI-04):
// - `ALL` holds every variant once, in declaration order, so its order is the derived `Ord` and
//   `ALL[t as usize] == t`;
// - every id is non-empty lower-case `snake_case` and no two types share an id;
// - each family is one contiguous block of the score order.
const _: () = {
    let all = InstrumentType::ALL;
    let mut i = 0;
    while i < all.len() {
        let instrument_type = all[i];
        assert!(
            instrument_type as usize == i,
            "InstrumentType::ALL lists every variant once, in declaration order"
        );
        assert!(
            is_snake_case(instrument_type.id()),
            "an instrument id is non-empty lower-case snake_case"
        );
        let mut j = i + 1;
        while j < all.len() {
            assert!(
                !same_text(instrument_type.id(), all[j].id()),
                "two instrument types share an id"
            );
            j += 1;
        }
        if i > 0 && instrument_type.family() as u8 != all[i - 1].family() as u8 {
            let mut k = 0;
            while k < i {
                assert!(
                    all[k].family() as u8 != instrument_type.family() as u8,
                    "a family forms one contiguous block of the score order"
                );
                k += 1;
            }
        }
        i += 1;
    }
};

/// Whether `id` is non-empty and holds only `a-z`, `0-9` and `_`.
pub(super) const fn is_snake_case(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
            return false;
        }
        i += 1;
    }
    true
}

/// Byte equality of two strings, usable in a `const` context.
pub(super) const fn same_text(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
