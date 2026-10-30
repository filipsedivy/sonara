//! Woodwind records.

use super::sources::{cite, reason};
use super::{
    PLAYERS_ESTIMATE, cited, estimate, gm_program, gm_substitute, not_used, pitched, players,
    unstated,
};
use crate::orchestration::articulation::Articulation::{
    AirTone, Bisbigliando, ConSordino, CrescendoSwell, Doit, Fall, FlutterTongue, Fortepiano,
    Glissando, Growl, Harmonics, KeyClick, Legato, Marcato, MultiTongue, Multiphonic, NonVibrato,
    Portato, Rip, Scoop, Sforzando, SlapTongue, Staccatissimo, Staccato, Sustain, Tenuto, Trill,
    TrillHalf, TrillWhole, VibratoOn,
};
use crate::orchestration::ensemble_profile::{EnsembleProfile, Forms};
use crate::orchestration::general_midi::SubstituteFit;
use crate::orchestration::id::{CategoryId, InstrumentType};
use crate::orchestration::instrument::{Instrument, InstrumentSpec, Provenance};
use crate::orchestration::names::Names;
use crate::orchestration::pitch::Clef;

pub(super) const PICCOLO: Instrument = Instrument::new(
    InstrumentType::Piccolo,
    InstrumentSpec {
        names: Names {
            en: "Piccolo",
            cs: "pikola",
            de: "Kleine Flöte",
            fr: "petite flûte",
            it: "ottavino",
            abbreviation: "Picc.",
        },
        aliases: aliases![
            En Singular: "piccolo", "piccolo flute";
            En Plural: "piccolos", "piccolo flutes";
            En Unmarked: "picc";
            Cs Singular: "pikola", "pikole", "pikolu", "pikolou", "mala fletna", "male fletne",
                "malou fletnu", "malou fletnou";
            Cs Unmarked: "pikoly", "male fletny";
            Cs Plural: "pikol", "pikolam", "pikolach", "pikolami", "malych fleten", "malym fletnam",
                "malych fletnach", "malymi fletnami";
            De Singular: "kleine flote", "pikkoloflote", "piccoloflote";
            De Plural: "kleine floten", "pikkolofloten", "piccolofloten";
            Fr Singular: "petite flute", "piccolo";
            Fr Plural: "petites flutes";
            It Singular: "ottavino", "flauto piccolo";
            It Plural: "ottavini", "flauti piccoli";
        ],
        category: None,
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
        gm: gm_program(73),
    },
);

pub(super) const FLUTE: Instrument = Instrument::new(
    InstrumentType::Flute,
    InstrumentSpec {
        names: Names {
            en: "Flute",
            cs: "flétna",
            de: "Große Flöte",
            fr: "flûte",
            it: "flauto",
            abbreviation: "Fl.",
        },
        aliases: aliases![
            En Singular: "flute", "concert flute";
            En Plural: "flutes", "concert flutes";
            Cs Singular: "fletna", "fletne", "fletnu", "fletnou", "pricna fletna", "pricne fletne",
                "pricnou fletnu", "pricnou fletnou";
            Cs Unmarked: "fletny", "pricne fletny";
            Cs Plural: "fleten", "fletnam", "fletnach", "fletnami", "pricnych fleten",
                "pricnym fletnam", "pricnych fletnach", "pricnymi fletnami";
            De Singular: "grosse flote", "flote", "querflote";
            De Plural: "grosse floten", "floten", "querfloten";
            Fr Singular: "flute", "flute traversiere";
            Fr Plural: "flutes", "flutes traversieres";
            It Singular: "flauto", "flauto traverso";
            It Plural: "flauti", "flauti traversi";
        ],
        category: None,
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
        gm: gm_program(74),
    },
);

pub(super) const ALTO_FLUTE: Instrument = Instrument::new(
    InstrumentType::AltoFlute,
    InstrumentSpec {
        names: Names {
            en: "Alto flute",
            cs: "altová flétna",
            de: "Altflöte",
            fr: "flûte alto",
            it: "flauto contralto",
            abbreviation: "Alto Fl.",
        },
        aliases: aliases![
            En Singular: "alto flute";
            En Plural: "alto flutes";
            En Unmarked: "alto fl";
            Cs Singular: "altova fletna", "altove fletne", "altovou fletnu", "altovou fletnou";
            Cs Unmarked: "altove fletny";
            Cs Plural: "altovych fleten", "altovym fletnam", "altovych fletnach",
                "altovymi fletnami";
            De Singular: "altflote";
            De Plural: "altfloten";
            Fr Singular: "flute alto", "flute en sol";
            Fr Plural: "flutes altos";
            It Singular: "flauto contralto", "flautone";
            It Plural: "flauti contralti";
        ],
        category: None,
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
        gm: gm_substitute(
            74,
            SubstituteFit::Approximate,
            reason!(
                "the GM1 program list has no alto flute; its pipe group runs from 73 Piccolo to 80 \
                    Ocarina (",
                wikipedia_general_midi,
                "). 74 Flute is the nearest, played at sounding pitch",
            ),
        ),
    },
);

pub(super) const BASS_FLUTE: Instrument = Instrument::new(
    InstrumentType::BassFlute,
    InstrumentSpec {
        names: Names {
            en: "Bass flute",
            cs: "basová flétna",
            de: "Bassquerflöte",
            fr: "flûte basse",
            it: "flauto basso",
            abbreviation: "B. Fl.",
        },
        aliases: aliases![
            En Singular: "bass flute";
            En Plural: "bass flutes";
            En Unmarked: "b fl";
            Cs Singular: "basova fletna", "basove fletne", "basovou fletnu", "basovou fletnou";
            Cs Unmarked: "basove fletny";
            Cs Plural: "basovych fleten", "basovym fletnam", "basovych fletnach",
                "basovymi fletnami";
            De Singular: "bassquerflote", "bassflote";
            De Plural: "bassquerfloten", "bassfloten";
            Fr Singular: "flute basse";
            Fr Plural: "flutes basses";
            It Singular: "flauto basso";
            It Plural: "flauti bassi";
        ],
        category: None,
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
        gm: gm_substitute(
            74,
            SubstituteFit::Approximate,
            reason!(
                "the GM1 program list has no bass flute (",
                wikipedia_general_midi,
                "). 74 Flute is the nearest, played at sounding pitch; a GM flute may not reach \
                    the lowest notes",
            ),
        ),
    },
);

pub(super) const OBOE: Instrument = Instrument::new(
    InstrumentType::Oboe,
    InstrumentSpec {
        names: Names {
            en: "Oboe",
            cs: "hoboj",
            de: "Oboe",
            fr: "hautbois",
            it: "oboe",
            abbreviation: "Ob.",
        },
        aliases: aliases![
            En Singular: "oboe", "hautboy";
            En Plural: "oboes", "hautboys";
            Cs Singular: "hoboj", "hobojem";
            Cs Unmarked: "hoboje", "hoboji";
            Cs Plural: "hoboju", "hobojum", "hobojich";
            De Singular: "oboe";
            De Plural: "oboen";
            Fr Unmarked: "hautbois";
            It Singular: "oboe";
            It Plural: "oboi";
        ],
        category: None,
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
        gm: gm_program(69),
    },
);

pub(super) const OBOE_DAMORE: Instrument = Instrument::new(
    InstrumentType::OboeDamore,
    InstrumentSpec {
        names: Names {
            en: "Oboe d'amore",
            cs: "hoboj d'amore",
            de: "Oboe d'amore",
            fr: "hautbois d'amour",
            it: "oboe d'amore",
            abbreviation: "Ob. d'am.",
        },
        aliases: aliases![
            En Singular: "oboe d amore";
            En Plural: "oboes d amore";
            En Unmarked: "ob d am";
            Cs Singular: "hoboj d amore", "hobojem d amore";
            Cs Unmarked: "hoboje d amore", "hoboji d amore";
            Cs Plural: "hoboju d amore", "hobojum d amore", "hobojich d amore";
            De Singular: "oboe d amore";
            De Plural: "oboen d amore";
            Fr Unmarked: "hautbois d amour";
            It Singular: "oboe d amore";
            It Plural: "oboi d amore";
        ],
        category: None,
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
        gm: gm_substitute(
            69,
            SubstituteFit::Approximate,
            reason!(
                "GM1 has no oboe d'amore: its reed group lists only the saxophones, Oboe, English \
                    Horn, Bassoon and Clarinet (",
                midi_gm1_sound_set_2021_10,
                "). 69 Oboe is the nearest, played at sounding pitch",
            ),
        ),
    },
);

pub(super) const ENGLISH_HORN: Instrument = Instrument::new(
    InstrumentType::EnglishHorn,
    InstrumentSpec {
        names: Names {
            en: "English horn",
            cs: "anglický roh",
            de: "Englischhorn",
            fr: "cor anglais",
            it: "corno inglese",
            abbreviation: "Eng. horn",
        },
        aliases: aliases![
            En Singular: "english horn", "cor anglais";
            En Plural: "english horns", "cors anglais";
            En Unmarked: "eng horn", "c ingl";
            Cs Singular: "anglicky roh", "anglickeho rohu", "anglickemu rohu", "anglickem rohu",
                "anglickym rohem";
            Cs Plural: "anglicke rohy", "anglickych rohu", "anglickym rohum", "anglickych rozich",
                "anglickymi rohy";
            De Singular: "englischhorn", "englisches horn";
            De Plural: "englischhorner", "englische horner";
            Fr Singular: "cor anglais";
            Fr Plural: "cors anglais";
            It Singular: "corno inglese";
            It Plural: "corni inglesi";
        ],
        category: None,
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
        gm: gm_program(70),
    },
);

pub(super) const CLARINET_EB: Instrument = Instrument::new(
    InstrumentType::ClarinetEb,
    InstrumentSpec {
        names: Names {
            en: "E♭ clarinet",
            cs: "Es klarinet",
            de: "Es-Klarinette",
            fr: "petite clarinette en mi♭",
            it: "clarinetto piccolo in mi♭",
            abbreviation: "Cl. picc. (E♭)",
        },
        aliases: aliases![
            En Singular: "eb clarinet", "e flat clarinet";
            En Plural: "eb clarinets", "e flat clarinets";
            En Unmarked: "cl picc eb";
            Cs Singular: "es klarinet", "es klarinetem";
            Cs Unmarked: "es klarinetu";
            Cs Plural: "es klarinety", "es klarinetum", "es klarinetech";
            De Singular: "es klarinette";
            De Plural: "es klarinetten";
            Fr Singular: "petite clarinette en mib", "petite clarinette";
            Fr Plural: "petites clarinettes";
            It Singular: "clarinetto piccolo in mib", "clarinetto piccolo", "quartino";
            It Plural: "clarinetti piccoli";
        ],
        category: Some(CategoryId::Clarinet),
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
        gm: gm_substitute(
            72,
            SubstituteFit::Approximate,
            reason!(
                "GM1 has no E-flat or other sopranino clarinet; 72 Clarinet is the nearest \
                    program (",
                wikipedia_general_midi,
                ")",
            ),
        ),
    },
);

pub(super) const CLARINET_BB: Instrument = Instrument::new(
    InstrumentType::ClarinetBb,
    InstrumentSpec {
        names: Names {
            en: "Clarinet in B♭",
            cs: "klarinet in B",
            de: "Klarinette in B",
            fr: "clarinette en si♭",
            it: "clarinetto in si♭",
            abbreviation: "Cl. (B♭)",
        },
        aliases: aliases![
            En Singular: "clarinet in bb", "clarinet in b flat", "bb clarinet", "b flat clarinet";
            En Plural: "clarinets in bb", "clarinets in b flat", "bb clarinets", "b flat clarinets";
            En Unmarked: "cl bb";
            Cs Singular: "klarinet in b", "klarinetem in b", "b klarinet", "b klarinetem";
            Cs Unmarked: "klarinetu in b", "b klarinetu";
            Cs Plural: "klarinety in b", "klarinetum in b", "klarinetech in b", "b klarinety",
                "b klarinetum", "b klarinetech";
            De Singular: "klarinette in b", "b klarinette";
            De Plural: "klarinetten in b", "b klarinetten";
            Fr Singular: "clarinette en sib";
            Fr Plural: "clarinettes en sib";
            It Singular: "clarinetto in sib";
            It Plural: "clarinetti in sib";
        ],
        category: Some(CategoryId::Clarinet),
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
        gm: gm_program(72),
    },
);

// Aliases: "A klarinet" and "a clarinet" are left out, since "a" is the Czech "and" and the
// English article.
pub(super) const CLARINET_A: Instrument = Instrument::new(
    InstrumentType::ClarinetA,
    InstrumentSpec {
        names: Names {
            en: "Clarinet in A",
            cs: "klarinet in A",
            de: "Klarinette in A",
            fr: "clarinette en la",
            it: "clarinetto in la",
            abbreviation: "Cl. (A)",
        },
        aliases: aliases![
            En Singular: "clarinet in a";
            En Plural: "clarinets in a";
            En Unmarked: "cl a";
            Cs Singular: "klarinet in a", "klarinetem in a";
            Cs Unmarked: "klarinetu in a";
            Cs Plural: "klarinety in a", "klarinetum in a", "klarinetech in a";
            De Singular: "klarinette in a", "a klarinette";
            De Plural: "klarinetten in a", "a klarinetten";
            Fr Singular: "clarinette en la";
            Fr Plural: "clarinettes en la";
            It Singular: "clarinetto in la";
            It Plural: "clarinetti in la";
        ],
        category: Some(CategoryId::Clarinet),
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
        gm: gm_program(72),
    },
);

pub(super) const BASS_CLARINET: Instrument = Instrument::new(
    InstrumentType::BassClarinet,
    InstrumentSpec {
        names: Names {
            en: "Bass clarinet",
            cs: "basklarinet",
            de: "Bassklarinette",
            fr: "clarinette basse",
            it: "clarinetto basso",
            abbreviation: "Cl. b.",
        },
        aliases: aliases![
            En Singular: "bass clarinet";
            En Plural: "bass clarinets";
            En Unmarked: "b cl";
            Cs Singular: "basklarinet", "basklarinetem", "basovy klarinet", "basoveho klarinetu",
                "basovemu klarinetu", "basovem klarinetu", "basovym klarinetem";
            Cs Unmarked: "basklarinetu";
            Cs Plural: "basklarinety", "basklarinetum", "basklarinetech", "basove klarinety",
                "basovych klarinetu", "basovym klarinetum", "basovych klarinetech",
                "basovymi klarinety";
            De Singular: "bassklarinette";
            De Plural: "bassklarinetten";
            Fr Singular: "clarinette basse";
            Fr Plural: "clarinettes basses";
            It Singular: "clarinetto basso", "clarone";
            It Plural: "clarinetti bassi", "claroni";
            It Unmarked: "cl b";
        ],
        category: Some(CategoryId::Clarinet),
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
        gm: gm_substitute(
            72,
            SubstituteFit::Approximate,
            reason!(
                "GM1 has no bass clarinet: its reed group has only 72 Clarinet and 71 Bassoon \
                    (",
                wikipedia_general_midi,
                "). 72 is the nearest timbre, 71 the alternative for the register",
            ),
        ),
    },
);

pub(super) const SAXOPHONE_SOPRANO: Instrument = Instrument::new(
    InstrumentType::SaxophoneSoprano,
    InstrumentSpec {
        names: Names {
            en: "Soprano saxophone",
            cs: "sopránový saxofon",
            de: "Sopransaxophon",
            fr: "saxophone soprano",
            it: "sassofono soprano",
            abbreviation: "S. Sax.",
        },
        aliases: aliases![
            En Singular: "soprano saxophone", "soprano sax";
            En Plural: "soprano saxophones", "soprano saxes";
            En Unmarked: "s sax";
            Cs Singular: "sopranovy saxofon", "sopranoveho saxofonu", "sopranovemu saxofonu",
                "sopranovem saxofonu", "sopranovym saxofonem", "sopransaxofon", "sopransaxofonem";
            Cs Plural: "sopranove saxofony", "sopranovych saxofonu", "sopranovym saxofonum",
                "sopranovych saxofonech", "sopranovymi saxofony", "sopransaxofony",
                "sopransaxofonum", "sopransaxofonech";
            Cs Unmarked: "sopransaxofonu";
            De Singular: "sopransaxophon";
            De Plural: "sopransaxophone";
            Fr Singular: "saxophone soprano";
            Fr Plural: "saxophones sopranos";
            It Singular: "sassofono soprano";
            It Plural: "sassofoni soprani";
        ],
        category: Some(CategoryId::Saxophone),
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
        gm: gm_program(65),
    },
);

// Aliases: "A. Sax." is left out, since it folds to "a sax", the English "a sax".
pub(super) const SAXOPHONE_ALTO: Instrument = Instrument::new(
    InstrumentType::SaxophoneAlto,
    InstrumentSpec {
        names: Names {
            en: "Alto saxophone",
            cs: "altový saxofon",
            de: "Altsaxophon",
            fr: "saxophone alto",
            it: "sassofono contralto",
            abbreviation: "A. Sax.",
        },
        aliases: aliases![
            En Singular: "alto saxophone", "alto sax";
            En Plural: "alto saxophones", "alto saxes";
            Cs Singular: "altovy saxofon", "altoveho saxofonu", "altovemu saxofonu",
                "altovem saxofonu", "altovym saxofonem", "altsaxofon", "altsaxofonem";
            Cs Plural: "altove saxofony", "altovych saxofonu", "altovym saxofonum",
                "altovych saxofonech", "altovymi saxofony", "altsaxofony", "altsaxofonum",
                "altsaxofonech";
            Cs Unmarked: "altsaxofonu";
            De Singular: "altsaxophon";
            De Plural: "altsaxophone";
            Fr Singular: "saxophone alto";
            Fr Plural: "saxophones altos";
            It Singular: "sassofono contralto", "sassofono alto";
            It Plural: "sassofoni contralti", "sassofoni alti";
        ],
        category: Some(CategoryId::Saxophone),
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
        gm: gm_program(66),
    },
);

pub(super) const SAXOPHONE_TENOR: Instrument = Instrument::new(
    InstrumentType::SaxophoneTenor,
    InstrumentSpec {
        names: Names {
            en: "Tenor saxophone",
            cs: "tenorový saxofon",
            de: "Tenorsaxophon",
            fr: "saxophone ténor",
            it: "sassofono tenore",
            abbreviation: "T. Sax.",
        },
        aliases: aliases![
            En Singular: "tenor saxophone", "tenor sax";
            En Plural: "tenor saxophones", "tenor saxes";
            En Unmarked: "t sax";
            Cs Singular: "tenorovy saxofon", "tenoroveho saxofonu", "tenorovemu saxofonu",
                "tenorovem saxofonu", "tenorovym saxofonem", "tenorsaxofon", "tenorsaxofonem";
            Cs Plural: "tenorove saxofony", "tenorovych saxofonu", "tenorovym saxofonum",
                "tenorovych saxofonech", "tenorovymi saxofony", "tenorsaxofony", "tenorsaxofonum",
                "tenorsaxofonech";
            Cs Unmarked: "tenorsaxofonu";
            De Singular: "tenorsaxophon";
            De Plural: "tenorsaxophone";
            Fr Singular: "saxophone tenor";
            Fr Plural: "saxophones tenors";
            It Singular: "sassofono tenore";
            It Plural: "sassofoni tenori";
        ],
        category: Some(CategoryId::Saxophone),
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
        gm: gm_program(67),
    },
);

pub(super) const SAXOPHONE_BARITONE: Instrument = Instrument::new(
    InstrumentType::SaxophoneBaritone,
    InstrumentSpec {
        names: Names {
            en: "Baritone saxophone",
            cs: "barytonový saxofon",
            de: "Baritonsaxophon",
            fr: "saxophone baryton",
            it: "sassofono baritono",
            abbreviation: "Bar. Sax.",
        },
        aliases: aliases![
            En Singular: "baritone saxophone", "baritone sax", "bari sax";
            En Plural: "baritone saxophones", "baritone saxes", "bari saxes";
            En Unmarked: "bar sax";
            Cs Singular: "barytonovy saxofon", "barytonoveho saxofonu", "barytonovemu saxofonu",
                "barytonovem saxofonu", "barytonovym saxofonem", "barytonsaxofon",
                "barytonsaxofonem";
            Cs Plural: "barytonove saxofony", "barytonovych saxofonu", "barytonovym saxofonum",
                "barytonovych saxofonech", "barytonovymi saxofony", "barytonsaxofony",
                "barytonsaxofonum", "barytonsaxofonech";
            Cs Unmarked: "barytonsaxofonu";
            De Singular: "baritonsaxophon";
            De Plural: "baritonsaxophone";
            Fr Singular: "saxophone baryton";
            Fr Plural: "saxophones barytons";
            It Singular: "sassofono baritono";
            It Plural: "sassofoni baritoni";
        ],
        category: Some(CategoryId::Saxophone),
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
        gm: gm_program(68),
    },
);

pub(super) const BASSOON: Instrument = Instrument::new(
    InstrumentType::Bassoon,
    InstrumentSpec {
        names: Names {
            en: "Bassoon",
            cs: "fagot",
            de: "Fagott",
            fr: "basson",
            it: "fagotto",
            abbreviation: "Fag.",
        },
        aliases: aliases![
            En Singular: "bassoon";
            En Plural: "bassoons";
            En Unmarked: "bsn";
            Cs Singular: "fagot", "fagotem";
            Cs Unmarked: "fagotu";
            Cs Plural: "fagoty", "fagotum", "fagotech";
            De Singular: "fagott";
            De Plural: "fagotte";
            Fr Singular: "basson";
            Fr Plural: "bassons";
            It Singular: "fagotto";
            It Plural: "fagotti";
            It Unmarked: "fag";
        ],
        category: None,
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
        gm: gm_program(71),
    },
);

pub(super) const CONTRABASSOON: Instrument = Instrument::new(
    InstrumentType::Contrabassoon,
    InstrumentSpec {
        names: Names {
            en: "Contrabassoon",
            cs: "kontrafagot",
            de: "Kontrafagott",
            fr: "contrebasson",
            it: "contrafagotto",
            abbreviation: "C-fag.",
        },
        aliases: aliases![
            En Singular: "contrabassoon", "double bassoon";
            En Plural: "contrabassoons", "double bassoons";
            En Unmarked: "cbsn";
            Cs Singular: "kontrafagot", "kontrafagotem";
            Cs Unmarked: "kontrafagotu";
            Cs Plural: "kontrafagoty", "kontrafagotum", "kontrafagotech";
            De Singular: "kontrafagott";
            De Plural: "kontrafagotte";
            Fr Singular: "contrebasson", "contre basson";
            Fr Plural: "contrebassons";
            It Singular: "contrafagotto";
            It Plural: "contrafagotti";
            It Unmarked: "c fag";
        ],
        category: None,
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
        gm: gm_substitute(
            71,
            SubstituteFit::Approximate,
            reason!(
                "GM1 has no contrabassoon: its reed group lists the bassoon only, and 44 \
                    Contrabass is a string (",
                midi_gm1_sound_set_2021_10,
                "). 71 Bassoon is the nearest, played at sounding pitch, though GM does not state \
                    how low a bassoon program reaches",
            ),
        ),
    },
);
