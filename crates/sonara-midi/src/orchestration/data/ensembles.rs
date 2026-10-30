//! The ensemble templates, one per [`EnsembleId`], with the string sizes and stage places they
//! share.
//!
//! Each template lists its parts in the order its score prints them and cites the works and
//! handbooks its roster follows; its provenance says what is estimated and which parts no
//! instrument type covers. The stage places are pans read off the seatings (see
//! [`STAGE_PANS`]); a chamber group is left centred.

use super::sources::{cite, reason};
use crate::orchestration::ensemble_profile::PlayerRange;
use crate::orchestration::ensembles::{
    Ensemble, EnsembleId, EnsembleSpec, Pan, Part, PartOrder, Players, Seating, StringSection,
    StringSize,
};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::id::InstrumentType::{
    AltoFlute, BassClarinet, BassDrum, Bassoon, Celesta, Cello, ChoirAlto, ChoirBass, ChoirSoprano,
    ChoirTenor, Cimbasso, ClarinetA, ClarinetBb, ClarinetEb, ClashCymbals, Contrabassoon, Cornet,
    Crotales, DoubleBass, EnglishHorn, Euphonium, Flugelhorn, Flute, Glockenspiel, Harp,
    Harpsichord, HornF, Oboe, OrganPipe, PianoConcertGrand, Piccolo, SaxophoneAlto,
    SaxophoneBaritone, SaxophoneSoprano, SaxophoneTenor, SnareDrum, SuspendedCymbal, TamTam,
    Timpani, Triangle, TromboneBass, TromboneTenor, TrumpetBb, TrumpetC, Tuba, TubularBells,
    Vibraphone, Viola, Violin, VoiceBaritone, VoiceSoprano, WagnerTubaBass, WagnerTubaTenor,
    Xylophone,
};
use crate::orchestration::instrument::{Family, Provenance};

const REQUIRED: bool = true;
const OPTIONAL: bool = false;

/// One player of the part's own.
const ONE: Players = Players::Own(PlayerRange::new(1, 1));

/// Both seatings, the American one the default.
const AMERICAN_FIRST: &[Seating] = &[Seating::American, Seating::Antiphonal];

/// Both seatings, the antiphonal one the default: the layout the composers of the Classical and
/// the late-Romantic orchestra wrote for.
const ANTIPHONAL_FIRST: &[Seating] = &[Seating::Antiphonal, Seating::American];

/// A part with every field but the alternatives and the note.
const fn part(
    slug: &'static str,
    name: &'static str,
    short: &'static str,
    instrument: InstrumentType,
    players: Players,
    required: bool,
    pan: Pan,
) -> Part {
    Part {
        slug,
        name,
        short,
        instrument,
        alternatives: &[],
        players,
        required,
        pan,
        note: "",
    }
}

/// A part of `instrument` with one player of its own, at the instrument's stage place.
const fn staged(
    slug: &'static str,
    name: &'static str,
    short: &'static str,
    instrument: InstrumentType,
    required: bool,
) -> Part {
    part(
        slug,
        name,
        short,
        instrument,
        ONE,
        required,
        stage(instrument),
    )
}

/// A part of `instrument` played by the player of part `by`, at the instrument's stage place.
const fn doubled(
    slug: &'static str,
    name: &'static str,
    short: &'static str,
    instrument: InstrumentType,
    by: &'static str,
    required: bool,
) -> Part {
    let players = Players::Doubling { by };
    part(
        slug,
        name,
        short,
        instrument,
        players,
        required,
        stage(instrument),
    )
}

/// A choir section of `min` to `max` singers, at its stage place.
const fn chorus(
    slug: &'static str,
    name: &'static str,
    short: &'static str,
    instrument: InstrumentType,
    min: u8,
    max: u8,
    required: bool,
) -> Part {
    let players = Players::Own(PlayerRange::new(min, max));
    part(
        slug,
        name,
        short,
        instrument,
        players,
        required,
        stage(instrument),
    )
}

/// A string section part, at the section's stage place.
const fn section(
    slug: &'static str,
    name: &'static str,
    short: &'static str,
    section: StringSection,
    required: bool,
) -> Part {
    let instrument = section.instrument_type();
    let players = Players::Section(section);
    part(
        slug,
        name,
        short,
        instrument,
        players,
        required,
        section_stage(section),
    )
}

/// A required part of a chamber group: one player, centred.
const fn chamber(
    slug: &'static str,
    name: &'static str,
    short: &'static str,
    instrument: InstrumentType,
) -> Part {
    part(slug, name, short, instrument, ONE, REQUIRED, Pan::CENTRE)
}

/// A part of a big band at the pan `pan`, the same in both seatings.
const fn band(
    slug: &'static str,
    name: &'static str,
    short: &'static str,
    instrument: InstrumentType,
    players: Players,
    required: bool,
    pan: f32,
) -> Part {
    part(
        slug,
        name,
        short,
        instrument,
        players,
        required,
        Pan::same(pan),
    )
}

/// `part` with the note `note`.
const fn noted(part: Part, note: &'static str) -> Part {
    Part { note, ..part }
}

/// `part` with the alternatives `alternatives`.
const fn or(part: Part, alternatives: &'static [InstrumentType]) -> Part {
    Part {
        alternatives,
        ..part
    }
}

/// The stage place of `instrument`, an orchestra's wind, brass, percussion, harp, keyboard or
/// voice part, in the two seatings ([`STAGE_PANS`]); the strings are placed by section. An
/// instrument the table does not place is a compile error where a template uses it.
#[expect(
    clippy::match_same_arms,
    reason = "one arm per row of the stage table, as the seatings place the instruments"
)]
const fn stage(instrument: InstrumentType) -> Pan {
    match instrument {
        Piccolo => Pan::same(-0.20),
        Flute | AltoFlute => Pan::same(-0.15),
        Oboe => Pan::same(0.10),
        EnglishHorn => Pan::same(0.20),
        ClarinetBb | ClarinetA => Pan::same(-0.10),
        ClarinetEb => Pan::same(-0.15),
        BassClarinet => Pan::same(-0.20),
        Bassoon => Pan::same(0.10),
        Contrabassoon => Pan::same(0.20),
        // Orchestras move the horns between the two sides: left in the American seating, right
        // in the antiphonal one.
        HornF | WagnerTubaTenor | WagnerTubaBass => Pan {
            american: -0.35,
            antiphonal: 0.35,
        },
        TrumpetBb | TrumpetC => Pan::same(0.10),
        TromboneTenor => Pan::same(0.25),
        TromboneBass => Pan::same(0.35),
        Euphonium | Tuba | Cimbasso => Pan::same(0.40),
        Timpani => Pan::same(-0.10),
        Harp => Pan::same(-0.60),
        Celesta | PianoConcertGrand | Harpsichord | OrganPipe => Pan::same(-0.45),
        VoiceSoprano | VoiceBaritone => Pan::CENTRE,
        ChoirSoprano => Pan::same(-0.35),
        ChoirAlto => Pan::same(-0.10),
        ChoirTenor => Pan::same(0.10),
        ChoirBass => Pan::same(0.35),
        other if matches!(other.family(), Family::Percussion) => Pan::same(0.20),
        _ => panic!("the stage table places no such instrument"),
    }
}

/// The stage place of a string section in the two seatings ([`STAGE_PANS`]).
const fn section_stage(section: StringSection) -> Pan {
    match section {
        StringSection::ViolinsI => Pan::same(-0.55),
        StringSection::ViolinsII => Pan {
            american: -0.30,
            antiphonal: 0.55,
        },
        StringSection::Violas => Pan {
            american: 0.10,
            antiphonal: 0.20,
        },
        StringSection::Cellos => Pan {
            american: 0.35,
            antiphonal: -0.20,
        },
        StringSection::Basses => Pan {
            american: 0.45,
            antiphonal: -0.35,
        },
    }
}

/// Where the pans of an orchestra's parts come from.
const STAGE_PANS: Provenance = Provenance::Estimate {
    reason: reason!(
        "the pans read the places of the strings in the American and antiphonal seatings (",
        wikipedia_string_section,
        "; ",
        jacksonville_symphony_seating,
        ") and the usual places of the winds, brass, percussion, harp, keyboards and voices off a \
            stage plan, seen from the audience: the winds in the middle behind the strings, the \
            horns, which orchestras move between the sides, on the left in the American seating \
            and on the right in the antiphonal one, the timpani at the back centre and the other \
            percussion at the back right, harp and keyboards behind the first violins, the choir \
            behind the orchestra with the sopranos on the left, the soloists in front at the \
            centre. Values stay within 0.6 of the centre so a dry mix sounds no wider than a \
            stage, and low instruments sit nearer the centre because they localize poorly; no \
            source states pan values",
    ),
};

/// Why a chamber group's parts are centred.
const CHAMBER_PANS: Provenance = Provenance::Estimate {
    reason: "no source that was read places the players of a small chamber group, so every part \
        stays centred",
};

// The sizes of the string body, by section: first violins, second violins, violas, cellos,
// double basses.

const FULL: StringSize = StringSize {
    slug: "full",
    name: "Full",
    counts: [16, 14, 12, 10, 8],
    provenance: Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "a full orchestra has 16 first violins, 14 second violins, 12 \
                violas, 10 cellos and 8 to 10 double basses",
            forsyth_second_scan: "pp. 378-379: a first-class symphony orchestra averages 16 \
                players to each violin part",
            spitfire_symphonic_strings: "60 string players recorded in their places, 16, 14, \
                12, 10 and 8",
        ),
    },
};

const MEDIUM: StringSize = StringSize {
    slug: "medium",
    name: "Medium",
    counts: [12, 10, 8, 6, 4],
    provenance: Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "a medium orchestra has 12 first violins, 10 second violins, 8 \
                violas, 6 cellos and 4 to 6 double basses",
        ),
    },
};

const SMALL: StringSize = StringSize {
    slug: "small",
    name: "Small",
    counts: [8, 6, 4, 3, 2],
    provenance: Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "a small orchestra has 8 first violins, 6 second violins, 4 violas, \
                3 cellos and 2 to 3 double basses; fewer than 8 first violins destroys the \
                balance with the wind",
        ),
    },
};

const DIE_WALKURE: StringSize = StringSize {
    slug: "die_walkure",
    name: "Die Walküre",
    counts: [16, 16, 12, 12, 8],
    provenance: Provenance::Cited {
        source: cite!(
            wikipedia_string_section: "Wagner's Die Walküre calls for 16 first and 16 second \
                violins, 12 violas, 12 cellos and 8 double basses",
        ),
    },
};

const GURRE_LIEDER: StringSize = StringSize {
    slug: "gurre_lieder",
    name: "Gurre-Lieder",
    counts: [20, 20, 16, 16, 12],
    provenance: Provenance::Cited {
        source: cite!(
            wikipedia_string_orchestra: "Schoenberg's Gurre-Lieder calls for 84 strings, 20, \
                20, 16, 16 and 12",
        ),
    },
};

const CHAMBER: StringSize = StringSize {
    slug: "chamber",
    name: "Chamber",
    counts: [6, 5, 4, 4, 2],
    provenance: Provenance::Cited {
        source: cite!(
            wikipedia_string_orchestra: "a chamber-sized string orchestra has 12 to 21 players, \
                from 4, 3, 2, 2 and 1 to 6, 5, 4, 4 and 2",
        ),
    },
};

const SMALL_CHAMBER: StringSize = StringSize {
    slug: "small_chamber",
    name: "Small chamber",
    counts: [4, 3, 2, 2, 1],
    provenance: Provenance::Cited {
        source: cite!(
            wikipedia_string_orchestra: "a chamber-sized string orchestra has 12 to 21 players, \
                from 4, 3, 2, 2 and 1 to 6, 5, 4, 4 and 2",
        ),
    },
};

// The five string sections as an orchestra's parts.

const VIOLINS_I: Part = section(
    "violins_1",
    "Violins I",
    "Vn. I",
    StringSection::ViolinsI,
    REQUIRED,
);
const VIOLINS_II: Part = section(
    "violins_2",
    "Violins II",
    "Vn. II",
    StringSection::ViolinsII,
    REQUIRED,
);
const VIOLAS: Part = section("violas", "Violas", "Va.", StringSection::Violas, REQUIRED);
const CELLOS: Part = section("cellos", "Cellos", "Vc.", StringSection::Cellos, REQUIRED);
const BASSES: Part = section("basses", "Basses", "Cb.", StringSection::Basses, REQUIRED);

// The Classical orchestra.

const NATURAL_HORN: &str = "natural horns in the period; the horn in F stands in";
const NATURAL_TRUMPET: &str = "natural trumpets in the period; the trumpet in C stands in";

const CLASSICAL_PARTS: &[Part] = &[
    staged("flute_1", "Flute 1", "Fl. 1", Flute, REQUIRED),
    noted(
        staged("flute_2", "Flute 2", "Fl. 2", Flute, OPTIONAL),
        "Haydn's Symphony No. 104; Mozart's No. 41 has one flute",
    ),
    staged("oboe_1", "Oboe 1", "Ob. 1", Oboe, REQUIRED),
    staged("oboe_2", "Oboe 2", "Ob. 2", Oboe, REQUIRED),
    noted(
        staged("clarinet_1", "Clarinet 1", "Cl. 1", ClarinetA, OPTIONAL),
        "in A in Haydn's Symphony No. 104; none in Mozart's No. 41",
    ),
    noted(
        staged("clarinet_2", "Clarinet 2", "Cl. 2", ClarinetA, OPTIONAL),
        "in A in Haydn's Symphony No. 104; none in Mozart's No. 41",
    ),
    staged("bassoon_1", "Bassoon 1", "Fag. 1", Bassoon, REQUIRED),
    staged("bassoon_2", "Bassoon 2", "Fag. 2", Bassoon, REQUIRED),
    noted(
        staged("horn_1", "Horn 1", "Hn. 1", HornF, REQUIRED),
        NATURAL_HORN,
    ),
    noted(
        staged("horn_2", "Horn 2", "Hn. 2", HornF, REQUIRED),
        NATURAL_HORN,
    ),
    noted(
        staged("trumpet_1", "Trumpet 1", "Tpt. 1", TrumpetC, OPTIONAL),
        NATURAL_TRUMPET,
    ),
    noted(
        staged("trumpet_2", "Trumpet 2", "Tpt. 2", TrumpetC, OPTIONAL),
        NATURAL_TRUMPET,
    ),
    noted(
        staged("timpani", "Timpani", "Timp.", Timpani, OPTIONAL),
        "one player on two drums, tuned to the tonic and the dominant",
    ),
    VIOLINS_I,
    VIOLINS_II,
    VIOLAS,
    noted(CELLOS, "read one Bassi part with the double basses"),
    noted(BASSES, "double the cellos an octave lower"),
];

const CLASSICAL: Ensemble = Ensemble::new(
    EnsembleId::ClassicalOrchestra,
    EnsembleSpec {
        name: "Classical orchestra",
        description: "Haydn and Mozart, about 1790: flutes, oboes, clarinets, bassoons, horns and \
            trumpets in pairs, timpani and strings.",
        order: PartOrder::FullScore,
        seatings: ANTIPHONAL_FIRST,
        string_sizes: &[SMALL, MEDIUM, FULL],
        parts: CLASSICAL_PARTS,
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_mozart_symphony_41: "one flute, two oboes, two bassoons, two horns, \
                        two trumpets in C, timpani and strings",
                    wikipedia_haydn_symphony_104: "two flutes, two oboes, two clarinets in A, two \
                        bassoons, two horns, two trumpets, timpani and strings",
                    rimsky_korsakov: "a pair of kettledrums tuned to the tonic and the dominant \
                        was the standard up to and including Beethoven",
                    wikipedia_string_section: "in the Classical era the cellos and double basses \
                        read one Bassi part, the basses an octave lower",
                    jacksonville_symphony_seating: "the orchestra switches to the antiphonal \
                        seating for classical repertoire",
                ),
            },
            Provenance::Estimate {
                reason: "no instrument type covers the natural horn and the natural trumpet of \
                    the period, so the horn in F and the trumpet in C stand in; no source that was \
                    read gives the string sections of Haydn's or Mozart's orchestras, so the \
                    default is Rimsky-Korsakov's small orchestra, at his floor of eight first \
                    violins; the timpani, which played with the trumpets, are optional like them",
            },
            STAGE_PANS,
        ],
    },
);

// The Beethoven orchestra.

const BEETHOVEN_CLARINET: &str = "in B♭ and C in Symphony No. 5, in A, B♭ and C in No. 9; no \
    type covers the clarinet in C, so the clarinet in B♭ stands in";
const FIFTH_FINALE: &str = "the finale of Symphony No. 5 and two movements of No. 9";
const NINTH_FINALE: &str = "the finale of Symphony No. 9";

const BEETHOVEN_PARTS: &[Part] = &[
    noted(
        staged("piccolo", "Piccolo", "Picc.", Piccolo, OPTIONAL),
        "the finales of Symphonies No. 5 and No. 9",
    ),
    staged("flute_1", "Flute 1", "Fl. 1", Flute, REQUIRED),
    staged("flute_2", "Flute 2", "Fl. 2", Flute, REQUIRED),
    staged("oboe_1", "Oboe 1", "Ob. 1", Oboe, REQUIRED),
    staged("oboe_2", "Oboe 2", "Ob. 2", Oboe, REQUIRED),
    noted(
        or(
            staged("clarinet_1", "Clarinet 1", "Cl. 1", ClarinetBb, REQUIRED),
            &[ClarinetA],
        ),
        BEETHOVEN_CLARINET,
    ),
    noted(
        or(
            staged("clarinet_2", "Clarinet 2", "Cl. 2", ClarinetBb, REQUIRED),
            &[ClarinetA],
        ),
        BEETHOVEN_CLARINET,
    ),
    staged("bassoon_1", "Bassoon 1", "Fag. 1", Bassoon, REQUIRED),
    staged("bassoon_2", "Bassoon 2", "Fag. 2", Bassoon, REQUIRED),
    noted(
        staged(
            "contrabassoon",
            "Contrabassoon",
            "C-fag.",
            Contrabassoon,
            OPTIONAL,
        ),
        "the finales of Symphonies No. 5 and No. 9",
    ),
    staged("horn_1", "Horn 1", "Hn. 1", HornF, REQUIRED),
    staged("horn_2", "Horn 2", "Hn. 2", HornF, REQUIRED),
    noted(
        staged("horn_3", "Horn 3", "Hn. 3", HornF, OPTIONAL),
        "Symphony No. 9",
    ),
    noted(
        staged("horn_4", "Horn 4", "Hn. 4", HornF, OPTIONAL),
        "Symphony No. 9",
    ),
    staged("trumpet_1", "Trumpet 1", "Tpt. 1", TrumpetC, REQUIRED),
    staged("trumpet_2", "Trumpet 2", "Tpt. 2", TrumpetC, REQUIRED),
    noted(
        staged(
            "alto_trombone",
            "Alto trombone",
            "A. Tbn.",
            TromboneTenor,
            OPTIONAL,
        ),
        "the finale of Symphony No. 5 and two movements of No. 9; no type covers the alto \
            trombone, so the tenor trombone stands in",
    ),
    noted(
        staged(
            "tenor_trombone",
            "Tenor trombone",
            "T. Tbn.",
            TromboneTenor,
            OPTIONAL,
        ),
        FIFTH_FINALE,
    ),
    noted(
        staged(
            "bass_trombone",
            "Bass trombone",
            "B. Tbn.",
            TromboneBass,
            OPTIONAL,
        ),
        FIFTH_FINALE,
    ),
    staged("timpani", "Timpani", "Timp.", Timpani, REQUIRED),
    noted(
        staged(
            "bass_drum",
            "Bass drum (Perc. 1)",
            "B.D.",
            BassDrum,
            OPTIONAL,
        ),
        NINTH_FINALE,
    ),
    noted(
        staged(
            "cymbals",
            "Cymbals (Perc. 2)",
            "Cym.",
            ClashCymbals,
            OPTIONAL,
        ),
        NINTH_FINALE,
    ),
    noted(
        staged(
            "triangle",
            "Triangle (Perc. 3)",
            "Trgl.",
            Triangle,
            OPTIONAL,
        ),
        NINTH_FINALE,
    ),
    VIOLINS_I,
    VIOLINS_II,
    VIOLAS,
    CELLOS,
    noted(
        BASSES,
        "independent of the cellos more and more often from Beethoven on",
    ),
];

const BEETHOVEN: Ensemble = Ensemble::new(
    EnsembleId::BeethovenOrchestra,
    EnsembleSpec {
        name: "Beethoven orchestra",
        description: "Beethoven's Symphonies No. 5 and No. 9: woodwinds and trumpets in pairs \
            with piccolo and contrabassoon, up to four horns, three trombones, timpani, \
            percussion and strings.",
        order: PartOrder::FullScore,
        seatings: ANTIPHONAL_FIRST,
        string_sizes: &[MEDIUM, SMALL, FULL],
        parts: BEETHOVEN_PARTS,
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_beethoven_symphony_5: "the finale adds piccolo, contrabassoon and \
                        three trombones, alto, tenor and bass; clarinets in B♭ and C; trumpets in \
                        C",
                    wikipedia_beethoven_symphony_9: "four horns; trombones in the second and \
                        fourth movements; piccolo, contrabassoon, bass drum, triangle and cymbals \
                        in the finale; clarinets in A, B♭ and C",
                    wikipedia_orchestra: "Beethoven added a third and a fourth horn, the \
                        piccolo, the contrabassoon and the trombones to the orchestra",
                    wikipedia_string_section: "independent double bass parts become frequent \
                        from Beethoven on",
                    jacksonville_symphony_seating: "the orchestra switches to the antiphonal \
                        seating for classical repertoire",
                ),
            },
            Provenance::Estimate {
                reason: "no instrument type covers the clarinet in C or the alto trombone, so the \
                    clarinet in B♭ and the tenor trombone stand in; the scores name the \
                    percussion instruments, not their players, so each has its own; the piccolo \
                    is an extra rather than a flutist's doubling; no source gives the string \
                    sections, so the default is Rimsky-Korsakov's medium orchestra",
            },
            STAGE_PANS,
        ],
    },
);

// The Romantic orchestra.

const ROMANTIC_PARTS: &[Part] = &[
    staged("flute_1", "Flute 1", "Fl. 1", Flute, REQUIRED),
    staged("flute_2", "Flute 2", "Fl. 2", Flute, REQUIRED),
    noted(
        staged("flute_3", "Flute 3", "Fl. 3", Flute, OPTIONAL),
        "Tchaikovsky's Symphony No. 6",
    ),
    noted(
        doubled(
            "piccolo",
            "Piccolo (Fl. 3)",
            "Picc.",
            Piccolo,
            "flute_3",
            OPTIONAL,
        ),
        "Tchaikovsky's Symphony No. 6",
    ),
    staged("oboe_1", "Oboe 1", "Ob. 1", Oboe, REQUIRED),
    staged("oboe_2", "Oboe 2", "Ob. 2", Oboe, REQUIRED),
    noted(
        or(
            staged("clarinet_1", "Clarinet 1", "Cl. 1", ClarinetBb, REQUIRED),
            &[ClarinetA],
        ),
        "in A in Tchaikovsky's Symphony No. 6",
    ),
    noted(
        or(
            staged("clarinet_2", "Clarinet 2", "Cl. 2", ClarinetBb, REQUIRED),
            &[ClarinetA],
        ),
        "in A in Tchaikovsky's Symphony No. 6",
    ),
    staged("bassoon_1", "Bassoon 1", "Fag. 1", Bassoon, REQUIRED),
    staged("bassoon_2", "Bassoon 2", "Fag. 2", Bassoon, REQUIRED),
    noted(
        staged(
            "contrabassoon",
            "Contrabassoon",
            "C-fag.",
            Contrabassoon,
            OPTIONAL,
        ),
        "Brahms's Symphony No. 1",
    ),
    staged("horn_1", "Horn 1", "Hn. 1", HornF, REQUIRED),
    staged("horn_2", "Horn 2", "Hn. 2", HornF, REQUIRED),
    staged("horn_3", "Horn 3", "Hn. 3", HornF, REQUIRED),
    staged("horn_4", "Horn 4", "Hn. 4", HornF, REQUIRED),
    staged("trumpet_1", "Trumpet 1", "Tpt. 1", TrumpetBb, REQUIRED),
    staged("trumpet_2", "Trumpet 2", "Tpt. 2", TrumpetBb, REQUIRED),
    staged(
        "trombone_1",
        "Trombone 1",
        "Tbn. 1",
        TromboneTenor,
        REQUIRED,
    ),
    staged(
        "trombone_2",
        "Trombone 2",
        "Tbn. 2",
        TromboneTenor,
        REQUIRED,
    ),
    staged(
        "trombone_3",
        "Trombone 3",
        "B. Tbn.",
        TromboneBass,
        REQUIRED,
    ),
    noted(
        staged("tuba", "Tuba", "Tba.", Tuba, OPTIONAL),
        "Tchaikovsky's Symphony No. 6",
    ),
    staged("timpani", "Timpani", "Timp.", Timpani, REQUIRED),
    noted(
        staged(
            "bass_drum",
            "Bass drum (Perc. 1)",
            "B.D.",
            BassDrum,
            OPTIONAL,
        ),
        "Tchaikovsky's Symphony No. 6",
    ),
    noted(
        staged(
            "cymbals",
            "Cymbals (Perc. 2)",
            "Cym.",
            ClashCymbals,
            OPTIONAL,
        ),
        "Tchaikovsky's Symphony No. 6",
    ),
    noted(
        doubled(
            "tam_tam",
            "Tam-tam (Perc. 1)",
            "T.-t.",
            TamTam,
            "bass_drum",
            OPTIONAL,
        ),
        "optional in Tchaikovsky's Symphony No. 6",
    ),
    staged("harp", "Harp", "Hp.", Harp, OPTIONAL),
    VIOLINS_I,
    VIOLINS_II,
    VIOLAS,
    CELLOS,
    BASSES,
];

const ROMANTIC: Ensemble = Ensemble::new(
    EnsembleId::RomanticOrchestra,
    EnsembleSpec {
        name: "Romantic orchestra",
        description: "Brahms and Tchaikovsky: woodwinds in pairs, four horns, two trumpets, three \
            trombones, tuba, timpani, percussion, harp and strings.",
        order: PartOrder::FullScore,
        seatings: AMERICAN_FIRST,
        string_sizes: &[FULL, MEDIUM, SMALL],
        parts: ROMANTIC_PARTS,
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_brahms_symphony_1: "two flutes, two oboes, two clarinets, two \
                        bassoons, contrabassoon, four horns, two trumpets, three trombones in the \
                        finale only, timpani and strings",
                    wikipedia_tchaikovsky_symphony_6: "three flutes, the third doubling piccolo, \
                        two oboes, two clarinets in A, two bassoons, four horns, two trumpets, \
                        three trombones, tuba, timpani, bass drum, cymbals, an optional tam-tam \
                        and strings",
                    rimsky_korsakov: "the orchestra in pairs has two of each woodwind, four \
                        horns, two trumpets, three trombones and a tuba; the third flute player \
                        takes the piccolo; most scores have one harp part",
                ),
            },
            Provenance::Estimate {
                reason: "the three trombones are two tenors and a bass, the usual orchestral \
                    set; the scores name the percussion instruments, not their players, so the \
                    bass drum and the tam-tam share one; the American seating is the default, a \
                    choice, since orchestras play this repertoire in both",
            },
            STAGE_PANS,
        ],
    },
);

// The late-Romantic orchestra.

const LATE_ROMANTIC_PARTS: &[Part] = &[
    noted(
        staged("piccolo", "Piccolo", "Picc.", Piccolo, REQUIRED),
        "its own player",
    ),
    staged("flute_1", "Flute 1", "Fl. 1", Flute, REQUIRED),
    staged("flute_2", "Flute 2", "Fl. 2", Flute, REQUIRED),
    staged("flute_3", "Flute 3", "Fl. 3", Flute, REQUIRED),
    noted(
        staged("flute_4", "Flute 4", "Fl. 4", Flute, OPTIONAL),
        "Mahler's Symphony No. 6",
    ),
    staged("oboe_1", "Oboe 1", "Ob. 1", Oboe, REQUIRED),
    staged("oboe_2", "Oboe 2", "Ob. 2", Oboe, REQUIRED),
    staged("oboe_3", "Oboe 3", "Ob. 3", Oboe, REQUIRED),
    noted(
        staged("oboe_4", "Oboe 4", "Ob. 4", Oboe, OPTIONAL),
        "Mahler's Symphony No. 6",
    ),
    noted(
        staged(
            "english_horn",
            "English horn",
            "Eng. horn",
            EnglishHorn,
            REQUIRED,
        ),
        "its own player; the fourth oboe doubles it in Ein Heldenleben",
    ),
    staged(
        "clarinet_eb",
        "E♭ clarinet",
        "Cl. picc.",
        ClarinetEb,
        REQUIRED,
    ),
    staged("clarinet_1", "Clarinet 1", "Cl. 1", ClarinetBb, REQUIRED),
    staged("clarinet_2", "Clarinet 2", "Cl. 2", ClarinetBb, REQUIRED),
    noted(
        staged("clarinet_3", "Clarinet 3", "Cl. 3", ClarinetBb, OPTIONAL),
        "Mahler's Symphony No. 6",
    ),
    staged(
        "bass_clarinet",
        "Bass clarinet",
        "Cl. b.",
        BassClarinet,
        REQUIRED,
    ),
    staged("bassoon_1", "Bassoon 1", "Fag. 1", Bassoon, REQUIRED),
    staged("bassoon_2", "Bassoon 2", "Fag. 2", Bassoon, REQUIRED),
    staged("bassoon_3", "Bassoon 3", "Fag. 3", Bassoon, REQUIRED),
    noted(
        staged("bassoon_4", "Bassoon 4", "Fag. 4", Bassoon, OPTIONAL),
        "the finale of Mahler's Symphony No. 6",
    ),
    staged(
        "contrabassoon",
        "Contrabassoon",
        "C-fag.",
        Contrabassoon,
        REQUIRED,
    ),
    staged("horn_1", "Horn 1", "Hn. 1", HornF, REQUIRED),
    staged("horn_2", "Horn 2", "Hn. 2", HornF, REQUIRED),
    staged("horn_3", "Horn 3", "Hn. 3", HornF, REQUIRED),
    staged("horn_4", "Horn 4", "Hn. 4", HornF, REQUIRED),
    staged("horn_5", "Horn 5", "Hn. 5", HornF, REQUIRED),
    staged("horn_6", "Horn 6", "Hn. 6", HornF, REQUIRED),
    staged("horn_7", "Horn 7", "Hn. 7", HornF, REQUIRED),
    staged("horn_8", "Horn 8", "Hn. 8", HornF, REQUIRED),
    noted(
        doubled(
            "wagner_tuba_tenor_1",
            "Tenor Wagner tuba 1 (Hn. 5)",
            "Ten. Tb. 1",
            WagnerTubaTenor,
            "horn_5",
            OPTIONAL,
        ),
        "the Ring",
    ),
    noted(
        doubled(
            "wagner_tuba_tenor_2",
            "Tenor Wagner tuba 2 (Hn. 6)",
            "Ten. Tb. 2",
            WagnerTubaTenor,
            "horn_6",
            OPTIONAL,
        ),
        "the Ring",
    ),
    noted(
        doubled(
            "wagner_tuba_bass_1",
            "Bass Wagner tuba 1 (Hn. 7)",
            "B. Tb. 1",
            WagnerTubaBass,
            "horn_7",
            OPTIONAL,
        ),
        "the Ring",
    ),
    noted(
        doubled(
            "wagner_tuba_bass_2",
            "Bass Wagner tuba 2 (Hn. 8)",
            "B. Tb. 2",
            WagnerTubaBass,
            "horn_8",
            OPTIONAL,
        ),
        "the Ring",
    ),
    staged("trumpet_1", "Trumpet 1", "Tpt. 1", TrumpetBb, REQUIRED),
    staged("trumpet_2", "Trumpet 2", "Tpt. 2", TrumpetBb, REQUIRED),
    staged("trumpet_3", "Trumpet 3", "Tpt. 3", TrumpetBb, REQUIRED),
    noted(
        staged("trumpet_4", "Trumpet 4", "Tpt. 4", TrumpetBb, REQUIRED),
        "Ein Heldenleben has five, three in B♭ and two in E♭; no type covers the E♭ trumpet, so \
            the trumpet in B♭ stands in",
    ),
    noted(
        staged("trumpet_5", "Trumpet 5", "Tpt. 5", TrumpetBb, OPTIONAL),
        "the finale of Mahler's Symphony No. 6",
    ),
    noted(
        staged("trumpet_6", "Trumpet 6", "Tpt. 6", TrumpetBb, OPTIONAL),
        "the finale of Mahler's Symphony No. 6",
    ),
    staged(
        "trombone_1",
        "Trombone 1",
        "Tbn. 1",
        TromboneTenor,
        REQUIRED,
    ),
    staged(
        "trombone_2",
        "Trombone 2",
        "Tbn. 2",
        TromboneTenor,
        REQUIRED,
    ),
    staged(
        "trombone_3",
        "Trombone 3",
        "B. Tbn.",
        TromboneBass,
        REQUIRED,
    ),
    noted(
        staged(
            "trombone_4",
            "Trombone 4",
            "B. Tbn. 2",
            TromboneBass,
            OPTIONAL,
        ),
        "the finale of Mahler's Symphony No. 6",
    ),
    noted(
        staged("tenor_tuba", "Tenor tuba", "Euph.", Euphonium, OPTIONAL),
        "Ein Heldenleben",
    ),
    staged("tuba", "Tuba", "Tba.", Tuba, REQUIRED),
    staged("timpani_1", "Timpani 1", "Timp. 1", Timpani, REQUIRED),
    noted(
        staged("timpani_2", "Timpani 2", "Timp. 2", Timpani, OPTIONAL),
        "Mahler's Symphony No. 6: six drums, two players",
    ),
    staged(
        "bass_drum",
        "Bass drum (Perc. 1)",
        "B.D.",
        BassDrum,
        REQUIRED,
    ),
    staged(
        "snare_drum",
        "Snare drum (Perc. 2)",
        "S.D.",
        SnareDrum,
        REQUIRED,
    ),
    staged(
        "cymbals",
        "Cymbals (Perc. 3)",
        "Cym.",
        ClashCymbals,
        REQUIRED,
    ),
    doubled(
        "suspended_cymbal",
        "Suspended cymbal (Perc. 3)",
        "Sus. cym.",
        SuspendedCymbal,
        "cymbals",
        REQUIRED,
    ),
    doubled(
        "tam_tam",
        "Tam-tam (Perc. 1)",
        "T.-t.",
        TamTam,
        "bass_drum",
        REQUIRED,
    ),
    doubled(
        "triangle",
        "Triangle (Perc. 2)",
        "Trgl.",
        Triangle,
        "snare_drum",
        REQUIRED,
    ),
    staged(
        "glockenspiel",
        "Glockenspiel (Perc. 4)",
        "Glock.",
        Glockenspiel,
        REQUIRED,
    ),
    doubled(
        "xylophone",
        "Xylophone (Perc. 4)",
        "Xyl.",
        Xylophone,
        "glockenspiel",
        REQUIRED,
    ),
    noted(
        doubled(
            "tubular_bells",
            "Tubular bells (Perc. 4)",
            "Tub. B.",
            TubularBells,
            "glockenspiel",
            REQUIRED,
        ),
        "stand in for Mahler's deep bells",
    ),
    staged("harp_1", "Harp 1", "Hp. 1", Harp, REQUIRED),
    staged("harp_2", "Harp 2", "Hp. 2", Harp, REQUIRED),
    noted(
        staged("harp_3", "Harp 3", "Hp. 3", Harp, OPTIONAL),
        "four harps at one point in Mahler's Symphony No. 6",
    ),
    noted(
        staged("harp_4", "Harp 4", "Hp. 4", Harp, OPTIONAL),
        "four harps at one point in Mahler's Symphony No. 6",
    ),
    noted(
        staged("celesta", "Celesta", "Cel.", Celesta, OPTIONAL),
        "Mahler's Symphony No. 6",
    ),
    VIOLINS_I,
    VIOLINS_II,
    VIOLAS,
    CELLOS,
    BASSES,
];

const LATE_ROMANTIC: Ensemble = Ensemble::new(
    EnsembleId::LateRomanticOrchestra,
    EnsembleSpec {
        name: "Late-Romantic orchestra",
        description: "Wagner, Mahler and Strauss: woodwinds in fours, eight horns who also play \
            the Wagner tubas, four to six trumpets, four trombones, tubas, two timpanists, four \
            percussionists, harps, celesta and strings.",
        order: PartOrder::FullScore,
        seatings: ANTIPHONAL_FIRST,
        string_sizes: &[DIE_WALKURE, FULL, GURRE_LIEDER],
        parts: LATE_ROMANTIC_PARTS,
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_mahler_symphony_6: "piccolo; four flutes, the third and fourth \
                        doubling piccolo; four oboes, the third and fourth doubling English horn, \
                        and an English horn; E♭ clarinet; three clarinets; bass clarinet; four \
                        bassoons, the fourth in the finale; contrabassoon; eight horns; six \
                        trumpets, the fifth and sixth in the finale; four trombones, the fourth \
                        in the finale; tuba; six timpani for two players; percussion with deep \
                        bells, cowbells, a hammer and a Rute; harps, four at one point; celesta",
                    wikipedia_ein_heldenleben: "piccolo with its own player; three flutes; three \
                        oboes and an English horn doubled by a fourth oboe; a separate E♭ \
                        clarinet; two clarinets; bass clarinet; three bassoons; contrabassoon; \
                        eight horns; five trumpets, three in B♭ and two in E♭; trombones; tenor \
                        tuba; tuba; timpani; percussion; two harps",
                    rimsky_korsakov: "the orchestra in fours gives the piccolo and the bass \
                        clarinet their own players; six or eight horns; Wagner's Ring adds a \
                        quartet of tenor and bass tubas, a bass trumpet and a contrabass \
                        trombone; a full orchestra should have three or four harps",
                    jacksonville_symphony_seating: "Mahler still wrote for antiphonal violins",
                ),
            },
            Provenance::Estimate {
                reason: "no instrument type covers the E♭ trumpet, so the trumpet in B♭ stands \
                    in; the bass trumpet, the contrabass trombone, the cowbells and the hammer \
                    have no type and are left out, and so is the Rute, a beater on the bass drum; \
                    tubular bells stand in for Mahler's deep bells; Strauss's tenor tuba is the \
                    euphonium; horn players 5 to 8 take the Wagner tubas, the tenors on 5 and 6 \
                    and the basses on 7 and 8; the English horn has its own player; four players \
                    share the nine percussion instruments; Die Walküre's strings are the default",
            },
            STAGE_PANS,
        ],
    },
);

// The film orchestra.

const FILM_PARTS: &[Part] = &[
    staged("flute_1", "Flute 1", "Fl. 1", Flute, REQUIRED),
    staged("flute_2", "Flute 2", "Fl. 2", Flute, REQUIRED),
    staged("flute_3", "Flute 3", "Fl. 3", Flute, OPTIONAL),
    doubled(
        "piccolo",
        "Piccolo (Fl. 3)",
        "Picc.",
        Piccolo,
        "flute_3",
        OPTIONAL,
    ),
    doubled(
        "alto_flute",
        "Alto flute (Fl. 2)",
        "Alto Fl.",
        AltoFlute,
        "flute_2",
        OPTIONAL,
    ),
    staged("oboe_1", "Oboe 1", "Ob. 1", Oboe, REQUIRED),
    staged("oboe_2", "Oboe 2", "Ob. 2", Oboe, REQUIRED),
    staged(
        "english_horn",
        "English horn",
        "Eng. horn",
        EnglishHorn,
        OPTIONAL,
    ),
    staged("clarinet_1", "Clarinet 1", "Cl. 1", ClarinetBb, REQUIRED),
    staged("clarinet_2", "Clarinet 2", "Cl. 2", ClarinetBb, REQUIRED),
    staged(
        "bass_clarinet",
        "Bass clarinet",
        "Cl. b.",
        BassClarinet,
        OPTIONAL,
    ),
    staged("bassoon_1", "Bassoon 1", "Fag. 1", Bassoon, REQUIRED),
    staged("bassoon_2", "Bassoon 2", "Fag. 2", Bassoon, REQUIRED),
    staged(
        "contrabassoon",
        "Contrabassoon",
        "C-fag.",
        Contrabassoon,
        OPTIONAL,
    ),
    staged("horn_1", "Horn 1", "Hn. 1", HornF, REQUIRED),
    staged("horn_2", "Horn 2", "Hn. 2", HornF, REQUIRED),
    staged("horn_3", "Horn 3", "Hn. 3", HornF, REQUIRED),
    staged("horn_4", "Horn 4", "Hn. 4", HornF, REQUIRED),
    staged("horn_5", "Horn 5", "Hn. 5", HornF, REQUIRED),
    staged("horn_6", "Horn 6", "Hn. 6", HornF, REQUIRED),
    staged("horn_7", "Horn 7", "Hn. 7", HornF, OPTIONAL),
    staged("horn_8", "Horn 8", "Hn. 8", HornF, OPTIONAL),
    staged("trumpet_1", "Trumpet 1", "Tpt. 1", TrumpetBb, REQUIRED),
    staged("trumpet_2", "Trumpet 2", "Tpt. 2", TrumpetBb, REQUIRED),
    staged("trumpet_3", "Trumpet 3", "Tpt. 3", TrumpetBb, REQUIRED),
    staged(
        "trombone_1",
        "Trombone 1",
        "Tbn. 1",
        TromboneTenor,
        REQUIRED,
    ),
    staged(
        "trombone_2",
        "Trombone 2",
        "Tbn. 2",
        TromboneTenor,
        REQUIRED,
    ),
    staged(
        "bass_trombone_1",
        "Bass trombone 1",
        "B. Tbn. 1",
        TromboneBass,
        REQUIRED,
    ),
    staged(
        "bass_trombone_2",
        "Bass trombone 2",
        "B. Tbn. 2",
        TromboneBass,
        OPTIONAL,
    ),
    staged("tuba", "Tuba", "Tba.", Tuba, REQUIRED),
    doubled(
        "cimbasso",
        "Cimbasso (Tba.)",
        "Cimb.",
        Cimbasso,
        "tuba",
        OPTIONAL,
    ),
    noted(
        staged("timpani", "Timpani", "Timp.", Timpani, REQUIRED),
        "one timpanist on four or five drums",
    ),
    staged(
        "bass_drum",
        "Bass drum (Perc. 1)",
        "B.D.",
        BassDrum,
        REQUIRED,
    ),
    staged(
        "snare_drum",
        "Snare drum (Perc. 2)",
        "S.D.",
        SnareDrum,
        REQUIRED,
    ),
    staged(
        "cymbals",
        "Cymbals (Perc. 3)",
        "Cym.",
        ClashCymbals,
        REQUIRED,
    ),
    doubled(
        "suspended_cymbal",
        "Suspended cymbal (Perc. 3)",
        "Sus. cym.",
        SuspendedCymbal,
        "cymbals",
        REQUIRED,
    ),
    doubled(
        "tam_tam",
        "Tam-tam (Perc. 1)",
        "T.-t.",
        TamTam,
        "bass_drum",
        REQUIRED,
    ),
    doubled(
        "glockenspiel",
        "Glockenspiel (Perc. 2)",
        "Glock.",
        Glockenspiel,
        "snare_drum",
        OPTIONAL,
    ),
    doubled(
        "xylophone",
        "Xylophone (Perc. 3)",
        "Xyl.",
        Xylophone,
        "cymbals",
        OPTIONAL,
    ),
    doubled(
        "tubular_bells",
        "Tubular bells (Perc. 1)",
        "Tub. B.",
        TubularBells,
        "bass_drum",
        OPTIONAL,
    ),
    noted(
        doubled(
            "crotales",
            "Crotales (Perc. 2)",
            "Crot.",
            Crotales,
            "snare_drum",
            OPTIONAL,
        ),
        "taken by the glockenspiel player",
    ),
    staged("harp", "Harp", "Hp.", Harp, REQUIRED),
    doubled(
        "celesta",
        "Celesta (Pno.)",
        "Cel.",
        Celesta,
        "piano",
        OPTIONAL,
    ),
    staged("piano", "Piano", "Pno.", PianoConcertGrand, OPTIONAL),
    chorus(
        "choir_sopranos",
        "Sopranos",
        "S.",
        ChoirSoprano,
        16,
        20,
        OPTIONAL,
    ),
    chorus("choir_altos", "Altos", "A.", ChoirAlto, 16, 20, OPTIONAL),
    chorus("choir_tenors", "Tenors", "T.", ChoirTenor, 16, 20, OPTIONAL),
    chorus(
        "choir_basses",
        "Basses (choir)",
        "B.",
        ChoirBass,
        16,
        20,
        OPTIONAL,
    ),
    VIOLINS_I,
    VIOLINS_II,
    VIOLAS,
    CELLOS,
    BASSES,
];

const FILM: Ensemble = Ensemble::new(
    EnsembleId::FilmOrchestra,
    EnsembleSpec {
        name: "Film orchestra",
        description: "A modern scoring orchestra: woodwinds in pairs or threes, six to eight \
            horns, three trumpets, tenor and bass trombones, tuba and cimbasso, timpani, \
            percussion, harp, keyboards, an optional choir and full strings.",
        order: PartOrder::FullScore,
        seatings: AMERICAN_FIRST,
        string_sizes: &[FULL, MEDIUM],
        parts: FILM_PARTS,
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    spitfire_symphonic_strings: "a symphonic string section of 60 players, 16, \
                        14, 12, 10 and 8, recorded in their places",
                    rimsky_korsakov: "the third flute player takes the piccolo; a medium chorus \
                        has 16 to 20 singers to a part",
                ),
            },
            Provenance::Estimate {
                reason: "no source that was read gives a film orchestra's roster: the parts \
                    follow recording-session practice and the sections sample libraries record, \
                    with six to eight horns, three trumpets, tenor and bass trombones, tuba and \
                    one timpanist; the tubist doubles the cimbasso, the percussionists the \
                    mallets and the pianist the celesta; the synthesizer layer of hybrid scores \
                    has no instrument type and is left out",
            },
            STAGE_PANS,
        ],
    },
);

// The chamber orchestra.

const CHAMBER_ORCHESTRA_PARTS: &[Part] = &[
    staged("flute_1", "Flute 1", "Fl. 1", Flute, REQUIRED),
    staged("flute_2", "Flute 2", "Fl. 2", Flute, REQUIRED),
    staged("oboe_1", "Oboe 1", "Ob. 1", Oboe, REQUIRED),
    staged("oboe_2", "Oboe 2", "Ob. 2", Oboe, REQUIRED),
    staged("clarinet_1", "Clarinet 1", "Cl. 1", ClarinetBb, OPTIONAL),
    staged("clarinet_2", "Clarinet 2", "Cl. 2", ClarinetBb, OPTIONAL),
    staged("bassoon_1", "Bassoon 1", "Fag. 1", Bassoon, REQUIRED),
    staged("bassoon_2", "Bassoon 2", "Fag. 2", Bassoon, REQUIRED),
    staged("horn_1", "Horn 1", "Hn. 1", HornF, REQUIRED),
    staged("horn_2", "Horn 2", "Hn. 2", HornF, REQUIRED),
    staged("trumpet_1", "Trumpet 1", "Tpt. 1", TrumpetC, OPTIONAL),
    staged("trumpet_2", "Trumpet 2", "Tpt. 2", TrumpetC, OPTIONAL),
    staged("timpani", "Timpani", "Timp.", Timpani, OPTIONAL),
    noted(
        staged(
            "harpsichord",
            "Harpsichord (continuo)",
            "Hpschd.",
            Harpsichord,
            OPTIONAL,
        ),
        "for Baroque repertoire, one per continuo group",
    ),
    VIOLINS_I,
    VIOLINS_II,
    VIOLAS,
    CELLOS,
    BASSES,
];

const CHAMBER_ORCHESTRA: Ensemble = Ensemble::new(
    EnsembleId::ChamberOrchestra,
    EnsembleSpec {
        name: "Chamber orchestra",
        description: "About 15 to 30 players, modelled on the Classical orchestra: woodwinds, \
            horns and trumpets in pairs, timpani, an optional harpsichord continuo and a small \
            string body.",
        order: PartOrder::FullScore,
        seatings: AMERICAN_FIRST,
        string_sizes: &[CHAMBER, SMALL_CHAMBER],
        parts: CHAMBER_ORCHESTRA_PARTS,
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_musical_ensemble: "a chamber orchestra has about 15 to 30 members",
                    wikipedia_orchestra: "a chamber orchestra has at most around fifty players",
                ),
            },
            Provenance::Estimate {
                reason: "the roster is modelled on the Classical orchestra with the strings of a \
                    21-player string orchestra; the harpsichord is the continuo of Baroque \
                    repertoire; six first violins lie below Rimsky-Korsakov's floor of eight, so \
                    the strings may need balancing against the winds",
            },
            STAGE_PANS,
        ],
    },
);

// The string orchestra.

const STRING_ORCHESTRA_PARTS: &[Part] = &[
    VIOLINS_I,
    VIOLINS_II,
    VIOLAS,
    CELLOS,
    noted(
        Part {
            required: OPTIONAL,
            ..BASSES
        },
        "usually, though not always, present",
    ),
];

const STRING_ORCHESTRA: Ensemble = Ensemble::new(
    EnsembleId::StringOrchestra,
    EnsembleSpec {
        name: "String orchestra",
        description: "First and second violins, violas, cellos and usually double basses.",
        order: PartOrder::FullScore,
        seatings: AMERICAN_FIRST,
        string_sizes: &[CHAMBER, SMALL_CHAMBER, FULL],
        parts: STRING_ORCHESTRA_PARTS,
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_string_orchestra: "first and second violins, violas, cellos and \
                        usually, though not always, double basses",
                ),
            },
            Provenance::Estimate {
                reason: "the 21-player body is the default, the largest of the chamber sizes the \
                    source gives",
            },
            STAGE_PANS,
        ],
    },
);

// The big band.

const BIG_BAND_PARTS: &[Part] = &[
    noted(
        band(
            "alto_sax_1",
            "Alto Sax 1",
            "A. Sax. 1",
            SaxophoneAlto,
            ONE,
            REQUIRED,
            0.0,
        ),
        "the lead",
    ),
    band(
        "soprano_sax",
        "Soprano Sax (A. Sax. 1)",
        "S. Sax.",
        SaxophoneSoprano,
        Players::Doubling { by: "alto_sax_1" },
        OPTIONAL,
        0.0,
    ),
    band(
        "alto_sax_2",
        "Alto Sax 2",
        "A. Sax. 2",
        SaxophoneAlto,
        ONE,
        REQUIRED,
        -0.25,
    ),
    band(
        "flute",
        "Flute (A. Sax. 2)",
        "Fl.",
        Flute,
        Players::Doubling { by: "alto_sax_2" },
        OPTIONAL,
        -0.25,
    ),
    band(
        "piccolo",
        "Piccolo (A. Sax. 2)",
        "Picc.",
        Piccolo,
        Players::Doubling { by: "alto_sax_2" },
        OPTIONAL,
        -0.25,
    ),
    noted(
        band(
            "tenor_sax_1",
            "Tenor Sax 1",
            "T. Sax. 1",
            SaxophoneTenor,
            ONE,
            REQUIRED,
            -0.50,
        ),
        "the solo chair",
    ),
    band(
        "clarinet",
        "Clarinet (T. Sax. 1)",
        "Cl.",
        ClarinetBb,
        Players::Doubling { by: "tenor_sax_1" },
        OPTIONAL,
        -0.50,
    ),
    band(
        "tenor_sax_2",
        "Tenor Sax 2",
        "T. Sax. 2",
        SaxophoneTenor,
        ONE,
        REQUIRED,
        0.25,
    ),
    band(
        "baritone_sax",
        "Baritone Sax",
        "Bar. Sax.",
        SaxophoneBaritone,
        ONE,
        REQUIRED,
        0.50,
    ),
    band(
        "bass_clarinet",
        "Bass clarinet (Bar. Sax.)",
        "Cl. b.",
        BassClarinet,
        Players::Doubling { by: "baritone_sax" },
        OPTIONAL,
        0.50,
    ),
    noted(
        band(
            "trumpet_1",
            "Trumpet 1",
            "Tpt. 1",
            TrumpetBb,
            ONE,
            REQUIRED,
            -0.10,
        ),
        "the lead",
    ),
    noted(
        band(
            "trumpet_2",
            "Trumpet 2",
            "Tpt. 2",
            TrumpetBb,
            ONE,
            REQUIRED,
            -0.30,
        ),
        "often the solo part",
    ),
    band(
        "flugelhorn",
        "Flugelhorn (Tpt. 2)",
        "Flghn.",
        Flugelhorn,
        Players::Doubling { by: "trumpet_2" },
        OPTIONAL,
        -0.30,
    ),
    band(
        "trumpet_3",
        "Trumpet 3",
        "Tpt. 3",
        TrumpetBb,
        ONE,
        REQUIRED,
        0.10,
    ),
    noted(
        band(
            "trumpet_4",
            "Trumpet 4",
            "Tpt. 4",
            TrumpetBb,
            ONE,
            REQUIRED,
            0.30,
        ),
        "often the solo part",
    ),
    band(
        "cornet",
        "Cornet (Tpt. 4)",
        "Cnt.",
        Cornet,
        Players::Doubling { by: "trumpet_4" },
        OPTIONAL,
        0.30,
    ),
    noted(
        band(
            "trombone_1",
            "Trombone 1",
            "Tbn. 1",
            TromboneTenor,
            ONE,
            REQUIRED,
            -0.10,
        ),
        "the lead",
    ),
    band(
        "trombone_2",
        "Trombone 2",
        "Tbn. 2",
        TromboneTenor,
        ONE,
        REQUIRED,
        -0.30,
    ),
    band(
        "trombone_3",
        "Trombone 3",
        "Tbn. 3",
        TromboneTenor,
        ONE,
        REQUIRED,
        0.10,
    ),
    band(
        "bass_trombone",
        "Bass Trombone",
        "B. Tbn.",
        TromboneBass,
        ONE,
        REQUIRED,
        0.30,
    ),
    band(
        "piano",
        "Piano",
        "Pno.",
        PianoConcertGrand,
        ONE,
        REQUIRED,
        -0.45,
    ),
    band("bass", "Bass", "Cb.", DoubleBass, ONE, REQUIRED, 0.0),
    band(
        "vibraphone",
        "Vibraphone",
        "Vib.",
        Vibraphone,
        ONE,
        OPTIONAL,
        0.20,
    ),
];

const BIG_BAND: Ensemble = Ensemble::new(
    EnsembleId::BigBand,
    EnsembleSpec {
        name: "Big band",
        description: "Five saxophones, four trumpets, four trombones and a rhythm section, with \
            the reeds and brass the saxophonists and trumpeters double.",
        order: PartOrder::Edition,
        seatings: &[],
        string_sizes: &[],
        parts: BIG_BAND_PARTS,
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_big_band: "saxophones, trumpets, trombones and a rhythm section of \
                        guitar, piano, double bass and drums, sometimes with vibraphone; the \
                        saxophonists often double flute, piccolo, clarinet, bass clarinet and \
                        soprano saxophone, the trumpeters flugelhorn and cornet; the fourth \
                        trombone is usually a bass trombone; the second or fourth trumpet takes \
                        the solos; the lead players sit in the middle of their sections and the \
                        solo chairs nearest the rhythm section",
                    taming_the_saxophone_score_layout: "a big-band score lists the saxophones, \
                        trumpets and trombones, then the rhythm section",
                ),
            },
            Provenance::Estimate {
                reason: "two altos, two tenors and a baritone with the first alto leading, and \
                    four trumpets, as big-band arrangements usually set them; no instrument type \
                    covers the guitar or the drum kit of the rhythm section, so they are left out \
                    and the template lists 15 of the 17 players; which saxophonist and which \
                    trumpeter doubles which instrument is a choice",
            },
            Provenance::Estimate {
                reason: "the leads sit in the middle of their sections and the solo chairs on \
                    the side of the rhythm section, assumed on the audience's left; the piano and \
                    the bass follow mix practice rather than a stage place, and the vibraphone \
                    sits where an orchestra's mallets do; a doubled instrument sits where its \
                    player does",
            },
        ],
    },
);

// Chorus and orchestra.

const CHOIR_AND_ORCHESTRA_PARTS: &[Part] = &[
    staged("piccolo", "Piccolo", "Picc.", Piccolo, REQUIRED),
    staged("flute_1", "Flute 1", "Fl. 1", Flute, REQUIRED),
    staged("flute_2", "Flute 2", "Fl. 2", Flute, REQUIRED),
    staged("oboe_1", "Oboe 1", "Ob. 1", Oboe, REQUIRED),
    staged("oboe_2", "Oboe 2", "Ob. 2", Oboe, REQUIRED),
    staged("clarinet_1", "Clarinet 1", "Cl. 1", ClarinetBb, REQUIRED),
    staged("clarinet_2", "Clarinet 2", "Cl. 2", ClarinetBb, REQUIRED),
    staged("bassoon_1", "Bassoon 1", "Fag. 1", Bassoon, REQUIRED),
    staged("bassoon_2", "Bassoon 2", "Fag. 2", Bassoon, REQUIRED),
    noted(
        staged(
            "contrabassoon",
            "Contrabassoon",
            "C-fag.",
            Contrabassoon,
            OPTIONAL,
        ),
        "ad libitum",
    ),
    staged("horn_1", "Horn 1", "Hn. 1", HornF, REQUIRED),
    staged("horn_2", "Horn 2", "Hn. 2", HornF, REQUIRED),
    staged("horn_3", "Horn 3", "Hn. 3", HornF, REQUIRED),
    staged("horn_4", "Horn 4", "Hn. 4", HornF, REQUIRED),
    staged("trumpet_1", "Trumpet 1", "Tpt. 1", TrumpetBb, REQUIRED),
    staged("trumpet_2", "Trumpet 2", "Tpt. 2", TrumpetBb, REQUIRED),
    staged(
        "trombone_1",
        "Trombone 1",
        "Tbn. 1",
        TromboneTenor,
        REQUIRED,
    ),
    staged(
        "trombone_2",
        "Trombone 2",
        "Tbn. 2",
        TromboneTenor,
        REQUIRED,
    ),
    staged(
        "trombone_3",
        "Trombone 3",
        "B. Tbn.",
        TromboneBass,
        REQUIRED,
    ),
    staged("tuba", "Tuba", "Tba.", Tuba, REQUIRED),
    staged("timpani", "Timpani", "Timp.", Timpani, REQUIRED),
    staged("harp_1", "Harp 1", "Hp. 1", Harp, REQUIRED),
    noted(
        staged("harp_2", "Harp 2", "Hp. 2", Harp, OPTIONAL),
        "the harp part, preferably doubled",
    ),
    noted(
        staged("organ", "Organ", "Org.", OrganPipe, OPTIONAL),
        "ad libitum",
    ),
    staged(
        "soprano_solo",
        "Soprano solo",
        "Sopr.",
        VoiceSoprano,
        REQUIRED,
    ),
    staged(
        "baritone_solo",
        "Baritone solo",
        "Bar.",
        VoiceBaritone,
        REQUIRED,
    ),
    chorus(
        "choir_sopranos",
        "Sopranos",
        "S.",
        ChoirSoprano,
        16,
        20,
        REQUIRED,
    ),
    chorus("choir_altos", "Altos", "A.", ChoirAlto, 16, 20, REQUIRED),
    chorus("choir_tenors", "Tenors", "T.", ChoirTenor, 16, 20, REQUIRED),
    chorus(
        "choir_basses",
        "Basses (choir)",
        "B.",
        ChoirBass,
        16,
        20,
        REQUIRED,
    ),
    noted(VIOLINS_I, "tacet in the first movement"),
    noted(VIOLINS_II, "tacet in the first movement"),
    VIOLAS,
    CELLOS,
    BASSES,
];

const CHOIR_AND_ORCHESTRA: Ensemble = Ensemble::new(
    EnsembleId::ChoirAndOrchestra,
    EnsembleSpec {
        name: "Choir and orchestra",
        description: "Brahms's Ein deutsches Requiem: soprano and baritone soloists, a four-part \
            chorus and a Romantic orchestra with piccolo, harp and organ.",
        order: PartOrder::FullScore,
        seatings: AMERICAN_FIRST,
        string_sizes: &[FULL, MEDIUM, SMALL],
        parts: CHOIR_AND_ORCHESTRA_PARTS,
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_ein_deutsches_requiem: "soprano and baritone soloists; a mixed \
                        chorus mostly in four parts; piccolo, two flutes, two oboes, two \
                        clarinets, two bassoons, contrabassoon ad libitum, four horns, two \
                        trumpets, three trombones, tuba, timpani, a harp part preferably \
                        doubled, organ ad libitum and strings, without violins in the first \
                        movement",
                    rimsky_korsakov: "a full chorus has 32 singers to a part, a medium one 16 to \
                        20 and a small one 8 to 10",
                    berlioz: "the soloists stand at the centre front",
                ),
            },
            Provenance::Estimate {
                reason: "the score states neither the chorus nor the string sections, so the \
                    template takes a medium chorus and the full string body; the third trombone \
                    is a bass trombone",
            },
            STAGE_PANS,
        ],
    },
);

// The small chamber groups.

const STRING_QUARTET: Ensemble = Ensemble::new(
    EnsembleId::StringQuartet,
    EnsembleSpec {
        name: "String quartet",
        description: "Two violins, a viola and a cello, one player to a part.",
        order: PartOrder::FullScore,
        seatings: &[],
        string_sizes: &[],
        parts: &[
            chamber("violin_1", "Violin 1", "Vn. 1", Violin),
            chamber("violin_2", "Violin 2", "Vn. 2", Violin),
            chamber("viola", "Viola", "Va.", Viola),
            chamber("cello", "Cello", "Vc.", Cello),
        ],
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_musical_ensemble: "a string quartet is two violins, a viola and a \
                        cello, one player to each part",
                ),
            },
            CHAMBER_PANS,
        ],
    },
);

const WIND_QUINTET: Ensemble = Ensemble::new(
    EnsembleId::WindQuintet,
    EnsembleSpec {
        name: "Wind quintet",
        description: "Flute, oboe, clarinet, horn and bassoon, one player to a part.",
        order: PartOrder::Edition,
        seatings: &[],
        string_sizes: &[],
        parts: &[
            chamber("flute", "Flute", "Fl.", Flute),
            chamber("oboe", "Oboe", "Ob.", Oboe),
            chamber("clarinet", "Clarinet", "Cl.", ClarinetBb),
            chamber("horn", "Horn", "Hn.", HornF),
            chamber("bassoon", "Bassoon", "Fag.", Bassoon),
        ],
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_musical_ensemble: "a wind quintet is a flute, an oboe, a clarinet, \
                        a horn and a bassoon",
                ),
            },
            Provenance::Estimate {
                reason: "editions commonly print the horn between the clarinet and the bassoon",
            },
            CHAMBER_PANS,
        ],
    },
);

const BRASS_QUINTET: Ensemble = Ensemble::new(
    EnsembleId::BrassQuintet,
    EnsembleSpec {
        name: "Brass quintet",
        description: "Two trumpets, horn, trombone and tuba, one player to a part.",
        order: PartOrder::Edition,
        seatings: &[],
        string_sizes: &[],
        parts: &[
            or(
                chamber("trumpet_1", "Trumpet 1", "Tpt. 1", TrumpetBb),
                &[TrumpetC],
            ),
            or(
                chamber("trumpet_2", "Trumpet 2", "Tpt. 2", TrumpetBb),
                &[TrumpetC],
            ),
            chamber("horn", "Horn", "Hn.", HornF),
            chamber("trombone", "Trombone", "Tbn.", TromboneTenor),
            chamber("tuba", "Tuba", "Tba.", Tuba),
        ],
        provenance: &[
            Provenance::Cited {
                source: cite!(
                    wikipedia_musical_ensemble: "a brass quintet is two trumpets, a horn, a \
                        trombone and a tuba",
                ),
            },
            Provenance::Estimate {
                reason: "editions commonly print the parts from high to low, the horn below the \
                    trumpets; the trumpets play in B♭ or in C",
            },
            CHAMBER_PANS,
        ],
    },
);

const PIANO_TRIO: Ensemble = Ensemble::new(
    EnsembleId::PianoTrio,
    EnsembleSpec {
        name: "Piano trio",
        description: "Violin, cello and piano, one player to a part.",
        order: PartOrder::Edition,
        seatings: &[],
        string_sizes: &[],
        parts: &[
            chamber("violin", "Violin", "Vn.", Violin),
            chamber("cello", "Cello", "Vc.", Cello),
            chamber("piano", "Piano", "Pno.", PianoConcertGrand),
        ],
        provenance: &[
            Provenance::Cited {
                source: cite!(wikipedia_piano_trio: "a violin, a cello and a piano"),
            },
            Provenance::Estimate {
                reason: "editions commonly print the piano at the bottom",
            },
            CHAMBER_PANS,
        ],
    },
);

/// Every ensemble template, one per id, in the order of [`EnsembleId::ALL`]: the record of an id
/// is at the id's place.
pub static ENSEMBLES: [Ensemble; EnsembleId::ALL.len()] = [
    CLASSICAL,
    BEETHOVEN,
    ROMANTIC,
    LATE_ROMANTIC,
    FILM,
    CHAMBER_ORCHESTRA,
    STRING_ORCHESTRA,
    BIG_BAND,
    CHOIR_AND_ORCHESTRA,
    STRING_QUARTET,
    WIND_QUINTET,
    BRASS_QUINTET,
    PIANO_TRIO,
];

// Compile-time check: the record at each place of `ENSEMBLES` is that of the id at the same place
// of `EnsembleId::ALL`, so a lookup by place cannot return another template.
const _: () = {
    let mut i = 0;
    while i < ENSEMBLES.len() {
        assert!(
            ENSEMBLES[i].id() as usize == i,
            "ENSEMBLES holds each id's template at the id's place"
        );
        i += 1;
    }
};

/// The template of `id`.
pub(in crate::orchestration) fn ensemble(id: EnsembleId) -> &'static Ensemble {
    // The check above places each template at its id's index, so the index is in bounds.
    &ENSEMBLES[id as usize]
}
