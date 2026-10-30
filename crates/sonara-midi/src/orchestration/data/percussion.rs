//! Percussion records.

use super::sources::{cite, reason};
use super::{PLAYERS_ESTIMATE, cited, estimate, not_used, pitched, players, unstated};
use crate::orchestration::articulation::Articulation::{
    Bowed, Choke, ConSordino, CrescendoSwell, Damped, DeadStroke, Drag, Flam, Fortepiano,
    Glissando, LetRing, Marcato, NonVibrato, Portamento, Rim, Rimshot, Roll, Scrape, ShakeStroke,
    SnaresOff, Staccato, Tremolo, TremoloMeasured, Trill, VibratoOn,
};
use crate::orchestration::ensemble_profile::{EnsembleProfile, Forms};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::instrument::{Instrument, InstrumentSpec, PitchModel, Provenance};
use crate::orchestration::pitch::Clef;

pub(super) const TIMPANI: Instrument = Instrument::new(
    InstrumentType::Timpani,
    InstrumentSpec {
        name: "Timpani",
        pitch: pitched(
            0,
            cited(
                36,
                61,
                cite!(
                    vsl_academy_timpani: "the largest kettledrum, the D drum (75-80 cm), plays \
                        \"C2, D2 - H2, C3\": low C2 = 36; the \"A kettledrum\" (50-60 cm) plays \
                        \"C3, D3 - Bb3, C#4\": C#4 = 61 is the extreme note",
                    vsl_synchron_timpani_3: "five Adams drums, 20 to 32 in, \"from C2 to C4\"",
                    wikipedia_timpani: "a 33-inch drum reaches C2; C4 = 60 is the top of a 20-in \
                        drum",
                    vsl_synchron_percussion_1: "a single 20-in drum mapped \"E3-C#4\"; timpani \
                        glissandos over C2-C#4";
                    "piccolo timpani of 12 in or less, which reach into the treble clef, are left \
                        out",
                ),
            ),
            Some(cited(
                38,
                57,
                cite!(
                    wikipedia_timpani: "a standard four-drum set covers \"roughly D2 to A3\"",
                    rimsky_korsakov: "E2-G#3 = 40-56",
                    yamaha_timpani_tp8300r: "32, 29, 26 and 23 in drums: D2-Ab3 = 38-56";
                    "the octave placement of the Yamaha letters is an estimate",
                ),
            )),
        ),
        clefs: &[Clef::Bass],
        articulations: &[
            Roll,
            Marcato,
            ConSordino,
            LetRing,
            Damped,
            Glissando,
            CrescendoSwell,
            TremoloMeasured,
            Fortepiano,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "rolls and measured rolls; crescendo, diminuendo and sfp in the \
                    roll; coperto muting with cloth or felt",
                forsyth: "coperto muting with cloth or felt",
                vsl_academy_timpani: "rolls and measured rolls; crescendo, diminuendo and sfp in \
                    the roll; coperto; secco, an immediately damped stroke; pedal and roll \
                    glissando",
                vsl_synchron_percussion_1: "secco strokes; pedal and roll glissando; dynamic, \
                    swelling rolls",
                wikipedia_timpani: "\"con sordino\" muting; pedal glissando";
                "marcato and let_ring as names for the accented and the ringing single stroke are \
                    estimates; no source uses these words",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(1, 1, "1 timpanist, 2 drums", PLAYERS_ESTIMATE),
            romantic: players(
                1,
                2,
                "1 timpanist, 3-4 drums; 2 or more timpanists in large scores",
                PLAYERS_ESTIMATE,
            ),
            film: players(1, 1, "1 timpanist, 4-5 drums", PLAYERS_ESTIMATE),
            chamber: not_used("usually none", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const BASS_DRUM: Instrument = Instrument::new(
    InstrumentType::BassDrum,
    InstrumentSpec {
        name: "Bass drum",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Bass, Clef::Percussion],
        articulations: &[Roll, Marcato, ConSordino, Damped, LetRing],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "roll, damp, muffled, the Ruthe",
                widor_second_scan: "roll with single or double stick",
                wikipedia_bass_drum: "rolls, repetitions",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: unstated(
                "18th-century opera, as local colour for Eastern scenes, with cymbals and triangle",
                Provenance::Cited {
                    source: cite!(
                        forsyth: "18th-century opera, as local colour for Eastern scenes, with \
                            cymbals and triangle",
                    ),
                },
            ),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(
                1,
                1,
                "often several drums layered in samples",
                PLAYERS_ESTIMATE,
            ),
            chamber: unstated("rare", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const SNARE_DRUM: Instrument = Instrument::new(
    InstrumentType::SnareDrum,
    InstrumentSpec {
        name: "Snare drum",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Treble, Clef::Percussion],
        articulations: &[
            Roll, Marcato, ConSordino, Flam, Drag, Rimshot, Rim, SnaresOff,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "roll, flam, drag, accents, muffled",
                widor_second_scan: "sans timbre",
                wikipedia_snare_drum: "rimshot, cross-stick",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: unstated(
                "rare; military and Turkish colour in opera",
                PLAYERS_ESTIMATE,
            ),
            romantic: players(
                1,
                1,
                "often also covering other battery instruments",
                PLAYERS_ESTIMATE,
            ),
            film: players(1, 1, "or a sampled ensemble of snares", PLAYERS_ESTIMATE),
            chamber: players(
                1,
                1,
                "a percussionist with a multi-percussion set-up",
                PLAYERS_ESTIMATE,
            ),
        }),
    },
);

pub(super) const CLASH_CYMBALS: Instrument = Instrument::new(
    InstrumentType::ClashCymbals,
    InstrumentSpec {
        name: "Clash cymbals",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Bass, Clef::Percussion],
        articulations: &[Roll, Marcato, LetRing, Damped, Choke],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "two-plate stroke, two-plate roll",
                widor_second_scan: "damped or let vibrate; clash roll",
                wikipedia_clash_cymbals: "crash",
                wikipedia_cymbal: "l.v., secco, damped against the body",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: unstated(
                "18th-century opera, as local colour for Eastern scenes",
                Provenance::Cited {
                    source: cite!(
                        forsyth: "18th-century opera, as local colour for Eastern scenes",
                    ),
                },
            ),
            romantic: players(1, 1, "one pair", PLAYERS_ESTIMATE),
            film: players(1, 1, "several pairs in samples", PLAYERS_ESTIMATE),
            chamber: unstated("rare", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const SUSPENDED_CYMBAL: Instrument = Instrument::new(
    InstrumentType::SuspendedCymbal,
    InstrumentSpec {
        name: "Suspended cymbal",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Bass, Clef::Percussion],
        articulations: &[
            Roll,
            Marcato,
            CrescendoSwell,
            LetRing,
            Damped,
            Choke,
            Bowed,
            Scrape,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "hard and soft stick strokes, two-stick roll, finger damping",
                widor_second_scan: "suspended roll",
                wikipedia_suspended_cymbal: "mallet roll, bow, scrape",
                wikipedia_cymbal: "roll swell, coin or triangle-beater scrape, bell",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used(
                "",
                Provenance::Estimate {
                    reason: "no source states its use in Classical scores",
                },
            ),
            romantic: players(1, 1, "often the cymbal player", PLAYERS_ESTIMATE),
            film: players(1, 2, "swells are a staple", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const TAM_TAM: Instrument = Instrument::new(
    InstrumentType::TamTam,
    InstrumentSpec {
        name: "Tam-tam",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Bass, Clef::Percussion],
        articulations: &[
            Roll,
            Marcato,
            LetRing,
            Damped,
            CrescendoSwell,
            Scrape,
            Bowed,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "single stroke, crescendo roll, damping",
                widor_second_scan: "damp before a change of harmony",
                wikipedia_gong: "priming, damped notes, scrape, bow",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used(
                "not used before Gossec, 1790 or 1791",
                Provenance::Cited {
                    source: cite!(
                        wikipedia_gong: "first used by Gossec, 1790 or 1791",
                    ),
                },
            ),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(
                1,
                1,
                "large tam-tam hits and scrapes are common",
                PLAYERS_ESTIMATE,
            ),
            chamber: unstated("rare", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const TRIANGLE: Instrument = Instrument::new(
    InstrumentType::Triangle,
    InstrumentSpec {
        name: "Triangle",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Treble, Clef::Percussion],
        articulations: &[Roll, Marcato, LetRing, Damped],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "strokes, groups, roll",
                widor_second_scan: "tremolo, pp at the top",
                wikipedia_triangle: "roll, hand damping",
                midi_gm1_sound_set_2021_01: "Mute Triangle and Open Triangle",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: unstated(
                "18th-century opera, as local colour for Eastern scenes",
                Provenance::Cited {
                    source: cite!(
                        forsyth: "18th-century opera, as local colour for Eastern scenes",
                    ),
                },
            ),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const TAMBOURINE: Instrument = Instrument::new(
    InstrumentType::Tambourine,
    InstrumentSpec {
        name: "Tambourine",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Treble, Clef::Percussion],
        articulations: &[Roll, Marcato, ShakeStroke],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "knuckles, shake roll, thumb",
                widor_second_scan: "hand, rustle, thumb",
                wikipedia_tambourine: "thumb roll with beeswax or rosin, wrist roll",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: unstated(
                "Mozart among the earliest",
                Provenance::Cited {
                    source: cite!(
                        wikipedia_tambourine: "Mozart among the earliest orchestral uses",
                    ),
                },
            ),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const CASTANETS: Instrument = Instrument::new(
    InstrumentType::Castanets,
    InstrumentSpec {
        name: "Castanets",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Treble, Clef::Percussion],
        articulations: &[Roll, Marcato],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "rhythms",
                widor_second_scan: "any rhythmic combination, accents",
                wikipedia_castanets: "roll; handle castanets for sustained rolls",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: unstated("rare", PLAYERS_ESTIMATE),
            romantic: players(1, 1, "Spanish colour", PLAYERS_ESTIMATE),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const WOOD_BLOCK: Instrument = Instrument::new(
    InstrumentType::WoodBlock,
    InstrumentSpec {
        name: "Wood block",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Percussion],
        articulations: &[Marcato, Roll],
        articulations_provenance: Provenance::Estimate {
            reason: reason!(
                "single strokes and rolls are general practice; ",
                wikipedia_woodblock,
                " states only stick against mallet strokes",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used(
                "",
                Provenance::Estimate {
                    reason: "no source states its use in Classical scores",
                },
            ),
            romantic: unstated("rare", PLAYERS_ESTIMATE),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const TEMPLE_BLOCKS: Instrument = Instrument::new(
    InstrumentType::TempleBlocks,
    InstrumentSpec {
        name: "Temple blocks",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::Percussion],
        articulations: &[Marcato, Roll, Glissando],
        articulations_provenance: Provenance::Estimate {
            reason: reason!(
                "strokes, rolls and a sweep across the set are general practice; ",
                wikipedia_temple_blocks,
                " names no techniques",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used(
                "",
                Provenance::Estimate {
                    reason: "no source states its use in Classical scores",
                },
            ),
            romantic: not_used(
                "",
                Provenance::Estimate {
                    reason: "no source states its use in Romantic scores",
                },
            ),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const SLEIGH_BELLS: Instrument = Instrument::new(
    InstrumentType::SleighBells,
    InstrumentSpec {
        name: "Sleigh bells",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Percussion],
        articulations: &[Roll, Marcato, ShakeStroke],
        articulations_provenance: Provenance::Estimate {
            reason: reason!(
                "the techniques are general practice; a hit and a shake exist as separate samples \
                    (",
                vsco2_ce,
                ")",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: unstated("rare", PLAYERS_ESTIMATE),
            romantic: players(
                1,
                1,
                "Mahler's 4th symphony",
                Provenance::Cited {
                    source: cite!(
                        wikipedia_jingle_bell: "sleigh bells in Mahler's 4th symphony",
                    ),
                },
            ),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: unstated("rare", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const MARACAS: Instrument = Instrument::new(
    InstrumentType::Maracas,
    InstrumentSpec {
        name: "Maracas",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Percussion],
        articulations: &[Roll, Marcato, ShakeStroke],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                wikipedia_maraca: "shaking; a rhythmic role",
                wikipedia_shaker: "shakers may be struck for accents";
                "roll is an estimate",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used(
                "",
                Provenance::Estimate {
                    reason: "no source states its use in Classical scores",
                },
            ),
            romantic: not_used(
                "",
                Provenance::Estimate {
                    reason: "no source states its use in Romantic scores",
                },
            ),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const SHAKER: Instrument = Instrument::new(
    InstrumentType::Shaker,
    InstrumentSpec {
        name: "Shaker",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Percussion],
        articulations: &[Roll, Marcato, ShakeStroke],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                wikipedia_shaker: "shaken back and forth, struck for accents";
                "roll is an estimate",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used(
                "",
                Provenance::Estimate {
                    reason: "no source states its use in Classical scores",
                },
            ),
            romantic: not_used(
                "",
                Provenance::Estimate {
                    reason: "no source states its use in Romantic scores",
                },
            ),
            film: players(1, 1, "common in hybrid scores", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const WHIP: Instrument = Instrument::new(
    InstrumentType::Whip,
    InstrumentSpec {
        name: "Whip",
        pitch: PitchModel::Unpitched,
        clefs: &[Clef::SingleLine, Clef::Percussion],
        articulations: &[Marcato],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                wikipedia_clapper: "a single crack; two-handed louder than one-handed",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used(
                "",
                Provenance::Estimate {
                    reason: "no source states its use in Classical scores",
                },
            ),
            romantic: players(
                1,
                1,
                "Mahler's 5th and 6th symphonies",
                Provenance::Cited {
                    source: cite!(
                        wikipedia_clapper: "the whip in Mahler's 5th and 6th symphonies",
                    ),
                },
            ),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const GLOCKENSPIEL: Instrument = Instrument::new(
    InstrumentType::Glockenspiel,
    InstrumentSpec {
        name: "Glockenspiel",
        pitch: pitched(
            24,
            cited(
                72,
                108,
                cite!(
                    vsl_academy_glockenspiel: "three-octave instruments, C5-C8, \"usual in the \
                        symphony orchestra\"",
                    yamaha_glockenspiels: "YG-2500 \"C52-E92\", that is C5-E8",
                    vsl_synchron_percussion_1: "a Yamaha 2500 mapped C4-E7, one octave below its \
                        sound";
                    "tops above C8 (E8, F8) are left out as model-specific",
                ),
            ),
            Some(cited(
                79,
                108,
                cite!(
                    hugill_pitched_percussion: "\"The normal written range is from G3-C6 (sounding \
                        two octaves higher)\", that is G5-C8",
                    vsl_academy_glockenspiel: "2.5 octaves, G5-C8",
                    yamaha_glockenspiels: "DG-1590A \"G59-C88\"";
                    "G5-C8 is the range every listed model covers (an estimate: the intersection \
                        of the listed models)",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[
            Staccato, Marcato, Tremolo, Trill, LetRing, Damped, Glissando, Bowed,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_glockenspiel: "tremolo of every kind, \"easy\" trills, glissandos with \
                    round mallets, damping by hand or pedal",
                vsl_synchron_percussion_1: "bowed and secco notes";
                "staccato, a damped note, and marcato are estimates",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used(
                "Mozart's keyboard glockenspiel is the exception",
                PLAYERS_ESTIMATE,
            ),
            romantic: players(0, 1, "", PLAYERS_ESTIMATE),
            film: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
            chamber: players(0, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const XYLOPHONE: Instrument = Instrument::new(
    InstrumentType::Xylophone,
    InstrumentSpec {
        name: "Xylophone",
        pitch: pitched(
            12,
            cited(
                60,
                108,
                cite!(
                    vsl_academy_xylophone: "four-octave instruments, C4-C8",
                    vsl_synchron_percussion_1: "an Adams Artist Alpha with range C4-C8",
                ),
            ),
            Some(cited(
                65,
                108,
                cite!(
                    vsl_academy_xylophone: "three and a half octaves, F4-C8",
                    yamaha_xylophones: "YX-500R and YX2035PR \"F45-C88\", that is F4-C8 (reading \
                        the key index is an estimate)",
                    wikipedia_xylophone: "sizes of \"3 1/2 or 4 octaves\"";
                    "the bounds are cited; choosing this compass as the comfortable one is an \
                        estimate: it is the compass shared by the \"3 1/2 or 4 octaves\" sizes of \
                        the encyclopedia, while VSL lists 3, 3 1/2 and 4 octaves without ranking \
                        them",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[Marcato, Tremolo, Glissando, DeadStroke, Bowed],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "tremolo of every kind, monophonic and polyphonic; glissandos",
                vsl_academy_xylophone: "tremolo, glissandos, dead strokes; the notes are naturally \
                    short",
                vsl_synchron_percussion_1: "bowed notes";
                "marcato is an estimate; no source names accents",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: players(0, 1, "a percussionist", PLAYERS_ESTIMATE),
            film: players(
                1,
                1,
                "a percussionist, often changing between mallet instruments",
                PLAYERS_ESTIMATE,
            ),
            chamber: players(0, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const VIBRAPHONE: Instrument = Instrument::new(
    InstrumentType::Vibraphone,
    InstrumentSpec {
        name: "Vibraphone",
        pitch: pitched(
            0,
            cited(
                48,
                89,
                cite!(
                    vsl_academy_vibraphone: "3.5 octaves, C3-F6",
                    wikipedia_vibraphone: "3.5 octaves, C3-F6",
                    yamaha_vibraphones: "YV-3910 \"C28-F69\"";
                    "the four-octave YV-4110 to C7 (96) is left out as uncommon",
                ),
            ),
            Some(cited(
                53,
                89,
                cite!(
                    vsl_academy_vibraphone: "the standard three octaves, F3-F6",
                    wikipedia_vibraphone: "the standard three octaves, F3-F6",
                    yamaha_vibraphones: "YV-2700 and YV-3710 \"F33-F69\"",
                    musser_m55b_pro_vibe: "a three-octave instrument",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[
            Staccato, Marcato, Tremolo, LetRing, Damped, DeadStroke, Glissando, Bowed, VibratoOn,
            NonVibrato, Portamento,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_vibraphone: "tremolo, glissando, dead stroke, bowing, a resonance \
                    glissando or pitch bend of up to \"a minor or a major second\", motor on \
                    (\"Vibrato\") and off (\"Vibrato stop\"), pedal and mallet damping",
                wikipedia_vibraphone: "a pitch bend of about a half step; motor and damping";
                "marcato is an estimate; the damper pedal itself is a control (CC 64), not an \
                    articulation",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: not_used("", PLAYERS_ESTIMATE),
            film: players(0, 1, "a percussionist", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const MARIMBA: Instrument = Instrument::new(
    InstrumentType::Marimba,
    InstrumentSpec {
        name: "Marimba",
        pitch: pitched(
            0,
            cited(
                36,
                96,
                cite!(
                    vsl_academy_marimba: "five octaves, C2-C7",
                    wikipedia_marimba: "five octaves, C2-C7",
                    yamaha_marimbas: "YM-6100 and YM-5100A \"C16-C76\"";
                    "instruments starting at A1 (33) and the YM-5104A to G7 (103) exist but are \
                        left out as rare",
                ),
            ),
            Some(cited(
                45,
                96,
                cite!(
                    wikipedia_marimba: "the 4.3-octave A2-C7 is \"the most common range\"",
                    vsl_academy_marimba: "the same size, without ranking it",
                    yamaha_marimbas: "YM-4600A, YM-2400 and YM-1430 \"A25-C76\"",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Marcato, Tremolo, Trill, Glissando, DeadStroke, Bowed],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_marimba: "tremolo of every kind, glissandos, dead strokes, bowing with \
                    a double-bass bow, easy trills; staccato needs extra accent";
                "marcato is an estimate",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: not_used("", PLAYERS_ESTIMATE),
            film: players(0, 1, "a percussionist", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a solo and ensemble instrument", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const TUBULAR_BELLS: Instrument = Instrument::new(
    InstrumentType::TubularBells,
    InstrumentSpec {
        name: "Tubular bells",
        pitch: pitched(
            0,
            cited(
                51,
                79,
                cite!(
                    vsl_academy_tubular_bells: "the 2 1/3-octave \"philharmonic\" set, Eb3-G5 (29 \
                        tubes); the pitch is the strike note, written and heard as such",
                    wikipedia_tubular_bells: "professional models reach G5",
                ),
            ),
            Some(cited(
                60,
                77,
                cite!(
                    wikipedia_tubular_bells: "a written range \"usually seen as C4 to F5\"",
                    vsl_academy_tubular_bells: "the 1 1/2-octave set, C4-F5 (18 tubes)",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[Marcato, Tremolo, LetRing, Damped, Glissando],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_tubular_bells: "single strokes, tremolo with great crescendo, damping \
                    by pedal or hand",
                vsl_synchron_percussion_1: "open, muted and glissando patches";
                "marcato is an estimate",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: players(0, 1, "", PLAYERS_ESTIMATE),
            film: players(1, 1, "a percussionist", PLAYERS_ESTIMATE),
            chamber: players(0, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const CROTALES: Instrument = Instrument::new(
    InstrumentType::Crotales,
    InstrumentSpec {
        name: "Crotales",
        pitch: pitched(
            24,
            cited(
                84,
                108,
                cite!(
                    hugill_pitched_percussion: "written C4-C6, sounding two octaves higher",
                    wikipedia_crotales: "sounding C6-C8, up to two octaves",
                    sonatina_symphonic_orchestra: "samples mapped C6-C8 = 84-108",
                ),
            ),
            Some(cited(
                84,
                108,
                cite!(
                    hugill_pitched_percussion: "the full two-octave set, written C4-C6";
                    "many sets are one octave only; which octave is the more common was not found",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[Marcato, Tremolo, LetRing, Damped, Bowed],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                wikipedia_crotales: "struck with hard mallets, bowed, or two discs struck together";
                "tremolo, let_ring, damped and marcato are estimates; no source that was read \
                    describes them for crotales",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: players(0, 1, "", PLAYERS_ESTIMATE),
            film: players(0, 1, "a percussionist", PLAYERS_ESTIMATE),
            chamber: players(0, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const CARILLON: Instrument = Instrument::new(
    InstrumentType::Carillon,
    InstrumentSpec {
        name: "Carillon",
        pitch: pitched(
            0,
            estimate(
                48,
                96,
                reason!(
                    "a representative four-octave concert carillon in C, non-transposing: a \
                        concert carillon has 45-50 bells, about four octaves (",
                    wikipedia_carillon,
                    "), and most instruments are 4 to 4 1/2 octaves (",
                    gcna_carillons,
                    "); the lowest pedal C is normally tenor C, \"the second space on the bass \
                        clef\", C3; many C-compass carillons lack the lowest C# and Eb bells, 49 \
                        and 51 here; real instruments range from 2 to more than 6 octaves and \
                        transpose from down a fourth to up an octave",
                ),
            ),
            None,
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[Tremolo, Marcato, LetRing],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                gcna_carillons: "touch-sensitive expression; the bells ring until the vibration \
                    dies; dampers only deaden",
                wikipedia_carillon: "tremolo by repeated batons",
                brink_carillon: "tremolo by repeated batons";
                "marcato is an estimate: accents come from the touch, and no source names a \
                    marcato",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("not an orchestral instrument", PLAYERS_ESTIMATE),
            romantic: not_used("not an orchestral instrument", PLAYERS_ESTIMATE),
            film: unstated("a sampled or recorded tower", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "a carillonneur; a solo instrument", PLAYERS_ESTIMATE),
        }),
    },
);
