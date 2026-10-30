//! The fallback tables: the neighbours of every instrument type and the substitutes of the
//! fifteen original articulations.
//!
//! Each neighbour and each row says why it was chosen. Most are musical judgements, estimates
//! from general orchestration practice or from the timbre of the instruments; a few follow a
//! handbook that names the substitute, and cite it.

use super::sources::{cite, reason};
use crate::orchestration::articulation::Articulation::{
    ColLegno, ConSordino, FlutterTongue, Harmonics, Legato, Marcato, Pizzicato, Roll, Spiccato,
    Staccatissimo, Staccato, SulPonticello, Sustain, Tenuto, Tremolo, Trill,
};
use crate::orchestration::articulation::{Articulation, Vibrato};
use crate::orchestration::fallback::{
    ArticulationSubstitute, Emulation, Manner, Neighbour, NeighbourList, Scope, StandIn,
    Substitution,
};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::id::InstrumentType::{
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
use crate::orchestration::instrument::Provenance;

const fn estimate(reason: &'static str) -> Provenance {
    Provenance::Estimate { reason }
}

/// `instrument` playing the part as written, for any passage, without asking first.
const fn plays(instrument: InstrumentType, provenance: Provenance) -> Neighbour {
    Neighbour {
        instrument,
        manner: Manner::AsWritten,
        only: None,
        asks_first: false,
        provenance,
    }
}

/// `neighbour`, confirmed by the user first.
const fn asking(neighbour: Neighbour) -> Neighbour {
    Neighbour {
        asks_first: true,
        ..neighbour
    }
}

/// `neighbour`, playing in `manner`.
const fn playing(manner: Manner, neighbour: Neighbour) -> Neighbour {
    Neighbour {
        manner,
        ..neighbour
    }
}

/// `neighbour`, for passages of `scope` only.
const fn only(scope: Scope, neighbour: Neighbour) -> Neighbour {
    Neighbour {
        only: Some(scope),
        ..neighbour
    }
}

const fn list(instrument: InstrumentType, neighbours: &'static [Neighbour]) -> NeighbourList {
    NeighbourList {
        instrument,
        neighbours,
    }
}

// Why a neighbour was chosen, where several share the reason.

const ANOTHER_SIZE: Provenance =
    estimate("the same instrument in another size: the nearest colour, in another register");
const ANOTHER_KEY: Provenance = estimate(
    "the same instrument in another key; parts are stored at sounding pitch, so only the range \
     differs",
);
const ADJACENT_STRING: Provenance = estimate(
    "general orchestration practice: the bowed string of the adjacent register, the nearest in \
     colour",
);
const REED_OF_SIMILAR_REGISTER: Provenance = estimate(
    "general orchestration practice: a reed instrument of a similar register, when no instrument \
     of the same kind is installed",
);
const BRASS_OF_OVERLAPPING_REGISTER: Provenance =
    estimate("general orchestration practice: brass of an overlapping register");
const HORN_PLAYERS_INSTRUMENT: Provenance = estimate(
    "horn players play the Wagner tubas with a horn mouthpiece, so the horn comes nearest in \
     colour",
);
const VALVED_TREBLE_BRASS: Provenance = estimate(
    "general orchestration practice: a valved brass instrument in Bb of the same register as the \
     trumpet, its tone rounder or brighter",
);
const MALLETS_OF_ADJACENT_REGISTER: Provenance =
    estimate("general orchestration practice: a mallet instrument of the adjacent register");
const METALLIC_RING: Provenance =
    estimate("a timbre judgement: tuned percussion with a metallic, bell-like ring");
const HIGH_METALLIC_RING: Provenance = estimate(
    "a timbre judgement: a high metallic ring; the unpitched stroke is played on the top note, \
     nearest the triangle's high shimmer",
);
const JINGLES: Provenance = estimate("a timbre judgement: both are jingles, shaken or struck");
const RATTLES: Provenance = estimate("a timbre judgement: both are rattles");
const WOODEN_BLOCKS: Provenance =
    estimate("a timbre judgement: hollow wooden blocks struck with a stick");
const PLUCKED_OF_SIMILAR_REGISTER: Provenance =
    estimate("general orchestration practice: a plucked string instrument of a similar register");
const OTHER_ELECTRIC_PIANO: Provenance = estimate("the other electric piano of the dataset");
const SOLO_BY_SECTION: Provenance = estimate(
    "the choir section of the same voice type sings the solo line; a section in place of a \
     soloist changes the part's role, so the user confirms it",
);

const FELT_TONE: Provenance =
    estimate("an ordinary piano played softer comes nearest the muffled felt tone");
const MEZZO_IN_A_CHORUS: Provenance = Provenance::Cited {
    source: cite!(
        rimsky_korsakov: "in a chorus mezzo-sopranos are classed with the second sopranos or the \
            first altos";
        "a section in place of a soloist changes the part's role, so the user confirms it",
    ),
};
const BARITONE_IN_A_CHORUS: Provenance = Provenance::Cited {
    source: cite!(
        rimsky_korsakov: "in a chorus baritones join the second tenors or the first basses";
        "a section in place of a soloist changes the part's role, so the user confirms it",
    ),
};

/// The neighbours of every instrument type, best first, at the type's place in score order (the
/// order of [`InstrumentType::ALL`]).
pub static NEIGHBOURS: [NeighbourList; InstrumentType::ALL.len()] = [
    // Woodwinds.
    list(Piccolo, &[plays(Flute, ANOTHER_SIZE)]),
    list(
        Flute,
        &[plays(AltoFlute, ANOTHER_SIZE), plays(Piccolo, ANOTHER_SIZE)],
    ),
    list(AltoFlute, &[plays(Flute, ANOTHER_SIZE)]),
    list(
        BassFlute,
        &[plays(AltoFlute, ANOTHER_SIZE), plays(Flute, ANOTHER_SIZE)],
    ),
    list(Oboe, &[plays(EnglishHorn, ANOTHER_SIZE)]),
    list(
        OboeDamore,
        &[plays(Oboe, ANOTHER_SIZE), plays(EnglishHorn, ANOTHER_SIZE)],
    ),
    list(
        EnglishHorn,
        &[
            plays(Oboe, ANOTHER_SIZE),
            plays(Bassoon, REED_OF_SIMILAR_REGISTER),
        ],
    ),
    list(ClarinetEb, &[plays(ClarinetBb, ANOTHER_SIZE)]),
    list(
        ClarinetBb,
        &[
            plays(ClarinetA, ANOTHER_KEY),
            plays(Oboe, REED_OF_SIMILAR_REGISTER),
        ],
    ),
    list(ClarinetA, &[plays(ClarinetBb, ANOTHER_KEY)]),
    list(
        BassClarinet,
        &[
            plays(Bassoon, REED_OF_SIMILAR_REGISTER),
            plays(ClarinetBb, ANOTHER_SIZE),
        ],
    ),
    list(
        SaxophoneSoprano,
        &[
            plays(SaxophoneAlto, ANOTHER_SIZE),
            plays(Oboe, REED_OF_SIMILAR_REGISTER),
        ],
    ),
    list(SaxophoneAlto, &[plays(SaxophoneTenor, ANOTHER_SIZE)]),
    list(
        SaxophoneTenor,
        &[
            plays(SaxophoneAlto, ANOTHER_SIZE),
            plays(SaxophoneBaritone, ANOTHER_SIZE),
        ],
    ),
    list(
        SaxophoneBaritone,
        &[
            plays(SaxophoneTenor, ANOTHER_SIZE),
            plays(BassClarinet, REED_OF_SIMILAR_REGISTER),
        ],
    ),
    list(
        Bassoon,
        &[
            plays(Contrabassoon, ANOTHER_SIZE),
            plays(BassClarinet, REED_OF_SIMILAR_REGISTER),
        ],
    ),
    list(Contrabassoon, &[plays(Bassoon, ANOTHER_SIZE)]),
    // Brass.
    list(
        HornF,
        &[
            plays(TromboneTenor, BRASS_OF_OVERLAPPING_REGISTER),
            plays(Euphonium, BRASS_OF_OVERLAPPING_REGISTER),
        ],
    ),
    list(
        WagnerTubaTenor,
        &[
            plays(HornF, HORN_PLAYERS_INSTRUMENT),
            plays(Euphonium, BRASS_OF_OVERLAPPING_REGISTER),
            plays(TromboneTenor, BRASS_OF_OVERLAPPING_REGISTER),
        ],
    ),
    list(
        WagnerTubaBass,
        &[
            plays(HornF, HORN_PLAYERS_INSTRUMENT),
            plays(Tuba, BRASS_OF_OVERLAPPING_REGISTER),
        ],
    ),
    list(TrumpetPiccolo, &[plays(TrumpetBb, ANOTHER_SIZE)]),
    list(TrumpetC, &[plays(TrumpetBb, ANOTHER_KEY)]),
    list(
        TrumpetBb,
        &[
            plays(Cornet, VALVED_TREBLE_BRASS),
            plays(Flugelhorn, VALVED_TREBLE_BRASS),
        ],
    ),
    list(Cornet, &[plays(TrumpetBb, VALVED_TREBLE_BRASS)]),
    list(
        Flugelhorn,
        &[
            playing(
                Manner::Softer,
                plays(
                    TrumpetBb,
                    estimate(
                        "the trumpet of the same pitch, played softer to come nearer the \
                         flugelhorn's darker, softer tone",
                    ),
                ),
            ),
            plays(HornF, BRASS_OF_OVERLAPPING_REGISTER),
        ],
    ),
    list(
        TromboneTenor,
        &[
            plays(TromboneBass, BRASS_OF_OVERLAPPING_REGISTER),
            plays(Euphonium, BRASS_OF_OVERLAPPING_REGISTER),
            plays(HornF, BRASS_OF_OVERLAPPING_REGISTER),
        ],
    ),
    list(
        TromboneBass,
        &[
            plays(TromboneTenor, BRASS_OF_OVERLAPPING_REGISTER),
            plays(Tuba, BRASS_OF_OVERLAPPING_REGISTER),
        ],
    ),
    list(
        Euphonium,
        &[
            plays(TromboneTenor, BRASS_OF_OVERLAPPING_REGISTER),
            plays(Tuba, BRASS_OF_OVERLAPPING_REGISTER),
            plays(HornF, BRASS_OF_OVERLAPPING_REGISTER),
        ],
    ),
    list(
        Tuba,
        &[
            plays(TromboneBass, BRASS_OF_OVERLAPPING_REGISTER),
            plays(Cimbasso, BRASS_OF_OVERLAPPING_REGISTER),
        ],
    ),
    list(
        Cimbasso,
        &[
            plays(Tuba, BRASS_OF_OVERLAPPING_REGISTER),
            plays(TromboneBass, BRASS_OF_OVERLAPPING_REGISTER),
        ],
    ),
    // Percussion.
    list(
        Timpani,
        &[asking(only(
            Scope::Accents,
            plays(
                BassDrum,
                estimate(
                    "general orchestration practice: the bass drum gives the weight of a \
                     timpani accent but not its pitch",
                ),
            ),
        ))],
    ),
    list(
        BassDrum,
        &[only(
            Scope::Accents,
            playing(
                Manner::OnLowestNote,
                plays(
                    Timpani,
                    estimate(
                        "general orchestration practice: the other low drum of the orchestra, \
                         whose lowest note comes nearest a bass drum accent",
                    ),
                ),
            ),
        )],
    ),
    list(SnareDrum, &[]),
    list(
        ClashCymbals,
        &[plays(
            SuspendedCymbal,
            estimate("a timbre judgement: the other cymbal, struck"),
        )],
    ),
    list(
        SuspendedCymbal,
        &[only(
            Scope::SingleStrokes,
            plays(
                ClashCymbals,
                estimate("a timbre judgement: the other cymbal, which plays single strokes only"),
            ),
        )],
    ),
    list(
        TamTam,
        &[playing(
            Manner::With(Roll),
            plays(
                SuspendedCymbal,
                estimate(
                    "a timbre judgement: a suspended cymbal roll is the nearest sustained \
                          metal wash",
                ),
            ),
        )],
    ),
    list(
        Triangle,
        &[
            playing(Manner::OnHighestNote, plays(Crotales, HIGH_METALLIC_RING)),
            playing(
                Manner::OnHighestNote,
                plays(Glockenspiel, HIGH_METALLIC_RING),
            ),
        ],
    ),
    list(Tambourine, &[plays(SleighBells, JINGLES)]),
    list(
        Castanets,
        &[plays(
            WoodBlock,
            estimate("a timbre judgement: a dry wooden click"),
        )],
    ),
    list(WoodBlock, &[plays(TempleBlocks, WOODEN_BLOCKS)]),
    list(TempleBlocks, &[plays(WoodBlock, WOODEN_BLOCKS)]),
    list(SleighBells, &[plays(Tambourine, JINGLES)]),
    list(Maracas, &[plays(Shaker, RATTLES)]),
    list(Shaker, &[plays(Maracas, RATTLES)]),
    list(Whip, &[]),
    list(
        Glockenspiel,
        &[
            asking(plays(Celesta, METALLIC_RING)),
            plays(Crotales, METALLIC_RING),
        ],
    ),
    list(
        Xylophone,
        &[
            plays(Marimba, MALLETS_OF_ADJACENT_REGISTER),
            plays(Glockenspiel, MALLETS_OF_ADJACENT_REGISTER),
        ],
    ),
    list(
        Vibraphone,
        &[
            plays(Glockenspiel, METALLIC_RING),
            asking(plays(Celesta, METALLIC_RING)),
        ],
    ),
    list(
        Marimba,
        &[
            plays(Xylophone, MALLETS_OF_ADJACENT_REGISTER),
            plays(Vibraphone, MALLETS_OF_ADJACENT_REGISTER),
        ],
    ),
    list(
        TubularBells,
        &[
            plays(
                Carillon,
                estimate("a timbre judgement: the bells of a tower, the nearest bell sound"),
            ),
            plays(
                Glockenspiel,
                estimate("a timbre judgement: another bell sound, higher and lighter"),
            ),
        ],
    ),
    list(Crotales, &[plays(Glockenspiel, METALLIC_RING)]),
    list(
        Carillon,
        &[plays(
            TubularBells,
            Provenance::Cited {
                source: cite!(
                    vsl_academy_tubular_bells: "tubular bells were first made as a substitute for \
                        church bells"
                ),
            },
        )],
    ),
    // Plucked strings.
    list(
        Harp,
        &[asking(only(
            Scope::Arpeggios,
            playing(
                Manner::Softer,
                plays(
                    PianoConcertGrand,
                    Provenance::Cited {
                        source: cite!(
                            forsyth: "in the orchestra the piano has served as a substitute for \
                                the harp or the celesta";
                            "playing it softer and for arpeggios only is an estimate",
                        ),
                    },
                ),
            ),
        ))],
    ),
    list(
        GuitarClassical,
        &[
            plays(Harp, PLUCKED_OF_SIMILAR_REGISTER),
            plays(Lute, PLUCKED_OF_SIMILAR_REGISTER),
        ],
    ),
    list(
        Lute,
        &[
            plays(GuitarClassical, PLUCKED_OF_SIMILAR_REGISTER),
            plays(Harp, PLUCKED_OF_SIMILAR_REGISTER),
        ],
    ),
    list(
        Mandolin,
        &[plays(GuitarClassical, PLUCKED_OF_SIMILAR_REGISTER)],
    ),
    // Keyboards.
    list(
        Celesta,
        &[
            plays(
                PianoUpright,
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "a missing celesta is replaced by an upright piano, not \
                            by the glockenspiel"
                    ),
                },
            ),
            asking(plays(
                Glockenspiel,
                estimate(reason!(
                    "a timbre judgement: the nearest bell sound, though ",
                    rimsky_korsakov,
                    " advises against it in place of a celesta",
                )),
            )),
        ],
    ),
    list(
        PianoConcertGrand,
        &[
            plays(PianoStudioGrand, ANOTHER_SIZE),
            plays(PianoUpright, ANOTHER_SIZE),
        ],
    ),
    list(
        PianoStudioGrand,
        &[
            plays(PianoConcertGrand, ANOTHER_SIZE),
            plays(PianoUpright, ANOTHER_SIZE),
        ],
    ),
    list(PianoUpright, &[plays(PianoStudioGrand, ANOTHER_SIZE)]),
    list(
        PianoFelt,
        &[
            playing(Manner::Softer, plays(PianoUpright, FELT_TONE)),
            playing(Manner::Softer, plays(PianoStudioGrand, FELT_TONE)),
        ],
    ),
    // No instrument comes near the sounds of the preparations.
    list(PianoPrepared, &[]),
    list(
        PianoHonkyTonk,
        &[plays(
            PianoUpright,
            estimate("the same instrument, kept in tune"),
        )],
    ),
    list(
        Harpsichord,
        &[
            plays(
                Clavichord,
                estimate("the other early keyboard of the dataset"),
            ),
            playing(
                Manner::Softer,
                plays(
                    PianoUpright,
                    estimate("a piano played softer comes nearest the light harpsichord tone"),
                ),
            ),
        ],
    ),
    list(
        Clavichord,
        &[playing(
            Manner::Softer,
            plays(
                Harpsichord,
                estimate(
                    "the other early keyboard, played softer to come nearer the quiet clavichord",
                ),
            ),
        )],
    ),
    list(
        ElectricPianoRhodes,
        &[plays(ElectricPianoWurlitzer, OTHER_ELECTRIC_PIANO)],
    ),
    list(
        ElectricPianoWurlitzer,
        &[plays(ElectricPianoRhodes, OTHER_ELECTRIC_PIANO)],
    ),
    list(
        Clavinet,
        &[plays(
            Harpsichord,
            estimate(
                "a timbre judgement: a keyboard of a similarly bright, percussive string tone",
            ),
        )],
    ),
    list(
        OrganPipe,
        &[only(
            Scope::Chords,
            plays(
                OrganHammond,
                estimate(
                    "the other organ of the dataset, nearest the pipe organ in sustained chords",
                ),
            ),
        )],
    ),
    list(
        OrganHammond,
        &[plays(OrganPipe, estimate("the other organ of the dataset"))],
    ),
    list(
        Accordion,
        &[plays(
            OrganPipe,
            estimate(
                "a timbre judgement on a sustained reed sound: a reed stop of the pipe organ \
                      comes nearest",
            ),
        )],
    ),
    // Voices: a soloist's part is sung by the choir section of the same voice type. No choir
    // section has a neighbour.
    list(
        VoiceSoprano,
        &[asking(plays(ChoirSoprano, SOLO_BY_SECTION))],
    ),
    list(
        VoiceMezzoSoprano,
        &[
            asking(plays(ChoirSoprano, MEZZO_IN_A_CHORUS)),
            asking(plays(ChoirAlto, MEZZO_IN_A_CHORUS)),
        ],
    ),
    list(VoiceAlto, &[asking(plays(ChoirAlto, SOLO_BY_SECTION))]),
    list(VoiceTenor, &[asking(plays(ChoirTenor, SOLO_BY_SECTION))]),
    list(
        VoiceBaritone,
        &[
            asking(plays(ChoirTenor, BARITONE_IN_A_CHORUS)),
            asking(plays(ChoirBass, BARITONE_IN_A_CHORUS)),
        ],
    ),
    list(VoiceBass, &[asking(plays(ChoirBass, SOLO_BY_SECTION))]),
    list(ChoirSoprano, &[]),
    list(ChoirAlto, &[]),
    list(ChoirTenor, &[]),
    list(ChoirBass, &[]),
    // Bowed strings.
    list(Violin, &[plays(Viola, ADJACENT_STRING)]),
    list(
        Viola,
        &[
            plays(Violin, ADJACENT_STRING),
            plays(Cello, ADJACENT_STRING),
        ],
    ),
    list(
        Cello,
        &[
            plays(Viola, ADJACENT_STRING),
            plays(DoubleBass, ADJACENT_STRING),
        ],
    ),
    list(DoubleBass, &[plays(Cello, ADJACENT_STRING)]),
];

// Compile-time check: the row at each place of `NEIGHBOURS` is that of the type at the same place
// of `InstrumentType::ALL`, so a lookup by place cannot return another type's neighbours, and
// every row passes its own check (PRI-04).
const _: () = {
    let mut i = 0;
    while i < NEIGHBOURS.len() {
        assert!(
            NEIGHBOURS[i].instrument as usize == i,
            "NEIGHBOURS holds each type's row at the type's place in score order"
        );
        assert!(
            NEIGHBOURS[i].check().is_ok(),
            "every neighbour fits the instrument whose part it plays"
        );
        i += 1;
    }
};

/// `articulation` played as written.
const fn articulation(articulation: Articulation) -> ArticulationSubstitute {
    ArticulationSubstitute {
        stand_in: StandIn::Articulation(articulation),
        rewrite: None,
        vibrato: None,
        only: None,
    }
}

/// `articulation` with its notes rewritten by `rewrite`.
const fn rewritten(articulation: Articulation, rewrite: Emulation) -> ArticulationSubstitute {
    ArticulationSubstitute {
        stand_in: StandIn::Articulation(articulation),
        rewrite: Some(rewrite),
        vibrato: None,
        only: None,
    }
}

/// `substitute`, for passages of `scope` only.
const fn suiting(scope: Scope, substitute: ArticulationSubstitute) -> ArticulationSubstitute {
    ArticulationSubstitute {
        only: Some(scope),
        ..substitute
    }
}

const fn row(
    articulation: Articulation,
    substitutes: &'static [ArticulationSubstitute],
    emulation: Option<Emulation>,
    reason: &'static str,
) -> Substitution {
    Substitution {
        articulation,
        substitutes,
        emulation,
        provenance: estimate(reason),
    }
}

/// Each note shortened to a quarter of its length.
const QUARTER: Emulation = Emulation::Shorten { divisor: 4 };

/// Each note played as repeated 32nd notes.
const THIRTY_SECONDS: Emulation = Emulation::Repeat { per_whole_note: 32 };

/// The rate of a written-out trill: 12 to 16 notes per second.
const TRILL_RATE: Emulation = Emulation::Alternate {
    slowest: 12,
    fastest: 16,
};

/// The substitution rows of the fifteen original articulations, in the order of
/// [`Articulation::ALL`]: row `i` is the row of `Articulation::ALL[i]`.
pub static SUBSTITUTIONS: [Substitution; 15] = [
    row(
        Sustain,
        &[
            articulation(Tenuto),
            articulation(Legato),
            articulation(Marcato),
        ],
        None,
        "a musical judgement: a tenuto is the nearest held note, then a legato patch with \
         detached notes, then the longest marcato, which adds accents",
    ),
    row(
        Legato,
        &[
            suiting(Scope::SingleLines, rewritten(Sustain, Emulation::Overlap)),
            articulation(Tenuto),
        ],
        Some(Emulation::Overlap),
        "a musical judgement: a sustain on one line with overlapping notes comes nearest, then a \
         tenuto; a sound that joins overlapping notes plays them legato",
    ),
    row(
        Staccato,
        &[
            articulation(Spiccato),
            articulation(Marcato),
            articulation(Staccatissimo),
            rewritten(Sustain, QUARTER),
        ],
        Some(QUARTER),
        "a musical judgement: spiccato, a short marcato and staccatissimo are the nearest short \
         notes; long notes cut to about a quarter of their length imitate the rest",
    ),
    row(
        Spiccato,
        &[
            articulation(Staccato),
            rewritten(ColLegno, Emulation::Soften),
            rewritten(Sustain, QUARTER),
        ],
        Some(QUARTER),
        "a musical judgement: staccato is the nearest short stroke, then a soft col legno, then \
         long notes cut short as for a staccato",
    ),
    row(
        Tenuto,
        &[articulation(Sustain), articulation(Legato)],
        None,
        "a musical judgement: a sustain, then a legato patch, holds each note its full length",
    ),
    row(
        Marcato,
        &[
            rewritten(Sustain, Emulation::Accent),
            suiting(Scope::ShortNotes, articulation(Staccato)),
        ],
        Some(Emulation::Accent),
        "a musical judgement: a sustain a dynamic step louder imitates the accent, and a staccato \
         serves the short notes",
    ),
    row(
        Pizzicato,
        &[
            ArticulationSubstitute {
                stand_in: StandIn::OtherEnsemble,
                rewrite: None,
                vibrato: None,
                only: None,
            },
            ArticulationSubstitute {
                stand_in: StandIn::Instrument(Harp),
                rewrite: None,
                vibrato: None,
                only: Some(Scope::SingleLines),
            },
            rewritten(Staccato, Emulation::Soften),
        ],
        None,
        "a musical judgement: the pizzicato of the other ensemble size, then a harp for single \
         lines in an orchestral context, then a soft staccato; no rewrite of bowed notes comes \
         near a pluck",
    ),
    row(
        Tremolo,
        &[articulation(Sustain), articulation(Staccato)],
        Some(THIRTY_SECONDS),
        "a musical judgement: a sustain, then a staccato; repeated 32nd notes with a small \
         variation in velocity imitate the tremolo",
    ),
    row(
        Trill,
        &[],
        Some(TRILL_RATE),
        "a musical judgement: no other articulation comes near a trill; the main and the upper \
         auxiliary note alternating at about 12 to 16 notes per second write it out",
    ),
    row(
        ColLegno,
        &[
            rewritten(Spiccato, Emulation::Soften),
            rewritten(Staccato, Emulation::Soften),
        ],
        None,
        "a musical judgement: a soft spiccato, then a soft staccato, comes nearest the light, dry \
         stroke of the wood",
    ),
    row(
        Harmonics,
        &[ArticulationSubstitute {
            stand_in: StandIn::Articulation(Sustain),
            rewrite: Some(Emulation::Soften),
            vibrato: Some(Vibrato::Off),
            only: None,
        }],
        None,
        "a musical judgement: a soft sustain without vibrato comes nearest the pure tone of a \
         harmonic, though its colour differs",
    ),
    row(
        SulPonticello,
        &[
            suiting(Scope::TremoloPassages, articulation(Tremolo)),
            articulation(Sustain),
        ],
        None,
        "a musical judgement: in a tremolo passage the tremolo keeps the motion, elsewhere an \
         ordinary sustain plays; no rewrite gives the glassy colour",
    ),
    row(
        ConSordino,
        &[],
        Some(Emulation::Soften),
        "a musical judgement: a mute holds over a passage, so the passage plays open with its own \
         articulations, a dynamic step softer",
    ),
    row(
        FlutterTongue,
        &[articulation(Tremolo), articulation(Sustain)],
        None,
        "a musical judgement: a tremolo, then a sustain; no rewrite of the notes imitates the \
         flutter",
    ),
    row(
        Roll,
        &[articulation(Tremolo)],
        Some(THIRTY_SECONDS),
        "a musical judgement: a tremolo, the roll of a mallet instrument, comes nearest; single \
         strokes as repeated 32nd notes with a small variation in velocity imitate the roll",
    ),
];

// Compile-time check: row `i` of `SUBSTITUTIONS` is the row of `Articulation::ALL[i]`, the rows
// cover exactly the original articulations, which come first in the vocabulary, and every row
// passes its own check (PRI-04).
const _: () = {
    let mut i = 0;
    while i < SUBSTITUTIONS.len() {
        let articulation = SUBSTITUTIONS[i].articulation;
        assert!(
            articulation as usize == i && articulation.is_stable(),
            "SUBSTITUTIONS holds the original articulations in vocabulary order"
        );
        assert!(
            SUBSTITUTIONS[i].check().is_ok(),
            "every substitute can stand in for its articulation"
        );
        i += 1;
    }
    assert!(
        !Articulation::ALL[SUBSTITUTIONS.len()].is_stable(),
        "SUBSTITUTIONS has a row for every original articulation"
    );
};
