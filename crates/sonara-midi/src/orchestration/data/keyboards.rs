//! Keyboard records.

use super::sources::{cite, reason};
use super::{
    PLAYERS_ESTIMATE, cited, estimate, gm_program, gm_substitute, not_used, pitched, players,
};
use crate::orchestration::articulation::Articulation::{
    Accent, AirTone, CrescendoSwell, Damped, Glissando, Harmonics, Legato, LetRing, Marcato,
    PalmMute, Pizzicato, Portato, Sforzando, Staccatissimo, Staccato, Sustain, Tenuto, Tremolo,
    Trill, VibratoOn,
};
use crate::orchestration::ensemble_profile::{EnsembleProfile, Forms};
use crate::orchestration::general_midi::{GmVoice, SubstituteFit};
use crate::orchestration::id::{CategoryId, InstrumentType};
use crate::orchestration::instrument::{Instrument, InstrumentSpec, Provenance};
use crate::orchestration::names::Names;
use crate::orchestration::pitch::Clef;

pub(super) const CELESTA: Instrument = Instrument::new(
    InstrumentType::Celesta,
    InstrumentSpec {
        names: Names {
            en: "Celesta",
            cs: "celesta",
            de: "Celesta",
            fr: "célesta",
            it: "celesta",
            abbreviation: "Cel.",
        },
        aliases: aliases![
            En Singular: "celesta", "celeste";
            En Plural: "celestas";
            En Unmarked: "cel";
            Cs Singular: "celesta", "celeste", "celestu", "celestou";
            Cs Unmarked: "celesty";
            Cs Plural: "celest", "celestam", "celestach", "celestami";
            De Singular: "celesta";
            Fr Singular: "celesta";
            Fr Plural: "celestas";
            It Singular: "celesta";
            It Plural: "celeste";
        ],
        category: None,
        pitch: pitched(
            12,
            cited(
                48,
                113,
                cite!(
                    wikipedia_celesta: "a playing range of C3-F8 (48-113)",
                    schiedmayer_celesta: "two current 5 1/2-octave models with 66 keys c-f5 and a \
                        5-octave model with 61 keys c-c5";
                    "reading the maker's note names as sounding is an estimate; the top is the \
                        envelope of current instruments, while the standard five-octave model \
                        stops at 108",
                ),
            ),
            Some(cited(
                60,
                108,
                cite!(
                    wikipedia_celesta: "the four-octave sounding range C4-C8 (top 4186 Hz) that \
                        every orchestral celesta has",
                    schiedmayer_celesta: "Mustel's first model, c1-c5",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Staccato, Legato, Trill, Tremolo],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "melodic, arpeggio and chordal playing with short melody notes";
                "trill and tremolo are estimates from keyboard practice; the damper pedal is a \
                    control (CC 64), not an articulation",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("invented in 1886", PLAYERS_ESTIMATE),
            romantic: players(1, 1, "from 1888-1891", PLAYERS_ESTIMATE),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(0, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_program(9),
    },
);

pub(super) const PIANO_CONCERT_GRAND: Instrument = Instrument::new(
    InstrumentType::PianoConcertGrand,
    InstrumentSpec {
        names: Names {
            en: "Concert grand piano",
            cs: "koncertní křídlo",
            de: "Konzertflügel",
            fr: "piano à queue de concert",
            it: "pianoforte a coda da concerto",
            abbreviation: "Pno.",
        },
        aliases: aliases![
            En Singular: "concert grand piano", "concert grand";
            En Plural: "concert grand pianos", "concert grands";
            Cs Singular: "koncertni kridlo", "koncertniho kridla", "koncertnimu kridlu",
                "koncertnim kridle", "koncertnim kridlem";
            Cs Plural: "koncertni kridla", "koncertnich kridel", "koncertnim kridlum",
                "koncertnich kridlech", "koncertnimi kridly";
            De Unmarked: "konzertflugel";
            Fr Singular: "piano a queue de concert", "grand queue de concert";
            It Singular: "pianoforte a coda da concerto", "gran coda";
        ],
        category: Some(CategoryId::Piano),
        pitch: pitched(
            0,
            cited(
                21,
                108,
                cite!(
                    wikipedia_piano: "most modern pianos have 88 keys, A0-C8 (MIDI 21-108)",
                    bosendorfer_290_imperial: "the Imperial has 97 keys",
                    wikipedia_imperial_bosendorfer: "the Imperial extends down to C0";
                    "the 97-key Bosendorfer 290 Imperial, down to C0 = 12, is not the default",
                ),
            ),
            Some(estimate(
                21,
                108,
                "equal to playable: every key is equally reachable, and no source that was read \
                    gives a narrower range",
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            Tremolo,
            Staccatissimo,
            Portato,
            Accent,
            Sforzando,
            Glissando,
            LetRing,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                berlioz: "trills on every note; ringing notes held by the damper pedal";
                "the others are estimates from general piano practice; no source that was read \
                    lists piano articulations",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(
                0,
                1,
                "1 as concerto soloist; none in the orchestra",
                PLAYERS_ESTIMATE,
            ),
            romantic: players(
                0,
                1,
                "1 soloist; 0-1 in the orchestra (sometimes four hands)",
                PLAYERS_ESTIMATE,
            ),
            film: players(0, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_program(1),
    },
);

pub(super) const PIANO_STUDIO_GRAND: Instrument = Instrument::new(
    InstrumentType::PianoStudioGrand,
    InstrumentSpec {
        names: Names {
            en: "Studio grand piano",
            cs: "křídlo",
            de: "Studioflügel",
            fr: "piano à queue",
            it: "pianoforte a coda",
            abbreviation: "Pno.",
        },
        aliases: aliases![
            En Singular: "studio grand piano", "studio grand";
            En Plural: "studio grand pianos", "studio grands";
            Cs Singular: "kridlo", "kridlu", "kridle", "kridlem", "salonni kridlo",
                "salonniho kridla", "salonnimu kridlu", "salonnim kridle", "salonnim kridlem",
                "studiove kridlo", "studioveho kridla", "studiovemu kridlu", "studiovem kridle",
                "studiovym kridlem";
            Cs Unmarked: "kridla";
            Cs Plural: "kridel", "kridlum", "kridlech", "kridly", "salonni kridla",
                "salonnich kridel", "salonnim kridlum", "salonnich kridlech", "salonnimi kridly",
                "studiova kridla", "studiovych kridel", "studiovym kridlum", "studiovych kridlech",
                "studiovymi kridly";
            De Unmarked: "studioflugel", "salonflugel";
            Fr Singular: "piano a queue", "demi queue", "trois quarts de queue";
            Fr Plural: "pianos a queue";
            It Singular: "pianoforte a coda", "mezza coda", "tre quarti di coda";
            It Plural: "pianoforti a coda";
        ],
        category: Some(CategoryId::Piano),
        pitch: pitched(
            0,
            cited(
                21,
                108,
                cite!(
                    yamaha_cx_grand_pianos: "the C1X to C7X all have 88 keys",
                    wikipedia_piano: "88 keys are A0-C8",
                ),
            ),
            Some(estimate(
                21,
                108,
                "equal to playable: every key is equally reachable",
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            Tremolo,
            Staccatissimo,
            Portato,
            Accent,
            Sforzando,
            Glissando,
            LetRing,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                berlioz: "trills on every note; ringing notes held by the damper pedal";
                "the others are estimates from general piano practice",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(
                1,
                1,
                "chamber music, accompaniment, teaching",
                PLAYERS_ESTIMATE,
            ),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(0, 1, "the recording studio piano", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_substitute(
            2,
            SubstituteFit::Approximate,
            reason!(
                "GM1 defines piano sounds, not piano sizes (",
                wikipedia_general_midi,
                "). 2 Bright Acoustic Piano is the choice for the studio grand, with 1 Acoustic \
                    Grand Piano equally defensible",
            ),
        ),
    },
);

pub(super) const PIANO_UPRIGHT: Instrument = Instrument::new(
    InstrumentType::PianoUpright,
    InstrumentSpec {
        names: Names {
            en: "Upright piano",
            cs: "pianino",
            de: "Pianino",
            fr: "piano droit",
            it: "pianoforte verticale",
            abbreviation: "Pno.",
        },
        aliases: aliases![
            En Singular: "upright piano";
            En Plural: "upright pianos";
            Cs Singular: "pianino", "pianinu", "pianinem";
            Cs Unmarked: "pianina";
            Cs Plural: "pianin", "pianinum", "pianinech", "pianiny";
            De Singular: "pianino", "hochklavier";
            De Plural: "pianinos", "hochklaviere";
            Fr Singular: "piano droit";
            Fr Plural: "pianos droits";
            It Singular: "pianoforte verticale";
            It Plural: "pianoforti verticali";
        ],
        category: Some(CategoryId::Piano),
        pitch: pitched(
            0,
            cited(
                21,
                108,
                cite!(
                    yamaha_u_upright_pianos: "the U1 and U3 have 88 keys",
                    wikipedia_piano: "88 keys are A0-C8";
                    "older or smaller uprights may have fewer keys (an estimate)",
                ),
            ),
            Some(estimate(
                21,
                108,
                "equal to playable: every key is equally reachable",
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            Tremolo,
            Staccatissimo,
            Portato,
            Accent,
            Sforzando,
            Glissando,
            LetRing,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                berlioz: "trills; ringing notes held by the pedal",
                askenfelt_jansson: "fast repetition is weaker than on a grand",
                steinert_piano_actions: "fast repetition is weaker than on a grand";
                "the others are estimates from general piano practice",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(0, 1, "a celesta substitute", PLAYERS_ESTIMATE),
            romantic: players(
                0,
                1,
                "Rimsky-Korsakov prefers it to a grand inside the orchestra",
                PLAYERS_ESTIMATE,
            ),
            film: players(0, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_substitute(
            1,
            SubstituteFit::Approximate,
            reason!(
                "GM1 has no upright piano: programs 1-8 are grand, electric and harpsichord \
                    sounds (",
                wikipedia_general_midi,
                "). 1 Acoustic Grand Piano is the nearest acoustic piano",
            ),
        ),
    },
);

pub(super) const PIANO_FELT: Instrument = Instrument::new(
    InstrumentType::PianoFelt,
    InstrumentSpec {
        names: Names {
            en: "Felt piano",
            cs: "pianino s moderátorem",
            de: "Klavier mit Moderator",
            fr: "piano feutré",
            it: "pianoforte con feltro",
            abbreviation: "Pno.",
        },
        aliases: aliases![
            En Singular: "felt piano";
            En Plural: "felt pianos";
            Cs Singular: "pianino s moderatorem", "pianinu s moderatorem", "pianinem s moderatorem",
                "plsteny klavir", "plsteneho klaviru", "plstenemu klaviru", "plstenem klaviru",
                "plstenym klavirem";
            Cs Unmarked: "pianina s moderatorem";
            Cs Plural: "pianin s moderatorem", "pianinum s moderatorem", "pianinech s moderatorem",
                "pianiny s moderatorem", "plstene klaviry", "plstenych klaviru",
                "plstenym klavirum", "plstenych klavirech", "plstenymi klaviry";
            De Singular: "klavier mit moderator", "filzklavier";
            De Plural: "filzklaviere";
            Fr Singular: "piano feutre";
            Fr Plural: "pianos feutres";
            It Singular: "pianoforte con feltro";
        ],
        category: Some(CategoryId::Piano),
        pitch: pitched(
            0,
            cited(
                21,
                108,
                cite!(
                    spitfire_originals_cinematic_soft_piano: "a felted Yamaha U3",
                    yamaha_u_upright_pianos: "the U3 has 88 keys",
                    wikipedia_piano: "88 keys are A0-C8";
                    "the sampled ranges of felt libraries were not checked",
                ),
            ),
            Some(estimate(
                21,
                108,
                "equal to playable; the felt is quietest and most characteristic in the middle \
                    register, which no source states",
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Legato, Staccato, Tenuto, Portato, LetRing],
        articulations_provenance: Provenance::Estimate {
            reason: reason!(
                "the felt removes most of the attack, so accented articulations lose their point; \
                    libraries expose only notes, pedal and envelope (",
                spitfire_originals_felt_piano,
                "; ",
                spitfire_originals_cinematic_soft_piano,
                "; ",
                native_instruments_una_corda,
                ")",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: not_used("", PLAYERS_ESTIMATE),
            film: players(
                1,
                1,
                "an intimate, close-miked solo piano",
                PLAYERS_ESTIMATE,
            ),
            chamber: players(0, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_substitute(
            1,
            SubstituteFit::Weak,
            reason!(
                "GM1 has no felt piano (",
                wikipedia_general_midi,
                "). 1 Acoustic Grand Piano at a low velocity is the nearest acoustic piano, \
                    without the muffled colour of the felt",
            ),
        ),
    },
);

pub(super) const PIANO_PREPARED: Instrument = Instrument::new(
    InstrumentType::PianoPrepared,
    InstrumentSpec {
        names: Names {
            en: "Prepared piano",
            cs: "preparovaný klavír",
            de: "präpariertes Klavier",
            fr: "piano préparé",
            it: "pianoforte preparato",
            abbreviation: "Prep. Pno.",
        },
        aliases: aliases![
            En Singular: "prepared piano";
            En Plural: "prepared pianos";
            En Unmarked: "prep pno";
            Cs Singular: "preparovany klavir", "preparovaneho klaviru", "preparovanemu klaviru",
                "preparovanem klaviru", "preparovanym klavirem";
            Cs Plural: "preparovane klaviry", "preparovanych klaviru", "preparovanym klavirum",
                "preparovanych klavirech", "preparovanymi klaviry";
            De Singular: "prapariertes klavier";
            De Plural: "praparierte klaviere";
            Fr Singular: "piano prepare";
            Fr Plural: "pianos prepares";
            It Singular: "pianoforte preparato";
        ],
        category: Some(CategoryId::Piano),
        pitch: pitched(
            0,
            cited(
                21,
                108,
                cite!(
                    wikipedia_piano: "the keyboard is the normal 88 keys, A0-C8",
                    best_service_john_cage_prepared_piano: "45 of the 88 keys are prepared for \
                        Cage's Sonatas and Interludes",
                    wikipedia_sonatas_and_interludes: "45 prepared keys; a prepared key may not \
                        sound its nominal pitch",
                    wikipedia_prepared_piano: "a prepared key may not sound its nominal pitch",
                ),
            ),
            Some(estimate(
                21,
                108,
                "equal to playable; which keys are usable depends on the preparation table of the \
                    piece",
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[
            Staccato, Marcato, Tremolo, Harmonics, Pizzicato, LetRing, Glissando, Damped, PalmMute,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                wikipedia_sonatas_and_interludes: "the soft pedal changes prepared sounds",
                best_service_john_cage_prepared_piano: "five sampled articulations, names not \
                    stated";
                "the list is an estimate: inside-piano techniques are general practice",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: not_used("", PLAYERS_ESTIMATE),
            film: players(0, 1, "texture and percussion", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "solo and ensemble works after 1940", PLAYERS_ESTIMATE),
        }),
        gm: GmVoice::NoEquivalent,
    },
);

pub(super) const PIANO_HONKY_TONK: Instrument = Instrument::new(
    InstrumentType::PianoHonkyTonk,
    InstrumentSpec {
        names: Names {
            en: "Honky-tonk piano",
            cs: "honky-tonk piano",
            de: "Honky-Tonk-Klavier",
            fr: "piano honky-tonk",
            it: "pianoforte honky-tonk",
            abbreviation: "Pno.",
        },
        aliases: aliases![
            En Singular: "honky tonk piano", "honky tonk";
            En Plural: "honky tonk pianos";
            Cs Unmarked: "honky tonk piano";
            Cs Singular: "rozladene pianino", "rozladeneho pianina", "rozladenemu pianinu",
                "rozladenem pianinu", "rozladenym pianinem";
            Cs Plural: "rozladena pianina", "rozladenych pianin", "rozladenym pianinum",
                "rozladenych pianinech", "rozladenymi pianiny";
            De Singular: "honky tonk klavier";
            Fr Singular: "piano honky tonk";
            It Singular: "pianoforte honky tonk";
        ],
        category: Some(CategoryId::Piano),
        pitch: pitched(
            0,
            estimate(
                21,
                108,
                reason!(
                    "an 88-key piano, A0-C8 (",
                    wikipedia_piano,
                    "); ",
                    wikipedia_honky_tonk_piano,
                    " names old upright pianos but no key count, and 19th-century uprights may \
                        have only 85 keys",
                ),
            ),
            Some(estimate(21, 108, "equal to playable")),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[
            Staccato, Marcato, Trill, Tremolo, Accent, Glissando, LetRing,
        ],
        articulations_provenance: Provenance::Estimate {
            reason: "ragtime and saloon piano practice; no source that was read lists \
                articulations",
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: not_used("", PLAYERS_ESTIMATE),
            film: players(1, 1, "western saloon, period scenes", PLAYERS_ESTIMATE),
            chamber: not_used("", PLAYERS_ESTIMATE),
        }),
        gm: gm_program(4),
    },
);

pub(super) const HARPSICHORD: Instrument = Instrument::new(
    InstrumentType::Harpsichord,
    InstrumentSpec {
        names: Names {
            en: "Harpsichord",
            cs: "cembalo",
            de: "Cembalo",
            fr: "clavecin",
            it: "clavicembalo",
            abbreviation: "Hpschd.",
        },
        aliases: aliases![
            En Singular: "harpsichord";
            En Plural: "harpsichords";
            En Unmarked: "hpschd", "cemb";
            Cs Singular: "cembalo", "cembalu", "cembalem";
            Cs Unmarked: "cembala";
            Cs Plural: "cembal", "cembalum", "cembalech", "cembaly";
            De Singular: "cembalo";
            De Plural: "cembali", "cembalos";
            Fr Singular: "clavecin";
            Fr Plural: "clavecins";
            It Singular: "clavicembalo", "cembalo";
            It Plural: "clavicembali", "cembali";
        ],
        category: None,
        pitch: pitched(
            0,
            cited(
                29,
                89,
                cite!(
                    wikipedia_harpsichord: "F1-F6 (FF-f''') as the traditional range of a \
                        five-octave instrument",
                    sonatina_symphonic_orchestra: "the Harpsichord 8' patch maps keys 29-89",
                    wikipedia_eight_foot_pitch: "16' and 4' choirs sound an octave below and above \
                        the key";
                    "the key compass at 8' pitch; a 16' choir sounds down to 17 and a 4' choir up \
                        to 101",
                ),
            ),
            Some(estimate(
                36,
                84,
                reason!(
                    "C2-C6, four octaves, which all but the smallest instruments have; ",
                    wikipedia_harpsichord,
                    " says the smallest have under four octaves and the largest just over five, \
                        without endpoints",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Legato, Staccato, Trill],
        articulations_provenance: Provenance::Estimate {
            reason: "keyboard practice; no source that was read lists harpsichord articulations; \
                registers (8', 4', 16'), the lute stop, the buff stop and the coupler are controls",
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(
                1,
                1,
                "continuo, Baroque and early Classical",
                PLAYERS_ESTIMATE,
            ),
            romantic: not_used("", PLAYERS_ESTIMATE),
            film: players(0, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_program(7),
    },
);

// Aliases: "Clav." names the clavinet, the instrument it abbreviates in current use.
pub(super) const CLAVICHORD: Instrument = Instrument::new(
    InstrumentType::Clavichord,
    InstrumentSpec {
        names: Names {
            en: "Clavichord",
            cs: "klavichord",
            de: "Clavichord",
            fr: "clavicorde",
            it: "clavicordo",
            abbreviation: "Clav.",
        },
        aliases: aliases![
            En Singular: "clavichord";
            En Plural: "clavichords";
            Cs Singular: "klavichord", "klavichordem";
            Cs Unmarked: "klavichordu";
            Cs Plural: "klavichordy", "klavichordum", "klavichordech";
            De Singular: "clavichord", "klavichord";
            De Plural: "clavichorde", "klavichorde";
            Fr Singular: "clavicorde";
            Fr Plural: "clavicordes";
            It Singular: "clavicordo";
            It Plural: "clavicordi";
        ],
        category: None,
        pitch: pitched(
            0,
            cited(
                29,
                96,
                cite!(
                    hogwood_clavichords: "the largest compass among eleven listed instruments is \
                        FF-c'''' (Lindholm, Stockholm, 1794); FF-f''' (29-89) is the common late \
                        German compass",
                    yale_hoffmann_clavichord: "the late German compass FF-f'''",
                ),
            ),
            Some(cited(
                36,
                86,
                cite!(
                    hogwood_clavichords: "C-d''' (36-86) is the compass all eleven listed \
                        instruments share (derived)",
                    wikipedia_clavichord: "instruments built before the 1730s were mostly four \
                        octaves, which may be narrower",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Sustain, Legato, Staccato, Trill, VibratoOn, Portato],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                wikipedia_clavichord: "the Bebung (vibrato) and the Tragen der Toene (portato), \
                    both named by C. P. E. Bach in the quotation it gives";
                "sustain, legato, staccato and trill are estimates from keyboard practice",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("a domestic and practice instrument", PLAYERS_ESTIMATE),
            romantic: not_used("", PLAYERS_ESTIMATE),
            film: players(0, 1, "amplified, as a colour", PLAYERS_ESTIMATE),
            chamber: players(
                1,
                1,
                "solo, or with one soft instrument or voice",
                PLAYERS_ESTIMATE,
            ),
        }),
        gm: gm_substitute(
            8,
            SubstituteFit::Weak,
            reason!(
                "GM1 has no clavichord; its piano group lists 7 Harpsichord and 8 Clavi (",
                wikipedia_general_midi,
                "). 8 Clavi, a struck-string sound, is the nearest, with 7 Harpsichord the \
                    alternative",
            ),
        ),
    },
);

pub(super) const ELECTRIC_PIANO_RHODES: Instrument = Instrument::new(
    InstrumentType::ElectricPianoRhodes,
    InstrumentSpec {
        names: Names {
            en: "Rhodes piano",
            cs: "Fender Rhodes",
            de: "Fender Rhodes",
            fr: "piano Rhodes",
            it: "Fender Rhodes",
            abbreviation: "Rhodes",
        },
        aliases: aliases![
            En Singular: "rhodes piano", "fender rhodes", "rhodes";
            En Plural: "rhodes pianos";
            Cs Unmarked: "fender rhodes", "rhodes";
            De Unmarked: "fender rhodes", "rhodes";
            Fr Singular: "piano rhodes", "rhodes";
            Fr Plural: "pianos rhodes";
            It Unmarked: "fender rhodes", "rhodes";
        ],
        category: Some(CategoryId::ElectricPiano),
        pitch: pitched(
            0,
            cited(
                21,
                108,
                cite!(
                    apple_logic_instruments: "88-key models exist",
                    wikipedia_rhodes_piano: "88-key models exist",
                    rhodes_service_manual: "chapter 6 numbers the tone bars of the 88-key pianos \
                        1-88, like a piano \"whose keys are identified A1 through C88\": key 1 = \
                        A0 = MIDI 21 and key 88 = C8 = MIDI 108 (MIDI = key number + 20)",
                ),
            ),
            Some(estimate(
                28,
                100,
                reason!(
                    "a modelling choice: comfortable is the span of the 73-key pianos, so a part \
                        inside it plays on every full-size Rhodes and on the 73-key sampled and \
                        modelled references; the span itself, keys \"E8 through E80\", E1-E7 = \
                        MIDI 28-100, is stated by ",
                    rhodes_service_manual,
                    " and, \"E1 to E7 (middle C = C4)\", by ",
                    sound_on_sound_scarbee_rsp_73,
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Sustain, Legato, Staccato, Tenuto, Marcato, Trill, Glissando],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                apple_logic_instruments: "up to 88 voices, \"allowing for glissandi over the \
                    entire keyboard range when the sustain pedal is depressed\"";
                "the others are estimates from general keyboard practice; no source that was read \
                    lists articulations for the Rhodes; its tremolo (\"vibrato\"), phaser, chorus \
                    and drive are effects, which are controls",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("the instrument dates from 1946 on", PLAYERS_ESTIMATE),
            romantic: not_used("the instrument dates from 1946 on", PLAYERS_ESTIMATE),
            film: players(1, 1, "a keyboard player", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_program(5),
    },
);

pub(super) const ELECTRIC_PIANO_WURLITZER: Instrument = Instrument::new(
    InstrumentType::ElectricPianoWurlitzer,
    InstrumentSpec {
        names: Names {
            en: "Wurlitzer electronic piano",
            cs: "Wurlitzer",
            de: "Wurlitzer Electronic Piano",
            fr: "piano électrique Wurlitzer",
            it: "pianoforte elettrico Wurlitzer",
            abbreviation: "Wurli.",
        },
        aliases: aliases![
            En Singular: "wurlitzer electronic piano", "wurlitzer electric piano", "wurlitzer",
                "wurli";
            En Plural: "wurlitzers";
            Cs Unmarked: "wurlitzer";
            De Singular: "wurlitzer electronic piano", "wurlitzer";
            Fr Singular: "piano electrique wurlitzer", "wurlitzer";
            It Singular: "pianoforte elettrico wurlitzer", "wurlitzer";
        ],
        category: Some(CategoryId::ElectricPiano),
        pitch: pitched(
            0,
            cited(
                33,
                96,
                cite!(
                    wurlitzer_service_manual: "\"64 Notes, A-13 thru c-76\", the 88-key positions \
                        13-76 = A1-C7 = MIDI 33-96 (MIDI = key number + 20)",
                    arturia_wurlitzer_v_manual: "the same 200A specification; from the A an octave \
                        above the lowest note of an 88-key piano to the C an octave below its top",
                    wikipedia_wurlitzer_electronic_piano: "from the A an octave above the lowest \
                        note of an 88-key piano to the C an octave below its top",
                    apple_logic_instruments: "\"64 keys (A to C)\"",
                ),
            ),
            Some(estimate(
                33,
                96,
                "every key is equally reachable on a keyboard and no source names a narrower span, \
                    so comfortable equals playable",
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Sustain, Legato, Staccato, Tenuto, Marcato, Trill, Glissando],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                apple_logic_instruments: "the Vintage Electric Piano, which includes the Wurlitzer \
                    models, allows 88 voices, \"allowing for glissandi over the entire keyboard \
                    range when the sustain pedal is depressed\"";
                "the others are estimates from general keyboard practice; no source that was read \
                    lists articulations for the Wurlitzer; its \"vibrato\" (tremolo) is an \
                    amplifier effect, a control",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("the instrument dates from 1954 on", PLAYERS_ESTIMATE),
            romantic: not_used("the instrument dates from 1954 on", PLAYERS_ESTIMATE),
            film: players(1, 1, "a keyboard player", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_program(5),
    },
);

pub(super) const CLAVINET: Instrument = Instrument::new(
    InstrumentType::Clavinet,
    InstrumentSpec {
        names: Names {
            en: "Clavinet",
            cs: "Clavinet",
            de: "Clavinet",
            fr: "Clavinet",
            it: "Clavinet",
            abbreviation: "Clav.",
        },
        aliases: aliases![
            En Singular: "clavinet";
            En Plural: "clavinets";
            En Unmarked: "clav";
            Cs Unmarked: "clavinet";
            De Singular: "clavinet";
            De Plural: "clavinets";
            Fr Singular: "clavinet";
            Fr Plural: "clavinets";
            It Unmarked: "clavinet";
        ],
        category: None,
        pitch: pitched(
            0,
            cited(
                29,
                88,
                cite!(
                    hohner_clavinet_d6_manual: "\"60 piano keys with a range from contra F to \
                        e'''\" and a \"keyboard range of 5 octaves (F1-e3)\": contra F = F1 = MIDI \
                        29, e''' = E6 = MIDI 88",
                    wikipedia_clavinet: "\"Most models have 60 keys ranging from F1 to E6\"",
                    apple_logic_instruments: "\"The 60-key range (F to E) of the original D6\"",
                    arturia_clavinet_v_manual: "\"60 notes\"",
                ),
            ),
            Some(estimate(
                29,
                88,
                "every key is equally reachable and no source names a narrower span, so \
                    comfortable equals playable",
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Sustain, Legato, Staccato, Marcato, Trill, Portato, Damped],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                hohner_clavinet_d6_manual: "portato (\"One has to play more or less Portato to \
                    produce Staccato\"); the right-hand slide \"puts a damper on the strings and \
                    produces a dull, dry sound\"; the note sounds while the key is held",
                arturia_clavinet_v_manual: "the damper slide likened to a guitarist's palm mute";
                "legato, staccato, marcato and trill are estimates from keyboard practice",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("the instrument dates from 1964 on", PLAYERS_ESTIMATE),
            romantic: not_used("the instrument dates from 1964 on", PLAYERS_ESTIMATE),
            film: players(1, 1, "a keyboard player", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_program(8),
    },
);

pub(super) const ORGAN_PIPE: Instrument = Instrument::new(
    InstrumentType::OrganPipe,
    InstrumentSpec {
        names: Names {
            en: "Pipe organ",
            cs: "varhany",
            de: "Orgel",
            fr: "orgue",
            it: "organo a canne",
            abbreviation: "Org.",
        },
        aliases: aliases![
            En Singular: "pipe organ";
            En Plural: "pipe organs";
            Cs Unmarked: "varhany", "varhan", "varhanam", "varhanach", "varhanami";
            De Singular: "orgel";
            De Plural: "orgeln";
            Fr Singular: "orgue";
            Fr Plural: "orgues";
            It Singular: "organo a canne";
            It Plural: "organi a canne";
        ],
        category: Some(CategoryId::Organ),
        pitch: pitched(
            0,
            cited(
                36,
                96,
                cite!(
                    wikipedia_pipe_organ: "manuals of 61 notes, C-c'''' (36-96), and a pedalboard \
                        of 30 or 32 notes, C-f' or g' (36-65 or 67)",
                    widor: "manuals of 61 notes, C-c''''",
                    wikipedia_organ_stop: "32' and 2' stops sound two octaves below and above the \
                        key",
                    wikipedia_eight_foot_pitch: "the footage names the pitch of a stop";
                    "the key compass at 8' pitch; with a 32' stop the lowest key sounds 12, with a \
                        2' stop the top key sounds 120",
                ),
            ),
            Some(cited(
                36,
                91,
                cite!(
                    widor: "Cavaille-Coll and Mutin manuals of 56 notes, C-g''' (36-91)",
                    sonatina_symphonic_orchestra: "the sampled stops span 36-91";
                    "parts within it fit organs of 56 and of 61 notes",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Sustain, Legato, Staccato, Tenuto, Trill, Glissando],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                berlioz: "the legato, bound style",
                widor: "the legato, bound style; pedal legatos and glissandos";
                "staccato, tenuto and trill are estimates from keyboard practice; registration, \
                    couplers, swell and tremulant are controls",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(0, 1, "Baroque and sacred works", PLAYERS_ESTIMATE),
            romantic: players(0, 1, "", PLAYERS_ESTIMATE),
            film: players(0, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_program(20),
    },
);

pub(super) const ORGAN_HAMMOND: Instrument = Instrument::new(
    InstrumentType::OrganHammond,
    InstrumentSpec {
        names: Names {
            en: "Hammond organ",
            cs: "Hammondovy varhany",
            de: "Hammondorgel",
            fr: "orgue Hammond",
            it: "organo Hammond",
            abbreviation: "Hammond Org.",
        },
        aliases: aliases![
            En Singular: "hammond organ", "hammond";
            En Plural: "hammond organs";
            En Unmarked: "hammond org";
            Cs Unmarked: "hammondovy varhany", "hammondovych varhan", "hammondovym varhanam",
                "hammondovych varhanach", "hammondovymi varhanami";
            De Singular: "hammondorgel";
            De Plural: "hammondorgeln";
            Fr Singular: "orgue hammond";
            Fr Plural: "orgues hammond";
            It Singular: "organo hammond";
            It Plural: "organi hammond";
        ],
        category: Some(CategoryId::Organ),
        pitch: pitched(
            0,
            cited(
                36,
                96,
                cite!(
                    wikipedia_hammond_organ: "two manuals of 61 notes",
                    apple_logic_vintage_b3: "the lowest playable MIDI note is 36, and a 61-note \
                        keyboard C-C plays the whole range, with preset keys on 24-35",
                    electric_druid_hammond: "the 8' fundamental uses tonewheels 13-73; the 91 \
                        tonewheels span 32.69-5924.62 Hz";
                    "the key compass at 8' pitch; the 91 tonewheels span 32.69-5924.62 Hz = MIDI \
                        24-114, which bounds what 16' and 1' drawbars can sound",
                ),
            ),
            Some(estimate(
                36,
                96,
                "no part of the manual is avoided in idiomatic writing, so comfortable equals \
                    playable",
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Sustain, Legato, Staccato, Glissando],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                apple_logic_vintage_b3: "legato against detached playing as the trigger condition \
                    of the harmonic percussion";
                "staccato and the palm glissando are estimates; drawbars, percussion, scanner \
                    vibrato, Leslie speed, key click and expression are controls",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: not_used("", PLAYERS_ESTIMATE),
            film: players(0, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "jazz, gospel, rock band", PLAYERS_ESTIMATE),
        }),
        gm: gm_program(17),
    },
);

// Aliases: "Acc." is left out, since it also abbreviates accent and accelerando.
pub(super) const ACCORDION: Instrument = Instrument::new(
    InstrumentType::Accordion,
    InstrumentSpec {
        names: Names {
            en: "Accordion",
            cs: "akordeon",
            de: "Akkordeon",
            fr: "accordéon",
            it: "fisarmonica",
            abbreviation: "Acc.",
        },
        aliases: aliases![
            En Singular: "accordion", "piano accordion";
            En Plural: "accordions";
            Cs Singular: "akordeon", "akordeonem";
            Cs Unmarked: "akordeonu";
            Cs Plural: "akordeony", "akordeonum", "akordeonech";
            De Singular: "akkordeon";
            De Plural: "akkordeons";
            Fr Singular: "accordeon";
            Fr Plural: "accordeons";
            It Singular: "fisarmonica";
            It Plural: "fisarmoniche";
        ],
        category: None,
        pitch: pitched(
            0,
            cited(
                36,
                98,
                cite!(
                    wikipedia_piano_accordion: "the right hand reaches written D7 (98) on 49-key \
                        piano accordions",
                    wikipedia_stradella_bass: "the Stradella bass reed set starts at C2 (36)",
                    wikipedia_accordion_reed_ranks: "16' and 4' reed ranks sound an octave below \
                        and above";
                    "the key compass at 8' (clarinet) pitch; a 16' (bassoon) reed sounds an octave \
                        lower and a 4' (piccolo) reed an octave higher, so a 41-key instrument \
                        sounds F2-A7 (41-105) and a 49-key one D2-D8",
                ),
            ),
            Some(cited(
                53,
                93,
                cite!(
                    wikipedia_piano_accordion: "F3-A6 (53-93), the written right-hand range of the \
                        standard 120-bass, 41-key piano accordion";
                    "notes below 53 belong to the left hand, whose Stradella bass buttons sound \
                        fixed pitch classes in fixed octaves",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Sustain, Legato, Staccato, Tremolo, CrescendoSwell, AirTone],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                wikipedia_accordion: "bellows swells and fades; the bellows shake, a rapid \
                    repetition; the whoosh of the silent air button";
                "legato and staccato are estimates; register switches are controls",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: players(0, 4, "rare", PLAYERS_ESTIMATE),
            film: players(0, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
        gm: gm_program(22),
    },
);
