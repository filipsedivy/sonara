//! Brass records.

use super::sources::{cite, reason};
use super::{PLAYERS_ESTIMATE, cited, estimate, not_used, pitched, players, unstated};
use crate::orchestration::articulation::Articulation::{
    Accent, BellsUp, ConSordino, CrescendoSwell, Cuivre, Doit, Fall, FlutterTongue, Fortepiano,
    Glissando, Growl, HalfValve, Legato, Marcato, MultiTongue, Multiphonic, MuteBucket, MuteCup,
    MuteHarmon, MutePlunger, NonVibrato, Portamento, Portato, Rip, Scoop, Sforzando, Shake,
    Staccatissimo, Staccato, Stopped, Sustain, Tenuto, Trill, TrillHalf, TrillWhole, VibratoOn,
    WahWah,
};
use crate::orchestration::ensemble_profile::{EnsembleProfile, Forms};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::instrument::{Instrument, InstrumentSpec, Provenance};
use crate::orchestration::pitch::Clef;

pub(super) const HORN_F: Instrument = Instrument::new(
    InstrumentType::HornF,
    InstrumentSpec {
        name: "Horn in F",
        pitch: pitched(
            -7,
            cited(
                35,
                77,
                cite!(
                    forsyth: "a chromatic compass of written C3-C6 by three valves, with a \
                        downward extension through a diminished fifth to written F#2: sounding \
                        B1-F5",
                    vsl_academy_horn: "\"The horn in F has a range from B1 - F5 (sounding)\"",
                ),
            ),
            Some(estimate(
                41,
                72,
                reason!(
                    "the union of the limits ",
                    forsyth,
                    " gives for the two pairs: the 1st and 3rd horns descend to written C3 \
                        (sounding F2 = 41), the 2nd and 4th ascend to written G5 (sounding C5 = \
                        72); the range comfortable for all four players is narrower, written G3-C5 \
                        (sounding 48-65)",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Stopped,
            Cuivre,
            BellsUp,
            Glissando,
            Rip,
            Fall,
            Staccatissimo,
            Sforzando,
            Fortepiano,
            CrescendoSwell,
            TrillWhole,
            TrillHalf,
            MultiTongue,
            Multiphonic,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                spitfire_studio_brass: "the horn patch list",
                forsyth: "the horn techniques",
                vsl_academy_horn: "the horn techniques",
                wikipedia_french_horn: "the horn techniques",
                vsco2_ce: "the sampled horn articulations",
                sonatina_symphonic_orchestra: "the sampled horn articulations",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(2, 2, "", PLAYERS_ESTIMATE),
            romantic: players(
                4,
                4,
                "6-8 in large scores; Wagner's Ring 8",
                PLAYERS_ESTIMATE,
            ),
            film: players(
                4,
                6,
                "typical in sampled sections; 6-8 or more for large scores",
                PLAYERS_ESTIMATE,
            ),
            chamber: players(1, 1, "wind quintet, brass quintet", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const WAGNER_TUBA_TENOR: Instrument = Instrument::new(
    InstrumentType::WagnerTubaTenor,
    InstrumentSpec {
        name: "Tenor Wagner tuba",
        pitch: pitched(
            -2,
            cited(
                27,
                82,
                cite!(
                    vsl_academy_wagner_tuba: "range Eb1-Bb5 with no transposition, that is \
                        sounding",
                ),
            ),
            Some(estimate(
                40,
                74,
                reason!(
                    "the low end E2 = 40 is where the chromatic, non-pedal compass begins (",
                    wagner_tuba_com_range,
                    "); the high end D5 = 74 reads the same source's \"notes above the concert D \
                        become uncertain except in the hands of professional players\" as D5 (the \
                        octave is not stated); the top lies inside Wagner's designed compass in \
                        actual sounds, Bb2-F5 = 46-77 (",
                    forsyth,
                    "), and the bottom extends below it because the low notes are called \
                        comfortable",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Portato,
            Sforzando,
            Fortepiano,
            Glissando,
            MultiTongue,
            VibratoOn,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_wagner_tuba: "the Wagner tuba techniques",
                vsl_synchron_wagner_tuba: "the Wagner tuba articulations";
                "tenuto and marcato are estimates from general brass notation; no source names \
                    them for this instrument",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Section,
            classical: not_used("the instrument was first used in 1876", PLAYERS_ESTIMATE),
            romantic: players(
                2,
                2,
                "with 2 bass Wagner tubas (the Ring; Bruckner 7-9)",
                PLAYERS_ESTIMATE,
            ),
            film: unstated("rare; 2 when used", PLAYERS_ESTIMATE),
            chamber: not_used("", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const WAGNER_TUBA_BASS: Instrument = Instrument::new(
    InstrumentType::WagnerTubaBass,
    InstrumentSpec {
        name: "Bass Wagner tuba",
        pitch: pitched(
            -7,
            cited(
                27,
                77,
                cite!(
                    vsl_academy_wagner_tuba: "range Eb1-F5 with no transposition, that is sounding",
                ),
            ),
            Some(estimate(
                34,
                67,
                reason!(
                    "Wagner's designed compass for the F pair, in actual sounds Bb1-G4 = 34-67 (",
                    forsyth,
                    "); taking it as the comfortable range is the estimate; no source calls notes \
                        above G4 comfortable on the bass, and Wagner tubas are \"less adept at the \
                        highest notes\" than horns of the same pitch (",
                    wikipedia_wagner_tuba,
                    "); at the bottom, a three-valve bass has a gap that includes A1, while most \
                        basses have a fourth valve (",
                    wagner_tuba_com_range,
                    ")",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Portato,
            Sforzando,
            Fortepiano,
            Glissando,
            MultiTongue,
            VibratoOn,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_wagner_tuba: "the Wagner tuba techniques",
                vsl_synchron_wagner_tuba: "the Wagner tuba articulations";
                "tenuto and marcato are estimates from general brass notation; no source names \
                    them for this instrument",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Section,
            classical: not_used("the instrument was first used in 1876", PLAYERS_ESTIMATE),
            romantic: players(
                2,
                2,
                "with 2 tenor Wagner tubas (the Ring; Bruckner 7-9)",
                PLAYERS_ESTIMATE,
            ),
            film: unstated("rare; 2 when used", PLAYERS_ESTIMATE),
            chamber: not_used("", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const TRUMPET_PICCOLO: Instrument = Instrument::new(
    InstrumentType::TrumpetPiccolo,
    InstrumentSpec {
        name: "Piccolo trumpet",
        pitch: pitched(
            10,
            cited(
                59,
                91,
                cite!(
                    musescore_bb_piccolo_trumpet: "a professional range starting at 59",
                    samplemodeling_trumpet_3_manual: "a top of \"G5\" = 91 in its naming with \
                        middle C = C3, \"the playing range of a top professional player\"",
                    vsl_academy_piccolo_trumpet: "\"As a rule the scale goes up to G6\", the 8th \
                        partial with the third valve down, G6 = 91",
                ),
            ),
            Some(cited(
                62,
                86,
                cite!(
                    vsl_academy_piccolo_trumpet: "\"Range of the piccolo trumpet in high Bb: D4 - \
                        G6\", low D4 = 62",
                    musescore_bb_piccolo_trumpet: "an amateur top of 86",
                    wikipedia_de_piccolotrompete: "the lowest note \"eingestrichenes d (max. \
                        des)\", without saying written or sounding";
                    "reading the German article's lowest note, \"eingestrichenes d (max. des)\", \
                        as sounding 62 on the Bb piccolo is an estimate",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Portato,
            Sforzando,
            Fortepiano,
            MultiTongue,
            VibratoOn,
            NonVibrato,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_piccolo_trumpet: "tonguing, mute, flutter, trills, sfz and fp, \
                    staccato, portato, legato, vibrato",
                vsl_studio_piccolo_trumpet: "tonguing, mute, flutter, trills, sfz and fp, \
                    staccato, portato, legato, vibrato; \"long notes\" and a \"marcato attack\"";
                "tenuto is an estimate",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(
                1,
                1,
                "taken by a trumpeter on high Baroque parts, often at the player's discretion",
                Provenance::Cited {
                    source: cite!(
                        wikipedia_piccolo_trumpet: "high Baroque parts, often played on the \
                            piccolo trumpet at the player's discretion",
                    ),
                },
            ),
            romantic: players(
                0,
                1,
                "Rimsky-Korsakov's small trumpet in Eb or D in Mlada",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "a small trumpet in Eb or D in Mlada",
                    ),
                },
            ),
            film: players(
                0,
                1,
                "doubled by a section trumpeter",
                Provenance::Estimate {
                    reason: "no source was read",
                },
            ),
            chamber: players(
                1,
                1,
                "",
                Provenance::Estimate {
                    reason: "the repertoire for trumpet and organ; no source was read",
                },
            ),
        }),
    },
);

pub(super) const TRUMPET_C: Instrument = Instrument::new(
    InstrumentType::TrumpetC,
    InstrumentSpec {
        name: "Trumpet in C",
        pitch: pitched(
            0,
            cited(
                54,
                89,
                cite!(
                    vsl_academy_trumpet_c: "\"Range of the trumpet in C: F#3 - C6 (occasionally \
                        F6)\"; \"Trumpet virtuosos also reach notes up to F6 and higher\"",
                ),
            ),
            Some(cited(
                54,
                84,
                cite!(
                    vsl_academy_trumpet_c: "\"As a rule the scale goes up to C6\"",
                    wikipedia_trumpet: "a standard range of F#3-C6",
                    musescore_c_trumpet: "a professional range of 54-85",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Accent,
            Sforzando,
            Fortepiano,
            CrescendoSwell,
            Portato,
            Glissando,
            HalfValve,
            Cuivre,
            MultiTongue,
            VibratoOn,
            NonVibrato,
            MuteCup,
            MuteHarmon,
            MuteBucket,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth_second_scan: "the trumpet techniques",
                vsl_academy_trumpet_c: "the trumpet techniques",
                wikipedia_trumpet: "the trumpet techniques",
                wikipedia_mute: "the trumpet mutes",
                samplemodeling_trumpet_3_manual: "the controller and key-switch articulations";
                "tenuto is an estimate",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(
                2,
                2,
                "",
                Provenance::Cited {
                    source: cite!(
                        forsyth_second_scan: "p. 91: two trumpets were regularly used in symphonic \
                            music",
                    ),
                },
            ),
            romantic: players(
                2,
                3,
                "up to 5",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "two trumpets in the pairs formation, three in the threes \
                            and fours formations; Strauss and Scriabin write for up to five",
                    ),
                },
            ),
            film: players(
                3,
                4,
                "",
                Provenance::Estimate {
                    reason: "common film-orchestra practice; no source was read",
                },
            ),
            chamber: players(
                1,
                2,
                "",
                Provenance::Estimate {
                    reason: "no source was read",
                },
            ),
        }),
    },
);

pub(super) const TRUMPET_BB: Instrument = Instrument::new(
    InstrumentType::TrumpetBb,
    InstrumentSpec {
        name: "Trumpet in B♭",
        pitch: pitched(
            -2,
            cited(
                52,
                91,
                cite!(
                    samplemodeling_trumpet_3_manual: "\"Active notes of the Bb Trumpets are in the \
                        range of E2 - G5\" with middle C = C3 = MIDI 60, that is MIDI 52-91, \"the \
                        playing range of a top professional player\"",
                ),
            ),
            Some(cited(
                52,
                82,
                cite!(
                    wikipedia_trumpet: "a standard range of written F#3-C6, sounding a whole step \
                        lower: 52-82",
                    forsyth_second_scan: "pp. 95-96: low F# to high C (the octaves, inferred from \
                        the prose, are an estimate)",
                    vsl_academy_trumpet_c: "\"as a rule the scale goes up to C6\"",
                    wikipedia_cs_trubka: "the written range ends at c3, written C6, the 8th \
                        harmonic",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Accent,
            Sforzando,
            Fortepiano,
            CrescendoSwell,
            Portato,
            Glissando,
            Portamento,
            HalfValve,
            Fall,
            Doit,
            Shake,
            Scoop,
            Rip,
            Growl,
            Cuivre,
            MultiTongue,
            VibratoOn,
            NonVibrato,
            MuteCup,
            MuteHarmon,
            MutePlunger,
            MuteBucket,
            WahWah,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth_second_scan: "the trumpet techniques",
                vsl_academy_trumpet_c: "the trumpet techniques",
                wikipedia_trumpet: "the trumpet techniques",
                wikipedia_mute: "the cup, harmon, plunger and bucket mutes",
                apple_logic_studio_horns: "the jazz articulations: falls, doits, shakes, scoops",
                apple_logic_studio_horns_keyswitches: "the key switches of those articulations",
                samplemodeling_trumpet_3_manual: "the controller and key-switch articulations",
                vsco2_ce: "the sampled trumpet articulations",
                sonatina_symphonic_orchestra: "the sampled trumpet articulations";
                "tenuto and rip are estimates",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(
                2,
                2,
                "",
                Provenance::Cited {
                    source: cite!(
                        forsyth_second_scan: "p. 91: \"Two Trumpets were regularly used in \
                            Symphonic music\"",
                    ),
                },
            ),
            romantic: players(
                2,
                3,
                "up to 5 (Strauss, Scriabin)",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "two trumpets in the pairs formation, three in the threes \
                            and fours formations (the threes alternatively two trumpets and two \
                            cornets); Strauss and Scriabin write for up to five",
                    ),
                },
            ),
            film: players(
                3,
                4,
                "",
                Provenance::Estimate {
                    reason: "common film-orchestra practice; no source was read",
                },
            ),
            chamber: players(
                1,
                2,
                "",
                Provenance::Estimate {
                    reason: "brass quintet practice; no source was read",
                },
            ),
        }),
    },
);

pub(super) const CORNET: Instrument = Instrument::new(
    InstrumentType::Cornet,
    InstrumentSpec {
        name: "Cornet in B♭",
        pitch: pitched(
            -2,
            cited(
                52,
                91,
                cite!(
                    samplemodeling_trumpet_3_manual: "\"Cornet E2 - G5\" with middle C = C3 = MIDI \
                        60, that is 52-91, \"the playing range of a top professional player\"",
                ),
            ),
            Some(cited(
                52,
                82,
                cite!(
                    forsyth_second_scan: "pp. 103-104: written compass low F# to top C, the Bb \
                        cornet sounding a tone lower: 52-82",
                    musescore_bb_cornet: "a professional range of 52-82",
                    yamaha_brass_family: "\"the same fingerings and range as a Bb trumpet\"",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Accent,
            Portato,
            MultiTongue,
            Glissando,
            HalfValve,
            Fall,
            Shake,
            Growl,
            VibratoOn,
            MuteCup,
            MuteHarmon,
            MutePlunger,
            WahWah,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth_second_scan: "tonguing, shakes, mute",
                vsl_studio_cornet: "staccato, portato, legato, flutter, repetitions",
                samplemodeling_trumpet_3_manual: "one controller and key-switch scheme for the \
                    whole package, which includes the cornet: falls, shakes, growl, mutes";
                "tenuto, the cup, harmon and plunger mutes and wah-wah on the cornet are estimates",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: not_used(
                "",
                Provenance::Estimate {
                    reason: reason!(
                        "the valved cornet is younger than the Classical repertoire: ",
                        forsyth_second_scan,
                        " calls it \"a youth not yet a hundred years old\" in 1914",
                    ),
                },
            ),
            romantic: players(
                2,
                2,
                "French and Russian scores, with or instead of trumpets",
                Provenance::Cited {
                    source: cite!(
                        forsyth_second_scan: "pp. 107-108: two cornets in French and Russian \
                            scores, with or instead of trumpets",
                        rimsky_korsakov: "two cornets with or instead of trumpets",
                    ),
                },
            ),
            film: players(
                0,
                1,
                "",
                Provenance::Estimate {
                    reason: "no source was read",
                },
            ),
            chamber: players(
                1,
                1,
                "",
                Provenance::Estimate {
                    reason: "no source was read",
                },
            ),
        }),
    },
);

pub(super) const FLUGELHORN: Instrument = Instrument::new(
    InstrumentType::Flugelhorn,
    InstrumentSpec {
        name: "Flugelhorn",
        pitch: pitched(
            -2,
            cited(
                52,
                84,
                cite!(
                    samplemodeling_trumpet_3_manual: "\"German Trumpet, Flugelhorn: E2 - C5\" with \
                        middle C = C3 = MIDI 60, that is 52-84",
                ),
            ),
            Some(cited(
                52,
                82,
                cite!(
                    forsyth_second_scan: "p. 165: the Bb flugel and the Bb cornet are \"precisely \
                        the same\" in compass and notation: 52-82",
                    musescore_flugelhorn: "a professional range of 52-82",
                    wikipedia_fr_bugle: "the same register as the Bb trumpet",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            Portato,
            MultiTongue,
            VibratoOn,
            Fall,
            Shake,
            Glissando,
            HalfValve,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_studio_flugelhorn: "long and short notes, legato, flutter tongue, trills, \
                    staccato, portato, repetitions",
                samplemodeling_trumpet_3_manual: "one controller scheme for the package, which \
                    includes the flugelhorn";
                "tenuto, marcato and the jazz effects are estimates",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: not_used(
                "",
                Provenance::Estimate {
                    reason: "not in the Classical orchestra; no source was read",
                },
            ),
            romantic: not_used(
                "",
                Provenance::Cited {
                    source: cite!(
                        forsyth_second_scan: "p. 163: saxhorns and flugelhorns \"have little \
                            orchestral significance\" in England",
                    ),
                },
            ),
            film: players(
                1,
                1,
                "doubled by a trumpeter",
                Provenance::Estimate {
                    reason: "no source was read",
                },
            ),
            chamber: players(
                1,
                1,
                "",
                Provenance::Estimate {
                    reason: "jazz combo or brass ensemble doubling; no source was read",
                },
            ),
        }),
    },
);

pub(super) const TROMBONE_TENOR: Instrument = Instrument::new(
    InstrumentType::TromboneTenor,
    InstrumentSpec {
        name: "Tenor trombone",
        pitch: pitched(
            0,
            cited(
                40,
                77,
                cite!(
                    vsl_academy_tenor_trombone: "E2-F5",
                    wikipedia_trombone: "lowest E2 without pedal notes; practical top F5",
                ),
            ),
            Some(cited(
                40,
                74,
                cite!(
                    wikipedia_trombone: "\"more conservatively D5\"",
                    wikipedia_cs_pozoun: "c2, C5 = 72, as the textbook top",
                    forsyth: "confine orchestral writing to Bb4 (70)";
                    "disputed: the Czech article gives c2 (C5 = 72) as the textbook top, and \
                        Forsyth (1914) advises confining orchestral writing to Bb4 (70)",
                ),
            )),
        ),
        clefs: &[Clef::Bass, Clef::Tenor, Clef::Alto],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Glissando,
            Staccatissimo,
            Sforzando,
            Fortepiano,
            CrescendoSwell,
            Rip,
            Fall,
            Doit,
            Cuivre,
            VibratoOn,
            Growl,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_tenor_trombone: "tonguing, flutter, vibrato, lip and slide trills, \
                    mutes, legato, slide glissando, sfz and fp",
                wikipedia_trombone: "the rules of the glissando; mutes",
                spitfire_studio_brass_pro: "the solo and a2 articulation lists, including rip, \
                    fall, cuivre and swell",
                spitfire_studio_brass_manual: "the solo and a2 articulation lists",
                apple_logic_10_4_studio_horns: "falls and doits",
                apple_logic_10_4_studio_horns_keyswitches: "growl on C#1 = 25 and doit on C2 = 36 \
                    for the lead instruments, among them Trombone 1",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(
                0,
                3,
                "an alto, tenor and bass trio in opera and church music and in Beethoven's \
                    symphonies No. 5 (finale) and No. 9; the alto was 'practically obsolete' by \
                    1914",
                PLAYERS_ESTIMATE,
            ),
            romantic: players(
                2,
                3,
                "2 tenors and 1 bass (France: 3 tenors)",
                PLAYERS_ESTIMATE,
            ),
            film: players(2, 3, "tenors, plus 1-2 bass trombones", PLAYERS_ESTIMATE),
            chamber: players(0, 1, "brass quintet", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const TROMBONE_BASS: Instrument = Instrument::new(
    InstrumentType::TromboneBass,
    InstrumentSpec {
        name: "Bass trombone",
        pitch: pitched(
            0,
            cited(
                22,
                77,
                cite!(
                    vsl_academy_bass_trombone: "Bb0-F5",
                    wikipedia_bass_trombone: "Bb0 with two valves; the top only as \"at least \
                        Bb4\"",
                    yamaha_trombone_guide: "Bb0 with two valves",
                ),
            ),
            Some(estimate(
                31,
                67,
                reason!(
                    "the low G1 lies in the pedal register that ",
                    vsl_academy_bass_trombone,
                    " calls easy on this bore and covers Berlioz's G#1 (",
                    wikipedia_bass_trombone,
                    "); the top G4 (67) is a major third below Kodaly's \"high B4\" (71), which \
                        the same article singles out as notable; the Sonatina samples cover 28-67 \
                        (",
                    sonatina_symphonic_orchestra,
                    "); ",
                    forsyth,
                    " (1914, the G bass) finds neither the pedal notes below low G# nor any notes \
                        above the high G of practical use, read as G#1-G4 (32-67) with derived \
                        octaves; his floor applies to the G bass with D attachment, while the \
                        modern two-valve Bb instrument is chromatic down to Bb0, so G1 is kept",
                ),
            )),
        ),
        clefs: &[Clef::Bass, Clef::Tenor],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Glissando,
            Staccatissimo,
            Sforzando,
            Fortepiano,
            CrescendoSwell,
            Rip,
            Fall,
            Cuivre,
            VibratoOn,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_bass_trombone: "tonguing, flutter, vibrato, trills, mute, slurs, \
                    glissando",
                spitfire_studio_brass_pro: "the bass trombone solo and a2 lists",
                spitfire_studio_brass_manual: "the bass trombone solo and a2 lists",
                orchestral_tools_berlin_brass: "the bass trombone articulations",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(
                1,
                1,
                "with alto and tenor, mainly in opera and church music",
                PLAYERS_ESTIMATE,
            ),
            romantic: players(
                0,
                1,
                "the 3rd trombone; in France often none after about 1830",
                PLAYERS_ESTIMATE,
            ),
            film: players(1, 2, "Spitfire records a2", PLAYERS_ESTIMATE),
            chamber: players(0, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const EUPHONIUM: Instrument = Instrument::new(
    InstrumentType::Euphonium,
    InstrumentSpec {
        name: "Euphonium",
        pitch: pitched(
            0,
            cited(
                23,
                74,
                cite!(
                    wikipedia_euphonium: "in contemporary solo repertoire the full working range \
                        takes in the pedals from Bb1 down to B0 and reaches at least D5 \
                        (paraphrased); without pedals the floor is B1 (35) on a compensating \
                        four-valve instrument, C2 (36) on a non-compensating one, E2 (40) on three \
                        valves",
                ),
            ),
            Some(cited(
                40,
                67,
                cite!(
                    wikipedia_euphonium: "\"The range from E2 to about G4 is easily accessible\"",
                ),
            )),
        ),
        clefs: &[Clef::Bass, Clef::Tenor, Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Staccatissimo,
            Sforzando,
            CrescendoSwell,
            VibratoOn,
            HalfValve,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                spitfire_studio_brass_pro: "the euphonium patches: FX Swell, Long, Long Sfz, Short \
                    Marcato, Staccatissimo, Tenuto, Legato",
                forsyth: "the muted tenor tuba in Strauss's Don Quixote, played on the euphonium";
                "trill, flutter tongue and half valve are estimates by analogy with the tuba \
                    (Forsyth; VSL, bass tuba); vibrato is an estimate from brass-band soloist \
                    practice",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: players(
                0,
                1,
                "as 'tenor tuba' (Strauss, from 1898)",
                PLAYERS_ESTIMATE,
            ),
            film: players(0, 1, "occasional colour", PLAYERS_ESTIMATE),
            chamber: players(
                1,
                2,
                "1 in brass ensembles; 2 in a British brass band",
                PLAYERS_ESTIMATE,
            ),
        }),
    },
);

pub(super) const TUBA: Instrument = Instrument::new(
    InstrumentType::Tuba,
    InstrumentSpec {
        name: "Tuba",
        pitch: pitched(
            0,
            cited(
                21,
                67,
                cite!(
                    vsl_academy_contrabass_tuba: "the low A0 of the contrabass tuba in Bb",
                    vsl_academy_bass_tuba: "the high G4 of the bass tuba in F";
                    "a composite over the sizes this type covers",
                ),
            ),
            Some(cited(
                29,
                60,
                cite!(
                    wikipedia_tuba: "F1 to C4, middle C, is easy to reach on every size of tuba \
                        (paraphrased)",
                ),
            )),
        ),
        clefs: &[Clef::Bass],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            FlutterTongue,
            ConSordino,
            Staccatissimo,
            Sforzando,
            Fortepiano,
            CrescendoSwell,
            Rip,
            Fall,
            HalfValve,
            VibratoOn,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_bass_tuba: "tonguing, flutter, staccato, vibrato, no true glissando, \
                    valve trills, mutes, legato, sforzato",
                forsyth: "shakes with pistons, mute",
                spitfire_studio_brass_pro: "the tuba list",
                spitfire_studio_brass_manual: "the tuba list",
                orchestral_tools_berlin_brass: "the tuba articulations, including trills",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used(
                "the chromatic bass tuba was patented in 1835",
                PLAYERS_ESTIMATE,
            ),
            romantic: players(1, 2, "a second in large works", PLAYERS_ESTIMATE),
            film: players(1, 2, "often with a cimbasso", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "brass quintet", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const CIMBASSO: Instrument = Instrument::new(
    InstrumentType::Cimbasso,
    InstrumentSpec {
        name: "Cimbasso",
        pitch: pitched(
            0,
            cited(
                29,
                65,
                cite!(
                    vsl_academy_cimbasso: "F1-F4, cimbasso in F",
                ),
            ),
            Some(estimate(
                30,
                60,
                reason!(
                    "the bottom leaves out F1, which ",
                    vsl_academy_cimbasso,
                    " calls \"the only pedal tone\" and pedal tones are not used as a rule, so F#1 \
                        (30) is the lowest valve note; the top is kept a fourth below the same \
                        source's F4 limit, because the part is a fourth-trombone bass line",
                ),
            )),
        ),
        clefs: &[Clef::Bass],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            FlutterTongue,
            ConSordino,
            Staccatissimo,
            Sforzando,
            CrescendoSwell,
            Cuivre,
            Rip,
            Fall,
            HalfValve,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_cimbasso: "single tonguing to MM 130, limited vibrato, double and \
                    triple tonguing difficult, flutter easy, no true glissando, mutes",
                spitfire_studio_brass_pro: "the cimbasso patches: Fall, FX Swell, Long, Long \
                    Cuivre, Rip, Short Marcato, Short Sfz, Staccatissimo, Tenuto, no legato patch",
                spitfire_studio_brass_manual: "the cimbasso list";
                "legato itself is an estimate (the legato heading of the VSL page is empty)",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: players(
                0,
                1,
                "1 in late Verdi and Puccini opera (as 'trombone basso')",
                PLAYERS_ESTIMATE,
            ),
            film: players(1, 1, "often beside a tuba", PLAYERS_ESTIMATE),
            chamber: not_used("", PLAYERS_ESTIMATE),
        }),
    },
);
