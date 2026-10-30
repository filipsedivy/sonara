//! Bowed string records.

use super::sources::{cite, reason};
use super::{cited, gm_program, pitched, players};
use crate::orchestration::articulation::Articulation::{
    BartokPizzicato, ColLegno, ColLegnoBattuto, ColLegnoTratto, ConSordino, CrescendoSwell,
    Flautando, Fortepiano, Glissando, Harmonics, HarmonicsArtificial, HarmonicsNatural, Legato,
    Marcato, NonVibrato, Pizzicato, Portamento, Portato, Spiccato, Staccato, SuUnaCorda,
    SulPonticello, SulTasto, Sustain, Tenuto, Tremolo, TremoloMeasured, Trill, TrillHalf,
    TrillWhole, VibratoOn,
};
use crate::orchestration::ensemble_profile::{EnsembleProfile, Forms};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::instrument::{Instrument, InstrumentSpec, Provenance};
use crate::orchestration::names::Names;
use crate::orchestration::pitch::Clef;

pub(super) const VIOLIN: Instrument = Instrument::new(
    InstrumentType::Violin,
    InstrumentSpec {
        names: Names {
            en: "Violin",
            cs: "housle",
            de: "Violine",
            fr: "violon",
            it: "violino",
            abbreviation: "Vn.",
        },
        aliases: aliases![
            En Singular: "violin", "fiddle";
            En Plural: "violins", "fiddles", "vns";
            En Unmarked: "vln";
            Cs Unmarked: "housle", "housli", "houslim", "houslich", "houslemi";
            De Singular: "violine", "geige";
            De Plural: "violinen", "geigen";
            Fr Singular: "violon";
            Fr Plural: "violons";
            It Singular: "violino";
            It Plural: "violini";
        ],
        category: None,
        pitch: pitched(
            0,
            cited(
                55,
                105,
                cite!(
                    vsl_academy_violin: "range G3-A7, harmonics to D8",
                    wikipedia_violin: "lowest G3; up to C8 by stopping at the end of the \
                        fingerboard or with artificial harmonics",
                ),
            ),
            Some(cited(
                55,
                93,
                cite!(
                    forsyth_second_scan: "p. 306: simplify passages above the top A, A6 (read from \
                        the scan)",
                    rimsky_korsakov: "a practical extreme limit of A6, read from the engraved \
                        example (the transcription prints A7)",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Spiccato,
            Tenuto,
            Marcato,
            Pizzicato,
            Tremolo,
            Trill,
            ColLegno,
            Harmonics,
            SulPonticello,
            ConSordino,
            Portato,
            SulTasto,
            Flautando,
            BartokPizzicato,
            ColLegnoBattuto,
            ColLegnoTratto,
            TremoloMeasured,
            HarmonicsNatural,
            HarmonicsArtificial,
            TrillHalf,
            TrillWhole,
            Portamento,
            Glissando,
            Fortepiano,
            CrescendoSwell,
            NonVibrato,
            VibratoOn,
            SuUnaCorda,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_violin: "the technique and notation lists",
                spitfire_symphony_orchestra_manual: "the patch list, including Marcato Attack and \
                    Legato",
                sonatina_symphonic_orchestra: "the sampled articulations",
                apple_logic_ipad_studio_strings_keyswitches: "the key switches of Studio Strings",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(
                12,
                20,
                "about 6-10 per violin part, first and second",
                Provenance::Estimate {
                    reason: "no source that was read states the section sizes of Haydn or Mozart",
                },
            ),
            romantic: players(
                30,
                32,
                "16 first and 14 second violins in the full orchestra; 16 and 16 in Wagner's Die \
                    Walkure",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "16 first and 14 second violins in the full orchestra",
                        wikipedia_string_section: "16 and 16 in Wagner's Die Walkure",
                    ),
                },
            ),
            film: players(
                30,
                30,
                "16 first and 14 second violins, as recorded for Spitfire Symphony Orchestra",
                Provenance::Cited {
                    source: cite!(
                        spitfire_symphony_orchestra_manual: "16 first and 14 second violins",
                    ),
                },
            ),
            chamber: players(
                1,
                2,
                "1 per part; a string quartet is two violins, viola and cello",
                Provenance::Cited {
                    source: cite!(
                        vsl_academy_viola: "a string quartet: two violins, viola, cello",
                        wikipedia_violin: "a string quartet: two violins, viola, cello",
                    ),
                },
            ),
        }),
        keyboard: None,
        gm: gm_program(41),
    },
);

pub(super) const VIOLA: Instrument = Instrument::new(
    InstrumentType::Viola,
    InstrumentSpec {
        names: Names {
            en: "Viola",
            cs: "viola",
            de: "Bratsche",
            fr: "alto",
            it: "viola",
            abbreviation: "Va.",
        },
        aliases: aliases![
            En Singular: "viola";
            En Plural: "violas";
            En Unmarked: "vla";
            Cs Singular: "viola", "viole", "violu", "violou";
            Cs Unmarked: "violy";
            Cs Plural: "viol", "violam", "violach", "violami";
            De Singular: "bratsche", "viola";
            De Plural: "bratschen", "violen";
            Fr Singular: "alto";
            Fr Plural: "altos";
            It Singular: "viola";
            It Plural: "viole";
        ],
        category: None,
        pitch: pitched(
            0,
            cited(
                48,
                93,
                cite!(
                    vsl_academy_viola: "range C3-A6, harmonics to E7",
                ),
            ),
            Some(cited(
                48,
                81,
                cite!(
                    rimsky_korsakov: "a practical extreme limit of A5 (the text and the engraved \
                        example agree; the alternative text of the web edition says A7)",
                    forsyth_second_scan: "E6 as the orchestral maximum and about C6 for ordinary \
                        routine",
                    vsl_academy_viola: "C3-G5 as the characteristic range",
                ),
            )),
        ),
        clefs: &[Clef::Alto, Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Spiccato,
            Tenuto,
            Marcato,
            Pizzicato,
            Tremolo,
            Trill,
            ColLegno,
            Harmonics,
            SulPonticello,
            ConSordino,
            Portato,
            SulTasto,
            Flautando,
            BartokPizzicato,
            ColLegnoBattuto,
            ColLegnoTratto,
            TremoloMeasured,
            HarmonicsNatural,
            HarmonicsArtificial,
            TrillHalf,
            TrillWhole,
            Portamento,
            Glissando,
            Fortepiano,
            CrescendoSwell,
            NonVibrato,
            VibratoOn,
            SuUnaCorda,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_viola: "the technique list",
                forsyth_second_scan: "the technique of the violin, a fifth lower",
                spitfire_symphony_orchestra_manual: "the violas patch list",
                sonatina_symphonic_orchestra: "the violas and the solo viola",
                apple_logic_ipad_studio_strings_keyswitches: "the key switches of Studio Strings",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(
                4,
                6,
                "",
                Provenance::Estimate {
                    reason: "no source that was read states the Classical section size",
                },
            ),
            romantic: players(
                4,
                12,
                "12 in a full, 8 in a medium and 4 in a small orchestra",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "12 violas in a full, 8 in a medium and 4 in a small \
                            orchestra",
                    ),
                },
            ),
            film: players(
                12,
                12,
                "as recorded for Spitfire Symphony Orchestra",
                Provenance::Cited {
                    source: cite!(
                        spitfire_symphony_orchestra_manual: "12 violas",
                    ),
                },
            ),
            chamber: players(
                1,
                1,
                "in a string quartet",
                Provenance::Cited {
                    source: cite!(
                        vsl_academy_viola: "one viola in a string quartet",
                    ),
                },
            ),
        }),
        keyboard: None,
        gm: gm_program(42),
    },
);

pub(super) const CELLO: Instrument = Instrument::new(
    InstrumentType::Cello,
    InstrumentSpec {
        names: Names {
            en: "Cello",
            cs: "violoncello",
            de: "Violoncello",
            fr: "violoncelle",
            it: "violoncello",
            abbreviation: "Vc.",
        },
        aliases: aliases![
            En Singular: "cello", "violoncello";
            En Plural: "cellos", "violoncellos";
            En Unmarked: "vlc";
            Cs Singular: "violoncello", "violoncellu", "violoncellem", "cello", "cellu", "cellem";
            Cs Unmarked: "violoncella", "cella";
            Cs Plural: "violoncell", "violoncellum", "violoncellech", "violoncelly", "cell",
                "cellum", "cellech", "celly";
            De Singular: "violoncello", "cello";
            De Plural: "violoncelli", "celli", "cellos";
            Fr Singular: "violoncelle";
            Fr Plural: "violoncelles";
            It Singular: "violoncello";
            It Plural: "violoncelli";
        ],
        category: None,
        pitch: pitched(
            0,
            cited(
                36,
                88,
                cite!(
                    widor_figures: "the lowest note C2; Widor's cello concerto reaches E6",
                    vsl_academy_cello: "C2-A5 stopped, harmonics to A7",
                ),
            ),
            Some(cited(
                36,
                76,
                cite!(
                    widor_figures: "E5 is the highest note recommended for orchestral use",
                    forsyth_second_scan: "p. 415: E5 a reasonable upward limit (read from the \
                        scan)",
                ),
            )),
        ),
        clefs: &[Clef::Bass, Clef::Tenor, Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Spiccato,
            Tenuto,
            Marcato,
            Pizzicato,
            Tremolo,
            Trill,
            ColLegno,
            Harmonics,
            SulPonticello,
            ConSordino,
            Portato,
            SulTasto,
            Flautando,
            BartokPizzicato,
            ColLegnoBattuto,
            ColLegnoTratto,
            TremoloMeasured,
            HarmonicsNatural,
            HarmonicsArtificial,
            TrillHalf,
            TrillWhole,
            Portamento,
            Glissando,
            Fortepiano,
            CrescendoSwell,
            NonVibrato,
            VibratoOn,
            SuUnaCorda,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_cello: "the technique list",
                widor_figures: "pizzicato, double stops, harmonics",
                spitfire_symphony_orchestra_manual: "the celli patch list",
                sonatina_symphonic_orchestra: "the celli and the solo cello",
                apple_logic_ipad_studio_strings_keyswitches: "the key switches of Studio Strings",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(
                3,
                6,
                "often reading one 'Bassi' part with the double basses",
                Provenance::Estimate {
                    reason: reason!(
                        "the count is an estimate; the shared Bassi part is stated by ",
                        wikipedia_string_section,
                    ),
                },
            ),
            romantic: players(
                3,
                12,
                "10 in a full, 6 in a medium and 3 in a small orchestra; 12 in Wagner's Die \
                    Walkure",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "10 cellos in a full, 6 in a medium and 3 in a small \
                            orchestra",
                        wikipedia_string_section: "12 in Wagner's Die Walkure",
                    ),
                },
            ),
            film: players(
                10,
                10,
                "as recorded for Spitfire Symphony Orchestra",
                Provenance::Cited {
                    source: cite!(
                        spitfire_symphony_orchestra_manual: "10 cellos",
                    ),
                },
            ),
            chamber: players(
                1,
                1,
                "in a string quartet",
                Provenance::Cited {
                    source: cite!(
                        vsl_academy_viola: "one cello in a string quartet",
                    ),
                },
            ),
        }),
        keyboard: None,
        gm: gm_program(43),
    },
);

pub(super) const DOUBLE_BASS: Instrument = Instrument::new(
    InstrumentType::DoubleBass,
    InstrumentSpec {
        names: Names {
            en: "Double bass",
            cs: "kontrabas",
            de: "Kontrabass",
            fr: "contrebasse",
            it: "contrabbasso",
            abbreviation: "Cb.",
        },
        aliases: aliases![
            En Singular: "double bass", "contrabass", "string bass", "upright bass", "bass";
            En Plural: "double basses", "contrabasses", "string basses", "upright basses", "basses";
            Cs Singular: "kontrabas", "kontrabasem";
            Cs Unmarked: "kontrabasu";
            Cs Plural: "kontrabasy", "kontrabasum", "kontrabasech";
            De Singular: "kontrabass";
            De Plural: "kontrabasse";
            Fr Singular: "contrebasse";
            Fr Plural: "contrebasses";
            It Singular: "contrabbasso";
            It Plural: "contrabbassi";
        ],
        category: None,
        pitch: pitched(
            -12,
            cited(
                23,
                67,
                cite!(
                    vsl_academy_double_bass: "B0-G4 sounding, B0 on the five-string bass; \
                        harmonics to G6",
                    wikipedia_double_bass: "B0 on five-string basses",
                ),
            ),
            Some(cited(
                28,
                55,
                cite!(
                    wikipedia_double_bass: "Classical orchestral parts rarely exceed E1-G3",
                    rimsky_korsakov: "a practical extreme limit of written G4, sounding G3 (read \
                        from the engraved example)",
                ),
            )),
        ),
        clefs: &[Clef::Bass, Clef::Tenor, Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Spiccato,
            Tenuto,
            Marcato,
            Pizzicato,
            Tremolo,
            Trill,
            ColLegno,
            Harmonics,
            SulPonticello,
            ConSordino,
            Portato,
            SulTasto,
            Flautando,
            BartokPizzicato,
            ColLegnoBattuto,
            ColLegnoTratto,
            TremoloMeasured,
            HarmonicsNatural,
            HarmonicsArtificial,
            TrillHalf,
            TrillWhole,
            Portamento,
            Glissando,
            Fortepiano,
            CrescendoSwell,
            NonVibrato,
            VibratoOn,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_double_bass: "the technique list, including the jazz slap",
                forsyth_second_scan: "harmonics, chords, pizzicato",
                spitfire_symphony_orchestra_manual: "the basses patch list",
                sonatina_symphonic_orchestra: "the basses and the solo bass",
                apple_logic_ipad_studio_strings_keyswitches: "the key switches of Studio Strings",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(
                2,
                4,
                "reading the shared 'Bassi' part an octave below the cellos",
                Provenance::Estimate {
                    reason: reason!(
                        "the count is an estimate; the shared Bassi part is stated by ",
                        wikipedia_string_section,
                        " and ",
                        forsyth_second_scan,
                    ),
                },
            ),
            romantic: players(
                2,
                10,
                "8-10 in a full, 4-6 in a medium and 2-3 in a small orchestra; 8 in Wagner's Die \
                    Walkure",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "8-10 double basses in a full, 4-6 in a medium and 2-3 in \
                            a small orchestra",
                        wikipedia_string_section: "8 in Wagner's Die Walkure",
                    ),
                },
            ),
            film: players(
                8,
                8,
                "as recorded for Spitfire Symphony Orchestra, all with low C extensions",
                Provenance::Cited {
                    source: cite!(
                        spitfire_symphony_orchestra_manual: "8 double basses, all with low C \
                            extensions",
                    ),
                },
            ),
            chamber: players(
                1,
                1,
                "when present: string quintet, octets",
                Provenance::Estimate {
                    reason: "no source that was read lists chamber works",
                },
            ),
        }),
        keyboard: None,
        gm: gm_program(44),
    },
);
