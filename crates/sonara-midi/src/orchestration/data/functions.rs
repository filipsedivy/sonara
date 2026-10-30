//! The function table: for a melody and a bass in each register band, the instruments that carry
//! it, in the order a writer reaches for them.
//!
//! The order across families follows Rimsky-Korsakov's ranking: the strings first, for expression
//! and endurance, then the woodwinds, the brass and the instruments whose colours tire the ear
//! sooner. Within a family the order is the one the source gives. "The clarinet" and "the trumpet"
//! of the handbooks stand for the B♭ and A clarinets and the B♭ and C trumpets, as in the doubling
//! tables.

use super::sources::cite;
use crate::orchestration::id::InstrumentType::{
    BassClarinet, Bassoon, Celesta, Cello, ClarinetA, ClarinetBb, ClarinetEb, Contrabassoon,
    DoubleBass, EnglishHorn, Euphonium, Flute, Glockenspiel, Harp, HornF, Oboe, Piccolo, Timpani,
    TromboneBass, TromboneTenor, TrumpetBb, TrumpetC, Tuba, Viola, Violin, Xylophone,
};
use crate::orchestration::instrument::Provenance;
use crate::orchestration::rules::RegisterBand::{Alto, Bass, High, Soprano, Sub, Tenor, Top};
use crate::orchestration::rules::{Carriers, Role};

/// The function table's rows: the melody in four groups of bands, from the top, and the bass.
pub static CARRIERS: [Carriers; 5] = [
    Carriers {
        role: Role::Melody,
        bands: &[High, Top],
        instruments: &[Violin, Piccolo, Flute, ClarinetEb, TrumpetC, TrumpetBb],
        colours: &[Glockenspiel, Xylophone, Celesta],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 35: the strings rank first \
                    for expression and endurance, and the colours of the wind and percussion \
                    tire the ear sooner; Ch. II, Melody in stringed instruments, pp. 36, 40-41 and \
                    44: the first violins carry a high melody, doubled an octave below from about \
                    the middle of the fifth octave; Ch. I, wood-wind remarks, p. 20: the piccolo \
                    extends the flute upward and the small clarinet is more penetrating than the \
                    B-flat clarinet in its top; Ch. II, Melody in the brass, pp. 53-54: the \
                    trumpets carry a melody only in fanfare-like or unmodulated diatonic phrases",
                vsl_academy_glockenspiel: "it doubles a melody in unison or one to three octaves \
                    above",
                vsl_academy_xylophone: "it accents the top notes of a melody",
                wikipedia_celesta: "it often duplicates a theme",
            ),
        },
    },
    Carriers {
        role: Role::Melody,
        bands: &[Alto, Soprano],
        instruments: &[
            Violin,
            Flute,
            Oboe,
            ClarinetBb,
            ClarinetA,
            EnglishHorn,
            HornF,
            TrumpetBb,
            TrumpetC,
        ],
        colours: &[],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Melody in stringed instruments, p. 36: the first \
                    violins carry a melody in the middle registers, sometimes the second violins \
                    or both in unison, which gives a fuller resonance without impairing the \
                    quality; Ch. II, Melody in the wood-wind, p. 46: flute, oboe, clarinet and \
                    English horn as solo colours; Ch. II, Melody in the brass, p. 54: the horn's \
                    genial and poetic tone in piano, and the trumpet",
            ),
        },
    },
    Carriers {
        role: Role::Melody,
        bands: &[Tenor],
        instruments: &[
            Cello,
            Viola,
            Bassoon,
            BassClarinet,
            EnglishHorn,
            ClarinetBb,
            ClarinetA,
            HornF,
            TromboneTenor,
            Euphonium,
        ],
        colours: &[],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Melody in stringed instruments, pp. 37-38: the cello \
                    on its A string for a tense, passionate cantabile, the viola for short \
                    characteristic phrases, generally doubled by other strings or by the \
                    wood-wind; the bassoon in its upper register, the bass clarinet, the English \
                    horn and the low clarinet; Ch. II, Melody in the brass, pp. 53-55: the horn, \
                    and the tenor trombone in diatonic phrases of energetic power";
                "the euphonium, a tenor brass voice beside the trombone, is an estimate"
            ),
        },
    },
    Carriers {
        role: Role::Melody,
        bands: &[Sub, Bass],
        instruments: &[
            Cello,
            DoubleBass,
            Bassoon,
            Contrabassoon,
            Tuba,
            TromboneBass,
        ],
        colours: &[],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Melody in stringed instruments, p. 38 (d) and p. 40 \
                    (f): the cellos and double basses in unison or octaves occasionally carry a \
                    low phrase, never the double basses without the cellos or bassoons; bassoons \
                    and double bassoon double the basses; the tuba and the bass trombone are dark \
                    and massive",
            ),
        },
    },
    Carriers {
        role: Role::Bass,
        bands: &[Sub, Bass],
        instruments: &[
            Cello,
            DoubleBass,
            Bassoon,
            Contrabassoon,
            BassClarinet,
            Tuba,
            TromboneBass,
        ],
        colours: &[Timpani, Harp],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Stringed instruments doubling in octaves, p. 43: the \
                    cellos and double basses in octaves, the usual construction of the bass; Ch. \
                    III, Harmony in the brass, pp. 82-83: the tuba an octave below the group's \
                    bass, the bass trombone and tuba in octaves, and a bassoon rather than a \
                    trombone or tuba under a horn chord",
                forsyth: "pp. 45-46: two timpani on the main harmonic notes of a passage";
                "the bassoons, double bassoon and bass clarinet on the bass, and the harp's bass \
                    octaves in quiet passages, are estimates"
            ),
        },
    },
];

// Compile-time checks of the function table: every row checks out, and a function has at most one
// row per band, so a lookup is unambiguous.
const _: () = {
    let mut i = 0;
    while i < CARRIERS.len() {
        let row = CARRIERS[i];
        assert!(
            row.check().is_ok(),
            "a row of the function table is invalid"
        );
        let mut j = i + 1;
        while j < CARRIERS.len() {
            let other = CARRIERS[j];
            if row.role as u8 == other.role as u8 {
                let mut a = 0;
                while a < row.bands.len() {
                    let mut b = 0;
                    while b < other.bands.len() {
                        assert!(
                            row.bands[a] as u8 != other.bands[b] as u8,
                            "a function has two rows for one band"
                        );
                        b += 1;
                    }
                    a += 1;
                }
            }
            j += 1;
        }
        i += 1;
    }
};
