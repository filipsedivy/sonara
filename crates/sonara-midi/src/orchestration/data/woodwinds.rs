//! Woodwind records.

use super::sources::{cite, reason};
use super::{PLAYERS_ESTIMATE, cited, estimate, not_used, pitched, players, unstated};
use crate::orchestration::articulation::Articulation::{
    AirTone, Bisbigliando, ConSordino, CrescendoSwell, Doit, Fall, FlutterTongue, Fortepiano,
    Glissando, Growl, Harmonics, KeyClick, Legato, Marcato, MultiTongue, Multiphonic, NonVibrato,
    Portato, Rip, Scoop, Sforzando, SlapTongue, Staccatissimo, Staccato, Sustain, Tenuto, Trill,
    TrillHalf, TrillWhole, VibratoOn,
};
use crate::orchestration::ensemble_profile::{EnsembleProfile, Forms};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::instrument::{Instrument, InstrumentSpec, Provenance};
use crate::orchestration::pitch::Clef;

pub(super) const PICCOLO: Instrument = Instrument::new(
    InstrumentType::Piccolo,
    InstrumentSpec {
        name: "Piccolo",
        pitch: pitched(
            12,
            cited(
                74,
                108,
                cite!(
                    vsl_academy_piccolo: "the piccolo in C has a range from D5 to C8, read as \
                        sounding because VSL gives its fundamental as C5 and says it sounds an \
                        octave higher than written",
                    forsyth: "the compass begins at written low D; the top written C only in fff; \
                        sounds an octave higher",
                    wikipedia_piccolo: "tessitura D5-C8",
                ),
            ),
            Some(cited(
                80,
                106,
                cite!(
                    vsl_academy_piccolo: "the middle register G#5-C7 and the upper register are \
                        used most often, except the very highest notes; D5-G5 sound dull and \
                        rather hollow",
                    forsyth: "better to confine the upward compass to the high Bb, written Bb6, \
                        sounding Bb7",
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
            Staccatissimo,
            MultiTongue,
            Sforzando,
            Fortepiano,
            CrescendoSwell,
            Rip,
            Fall,
            KeyClick,
            AirTone,
            VibratoOn,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                spitfire_studio_woodwinds_pro: "the piccolo patch list",
                vsl_academy_piccolo: "the piccolo techniques",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(
                0,
                1,
                "played by the 2nd flute when needed",
                PLAYERS_ESTIMATE,
            ),
            romantic: players(
                1,
                1,
                "taken by the 2nd or 3rd flute; occasionally 2",
                PLAYERS_ESTIMATE,
            ),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(0, 1, "rare", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const FLUTE: Instrument = Instrument::new(
    InstrumentType::Flute,
    InstrumentSpec {
        name: "Flute",
        pitch: pitched(
            0,
            cited(
                59,
                98,
                cite!(
                    vsl_academy_concert_flute: "standard range B3-D7, forced up to F7",
                    wikipedia_western_concert_flute: "highest usually C7 or, in more modern \
                        literature, D7; the B foot extends down to B3",
                ),
            ),
            Some(cited(
                60,
                94,
                cite!(
                    widor: "of the three-octave chromatic scale of 37 degrees the first 35 can be \
                        attacked and sustained ff or piano at will; the 36th and 37th are harsh, \
                        the 37th impossible piano (the end notes, printed as images, read as \
                        C4-C7, so the 35th is Bb6)",
                    berlioz: "B and C should not be written pianissimo; Bb sustains piano",
                    forsyth: "the compass begins at middle C",
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
            Harmonics,
            Staccatissimo,
            Portato,
            MultiTongue,
            Sforzando,
            CrescendoSwell,
            Rip,
            Fall,
            Glissando,
            KeyClick,
            SlapTongue,
            AirTone,
            Multiphonic,
            VibratoOn,
            NonVibrato,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "legato, staccato, shakes, double and triple tonguing, flutter-tonguing",
                vsl_academy_concert_flute: "the flute techniques",
                spitfire_studio_woodwinds_pro: "the flute patch list";
                "portato is an estimate",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(
                1,
                2,
                "Mozart's symphonies 1; the Classical tradition 2",
                PLAYERS_ESTIMATE,
            ),
            romantic: players(
                2,
                3,
                "the 2nd or 3rd doubling piccolo or alto flute",
                PLAYERS_ESTIMATE,
            ),
            film: players(3, 3, "a3 section patches", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const ALTO_FLUTE: Instrument = Instrument::new(
    InstrumentType::AltoFlute,
    InstrumentSpec {
        name: "Alto flute",
        pitch: pitched(
            -5,
            cited(
                55,
                91,
                cite!(
                    forsyth: "fingering and written compass precisely those of the flute, sounding \
                        a perfect fourth lower; three octaves producible with ease",
                    wikipedia_alto_flute: "G3 to G6, altissimo to Db7",
                ),
            ),
            Some(estimate(
                55,
                81,
                reason!(
                    "the upper bound A5 leaves out the top sixth (about Bb5-G6), which tends to \
                        sharpness and is of poorer quality (",
                    forsyth,
                    ", whose exact limit note is an image); the pianissimo scale of the alto flute \
                        recordings stops at G5 (",
                    iowa_mis_alto_flute,
                    ")",
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
            Staccatissimo,
            Portato,
            MultiTongue,
            Sforzando,
            CrescendoSwell,
            Rip,
            Fall,
            KeyClick,
            AirTone,
            VibratoOn,
            NonVibrato,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                spitfire_studio_woodwinds_pro: "the alto flute patch list",
                vsl_studio_alto_flute: "the alto flute articulation list",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: players(0, 1, "taken by the 2nd or 3rd flute", PLAYERS_ESTIMATE),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(0, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const BASS_FLUTE: Instrument = Instrument::new(
    InstrumentType::BassFlute,
    InstrumentSpec {
        name: "Bass flute",
        pitch: pitched(
            -12,
            cited(
                47,
                84,
                cite!(
                    wikipedia_bass_flute: "sounding C3, or B2 with a B foot joint, to C6; written \
                        an octave above sounding",
                ),
            ),
            Some(cited(
                48,
                81,
                cite!(
                    wikipedia_bass_flute: "tessitura C3-A5; notes written above A6 are not often \
                        used",
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
            Staccatissimo,
            Portato,
            MultiTongue,
            Sforzando,
            CrescendoSwell,
            Rip,
            Fall,
            VibratoOn,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                spitfire_studio_woodwinds_pro: "the bass flute patch list",
                vsl_studio_bass_flute: "the bass flute articulation list",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: not_used("rare", PLAYERS_ESTIMATE),
            film: players(0, 1, "", PLAYERS_ESTIMATE),
            chamber: players(0, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const OBOE: Instrument = Instrument::new(
    InstrumentType::Oboe,
    InstrumentSpec {
        name: "Oboe",
        pitch: pitched(
            0,
            cited(
                58,
                91,
                cite!(
                    wikipedia_oboe: "Bb3 to G6",
                    vsl_academy_oboe: "Bb3 to G6, with A6 as an extreme",
                ),
            ),
            Some(cited(
                60,
                87,
                cite!(
                    wikipedia_oboe: "common tessitura C4 to Eb6",
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
            CrescendoSwell,
            TrillHalf,
            TrillWhole,
            Glissando,
            Multiphonic,
            KeyClick,
            SlapTongue,
            VibratoOn,
            NonVibrato,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth_second_scan: "the oboe techniques",
                rimsky_korsakov: "the oboe techniques",
                vsl_academy_oboe: "the oboe techniques",
                vsl_synchron_woodwinds: "the oboe articulations and key switches";
                "tenuto is an estimate from notation practice; no source read lists it",
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
                        forsyth_second_scan: "two oboes in the Classical orchestra",
                        rimsky_korsakov: "two oboes in the Classical orchestra",
                    ),
                },
            ),
            romantic: players(
                2,
                3,
                "plus the English horn (Wagner from the Ring: 3 and the English horn)",
                Provenance::Cited {
                    source: cite!(
                        forsyth_second_scan: "two or three oboes, plus the English horn",
                        rimsky_korsakov: "two or three oboes, plus the English horn",
                    ),
                },
            ),
            film: players(
                2,
                3,
                "",
                Provenance::Estimate {
                    reason: "no source on film orchestras was read",
                },
            ),
            chamber: players(
                1,
                1,
                "wind quintet, one per part",
                Provenance::Estimate {
                    reason: "no source was read",
                },
            ),
        }),
    },
);

pub(super) const OBOE_DAMORE: Instrument = Instrument::new(
    InstrumentType::OboeDamore,
    InstrumentSpec {
        name: "Oboe d'amore",
        pitch: pitched(
            -3,
            cited(
                56,
                88,
                cite!(
                    wikipedia_de_oboe_d_amore: "sounding a to e3, Ab3-E6",
                ),
            ),
            Some(cited(
                56,
                85,
                cite!(
                    forsyth_second_scan: "compass in 1914 written B3 to high E, two octaves and a \
                        fourth, no low-Bb key: sounding G#3-C#6";
                    "taking that older full compass as the comfortable range today is an estimate; \
                        no source calls it comfortable",
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
            TrillHalf,
            TrillWhole,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_synchron_oboe_d_amore: "sustain, legato, staccato, flutter tongue, trills, \
                    portato",
                forsyth_second_scan: "sustain, legato, staccato, flutter tongue, trills, portato";
                "tenuto and marcato are estimates by analogy with the oboe, whose technique \
                    Forsyth calls identical; trill_half and trill_whole are estimates, since the \
                    product page says only \"trills\"",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(
                1,
                2,
                "Baroque: 1, 2, or 2 with 2 oboi da caccia (Bach); out of use in the Classical era",
                Provenance::Cited {
                    source: cite!(
                        forsyth_second_scan: "one or two in Bach, or two with two oboi da caccia",
                        wikipedia_oboe_d_amore: "out of use in the Classical era",
                    ),
                },
            ),
            romantic: unstated(
                "rare; 1 in Strauss's Sinfonia Domestica",
                Provenance::Cited {
                    source: cite!(
                        forsyth_second_scan: "one in Strauss's Sinfonia Domestica",
                    ),
                },
            ),
            film: unstated(
                "rare",
                Provenance::Estimate {
                    reason: "no source was read",
                },
            ),
            chamber: unstated(
                "Bach cantatas and concerto",
                Provenance::Cited {
                    source: cite!(
                        wikipedia_oboe_d_amore: "Bach cantatas and concerto",
                    ),
                },
            ),
        }),
    },
);

pub(super) const ENGLISH_HORN: Instrument = Instrument::new(
    InstrumentType::EnglishHorn,
    InstrumentSpec {
        name: "English horn",
        pitch: pitched(
            -7,
            cited(
                52,
                84,
                cite!(
                    wikipedia_cor_anglais: "sounding E3 to C6; Eb3 with an optional low-Bb key",
                ),
            ),
            Some(cited(
                52,
                81,
                cite!(
                    vsl_academy_english_horn: "E3-A5",
                    forsyth_second_scan: "written B3-E6, sounding E3-A5",
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
            CrescendoSwell,
            TrillHalf,
            TrillWhole,
            Glissando,
            Bisbigliando,
            VibratoOn,
            NonVibrato,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "the English horn techniques",
                vsl_academy_english_horn: "the English horn techniques",
                vsl_synchron_woodwinds: "the English horn articulations";
                "tenuto is an estimate from notation practice",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used(
                "usually none in the standard Classical orchestra; 2 in the Schwarzenberg Harmonie \
                    of 1771; two parts in older opera, such as Gluck's Orfeo",
                Provenance::Estimate {
                    reason: reason!(
                        "no source that was read gives Classical orchestra sizes; the Harmonie of \
                            1771 is from ",
                        wikipedia_cor_anglais,
                        " and the parts in Gluck's Orfeo from ",
                        forsyth_second_scan,
                    ),
                },
            ),
            romantic: players(
                1,
                1,
                "as an extra, or taken by oboe II, III or IV",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "one English horn, an extra player or one of the oboists",
                        vsl_academy_english_horn: "one English horn, an extra player or one of the \
                            oboists",
                    ),
                },
            ),
            film: players(
                1,
                1,
                "",
                Provenance::Estimate {
                    reason: "no film source was read",
                },
            ),
            chamber: unstated(
                "trios with two oboes (Beethoven)",
                Provenance::Cited {
                    source: cite!(
                        widor: "trios with two oboes (Beethoven)",
                    ),
                },
            ),
        }),
    },
);

pub(super) const CLARINET_EB: Instrument = Instrument::new(
    InstrumentType::ClarinetEb,
    InstrumentSpec {
        name: "E♭ clarinet",
        pitch: pitched(
            3,
            cited(
                55,
                96,
                cite!(
                    wikipedia_e_flat_clarinet: "written E3-A6, sounding G3-C7",
                ),
            ),
            Some(cited(
                63,
                94,
                cite!(
                    forsyth: "a written compass equal to the bass clarinet's (three octaves and a \
                        minor third), with the bottom fifth \"equally rarely employed\"",
                    wikipedia_e_flat_clarinet: "many books give the top as written G6";
                    "the compass is taken to start on written E3 (an estimate: the compass is \
                        printed as an image), giving written E3-G6, so without the bottom fifth \
                        written C4-G6, sounding 63-94",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Trill,
            FlutterTongue,
            Staccatissimo,
            Portato,
            Sforzando,
            TrillHalf,
            TrillWhole,
            CrescendoSwell,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                orchestral_tools_berlin_woodwinds_additional: "legato, sustains, staccato, short \
                    portato, half- and whole-tone trills",
                ilio_synchron_clarinet_eb: "short and long notes, legato, flutter tongue, trills, \
                    repetitions, crescendos, diminuendos, sforzato, sforzatissimo",
                forsyth: "the \"hail-stone\" staccato";
                "staccatissimo is an estimate from Forsyth's very sharp and pointed staccato",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("rare before Berlioz", PLAYERS_ESTIMATE),
            romantic: players(
                1,
                1,
                "often doubled by the 2nd or 3rd clarinettist",
                PLAYERS_ESTIMATE,
            ),
            film: players(1, 1, "colour", PLAYERS_ESTIMATE),
            chamber: unstated("rare", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const CLARINET_BB: Instrument = Instrument::new(
    InstrumentType::ClarinetBb,
    InstrumentSpec {
        name: "Clarinet in B♭",
        pitch: pitched(
            -2,
            cited(
                50,
                94,
                cite!(
                    wikipedia_clarinet: "written E3-C7, sounding D3-Bb6",
                    forsyth: "the top written C \"just playable\"",
                    berlioz: "written low E sounds D",
                ),
            ),
            Some(cited(
                50,
                91,
                cite!(
                    widor: "42 notes, with A in altissimo as the upper limit for the great \
                        majority of players: written E3-A6, sounding 50-91",
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
            Staccatissimo,
            Portato,
            Sforzando,
            TrillHalf,
            TrillWhole,
            MultiTongue,
            Glissando,
            Growl,
            CrescendoSwell,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                spitfire_bbc_so_pro_manual: "the clarinet techniques",
                orchestral_tools_berlin_woodwinds: "clarinets 1 and 2",
                forsyth: "shakes, tremolos, kinds of tonguing",
                wikipedia_clarinet: "glissando, growling, double tongue, flutter tongue",
                berlioz: "swell and die away",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(2, 2, "each with a Bb and an A instrument", PLAYERS_ESTIMATE),
            romantic: players(
                2,
                3,
                "Bb or A, plus the Eb and the bass clarinet",
                PLAYERS_ESTIMATE,
            ),
            film: players(1, 3, "solo or an a3 section", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const CLARINET_A: Instrument = Instrument::new(
    InstrumentType::ClarinetA,
    InstrumentSpec {
        name: "Clarinet in A",
        pitch: pitched(
            -3,
            cited(
                49,
                93,
                cite!(
                    wikipedia_clarinet: "written E3-C7, sounding C#3-A6",
                    forsyth: "the same written compass as the Bb clarinet, sounding a minor third \
                        lower",
                    widor: "descends to C#",
                ),
            ),
            Some(cited(
                49,
                90,
                cite!(
                    widor: "the practical compass, written E3-A6, applies \"equally to both\" Bb \
                        and A: sounding 49-90",
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
            Staccatissimo,
            Portato,
            Sforzando,
            TrillHalf,
            TrillWhole,
            MultiTongue,
            Glissando,
            Growl,
            CrescendoSwell,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                widor: "the Bb and A clarinets \"have the same mechanism, the same qualities\"";
                "the sources of the Bb clarinet's list therefore apply; no library read samples an \
                    A clarinet separately",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(
                2,
                2,
                "the same 2 players as the Bb parts, switching instrument",
                PLAYERS_ESTIMATE,
            ),
            romantic: players(2, 3, "switching between Bb and A", PLAYERS_ESTIMATE),
            film: unstated(
                "rarely specified; Bb section patches cover it except sounding 49",
                PLAYERS_ESTIMATE,
            ),
            chamber: players(1, 1, "Mozart and Brahms quintets", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const BASS_CLARINET: Instrument = Instrument::new(
    InstrumentType::BassClarinet,
    InstrumentSpec {
        name: "Bass clarinet",
        pitch: pitched(
            -14,
            cited(
                34,
                86,
                cite!(
                    wikipedia_bass_clarinet: "written C3 or Eb3 to E7, sounding Bb1 or Db2 to D6",
                    yamaha_ycl_622ii: "an instrument to low C",
                    buffet_crampon_tosca: "a bass clarinet descending to low C",
                ),
            ),
            Some(cited(
                37,
                74,
                cite!(
                    wikipedia_bass_clarinet: "the highest note commonly encountered in modern solo \
                        literature is written E6, sounding D5 = 74; most modern Boehm bass \
                        clarinets descend to written Eb3",
                    orchestral_tools_berlin_woodwinds_additional: "a sampled top of 73-76",
                    spitfire_bbc_so_pro_manual: "a sampled top of 73-76",
                    sonatina_symphonic_orchestra: "a sampled top of 73-76";
                    "the bottom is an estimate: written Eb3, sounding 37, the extension most \
                        modern Boehm bass clarinets have, taken as the usual bottom",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Marcato,
            Trill,
            FlutterTongue,
            Staccatissimo,
            Portato,
            Sforzando,
            TrillHalf,
            TrillWhole,
            MultiTongue,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                spitfire_bbc_so_pro_manual: "legato, long, trills of a major and a minor second, \
                    staccatissimo, marcato, long flutter",
                orchestral_tools_berlin_woodwinds_additional: "legato, sustains, staccato, short \
                    staccato, short and long portato, sforzando, double and triple tonguing, half- \
                    and whole-tone trills",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: players(
                1,
                1,
                "the 3rd clarinettist doubling, or a separate 4th player",
                PLAYERS_ESTIMATE,
            ),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "rare", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const SAXOPHONE_SOPRANO: Instrument = Instrument::new(
    InstrumentType::SaxophoneSoprano,
    InstrumentSpec {
        name: "Soprano saxophone",
        pitch: pitched(
            -2,
            cited(
                56,
                94,
                cite!(
                    wikipedia_soprano_saxophone: "written low Bb3 sounds Ab3 = 56",
                    musescore_soprano_saxophone: "a professional range with altissimo up to \
                        written C7, sounding 94";
                    "no handbook read states an altissimo ceiling",
                ),
            ),
            Some(cited(
                56,
                87,
                cite!(
                    wikipedia_saxophone: "written Bb3 to the traditional top keyed F6",
                    wikipedia_soprano_saxophone: "transposition -2; the F#6 key, sounding E6 = 88, \
                        is left outside the comfortable range",
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
            Harmonics,
            Staccatissimo,
            Fortepiano,
            CrescendoSwell,
            VibratoOn,
            NonVibrato,
            TrillHalf,
            TrillWhole,
            Glissando,
            Scoop,
            Fall,
            Doit,
            Growl,
            SlapTongue,
            Multiphonic,
            KeyClick,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth_second_scan: "shakes, including half- and whole-tone trills; swells; \
                    legato",
                widor: "shakes, swells, legato",
                berlioz: "shakes, swells, legato",
                wikipedia_saxophone_technique: "jaw vibrato, flutter, slap, multiphonics, \
                    overtones, glissando, subtone, growl",
                wikipedia_growling: "growl",
                wikipedia_extended_technique: "key noise";
                "staccato, staccatissimo, tenuto, marcato and fortepiano are estimates from \
                    general wind practice, and scoop, fall and doit by analogy with the alto, \
                    because no library read covers the soprano",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("the saxophone dates from the 1840s", PLAYERS_ESTIMATE),
            romantic: players(
                0,
                1,
                "rare (Saint-Saens, d'Indy, Strauss's quartet)",
                PLAYERS_ESTIMATE,
            ),
            film: players(
                0,
                1,
                "normally a double for an alto or tenor player",
                PLAYERS_ESTIMATE,
            ),
            chamber: players(
                1,
                1,
                "the top voice of the SATB saxophone quartet",
                PLAYERS_ESTIMATE,
            ),
        }),
    },
);

pub(super) const SAXOPHONE_ALTO: Instrument = Instrument::new(
    InstrumentType::SaxophoneAlto,
    InstrumentSpec {
        name: "Alto saxophone",
        pitch: pitched(
            -9,
            cited(
                49,
                87,
                cite!(
                    wikipedia_alto_saxophone: "concert Db3 = 49",
                    musescore_alto_saxophone: "a professional range with altissimo up to written \
                        C7, sounding 87";
                    "no handbook read states an altissimo ceiling",
                ),
            ),
            Some(cited(
                49,
                80,
                cite!(
                    wikipedia_alto_saxophone: "concert Db3 to concert Ab5 = 80, the range without \
                        the high F# key",
                    wikipedia_saxophone: "written Bb3-F6",
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
            Harmonics,
            Staccatissimo,
            Fortepiano,
            CrescendoSwell,
            VibratoOn,
            NonVibrato,
            TrillHalf,
            TrillWhole,
            Glissando,
            Scoop,
            Fall,
            Doit,
            Rip,
            Growl,
            SlapTongue,
            Multiphonic,
            KeyClick,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                apple_logic_10_4_studio_horns: "growl, falls, doit, scoop, trill for saxes",
                apple_logic_10_4_studio_horns_keyswitches: "the key switches: growl, falls, doit, \
                    scoop, trill for saxes",
                native_instruments_session_horns_pro: "vibrato and non-vibrato sustains, legato \
                    intervals, semitone and whole-tone trills, growl, staccatissimo, fp crescendo, \
                    falls, doits, rip",
                wikipedia_saxophone_technique: "the saxophone techniques",
                wikipedia_growling: "growl",
                wikipedia_extended_technique: "key noise",
                forsyth_second_scan: "shakes, swells, legato",
                berlioz: "shakes, swells, legato";
                "tenuto is an estimate",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("the saxophone dates from the 1840s", PLAYERS_ESTIMATE),
            romantic: players(
                0,
                1,
                "French opera and Bizet; Strauss's quartet",
                PLAYERS_ESTIMATE,
            ),
            film: players(
                1,
                2,
                "big-band style scoring uses 2 (lead alto and second alto)",
                PLAYERS_ESTIMATE,
            ),
            chamber: players(1, 1, "in the SATB saxophone quartet", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const SAXOPHONE_TENOR: Instrument = Instrument::new(
    InstrumentType::SaxophoneTenor,
    InstrumentSpec {
        name: "Tenor saxophone",
        pitch: pitched(
            -14,
            cited(
                44,
                82,
                cite!(
                    wikipedia_tenor_saxophone: "concert Ab2 = 44",
                    musescore_tenor_saxophone: "a professional range with altissimo up to written \
                        C7, sounding 82";
                    "no handbook read states an altissimo ceiling",
                ),
            ),
            Some(cited(
                44,
                75,
                cite!(
                    wikipedia_saxophone: "written Bb3 to the traditional top keyed F6",
                    wikipedia_tenor_saxophone: "transposition -14; the F#6 key, concert E5 = 76, \
                        is left outside the comfortable range",
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
            Harmonics,
            Staccatissimo,
            Fortepiano,
            CrescendoSwell,
            VibratoOn,
            NonVibrato,
            TrillHalf,
            TrillWhole,
            Glissando,
            Scoop,
            Fall,
            Doit,
            Rip,
            Growl,
            SlapTongue,
            Multiphonic,
            KeyClick,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                apple_logic_10_4_studio_horns: "the tenor saxophone articulations",
                apple_logic_10_4_studio_horns_keyswitches: "the key switches, naming Tenor Sax 1",
                native_instruments_session_horns_pro: "the articulation chart",
                moforte_geoshred_tenor_sax: "a physical model with growl, flutter and key noise",
                vcsl: "vibrato, non-vibrato and staccato tenor samples",
                wikipedia_saxophone_technique: "the saxophone techniques",
                wikipedia_growling: "growl",
                wikipedia_extended_technique: "key noise",
                forsyth_second_scan: "shakes, swells, legato",
                berlioz: "shakes, swells, legato";
                "tenuto is an estimate",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("the saxophone dates from the 1840s", PLAYERS_ESTIMATE),
            romantic: players(0, 1, "rare (d'Indy; Strauss's quartet)", PLAYERS_ESTIMATE),
            film: players(
                1,
                2,
                "big-band style scoring uses 2 (tenor 1 is a solo chair)",
                PLAYERS_ESTIMATE,
            ),
            chamber: players(1, 1, "in the SATB saxophone quartet", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const SAXOPHONE_BARITONE: Instrument = Instrument::new(
    InstrumentType::SaxophoneBaritone,
    InstrumentSpec {
        name: "Baritone saxophone",
        pitch: pitched(
            -21,
            cited(
                36,
                75,
                cite!(
                    wikipedia_baritone_saxophone: "\"Most models have a key for low A (sounding \
                        C)\": C2 = 36",
                    musescore_baritone_saxophone: "a professional range with altissimo up to \
                        written C7, sounding 75";
                    "no handbook read states an altissimo ceiling",
                ),
            ),
            Some(cited(
                37,
                68,
                cite!(
                    musescore_baritone_saxophone: "an amateur low of written Bb3, concert Db2 = 37",
                    wikipedia_saxophone: "the traditional top keyed F6; transposition -21",
                    wikipedia_baritone_saxophone: "the low A and the F#6 key, concert A4 = 69, \
                        left outside the comfortable range",
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
            Harmonics,
            Staccatissimo,
            Fortepiano,
            CrescendoSwell,
            VibratoOn,
            NonVibrato,
            TrillHalf,
            TrillWhole,
            Glissando,
            Scoop,
            Fall,
            Doit,
            Rip,
            Growl,
            SlapTongue,
            Multiphonic,
            KeyClick,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                native_instruments_session_horns_pro: "includes the baritone; its per-instrument \
                    chart was not verified",
                wikipedia_saxophone_technique: "the saxophone techniques",
                wikipedia_growling: "growl",
                wikipedia_extended_technique: "key noise",
                forsyth_second_scan: "shakes, swells",
                berlioz: "shakes, swells";
                "tenuto is an estimate, and so is a baritone in Logic Studio Horns, which names \
                    only Alto Sax 1 and Tenor Sax 1",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("the saxophone dates from the 1840s", PLAYERS_ESTIMATE),
            romantic: players(
                0,
                1,
                "rare (Strauss's quartet, a baritone in F)",
                PLAYERS_ESTIMATE,
            ),
            film: players(0, 1, "1 in big-band style scoring", PLAYERS_ESTIMATE),
            chamber: players(
                1,
                1,
                "the bass voice of the SATB saxophone quartet",
                PLAYERS_ESTIMATE,
            ),
        }),
    },
);

pub(super) const BASSOON: Instrument = Instrument::new(
    InstrumentType::Bassoon,
    InstrumentSpec {
        name: "Bassoon",
        pitch: pitched(
            0,
            cited(
                34,
                80,
                cite!(
                    wikipedia_bassoon: "Bb1 to Ab5 at the extreme; A1 only with an extension",
                    vsl_academy_bassoon: "Bb1 to Ab5 at the extreme",
                ),
            ),
            Some(cited(
                34,
                72,
                cite!(
                    wikipedia_bassoon: "writing rarely goes above C5",
                    prout_figures: "Bb4 as the top",
                    forsyth_second_scan: "Bb4 as the top",
                    widor: "Bb4 as the top",
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
            Portato,
            Sforzando,
            CrescendoSwell,
            TrillHalf,
            TrillWhole,
            Glissando,
            Multiphonic,
            KeyClick,
            SlapTongue,
            VibratoOn,
            NonVibrato,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth_second_scan: "the bassoon techniques",
                rimsky_korsakov: "the bassoon techniques",
                vsl_academy_bassoon: "the bassoon techniques",
                vsl_synchron_woodwinds: "the bassoon articulations";
                "tenuto is an estimate from notation practice",
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
                        forsyth_second_scan: "two bassoons in the Classical orchestra",
                        vsl_academy_bassoon: "two bassoons in the Classical orchestra",
                    ),
                },
            ),
            romantic: players(
                2,
                4,
                "plus the contrabassoon (Wagner's Ring 4; Mahler 4 and the contrabassoon)",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "two to four bassoons, plus the contrabassoon",
                        vsl_academy_bassoon: "four in Wagner's Ring; four and the contrabassoon in \
                            Mahler",
                    ),
                },
            ),
            film: players(
                2,
                3,
                "",
                Provenance::Estimate {
                    reason: "no film source was read",
                },
            ),
            chamber: players(
                1,
                1,
                "wind quintet",
                Provenance::Estimate {
                    reason: "no source was read",
                },
            ),
        }),
    },
);

pub(super) const CONTRABASSOON: Instrument = Instrument::new(
    InstrumentType::Contrabassoon,
    InstrumentSpec {
        name: "Contrabassoon",
        pitch: pitched(
            -12,
            cited(
                22,
                62,
                cite!(
                    wikipedia_contrabassoon: "Bb0 to D4; A0 only with an A bell",
                    heckel_contrabassoon: "Bb0 to D4",
                ),
            ),
            Some(cited(
                22,
                57,
                cite!(
                    wikipedia_contrabassoon: "the highest fourth is rarely scored",
                    forsyth_second_scan: "the top fourth of the Heckel instrument is bad",
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
            FlutterTongue,
            Sforzando,
            Fortepiano,
            VibratoOn,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                vsl_academy_contrabassoon: "the contrabassoon techniques",
                forsyth_second_scan: "the contrabassoon techniques",
                widor: "the contrabassoon techniques";
                "tenuto and marcato are estimates from notation practice",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(
                0,
                1,
                "Haydn's Creation; Beethoven's 5th and 9th symphonies",
                Provenance::Cited {
                    source: cite!(
                        vsl_academy_contrabassoon: "Haydn's Creation; Beethoven's 5th and 9th \
                            symphonies",
                    ),
                },
            ),
            romantic: players(
                1,
                1,
                "often taken by bassoon III",
                Provenance::Cited {
                    source: cite!(
                        forsyth_second_scan: "one contrabassoon, often taken by the 3rd bassoon",
                        rimsky_korsakov: "one contrabassoon, often taken by the 3rd bassoon",
                        vsl_academy_contrabassoon: "one contrabassoon, often taken by the 3rd \
                            bassoon",
                    ),
                },
            ),
            film: players(
                1,
                1,
                "",
                Provenance::Estimate {
                    reason: "no film source was read",
                },
            ),
            chamber: unstated(
                "rare",
                Provenance::Estimate {
                    reason: "no source was read",
                },
            ),
        }),
    },
);
