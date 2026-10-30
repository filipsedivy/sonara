//! Voice records.

use super::sources::{cite, reason};
use super::{PLAYERS_ESTIMATE, cited, pitched, players, unstated};
use crate::orchestration::articulation::Articulation::{
    CrescendoSwell, Falsetto, Humming, Legato, Marcato, NonVibrato, Portamento, Sforzando,
    Staccato, Sustain, Tenuto, Text, Trill, VibratoOn, Vowel, Whisper,
};
use crate::orchestration::ensemble_profile::{EnsembleProfile, Forms};
use crate::orchestration::id::{CategoryId, InstrumentType};
use crate::orchestration::instrument::{Instrument, InstrumentSpec, Provenance};
use crate::orchestration::names::Names;
use crate::orchestration::pitch::Clef;

// Aliases: a solo voice has the singular forms and a choir section the plural ("tenor",
// "tenors"). "Ten." and "Bar." are left out, since they are also English words.
pub(super) const VOICE_SOPRANO: Instrument = Instrument::new(
    InstrumentType::VoiceSoprano,
    InstrumentSpec {
        names: Names {
            en: "Soprano",
            cs: "Soprán",
            de: "Sopran",
            fr: "Soprano",
            it: "Soprano",
            abbreviation: "Sopr.",
        },
        aliases: aliases![
            En Singular: "soprano";
            En Unmarked: "sopr";
            Cs Singular: "sopran", "sopranu", "sopranem";
            De Singular: "sopran";
            Fr Singular: "soprano";
            It Singular: "soprano";
        ],
        category: Some(CategoryId::SoloVoice),
        pitch: pitched(
            0,
            cited(
                60,
                88,
                cite!(
                    rimsky_korsakov: "table F, soloists, soprano: outer, dotted limits C4-E6 (read \
                        from the engraved table)",
                    wikipedia_voice_type: "a typical range of C4-C6, inside it",
                    wikipedia_fach: "A3 for spinto and dramatic sopranos, F6 for coloratura";
                    "the Fach classification extends to A3 (spinto, dramatic) and F6 (coloratura); \
                        recorded, not used",
                ),
            ),
            Some(cited(
                62,
                83,
                cite!(
                    rimsky_korsakov: "table F: solid exceptional D4-E4, normal bracket F4-G5, \
                        solid exceptional A5-B5 (the upper boundary read at 8x zoom)",
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
            Vowel,
            Text,
            Humming,
            Whisper,
            CrescendoSwell,
            Sforzando,
            VibratoOn,
            NonVibrato,
            Portamento,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "legato, staccato, florid passages, vowels",
                wikipedia_humming: "humming",
                soundiron_voices_of_rapture: "true legato vowels, vowel staccatos, Latin syllable \
                    sustains and staccatos for a solo soprano",
                wikipedia_symphonic_choirs: "whispers";
                "tenuto, marcato, trill, sforzando, crescendo_swell, portamento and vibrato \
                    control are estimates: general vocal practice; no source that was read lists \
                    them",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(1, 1, "", PLAYERS_ESTIMATE),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(
                1,
                1,
                "solo libraries record one singer",
                Provenance::Cited {
                    source: cite!(
                        soundiron_voices_of_rapture: "one singer per solo library",
                    ),
                },
            ),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const VOICE_MEZZO_SOPRANO: Instrument = Instrument::new(
    InstrumentType::VoiceMezzoSoprano,
    InstrumentSpec {
        names: Names {
            en: "Mezzo-soprano",
            cs: "Mezzosoprán",
            de: "Mezzosopran",
            fr: "Mezzo-soprano",
            it: "Mezzosoprano",
            abbreviation: "M.-sopr.",
        },
        aliases: aliases![
            En Singular: "mezzo soprano", "mezzo";
            En Unmarked: "m sopr";
            Cs Singular: "mezzosopran", "mezzosopranu", "mezzosopranem";
            De Singular: "mezzosopran";
            Fr Singular: "mezzo soprano";
            It Singular: "mezzosoprano";
        ],
        category: Some(CategoryId::SoloVoice),
        pitch: pitched(
            0,
            cited(
                57,
                84,
                cite!(
                    rimsky_korsakov: "table F, soloists, mezzo-soprano: outer, dotted limits A3-C6 \
                        (read from the engraved table)",
                    wikipedia_voice_type: "a typical range of A3-A5, inside it",
                    wikipedia_fach: "G3-B5";
                    "the Fach range G3-B5 is recorded, not used",
                ),
            ),
            Some(cited(
                59,
                79,
                cite!(
                    rimsky_korsakov: "table F: solid exceptional B3-C4, normal octave D4-D5, solid \
                        exceptional E5-G5 (the last solid note, G5, read by calibrating the \
                        engraving against the chorus rows, whose counts the text fixes; to within \
                        one diatonic step)",
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
            Vowel,
            Text,
            Humming,
            Whisper,
            CrescendoSwell,
            Sforzando,
            VibratoOn,
            NonVibrato,
            Portamento,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "legato, staccato, florid passages, vowels",
                wikipedia_humming: "humming";
                "the rest are estimates from general vocal practice; no library that was read \
                    records a solo mezzo-soprano",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(1, 1, "", PLAYERS_ESTIMATE),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const VOICE_ALTO: Instrument = Instrument::new(
    InstrumentType::VoiceAlto,
    InstrumentSpec {
        names: Names {
            en: "Contralto",
            cs: "Alt",
            de: "Alt",
            fr: "Contralto",
            it: "Contralto",
            abbreviation: "C.-alto",
        },
        aliases: aliases![
            En Singular: "contralto", "alto";
            En Unmarked: "c alto";
            Cs Singular: "alt", "altu", "altem";
            De Singular: "alt";
            Fr Singular: "contralto";
            It Singular: "contralto";
        ],
        category: Some(CategoryId::SoloVoice),
        pitch: pitched(
            0,
            cited(
                53,
                79,
                cite!(
                    rimsky_korsakov: "table F, soloists, contralto: outer, dotted limits F3-G5 \
                        (read from the engraved table)",
                    wikipedia_voice_type: "a typical range of F3-F5, inside it",
                    wikipedia_fach: "deep contralto C3-E5, dramatic contralto F3-A5";
                    "the Fach ranges, deep contralto C3-E5 and dramatic contralto F3-A5, are \
                        recorded, not used",
                ),
            ),
            Some(cited(
                55,
                77,
                cite!(
                    rimsky_korsakov: "table F: solid exceptional G3-A3, normal octave B3-B4, solid \
                        exceptional C5-F5 (the boundary note F5 read as solid by calibrating the \
                        engraving against the chorus rows, whose counts the text fixes; to within \
                        one diatonic step)",
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
            Vowel,
            Text,
            Humming,
            Whisper,
            CrescendoSwell,
            Sforzando,
            VibratoOn,
            NonVibrato,
            Portamento,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "legato, staccato, vowels",
                wikipedia_humming: "humming",
                soundiron_voices_of_rapture: "a solo alto: true legato vowels, vowel sustains and \
                    staccatos, Latin syllables",
                wikipedia_symphonic_choirs: "whispers by altos";
                "tenuto, marcato, trill, sforzando, crescendo_swell, portamento and vibrato \
                    control are estimates: general vocal practice; no source that was read lists \
                    them",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(1, 1, "", PLAYERS_ESTIMATE),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(
                1,
                1,
                "one singer per solo library",
                Provenance::Cited {
                    source: cite!(
                        soundiron_voices_of_rapture: "one singer per solo library",
                    ),
                },
            ),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const VOICE_TENOR: Instrument = Instrument::new(
    InstrumentType::VoiceTenor,
    InstrumentSpec {
        names: Names {
            en: "Tenor",
            cs: "Tenor",
            de: "Tenor",
            fr: "Ténor",
            it: "Tenore",
            abbreviation: "Ten.",
        },
        aliases: aliases![
            En Singular: "tenor";
            Cs Singular: "tenor", "tenoru", "tenorem", "tenora", "tenorovi";
            De Singular: "tenor";
            Fr Singular: "tenor";
            It Singular: "tenore";
        ],
        category: Some(CategoryId::SoloVoice),
        pitch: pitched(
            0,
            cited(
                48,
                74,
                cite!(
                    rimsky_korsakov: "table F, soloists, tenor: printed in treble clef as written \
                        C4-D6, sounding C3-D5 (read from the engraved table)",
                    wikipedia_tenor: "the treble-8vb clef convention; an operatic Bb2",
                    wikipedia_voice_type: "a typical range of C3-C5, inside it",
                    wikipedia_fach: "B2";
                    "reading the treble clef an octave down is an estimate from the treble-8vb \
                        convention; the operatic Bb2 and the Fach B2 are recorded, not used",
                ),
            ),
            Some(cited(
                50,
                72,
                cite!(
                    rimsky_korsakov: "table F: solid exceptional D3-E3, normal octave F3-F4, solid \
                        exceptional G4-C5, sounding (the last solid note, C5, read by calibrating \
                        the engraving against the chorus rows, whose counts the text fixes; to \
                        within one diatonic step)",
                ),
            )),
        ),
        clefs: &[Clef::Treble8vb],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Trill,
            Vowel,
            Text,
            Humming,
            Whisper,
            CrescendoSwell,
            Sforzando,
            VibratoOn,
            NonVibrato,
            Portamento,
            Falsetto,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "legato, staccato, vowels",
                berlioz: "the head voice or falsetto of tenors",
                wikipedia_humming: "humming",
                soundiron_voices_of_rapture: "a solo tenor: true legato vowels, vowel sustains and \
                    staccatos, Latin syllables";
                "tenuto, marcato, trill, sforzando, crescendo_swell, portamento, vibrato control \
                    and whisper are estimates: general vocal practice; no source that was read \
                    lists them",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(1, 1, "", PLAYERS_ESTIMATE),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(
                1,
                1,
                "one singer per solo library",
                Provenance::Cited {
                    source: cite!(
                        soundiron_voices_of_rapture: "one singer per solo library",
                    ),
                },
            ),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const VOICE_BARITONE: Instrument = Instrument::new(
    InstrumentType::VoiceBaritone,
    InstrumentSpec {
        names: Names {
            en: "Baritone",
            cs: "Baryton",
            de: "Bariton",
            fr: "Baryton",
            it: "Baritono",
            abbreviation: "Bar.",
        },
        aliases: aliases![
            En Singular: "baritone";
            Cs Singular: "baryton", "barytonu", "barytonem", "barytona", "barytonovi";
            De Singular: "bariton";
            Fr Singular: "baryton";
            It Singular: "baritono";
        ],
        category: Some(CategoryId::SoloVoice),
        pitch: pitched(
            0,
            cited(
                45,
                69,
                cite!(
                    rimsky_korsakov: "table F, soloists, baritone: outer, dotted limits A2-A4 \
                        (read from the engraved table)",
                    wikipedia_voice_type: "a typical range of A2-A4, which agrees",
                    wikipedia_fach: "G2-B4",
                    wikipedia_cs_hlasovy_obor: "G2-G4";
                    "the Fach range G2-B4 and the Czech G2-G4 are recorded, not used",
                ),
            ),
            Some(cited(
                47,
                67,
                cite!(
                    rimsky_korsakov: "table F: solid exceptional B2-C3, normal octave D3-D4, solid \
                        exceptional E4-G4 (the boundary note G4 read as solid by calibrating the \
                        engraving against the chorus rows, whose counts the text fixes; to within \
                        one diatonic step)",
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
            Vowel,
            Text,
            Humming,
            Whisper,
            CrescendoSwell,
            Sforzando,
            VibratoOn,
            NonVibrato,
            Portamento,
            Falsetto,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "legato, staccato, vowels",
                berlioz: "head notes of baritones only in light buffo style",
                wikipedia_humming: "humming";
                "the rest are estimates from general vocal practice; no library that was read \
                    records a solo baritone",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(1, 1, "", PLAYERS_ESTIMATE),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(1, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const VOICE_BASS: Instrument = Instrument::new(
    InstrumentType::VoiceBass,
    InstrumentSpec {
        names: Names {
            en: "Bass",
            cs: "Bas",
            de: "Bass",
            fr: "Basse",
            it: "Basso",
            abbreviation: "B.",
        },
        aliases: aliases![
            En Singular: "bass";
            Cs Singular: "bas", "basu", "basem", "basa", "basovi";
            De Singular: "bass";
            Fr Singular: "basse";
            It Singular: "basso";
        ],
        category: Some(CategoryId::SoloVoice),
        pitch: pitched(
            0,
            cited(
                40,
                65,
                cite!(
                    rimsky_korsakov: "table F, soloists, bass: outer, dotted limits E2-F4 (read \
                        from the engraved table)",
                    wikipedia_voice_type: "E2-E4, inside it",
                    wikipedia_bass_voice: "E2-E4; the basso profondo reaches C2",
                    wikipedia_fach: "the basso profondo reaches C2";
                    "the basso profondo C2 is recorded, not used",
                ),
            ),
            Some(cited(
                43,
                64,
                cite!(
                    rimsky_korsakov: "table F: solid exceptional G2-A2, normal octave B2-B3, solid \
                        exceptional C4-E4 (the boundary note E4 read as solid by calibrating the \
                        engraving against the chorus rows, whose counts the text fixes; to within \
                        one diatonic step)",
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
            Vowel,
            Text,
            Humming,
            Whisper,
            CrescendoSwell,
            Sforzando,
            VibratoOn,
            NonVibrato,
            Portamento,
            Falsetto,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "legato, staccato, vowels",
                berlioz: "head notes only in buffo style",
                wikipedia_humming: "humming",
                soundiron_voices_of_rapture: "a solo bass: true legato vowels, vowel sustains and \
                    staccatos, Latin syllables";
                "tenuto, marcato, trill, sforzando, crescendo_swell, portamento, vibrato control \
                    and whisper are estimates: general vocal practice; no source that was read \
                    lists them",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(1, 1, "", PLAYERS_ESTIMATE),
            romantic: players(1, 1, "", PLAYERS_ESTIMATE),
            film: players(
                1,
                1,
                "one singer per solo library",
                Provenance::Cited {
                    source: cite!(
                        soundiron_voices_of_rapture: "one singer per solo library",
                    ),
                },
            ),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

// The German section names are compounds with "Chor" ("Chorsopran"): "Sopran" alone is the
// solo voice's name.
pub(super) const CHOIR_SOPRANO: Instrument = Instrument::new(
    InstrumentType::ChoirSoprano,
    InstrumentSpec {
        names: Names {
            en: "Sopranos",
            cs: "Soprány",
            de: "Chorsopran",
            fr: "Sopranos",
            it: "Soprani",
            abbreviation: "S.",
        },
        aliases: aliases![
            En Plural: "sopranos";
            Cs Plural: "soprany", "sopranu", "sopranum", "sopranech";
            De Unmarked: "chorsopran";
            De Plural: "soprane";
            Fr Plural: "sopranos";
            It Plural: "soprani";
        ],
        category: Some(CategoryId::Choir),
        pitch: pitched(
            0,
            cited(
                60,
                84,
                cite!(
                    rimsky_korsakov: "table F, chorus, soprano: outer, dotted limits C4-C6 (read \
                        from the engraved table)",
                    berlioz: "C4-Bb5, inside it",
                ),
            ),
            Some(cited(
                62,
                81,
                cite!(
                    rimsky_korsakov: "table F: the normal octave F4-F5 plus the solid exceptional \
                        brackets D4-E4 and G5-A5",
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
            Vowel,
            Text,
            Humming,
            Whisper,
            CrescendoSwell,
            Sforzando,
            VibratoOn,
            NonVibrato,
            Portamento,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "legato, staccato (\"the rapid change from staccato to legato\"), \
                    vowels",
                wikipedia_humming: "humming",
                sound_on_sound_eric_whitacre_choir: "looped sustains, legato, \"mmm\", a breathy \
                    ooh, non-vibrato to vibrato",
                musictech_hollywood_choirs: "staccato, legato, WordBuilder text",
                epicomposer_hollywood_choirs: "staccato, legato, WordBuilder text",
                wikipedia_symphonic_choirs: "whispers";
                "tenuto, marcato, sforzando, crescendo_swell and portamento are estimates: general \
                    vocal notation practice; no source that was read lists them for choirs",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Section,
            classical: players(
                4,
                10,
                "per part; Baroque choirs were 'usually quite small'",
                Provenance::Estimate {
                    reason: reason!(
                        "4-10 per part is an estimate; that Baroque choirs were \"usually quite \
                            small\" is from ",
                        wikipedia_choir,
                    ),
                },
            ),
            romantic: players(
                8,
                32,
                "per part: a full chorus 32, a medium one 16-20, a small one 8-10; women often \
                    predominate and the firsts get more voices",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "a full chorus has 32 per part, a medium one 16-20, a \
                            small one 8-10; women often predominate and the firsts get more voices",
                    ),
                },
            ),
            film: unstated(
                "Epic Choir: 50 singers in two sections (S+A, T+B); Eric Whitacre Choir: 22 \
                    singers SATB",
                Provenance::Cited {
                    source: cite!(
                        spitfire_originals_epic_choir: "50 singers in two sections, sopranos with \
                            altos and tenors with basses",
                        sound_on_sound_eric_whitacre_choir: "22 singers, SATB",
                    ),
                },
            ),
            chamber: unstated(
                "a chamber choir has roughly 8 to 40 singers in total",
                Provenance::Cited {
                    source: cite!(
                        wikipedia_chamber_choir: "a chamber choir has roughly 8 to 40 singers in \
                            total",
                    ),
                },
            ),
        }),
    },
);

pub(super) const CHOIR_ALTO: Instrument = Instrument::new(
    InstrumentType::ChoirAlto,
    InstrumentSpec {
        names: Names {
            en: "Altos",
            cs: "Alty",
            de: "Choralt",
            fr: "Altos",
            it: "Contralti",
            abbreviation: "A.",
        },
        aliases: aliases![
            En Plural: "altos", "contraltos";
            Cs Plural: "alty", "altu", "altum", "altech";
            De Unmarked: "choralt";
            Fr Plural: "altos";
            It Plural: "contralti";
        ],
        category: Some(CategoryId::Choir),
        pitch: pitched(
            0,
            cited(
                53,
                77,
                cite!(
                    rimsky_korsakov: "table F, chorus, contralto: outer limits G3-F5, dotted E5-F5 \
                        (read from the engraved table)",
                    berlioz: "the contralto \"from the low F to the high Eb\", F3-Eb5, which \
                        widens the low end to F3",
                    wikipedia_alto: "a choral alto range of F3-F5, which agrees",
                ),
            ),
            Some(cited(
                55,
                74,
                cite!(
                    rimsky_korsakov: "table F: solid exceptional G3-A3, normal octave B3-B4, solid \
                        exceptional C5-D5",
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
            Vowel,
            Text,
            Humming,
            Whisper,
            CrescendoSwell,
            Sforzando,
            VibratoOn,
            NonVibrato,
            Portamento,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "legato, staccato, vowels",
                wikipedia_humming: "humming",
                sound_on_sound_eric_whitacre_choir: "sustains, legato, \"mmm\", non-vibrato to \
                    vibrato",
                musictech_hollywood_choirs: "staccato, legato, WordBuilder text",
                epicomposer_hollywood_choirs: "staccato, legato, WordBuilder text",
                wikipedia_symphonic_choirs: "whispers by altos";
                "tenuto, marcato, sforzando, crescendo_swell and portamento are estimates: general \
                    vocal notation practice; no source that was read lists them for choirs",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Section,
            classical: players(
                4,
                10,
                "per part; Baroque choirs were 'usually quite small'",
                Provenance::Estimate {
                    reason: reason!(
                        "4-10 per part is an estimate; that Baroque choirs were \"usually quite \
                            small\" is from ",
                        wikipedia_choir,
                    ),
                },
            ),
            romantic: players(
                8,
                32,
                "per part: a full chorus 32, a medium one 16-20, a small one 8-10",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "a full chorus has 32 per part, a medium one 16-20, a \
                            small one 8-10",
                    ),
                },
            ),
            film: unstated(
                "Epic Choir records sopranos and altos as one section of a 50-voice choir; Eric \
                    Whitacre Choir: 22 singers SATB",
                Provenance::Cited {
                    source: cite!(
                        spitfire_originals_epic_choir: "sopranos and altos as one section of a \
                            50-voice choir",
                        sound_on_sound_eric_whitacre_choir: "22 singers, SATB",
                    ),
                },
            ),
            chamber: unstated(
                "a chamber choir has roughly 8 to 40 singers in total",
                Provenance::Cited {
                    source: cite!(
                        wikipedia_chamber_choir: "a chamber choir has roughly 8 to 40 singers in \
                            total",
                    ),
                },
            ),
        }),
    },
);

pub(super) const CHOIR_TENOR: Instrument = Instrument::new(
    InstrumentType::ChoirTenor,
    InstrumentSpec {
        names: Names {
            en: "Tenors",
            cs: "Tenory",
            de: "Chortenor",
            fr: "Ténors",
            it: "Tenori",
            abbreviation: "T.",
        },
        aliases: aliases![
            En Plural: "tenors";
            Cs Plural: "tenory", "tenoru", "tenorum", "tenorech", "tenori";
            De Unmarked: "chortenor";
            De Plural: "tenore";
            Fr Plural: "tenors";
            It Plural: "tenori";
        ],
        category: Some(CategoryId::Choir),
        pitch: pitched(
            0,
            cited(
                48,
                70,
                cite!(
                    rimsky_korsakov: "table F, chorus, tenor: printed in treble clef as written \
                        C4-A5, sounding C3-A4 (read from the engraved table)",
                    berlioz: "the tenor goes \"up to the A and Bb above\"; \"the Bb must be fixed \
                        as the limit of a tenor singer's chest voice\", which widens the top to \
                        Bb4",
                    wikipedia_tenor: "the treble-8vb clef convention";
                    "reading the treble clef an octave down is an estimate from the treble-8vb \
                        convention",
                ),
            ),
            Some(cited(
                50,
                69,
                cite!(
                    rimsky_korsakov: "table F: solid exceptional D3-E3, normal octave F3-F4, solid \
                        exceptional G4-A4, sounding",
                ),
            )),
        ),
        clefs: &[Clef::Treble8vb, Clef::Bass],
        articulations: &[
            Sustain,
            Legato,
            Staccato,
            Tenuto,
            Marcato,
            Vowel,
            Text,
            Humming,
            Whisper,
            CrescendoSwell,
            Sforzando,
            VibratoOn,
            NonVibrato,
            Portamento,
            Falsetto,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "legato, staccato, vowels",
                berlioz: "the head voice or falsetto of tenors",
                wikipedia_humming: "humming",
                sound_on_sound_eric_whitacre_choir: "sustains, legato, \"mmm\"",
                musictech_hollywood_choirs: "staccato, legato, WordBuilder text",
                epicomposer_hollywood_choirs: "staccato, legato, WordBuilder text";
                "tenuto, marcato, sforzando, crescendo_swell, portamento and whisper are \
                    estimates: general vocal notation practice; no source that was read lists them \
                    for choirs",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Section,
            classical: players(
                4,
                10,
                "per part; Baroque choirs were 'usually quite small'",
                Provenance::Estimate {
                    reason: reason!(
                        "4-10 per part is an estimate; that Baroque choirs were \"usually quite \
                            small\" is from ",
                        wikipedia_choir,
                    ),
                },
            ),
            romantic: players(
                8,
                32,
                "per part: a full chorus 32, a medium one 16-20, a small one 8-10; men usually \
                    fewer than women",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "a full chorus has 32 per part, a medium one 16-20, a \
                            small one 8-10; men are usually fewer than women",
                    ),
                },
            ),
            film: unstated(
                "Epic Choir records tenors and basses as one section of a 50-voice choir; Eric \
                    Whitacre Choir: 22 singers SATB",
                Provenance::Cited {
                    source: cite!(
                        spitfire_originals_epic_choir: "tenors and basses as one section of a \
                            50-voice choir",
                        sound_on_sound_eric_whitacre_choir: "22 singers, SATB",
                    ),
                },
            ),
            chamber: unstated(
                "a chamber choir has roughly 8 to 40 singers in total",
                Provenance::Cited {
                    source: cite!(
                        wikipedia_chamber_choir: "a chamber choir has roughly 8 to 40 singers in \
                            total",
                    ),
                },
            ),
        }),
    },
);

pub(super) const CHOIR_BASS: Instrument = Instrument::new(
    InstrumentType::ChoirBass,
    InstrumentSpec {
        names: Names {
            en: "Basses",
            cs: "Basy",
            de: "Chorbass",
            fr: "Basses",
            it: "Bassi",
            abbreviation: "B.",
        },
        aliases: aliases![
            En Plural: "basses";
            Cs Plural: "basy", "basu", "basum", "basech";
            De Unmarked: "chorbass";
            De Plural: "basse";
            Fr Plural: "basses";
            It Plural: "bassi";
        ],
        category: Some(CategoryId::Choir),
        pitch: pitched(
            0,
            cited(
                36,
                66,
                cite!(
                    rimsky_korsakov: "table F, chorus, bass: outer, dotted limits C2-E4 (read from \
                        the engraved table)",
                    berlioz: "\"ten of them, at least, could give, in chest voice, a high F#\", \
                        used by Gluck for all basses, which widens the top to F#4; the \
                        North-German and Russian basso profondo \"D's and C's below the stave\" corroborate \
                        the C2 bottom",
                ),
            ),
            Some(cited(
                43,
                62,
                cite!(
                    rimsky_korsakov: "table F: solid exceptional G2-A2, normal octave B2-B3, solid \
                        exceptional C4-D4",
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
            Vowel,
            Text,
            Humming,
            Whisper,
            CrescendoSwell,
            Sforzando,
            VibratoOn,
            NonVibrato,
            Portamento,
            Falsetto,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "legato, staccato, vowels",
                berlioz: "head notes of basses only in light buffo style",
                wikipedia_humming: "humming",
                sound_on_sound_eric_whitacre_choir: "sustains, legato, \"mmm\"",
                musictech_hollywood_choirs: "staccato, legato, WordBuilder text",
                epicomposer_hollywood_choirs: "staccato, legato, WordBuilder text";
                "tenuto, marcato, sforzando, crescendo_swell, portamento and whisper are \
                    estimates: general vocal notation practice; no source that was read lists them \
                    for choirs",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Section,
            classical: players(
                4,
                10,
                "per part; Baroque choirs were 'usually quite small'",
                Provenance::Estimate {
                    reason: reason!(
                        "4-10 per part is an estimate; that Baroque choirs were \"usually quite \
                            small\" is from ",
                        wikipedia_choir,
                    ),
                },
            ),
            romantic: players(
                8,
                32,
                "per part: a full chorus 32, a medium one 16-20, a small one 8-10",
                Provenance::Cited {
                    source: cite!(
                        rimsky_korsakov: "a full chorus has 32 per part, a medium one 16-20, a \
                            small one 8-10",
                    ),
                },
            ),
            film: unstated(
                "Epic Choir records tenors and basses as one section of a 50-voice choir; Eric \
                    Whitacre Choir: 22 singers SATB",
                Provenance::Cited {
                    source: cite!(
                        spitfire_originals_epic_choir: "tenors and basses as one section of a \
                            50-voice choir",
                        sound_on_sound_eric_whitacre_choir: "22 singers, SATB",
                    ),
                },
            ),
            chamber: unstated(
                "a chamber choir has roughly 8 to 40 singers in total",
                Provenance::Cited {
                    source: cite!(
                        wikipedia_chamber_choir: "a chamber choir has roughly 8 to 40 singers in \
                            total",
                    ),
                },
            ),
        }),
    },
);
