//! Brass records.

use super::sources::{cite, reason};
use super::{
    PLAYERS_ESTIMATE, cited, estimate, gm_program, gm_substitute, not_used, pitched, players,
    unstated,
};
use crate::orchestration::articulation::Articulation::{
    Accent, BellsUp, ConSordino, CrescendoSwell, Cuivre, Doit, Fall, FlutterTongue, Fortepiano,
    Glissando, Growl, HalfValve, Legato, Marcato, MultiTongue, Multiphonic, MuteBucket, MuteCup,
    MuteHarmon, MutePlunger, NonVibrato, Portamento, Portato, Rip, Scoop, Sforzando, Shake,
    Staccatissimo, Staccato, Stopped, Sustain, Tenuto, Trill, TrillHalf, TrillWhole, VibratoOn,
    WahWah,
};
use crate::orchestration::ensemble_profile::{EnsembleProfile, Forms};
use crate::orchestration::general_midi::SubstituteFit;
use crate::orchestration::id::{CategoryId, InstrumentType};
use crate::orchestration::instrument::{Instrument, InstrumentSpec, Provenance};
use crate::orchestration::names::Names;
use crate::orchestration::pitch::Clef;
use crate::orchestration::playability::{Fact, FactKind, Span};

/// Putting on and taking off a brass mute, which no source that was read times.
const MUTE_CHANGE: Fact = Fact {
    kind: FactKind::MuteChange {
        on: Span::ModerateBeats(8),
        off: Span::ModerateBeats(8),
    },
    provenance: Provenance::Estimate {
        reason: reason!(
            "no source that was read gives the time a brass player needs for a mute; about two \
                bars of moderate time each way, as Forsyth (",
            forsyth_second_scan,
            ") p. 374 allows for fixing a string mute",
        ),
    },
};

/// The longest note a trumpeter holds on one breath, from Widor's experiments.
const TRUMPET_BREATH: Fact = Fact {
    kind: FactKind::Breath {
        held_note: Span::ModerateBeats(48),
    },
    provenance: Provenance::Cited {
        source: cite!(
            widor: "p. 70: piano, in moderate tempo (crotchet = 120), the trumpet holds a note \
                of 8 or 9 bars in the low register and of 12 or 14 bars in the medium register; \
                half as long forte";
            "the fact keeps the 12 bars, at four beats to the bar; Widor's trumpet is the \
                trumpet in F of his day, taken here for the trumpets in C and Bb"
        ),
    },
};

// Aliases: never the bare Czech "roh", which also means a corner.
pub(super) const HORN_F: Instrument = Instrument::new(
    InstrumentType::HornF,
    InstrumentSpec {
        names: Names {
            en: "Horn in F",
            cs: "lesní roh",
            de: "Horn",
            fr: "cor",
            it: "corno",
            abbreviation: "Hn.",
        },
        aliases: aliases![
            En Singular: "horn in f", "horn", "french horn";
            En Plural: "horns in f", "horns", "french horns";
            Cs Singular: "lesni roh", "lesniho rohu", "lesnimu rohu", "lesnim rohu", "lesnim rohem";
            Cs Plural: "lesni rohy", "lesnich rohu", "lesnim rohum", "lesnich rozich",
                "lesnimi rohy";
            De Singular: "horn", "waldhorn", "ventilhorn";
            De Plural: "horner", "waldhorner", "ventilhorner";
            Fr Singular: "cor", "cor d harmonie", "cor a pistons";
            Fr Plural: "cors";
            It Singular: "corno", "corno ventile";
            It Plural: "corni";
        ],
        category: None,
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
        keyboard: None,
        facts: &[MUTE_CHANGE],
        gm: gm_program(61),
    },
);

pub(super) const WAGNER_TUBA_TENOR: Instrument = Instrument::new(
    InstrumentType::WagnerTubaTenor,
    InstrumentSpec {
        names: Names {
            en: "Tenor Wagner tuba",
            cs: "tenorová Wagnerova tuba",
            de: "Tenor-Wagnertuba",
            fr: "tuba wagnérien ténor",
            it: "tuba wagneriana tenore",
            abbreviation: "Ten. Tb.",
        },
        aliases: aliases![
            En Singular: "tenor wagner tuba";
            En Plural: "tenor wagner tubas";
            En Unmarked: "ten tb";
            Cs Singular: "tenorova wagnerova tuba", "tenorove wagnerove tube",
                "tenorovou wagnerovu tubu", "tenorovou wagnerovou tubou";
            Cs Unmarked: "tenorove wagnerovy tuby";
            Cs Plural: "tenorovych wagnerovych tub", "tenorovym wagnerovym tubam",
                "tenorovych wagnerovych tubach", "tenorovymi wagnerovymi tubami";
            De Singular: "tenor wagnertuba", "tenortuba";
            De Plural: "tenor wagnertuben", "tenortuben";
            Fr Singular: "tuba wagnerien tenor";
            Fr Plural: "tubas wagneriens tenors";
            It Singular: "tuba wagneriana tenore";
            It Plural: "tube wagneriane tenori";
        ],
        category: Some(CategoryId::WagnerTuba),
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
        keyboard: None,
        facts: &[MUTE_CHANGE],
        gm: gm_substitute(
            61,
            SubstituteFit::Weak,
            reason!(
                "GM1 has no Wagner tuba: its brass programs are 57 Trumpet to 64 SynthBrass 2 (",
                midi_gm1_sound_set_2021_01,
                "). 61 French Horn shares the horn mouthpiece and players and is closer than 59 \
                    Tuba, but it is a different instrument",
            ),
        ),
    },
);

pub(super) const WAGNER_TUBA_BASS: Instrument = Instrument::new(
    InstrumentType::WagnerTubaBass,
    InstrumentSpec {
        names: Names {
            en: "Bass Wagner tuba",
            cs: "basová Wagnerova tuba",
            de: "Bass-Wagnertuba",
            fr: "tuba wagnérien basse",
            it: "tuba wagneriana bassa",
            abbreviation: "B. Tb.",
        },
        aliases: aliases![
            En Singular: "bass wagner tuba";
            En Plural: "bass wagner tubas";
            En Unmarked: "b tb";
            Cs Singular: "basova wagnerova tuba", "basove wagnerove tube", "basovou wagnerovu tubu",
                "basovou wagnerovou tubou";
            Cs Unmarked: "basove wagnerovy tuby";
            Cs Plural: "basovych wagnerovych tub", "basovym wagnerovym tubam",
                "basovych wagnerovych tubach", "basovymi wagnerovymi tubami";
            De Singular: "bass wagnertuba", "basstuba";
            De Plural: "bass wagnertuben", "basstuben";
            Fr Singular: "tuba wagnerien basse";
            Fr Plural: "tubas wagneriens basses";
            It Singular: "tuba wagneriana bassa";
            It Plural: "tube wagneriane basse";
        ],
        category: Some(CategoryId::WagnerTuba),
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
        keyboard: None,
        facts: &[MUTE_CHANGE],
        gm: gm_substitute(
            61,
            SubstituteFit::Weak,
            reason!(
                "GM1 has no Wagner tuba (",
                midi_gm1_sound_set_2021_01,
                "). 61 French Horn shares the horn mouthpiece and players; 59 Tuba would \
                    misrepresent the mouthpiece and the timbre",
            ),
        ),
    },
);

pub(super) const TRUMPET_PICCOLO: Instrument = Instrument::new(
    InstrumentType::TrumpetPiccolo,
    InstrumentSpec {
        names: Names {
            en: "Piccolo trumpet",
            cs: "piccolo trubka",
            de: "Piccolotrompete",
            fr: "trompette piccolo",
            it: "tromba piccola",
            abbreviation: "P. Tpt. B♭",
        },
        aliases: aliases![
            En Singular: "piccolo trumpet";
            En Plural: "piccolo trumpets";
            En Unmarked: "p tpt bb";
            Cs Singular: "piccolo trubka", "piccolo trubce", "piccolo trubku", "piccolo trubkou";
            Cs Unmarked: "piccolo trubky";
            Cs Plural: "piccolo trubek", "piccolo trubkam", "piccolo trubkach", "piccolo trubkami";
            De Singular: "piccolotrompete", "pikkolotrompete", "kleine trompete";
            De Plural: "piccolotrompeten", "pikkolotrompeten";
            Fr Singular: "trompette piccolo";
            Fr Plural: "trompettes piccolos";
            It Singular: "tromba piccola", "trombino";
            It Plural: "trombe piccole";
        ],
        category: Some(CategoryId::Trumpet),
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
        keyboard: None,
        facts: &[MUTE_CHANGE],
        gm: gm_substitute(
            57,
            SubstituteFit::Approximate,
            reason!(
                "GM1 has no piccolo trumpet; 57 Trumpet is the nearest program (",
                midi_gm1_specification,
                ")",
            ),
        ),
    },
);

pub(super) const TRUMPET_C: Instrument = Instrument::new(
    InstrumentType::TrumpetC,
    InstrumentSpec {
        names: Names {
            en: "Trumpet in C",
            cs: "trubka in C",
            de: "Trompete in C",
            fr: "trompette en ut",
            it: "tromba in do",
            abbreviation: "C Tpt.",
        },
        aliases: aliases![
            En Singular: "trumpet in c", "c trumpet";
            En Plural: "trumpets in c", "c trumpets";
            En Unmarked: "c tpt";
            Cs Singular: "trubka in c", "trubce in c", "trubku in c", "trubkou in c", "c trubka",
                "c trubce", "c trubku", "c trubkou";
            Cs Unmarked: "trubky in c", "c trubky";
            Cs Plural: "trubek in c", "trubkam in c", "trubkach in c", "trubkami in c", "c trubek",
                "c trubkam", "c trubkach", "c trubkami";
            De Singular: "trompete in c", "c trompete";
            De Plural: "trompeten in c", "c trompeten";
            Fr Singular: "trompette en ut";
            Fr Plural: "trompettes en ut";
            It Singular: "tromba in do";
            It Plural: "trombe in do";
        ],
        category: Some(CategoryId::Trumpet),
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
        keyboard: None,
        facts: &[TRUMPET_BREATH, MUTE_CHANGE],
        gm: gm_program(57),
    },
);

pub(super) const TRUMPET_BB: Instrument = Instrument::new(
    InstrumentType::TrumpetBb,
    InstrumentSpec {
        names: Names {
            en: "Trumpet in B♭",
            cs: "trubka in B",
            de: "Trompete in B",
            fr: "trompette en si♭",
            it: "tromba in si♭",
            abbreviation: "B♭ Tpt.",
        },
        aliases: aliases![
            En Singular: "trumpet in bb", "trumpet in b flat", "bb trumpet", "b flat trumpet";
            En Plural: "trumpets in bb", "trumpets in b flat", "bb trumpets", "b flat trumpets";
            En Unmarked: "bb tpt";
            Cs Singular: "trubka in b", "trubce in b", "trubku in b", "trubkou in b", "b trubka",
                "b trubce", "b trubku", "b trubkou";
            Cs Unmarked: "trubky in b", "b trubky";
            Cs Plural: "trubek in b", "trubkam in b", "trubkach in b", "trubkami in b", "b trubek",
                "b trubkam", "b trubkach", "b trubkami";
            De Singular: "trompete in b", "b trompete";
            De Plural: "trompeten in b", "b trompeten";
            Fr Singular: "trompette en sib";
            Fr Plural: "trompettes en sib";
            It Singular: "tromba in sib";
            It Plural: "trombe in sib";
        ],
        category: Some(CategoryId::Trumpet),
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
        keyboard: None,
        facts: &[TRUMPET_BREATH, MUTE_CHANGE],
        gm: gm_program(57),
    },
);

pub(super) const CORNET: Instrument = Instrument::new(
    InstrumentType::Cornet,
    InstrumentSpec {
        names: Names {
            en: "Cornet in B♭",
            cs: "kornet",
            de: "Kornett",
            fr: "cornet à pistons",
            it: "cornetta",
            abbreviation: "B♭ Cnt.",
        },
        aliases: aliases![
            En Singular: "cornet in bb", "cornet";
            En Plural: "cornets in bb", "cornets";
            En Unmarked: "bb cnt";
            Cs Singular: "kornet", "kornetem";
            Cs Unmarked: "kornetu";
            Cs Plural: "kornety", "kornetum", "kornetech";
            De Singular: "kornett";
            De Plural: "kornette";
            Fr Singular: "cornet a pistons";
            Fr Plural: "cornets a pistons";
            It Singular: "cornetta";
            It Plural: "cornette";
        ],
        category: None,
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
        keyboard: None,
        facts: &[MUTE_CHANGE],
        gm: gm_substitute(
            57,
            SubstituteFit::Approximate,
            reason!(
                "GM1 has no cornet; 57 Trumpet is the nearest program (",
                midi_gm1_specification,
                ")",
            ),
        ),
    },
);

pub(super) const FLUGELHORN: Instrument = Instrument::new(
    InstrumentType::Flugelhorn,
    InstrumentSpec {
        names: Names {
            en: "Flugelhorn",
            cs: "křídlovka",
            de: "Flügelhorn",
            fr: "bugle",
            it: "flicorno soprano",
            abbreviation: "Flghn.",
        },
        aliases: aliases![
            En Singular: "flugelhorn";
            En Plural: "flugelhorns";
            En Unmarked: "flghn";
            Cs Singular: "kridlovka", "kridlovce", "kridlovku", "kridlovkou";
            Cs Unmarked: "kridlovky";
            Cs Plural: "kridlovek", "kridlovkam", "kridlovkach", "kridlovkami";
            De Singular: "flugelhorn";
            De Plural: "flugelhorner";
            Fr Singular: "bugle";
            Fr Plural: "bugles";
            It Singular: "flicorno soprano", "flicorno";
            It Plural: "flicorni soprani", "flicorni";
        ],
        category: None,
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
        keyboard: None,
        facts: &[],
        gm: gm_substitute(
            57,
            SubstituteFit::Weak,
            reason!(
                "GM1 has no flugelhorn (",
                midi_gm1_specification,
                "). 57 Trumpet is the nearest by family, with 61 French Horn the alternative for \
                    its soft colour",
            ),
        ),
    },
);

pub(super) const TROMBONE_TENOR: Instrument = Instrument::new(
    InstrumentType::TromboneTenor,
    InstrumentSpec {
        names: Names {
            en: "Tenor trombone",
            cs: "tenorový pozoun",
            de: "Tenorposaune",
            fr: "trombone ténor",
            it: "trombone tenore",
            abbreviation: "Tbn.",
        },
        aliases: aliases![
            En Singular: "tenor trombone";
            En Plural: "tenor trombones";
            Cs Singular: "tenorovy pozoun", "tenoroveho pozounu", "tenorovemu pozounu",
                "tenorovem pozounu", "tenorovym pozounem", "tenorovy trombon",
                "tenoroveho trombonu", "tenorovemu trombonu", "tenorovem trombonu",
                "tenorovym trombonem";
            Cs Plural: "tenorove pozouny", "tenorovych pozounu", "tenorovym pozounum",
                "tenorovych pozounech", "tenorovymi pozouny", "tenorove trombony",
                "tenorovych trombonu", "tenorovym trombonum", "tenorovych trombonech",
                "tenorovymi trombony";
            De Singular: "tenorposaune";
            De Plural: "tenorposaunen";
            Fr Singular: "trombone tenor";
            Fr Plural: "trombones tenors";
            It Singular: "trombone tenore";
            It Plural: "tromboni tenori";
        ],
        category: Some(CategoryId::Trombone),
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
        keyboard: None,
        facts: &[MUTE_CHANGE],
        gm: gm_program(58),
    },
);

pub(super) const TROMBONE_BASS: Instrument = Instrument::new(
    InstrumentType::TromboneBass,
    InstrumentSpec {
        names: Names {
            en: "Bass trombone",
            cs: "basový pozoun",
            de: "Bassposaune",
            fr: "trombone basse",
            it: "trombone basso",
            abbreviation: "B. Tbn.",
        },
        aliases: aliases![
            En Singular: "bass trombone";
            En Plural: "bass trombones";
            En Unmarked: "b tbn";
            Cs Singular: "basovy pozoun", "basoveho pozounu", "basovemu pozounu", "basovem pozounu",
                "basovym pozounem", "basovy trombon", "basoveho trombonu", "basovemu trombonu",
                "basovem trombonu", "basovym trombonem";
            Cs Plural: "basove pozouny", "basovych pozounu", "basovym pozounum",
                "basovych pozounech", "basovymi pozouny", "basove trombony", "basovych trombonu",
                "basovym trombonum", "basovych trombonech", "basovymi trombony";
            De Singular: "bassposaune";
            De Plural: "bassposaunen";
            Fr Singular: "trombone basse";
            Fr Plural: "trombones basses";
            It Singular: "trombone basso";
            It Plural: "tromboni bassi";
        ],
        category: Some(CategoryId::Trombone),
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
        keyboard: None,
        facts: &[MUTE_CHANGE],
        gm: gm_program(58),
    },
);

pub(super) const EUPHONIUM: Instrument = Instrument::new(
    InstrumentType::Euphonium,
    InstrumentSpec {
        names: Names {
            en: "Euphonium",
            cs: "eufonium",
            de: "Euphonium",
            fr: "saxhorn basse",
            it: "eufonio",
            abbreviation: "Euph.",
        },
        aliases: aliases![
            En Singular: "euphonium", "tenor tuba";
            En Plural: "euphoniums", "tenor tubas";
            En Unmarked: "euph";
            Cs Singular: "eufonium", "eufoniu", "eufoniem";
            Cs Unmarked: "eufonia";
            Cs Plural: "eufonii", "eufoniim", "eufoniich";
            De Singular: "euphonium", "tenortuba", "tenorbasshorn";
            De Plural: "euphonien", "tenortuben";
            Fr Singular: "saxhorn basse", "tuba basse";
            Fr Plural: "saxhorns basses";
            It Singular: "eufonio";
            It Plural: "eufoni";
        ],
        category: None,
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
        keyboard: None,
        facts: &[MUTE_CHANGE],
        gm: gm_substitute(
            59,
            SubstituteFit::Approximate,
            reason!(
                "GM1 has no euphonium (",
                wikipedia_general_midi,
                "; ",
                printmusic_2010_gm_table,
                "). 59 Tuba is the nearest conical, tuba-family timbre, while 58 Trombone \
                    matches the register better",
            ),
        ),
    },
);

pub(super) const TUBA: Instrument = Instrument::new(
    InstrumentType::Tuba,
    InstrumentSpec {
        names: Names {
            en: "Tuba",
            cs: "tuba",
            de: "Tuba",
            fr: "tuba",
            it: "tuba",
            abbreviation: "Tba.",
        },
        aliases: aliases![
            En Singular: "tuba", "bass tuba", "contrabass tuba";
            En Plural: "tubas", "bass tubas", "contrabass tubas";
            Cs Singular: "tuba", "tube", "tubu", "tubou";
            Cs Unmarked: "tuby";
            Cs Plural: "tub", "tubam", "tubach", "tubami";
            De Singular: "tuba", "basstuba", "kontrabasstuba";
            De Plural: "tuben", "basstuben", "kontrabasstuben";
            Fr Singular: "tuba", "tuba basse", "tuba contrebasse";
            Fr Plural: "tubas";
            It Singular: "tuba", "tuba bassa";
            It Plural: "tube";
        ],
        category: None,
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
        keyboard: None,
        facts: &[MUTE_CHANGE],
        gm: gm_program(59),
    },
);

pub(super) const CIMBASSO: Instrument = Instrument::new(
    InstrumentType::Cimbasso,
    InstrumentSpec {
        names: Names {
            en: "Cimbasso",
            cs: "cimbasso",
            de: "Cimbasso",
            fr: "cimbasso",
            it: "cimbasso",
            abbreviation: "Cimb.",
        },
        aliases: aliases![
            En Singular: "cimbasso";
            En Unmarked: "cimb";
            Cs Unmarked: "cimbasso";
            De Singular: "cimbasso";
            Fr Singular: "cimbasso";
            It Singular: "cimbasso";
            It Plural: "cimbassi";
        ],
        category: None,
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
        keyboard: None,
        facts: &[MUTE_CHANGE],
        gm: gm_substitute(
            58,
            SubstituteFit::Approximate,
            reason!(
                "GM1 has no cimbasso: its brass group 57-64 is Trumpet, Trombone, Tuba, Muted \
                    Trumpet, French Horn, Brass Section and two synth brass (",
                wikipedia_general_midi,
                "; ",
                printmusic_2010_gm_table,
                "). 58 Trombone is the nearest, because the sound belongs to the trombone bass; \
                    59 Tuba is the second choice",
            ),
        ),
    },
);
