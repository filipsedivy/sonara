//! Harp and plucked string records.

use super::sources::{cite, reason};
use super::{PLAYERS_ESTIMATE, cited, estimate, not_used, pitched, players};
use crate::orchestration::articulation::Articulation::{
    BartokPizzicato, Bisbigliando, Damped, Glissando, Harmonics, HarmonicsArtificial,
    HarmonicsNatural, Legato, LetRing, PalmMute, Portamento, PresDeLaTable, Staccato,
    SulPonticello, SulTasto, Tremolo, Trill, VibratoOn, Xylophonic,
};
use crate::orchestration::ensemble_profile::{EnsembleProfile, Forms};
use crate::orchestration::id::InstrumentType;
use crate::orchestration::instrument::{Instrument, InstrumentSpec, Provenance};
use crate::orchestration::pitch::Clef;

pub(super) const HARP: Instrument = Instrument::new(
    InstrumentType::Harp,
    InstrumentSpec {
        name: "Harp",
        pitch: pitched(
            0,
            cited(
                23,
                103,
                cite!(
                    rimsky_korsakov: "the range begins at C1 flat, with C4 = middle C",
                    wikipedia_pedal_harp: "low C flat 1 = 23 needs the lowest C string, which has \
                        no pedal, tuned to C flat before the performance; the top G string has no \
                        pedal and is normally G natural",
                    hugill_harps: "range C1-G7; top G7 = 103";
                    "G#7 = 104 is also obtainable, by retuning the unpedalled top G string before \
                        the piece or with a top-G mechanism (Forsyth), but no source that was read \
                        gives it as the end of the range, so it is left out",
                ),
            ),
            Some(estimate(
                36,
                96,
                reason!(
                    "no source that was read gives endpoints; C2-C7 is built from ",
                    berlioz,
                    " (chords in the lowest part are confused and fit only for doubling a bass an \
                        octave below; the top octave sounds hard when struck forcefully) and ",
                    rimsky_korsakov,
                    " (the extreme notes are for special circumstances; rapid figures are clear \
                        only in the upper register)",
                ),
            )),
        ),
        clefs: &[Clef::Treble, Clef::Bass],
        articulations: &[
            Harmonics,
            Tremolo,
            Trill,
            Glissando,
            Bisbigliando,
            PresDeLaTable,
            LetRing,
            Damped,
            Portamento,
            Xylophonic,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "harmonics, tremolo, trill, glissando, bisbigliando, pres de la table, \
                    laissez vibrer, sons etouffes, the pedal glissando",
                berlioz: "harmonics, tremolo, trill, glissando, bisbigliando, pres de la table, \
                    laissez vibrer, sons etouffes",
                hugill_harps: "harmonics, tremolo, trill, glissando, bisbigliando, pres de la \
                    table, laissez vibrer, sons etouffes, the pedal glissando",
                vsl_synchron_harp_player_manual: "the sampled harp articulations",
                spitfire_harp_redux_manual: "the sampled harp articulations";
                "xylophonic is an estimate: the only source found could not be opened",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Both,
            classical: players(0, 1, "rare before the Romantic era", PLAYERS_ESTIMATE),
            romantic: players(1, 2, "normal; 2-4 in large scores", PLAYERS_ESTIMATE),
            film: players(1, 2, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const GUITAR_CLASSICAL: Instrument = Instrument::new(
    InstrumentType::GuitarClassical,
    InstrumentSpec {
        name: "Classical guitar",
        pitch: pitched(
            -12,
            estimate(
                40,
                83,
                reason!(
                    "the low E2 = 40, the open 6th string at 82.41 Hz, is stated by ",
                    wikipedia_classical_guitar,
                    "; the top B5 = 83 is derived from three facts that agree: 19 frets above the \
                        open E4 first string (the same article), each string spanning an octave \
                        plus a fifth (",
                    hugill_fretted_strings,
                    ") and a compass of three octaves and a fifth (",
                    berlioz,
                    "); no source that was read names B5",
                ),
            ),
            Some(estimate(
                40,
                76,
                "E2-E5, up to the 12th fret of the first string; no source states a comfortable \
                    range",
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[
            Legato,
            Tremolo,
            Trill,
            Harmonics,
            SulPonticello,
            HarmonicsNatural,
            HarmonicsArtificial,
            SulTasto,
            PalmMute,
            BartokPizzicato,
            VibratoOn,
            LetRing,
            Damped,
            Glissando,
        ],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "natural and artificial harmonics, shakes, tremolo, slurred scales, \
                    vibrato, ponticello and tasto positions, damping",
                berlioz: "harmonics, shakes, reiterations, slurred scales",
                hugill_fretted_strings: "harmonics, tremolo, rasgueado, strumming, ponticello and \
                    tasto, pizzicato, snap, percussive effects",
                wikipedia_classical_guitar: "harmonics, tremolo, rasgueado, vibrato, damping";
                "palm_mute for the guitar's \"pizzicato\", bartok_pizzicato for the \"snap\" and \
                    the glissando are estimates; the fretted-string pages give only the headings",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("opera stage colour only", PLAYERS_ESTIMATE),
            romantic: players(0, 1, "opera colour; Mahler 7, movement 4", PLAYERS_ESTIMATE),
            film: players(0, 1, "often recorded separately", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const LUTE: Instrument = Instrument::new(
    InstrumentType::Lute,
    InstrumentSpec {
        name: "Lute",
        pitch: pitched(
            -12,
            estimate(
                36,
                79,
                reason!(
                    "a Renaissance lute nominally in G with up to 10 courses: the low C2 = 36 \
                        assumes a 10-course bass; the top G5 = 79 is the open G4 treble course \
                        plus one octave, because some wooden frets glued to the body (which some \
                        consider anachronistic) let the highest courses be stopped up to a full \
                        octave above the open string (",
                    wikipedia_lute,
                    ", which does not state the octave of the open strings)",
                ),
            ),
            Some(estimate(
                43,
                74,
                reason!(
                    "G2-D5 = 43-74, the six main courses of a 6-course tenor lute in G (",
                    wikipedia_lute,
                    ") and the lower part of the fingerboard",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[Trill, LetRing],
        articulations_provenance: Provenance::Estimate {
            reason: "no source on lute technique was opened; ornaments (trill) and ringing are \
                general early-music practice",
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: not_used("", PLAYERS_ESTIMATE),
            romantic: not_used("", PLAYERS_ESTIMATE),
            film: players(0, 1, "period and fantasy scores", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "Baroque continuo", PLAYERS_ESTIMATE),
        }),
    },
);

pub(super) const MANDOLIN: Instrument = Instrument::new(
    InstrumentType::Mandolin,
    InstrumentSpec {
        name: "Mandolin",
        pitch: pitched(
            0,
            estimate(
                55,
                91,
                reason!(
                    "the low G3 = 55 is stated by ",
                    wikipedia_mandolin,
                    " (tuning G3 D4 A4 E5) and ",
                    forsyth,
                    " (low G, the 4th string); the top G6 = 91 is derived from the compass of \
                        about three octaves above G3 in ",
                    berlioz,
                ),
            ),
            Some(estimate(
                55,
                88,
                reason!(
                    "derived from ",
                    forsyth,
                    ": a working compass from the low G of about two octaves and a major sixth, \
                        G3-E6 = 55-88",
                ),
            )),
        ),
        clefs: &[Clef::Treble],
        articulations: &[Staccato, Tremolo, LetRing],
        articulations_provenance: Provenance::Cited {
            source: cite!(
                forsyth: "single staccato strokes; the plectrum tremolo as the sustained note; \
                    four-course chords swept with the plectrum",
                berlioz: "single staccato strokes; the plectrum tremolo",
                wikipedia_mandolin: "the plectrum tremolo; chords swept with the plectrum";
                "let_ring is an estimate",
            ),
        },
        ensemble: Some(EnsembleProfile {
            forms: Forms::Solo,
            classical: players(0, 1, "Mozart's Don Giovanni", PLAYERS_ESTIMATE),
            romantic: players(
                0,
                1,
                "Verdi's Otello; Mahler 7, 8, Das Lied von der Erde",
                PLAYERS_ESTIMATE,
            ),
            film: players(0, 1, "", PLAYERS_ESTIMATE),
            chamber: players(1, 1, "", PLAYERS_ESTIMATE),
        }),
    },
);
