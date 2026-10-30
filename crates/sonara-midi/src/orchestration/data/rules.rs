//! The rule catalogue, one record per [`RuleId`], the thresholds its checks compare with, the
//! doubling tables, the weights and register factors, and the low-interval limits.
//!
//! Each rule cites the handbook passage it comes from; the numbers a check compares with are named
//! thresholds with their own provenance, most of them estimates that turn a source's words into a
//! number. How serious a finding is follows how firmly the source states the rule, an estimate
//! throughout.

use super::sources::{cite, reason};
use crate::orchestration::articulation::Articulation::{
    ColLegno, ColLegnoBattuto, ConSordino, Harmonics, Pizzicato, Roll, Stopped,
};
use crate::orchestration::ensemble_profile::PlayerRange;
use crate::orchestration::id::InstrumentType;
use crate::orchestration::id::InstrumentType::{
    AltoFlute, BassClarinet, BassDrum, BassFlute, Bassoon, Celesta, Cello, Cimbasso, ClarinetA,
    ClarinetBb, ClarinetEb, ClashCymbals, Contrabassoon, Cornet, Crotales, DoubleBass, EnglishHorn,
    Euphonium, Flugelhorn, Flute, Glockenspiel, Harp, HornF, Oboe, OboeDamore, PianoConcertGrand,
    PianoStudioGrand, PianoUpright, Piccolo, SaxophoneAlto, SaxophoneBaritone, SaxophoneSoprano,
    SaxophoneTenor, SnareDrum, TamTam, Timpani, TromboneBass, TromboneTenor, TrumpetBb, TrumpetC,
    TrumpetPiccolo, Tuba, TubularBells, Viola, Violin, WagnerTubaBass, WagnerTubaTenor, Xylophone,
};
use crate::orchestration::instrument::{Family, Provenance};
use crate::orchestration::pitch::{PitchRange, SoundingPitch};
use crate::orchestration::playability::Span;
use crate::orchestration::rules::RuleInput::{
    Articulation, Dynamic, Harmony, HarpPedals, Instrument, Meter, Notes, Part, RegisterFactors,
    Role, Tempo, TimpaniTuning, Weights,
};
use crate::orchestration::rules::{
    Applies, Avoid, Combination, DynamicBand, IntervalLimit, IntervalLimits, Level, Limit, Member,
    NamedDoubling, Prevails, RegisterFactor, Rule, RuleId, RuleInput, RuleSpec, Selector, Severity,
    Spacing, Threshold, WeightRow, WoodwindOrder,
};

const HINT: Severity = Severity::Fixed(Level::Hint);
const WARNING: Severity = Severity::Fixed(Level::Warning);
const ERROR: Severity = Severity::Fixed(Level::Error);

/// A hint at pp to mf, a warning at f and ff.
const HINT_UNLESS_LOUD: Severity = Severity::ByDynamic {
    soft: Level::Hint,
    loud: Level::Warning,
};

/// The rule `id` with its values.
const fn rule(
    id: RuleId,
    title: &'static str,
    check: &'static str,
    inputs: &'static [RuleInput],
    severity: Severity,
    provenance: Provenance,
) -> Rule {
    Rule::new(
        id,
        RuleSpec {
            title,
            check,
            inputs,
            severity,
            provenance,
        },
    )
}

/// The sounding pitch of MIDI note `note`; one outside 0–127 in a `const` item is a compile error.
const fn pitch(note: i64) -> SoundingPitch {
    match SoundingPitch::new(note) {
        Some(pitch) => pitch,
        None => panic!("a MIDI note is 0-127"),
    }
}

// Range.

const RNG_01: Rule = rule(
    RuleId::Rng01,
    "Note outside the playable range",
    "a note lies below the lowest or above the highest playable note of its instrument",
    &[Instrument, Notes],
    ERROR,
    Provenance::Estimate {
        reason: "the check reads the playable range of the instrument's record, which carries its \
            own provenance; a note outside it cannot be played",
    },
);

const RNG_02: Rule = rule(
    RuleId::Rng02,
    "Note outside the comfortable range",
    "a note is playable but lies outside the comfortable range of its instrument",
    &[Instrument, Notes],
    WARNING,
    Provenance::Estimate {
        reason: "the check reads the comfortable range of the instrument's record, which carries \
            its own provenance",
    },
);

const RNG_03: Rule = rule(
    RuleId::Rng03,
    "Sustained extreme high brass",
    "a brass part holds or repeats notes above its comfortable range for more than about two \
        bars of moderate 4/4",
    &[Instrument, Notes, Tempo],
    WARNING,
    Provenance::Cited {
        source: cite!(
            widor: "p. 198: keep each instrument in its best register and avoid sustained notes \
                at the extreme top; the brass player's lips tire under the pressure of high notes";
            "the length is an estimate"
        ),
    },
);

const RNG_04: Rule = rule(
    RuleId::Rng04,
    "Extreme-register melody undoubled",
    "a violin or woodwind melody above MIDI 90 has no doubling an octave below, or a melody below \
        MIDI 42 has none an octave above",
    &[Instrument, Role, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, pp. 44 and 50: a melody in the extreme top is doubled an \
                octave lower, and one in the extreme bottom an octave higher";
            "the MIDI limits are an estimate"
        ),
    },
);

// Spacing.

const SPC_01: Rule = rule(
    RuleId::Spc01,
    "Low-interval limit",
    "two notes sound together and the lower note of their interval lies below that interval's \
        low limit",
    &[Instrument, Notes, Dynamic],
    Severity::ByCase {
        usual: Level::Hint,
        raised: Level::Warning,
        case: "the interval sounds at f or ff, or a brass instrument plays it",
    },
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Limits of orchestral range, p. 106: music centred in the \
                lowest octaves suffers, as the proximity of harmonic intervals creates a bad \
                effect",
            rogers_big_band_voicings: "below the middle line of the bass staff keep the \
                intervals consonant",
            hoffmann_low_interval_limits: "some intervals can sit lower than others; how muddy \
                an interval sounds depends on the dynamic and on the sound's upper harmonics, so \
                soft low strings forgive more than loud brass";
            "the limit of each interval is an estimate"
        ),
    },
);

const SPC_02: Rule = rule(
    RuleId::Spc02,
    "Dissonance in the low bass",
    "a second, fourth, tritone, seventh or ninth lies between two parts with its lower note below \
        D3 (MIDI 50); stricter than the low-interval limits, which admit some of these lower",
    &[Instrument, Notes],
    Severity::ByCase {
        usual: Level::Hint,
        raised: Level::Warning,
        case: "both parts belong to one family",
    },
    Provenance::Cited {
        source: cite!(
            rogers_big_band_voicings: "below the middle line of the bass staff, D3, keep the \
                intervals consonant, ideally a tenth, fifth or octave; fourths sound bad low; the \
                horns of a big band are alike in timbre and cannot clear the texture themselves";
            "the stricter grade within one family follows that reason and is an estimate"
        ),
    },
);

const SPC_03: Rule = rule(
    RuleId::Spc03,
    "Bass too far below the harmony",
    "a sustained bass lies more than an octave below the next part up",
    &[Harmony, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Distribution of notes in chords, p. 67: the bass should \
                rarely lie at a greater distance than an octave from the part directly above it",
        ),
    },
);

const SPC_04: Rule = rule(
    RuleId::Spc04,
    "Hole in the middle",
    "two adjacent upper parts of a sustained chord lie more than an octave apart",
    &[Harmony, Notes, Dynamic],
    HINT_UNLESS_LOUD,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, pp. 68 and 78: modern orchestrators do not allow any void \
                in the intermediate parts; wide empty intervals between upper and lower parts are \
                worst in forte";
            "the octave is an estimate"
        ),
    },
);

const SPC_05: Rule = rule(
    RuleId::Spc05,
    "Spacing against the harmonic series",
    "near the bottom of a chord, the interval between two adjacent parts is wider than the \
        interval below it",
    &[Harmony, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Distribution of notes in chords, p. 67, and Remarks, p. \
                78: following the natural harmonic series, wide intervals lie in the bass, fifths \
                and fourths in the middle and thirds or seconds in the upper register",
        ),
    },
);

const SPC_06: Rule = rule(
    RuleId::Spc06,
    "Bass doubled upward or mixed into the upper parts",
    "in wide spacing an upper part doubles the bass's pitch class, or the bass crosses above the \
        tenor",
    &[Harmony, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Number of harmonic parts, p. 64: the bass is doubled in \
                the lower octave only; in wide spacing doubling the bass creates an effect of \
                heaviness, and the bass should never mix with the other parts",
        ),
    },
);

const SPC_07: Rule = rule(
    RuleId::Spc07,
    "Consecutive octaves between upper parts",
    "two independent upper parts move in parallel octaves, other than one line doubled \
        throughout",
    &[Harmony, Notes],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, p. 65: consecutive octaves between the upper parts are \
                not permissible",
        ),
    },
);

const SPC_08: Rule = rule(
    RuleId::Spc08,
    "Bass of an inverted seventh chord doubled",
    "an upper part doubles the bass note of an inverted dominant seventh or other seventh chord",
    &[Harmony, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, p. 65: the bass of an inversion of the dominant chord \
                should never be doubled in any of the upper parts, which applies also to other \
                chords of the seventh",
        ),
    },
);

// Doubling.

const DBL_01: Rule = rule(
    RuleId::Dbl01,
    "Unknown unison pairing",
    "two instruments play a unison the named doublings do not list; when they list it, the \
        finding reports the colour they give",
    &[Instrument, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, Combination in unison, p. 47, Combination of strings and \
                wind, p. 58, and Combination of strings and brass, p. 61: the colour of each \
                unison of two timbres",
        ),
    },
);

const DBL_02: Rule = rule(
    RuleId::Dbl02,
    "Woodwind order inverted in a doubling",
    "in an octave, third or sixth doubling an instrument of a lower woodwind branch lies above one \
        of a higher branch, such as a bassoon above a clarinet",
    &[Instrument, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, pp. 47, 49 and 52: departing from the order flute, oboe, \
                clarinet, bassoon gives a far-fetched, unnatural tone, and inverted thirds and \
                sixths sound strained and forced",
        ),
    },
);

const DBL_03: Rule = rule(
    RuleId::Dbl03,
    "Same-colour winds in octaves without strings",
    "two players of one woodwind instrument play in octaves and no string part doubles them",
    &[Instrument, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, pp. 49-50: two flutes, two clarinets or two bassoons in \
                octaves are not to be recommended, as their registers do not correspond, unless \
                strings double both",
        ),
    },
);

const DBL_04: Rule = rule(
    RuleId::Dbl04,
    "Woodwind octave above a trombone",
    "a woodwind doubles a trombone an octave higher and no trumpet does",
    &[Instrument, Notes],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, Combination of wind and brass in octaves, p. 57: woodwind \
                instruments should not be used to double a trombone in the octave above; \
                trumpets are more suitable",
        ),
    },
);

const DBL_05: Rule = rule(
    RuleId::Dbl05,
    "Divided low strings in octaves",
    "the two halves of one viola, cello or double-bass section move in octaves",
    &[Part, Notes],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, Note II, p. 44: divided violas, cellos or double basses in \
                octaves play on strings that do not correspond, and unity of tone is impaired; \
                the violins are the exception",
        ),
    },
);

const DBL_06: Rule = rule(
    RuleId::Dbl06,
    "Violins divided in octaves, too few players",
    "the first and the second violins are both divided in octaves, the string body is smaller \
        than the medium orchestra's, and no wind doubles them",
    &[Instrument, Part, Notes],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, p. 41: violins divided in octaves halve the players on \
                each line and lose resonance, worst in small orchestras; Ch. I, p. 6: the medium \
                orchestra has 12 first and 10 second violins",
        ),
    },
);

const DBL_07: Rule = rule(
    RuleId::Dbl07,
    "Expressive solo doubled",
    "a part marked solo and expressive, playing p, is doubled in unison by another instrument",
    &[Part, Role, Dynamic, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, Combination in unison, p. 48: phrases demanding diversity \
                of expression should be entrusted to solo instruments of simple timbre; doubling \
                restricts the variety of colour and expression",
        ),
    },
);

const DBL_08: Rule = rule(
    RuleId::Dbl08,
    "Double-bass phrase unsupported",
    "a melodic double-bass phrase has no cello or bassoon in unison or an octave away",
    &[Instrument, Role, Notes],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, p. 38: no phrase of any importance is given to the double \
                bass without the support of cellos or bassoons",
        ),
    },
);

const DBL_09: Rule = rule(
    RuleId::Dbl09,
    "Strings and brass in unison expected to blend",
    "a string section and a brass instrument share a melody in unison with no woodwind",
    &[Instrument, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, Combination of strings and brass, p. 61: in the best \
                unisons of strings and brass each can be heard separately",
        ),
    },
);

// Balance.

const BAL_01: Rule = rule(
    RuleId::Bal01,
    "Unequal chord members",
    "in a sustained chord, the summed weight of a harmonic part is less than half that of the \
        strongest part other than the melody",
    &[Instrument, Harmony, Dynamic, Weights, RegisterFactors],
    HINT_UNLESS_LOUD,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, String harmony, p. 69: the resonance of different \
                harmonic parts must be equally balanced";
            "the half is an estimate"
        ),
    },
);

const BAL_02: Rule = rule(
    RuleId::Bal02,
    "Single horns against trumpets or trombones",
    "in a forte brass chord one horn plays each horn part and the horns are not marked a step \
        louder",
    &[Instrument, Part, Dynamic],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Harmony in the brass, pp. 84-85: in forte the horns are \
                doubled when they are mixed with trumpets or trombones, or marked one degree \
                louder than the other instruments",
        ),
    },
);

const BAL_03: Rule = rule(
    RuleId::Bal03,
    "Melody not on top of the weight",
    "the melody's weight is less than 1.5 times the accompaniment's in its own register band and \
        the bands next to it",
    &[Role, Notes, Weights, RegisterFactors],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, p. 36: a melody in the middle of the orchestra is not so \
                prominent and needs help; Ch. IV, Tone quality as a harmonic force, pp. 114-115: \
                a harmonic basis of woodwind or brass should be of inferior dynamic power";
            "the margin is an estimate"
        ),
    },
);

const BAL_04: Rule = rule(
    RuleId::Bal04,
    "Woodwind under-weighted against brass",
    "in a forte chord the woodwind parts weigh less than the brass parts; mark the woodwind a step \
        louder or double it",
    &[Instrument, Dynamic, Weights],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Harmony in combined groups, pp. 90-94: in difficult cases \
                the woodwind is marked one degree louder than the brass",
        ),
    },
);

const BAL_05: Rule = rule(
    RuleId::Bal05,
    "Non-sustaining melody against a sustained forte",
    "a harp, celesta, piano or pizzicato strings carry the melody while sustaining groups play f",
    &[Instrument, Role, Dynamic, Articulation],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, Harp, p. 29: three or four harps are needed against a full \
                orchestra in forte; Comparison of resonance, pp. 33-34: pizzicato strings, the \
                piano played softly and the celesta are easily overpowered by the sustaining \
                groups",
        ),
    },
);

const BAL_06: Rule = rule(
    RuleId::Bal06,
    "Heavy percussion at fortissimo",
    "the bass drum, clash cymbals, tam-tam and a snare-drum roll play together at ff over a \
        melody; together they overpower any tutti",
    &[Instrument, Dynamic, Articulation],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Use of percussion instruments, p. 117: the bass drum, \
                cymbals, gong and a tremolo on the side drum, played fortissimo, are sufficient \
                to overpower any orchestral tutti",
        ),
    },
);

const BAL_07: Rule = rule(
    RuleId::Bal07,
    "A family without its own bass",
    "a family's sustained chord has no member on the chord's bass note",
    &[Instrument, Harmony],
    HINT,
    Provenance::Cited {
        source: cite!(
            widor: "p. 197: write so that each group can be heard without the others, each \
                giving complete harmony with its true bass",
        ),
    },
);

const BAL_08: Rule = rule(
    RuleId::Bal08,
    "Solo string over heavy accompaniment",
    "a solo string carries the melody while the accompaniment weighs more than 2 units",
    &[Part, Role, Weights],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Soli in the strings, p. 105: the comparatively weak tone \
                of the solo instrument needs a light, transparent accompaniment";
            "the 2 units are an estimate"
        ),
    },
);

const BAL_09: Rule = rule(
    RuleId::Bal09,
    "String body too small for the winds",
    "fewer than 8 first violins play with winds in pairs or more",
    &[Part],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, Stringed instruments, p. 6: fewer than 8 first violins is a \
                mistake, as the balance between strings and wind is completely destroyed",
        ),
    },
);

// Masking.

const MSK_01: Rule = rule(
    RuleId::Msk01,
    "Accompaniment in the voice's register",
    "a sustained accompaniment lies in the same register band as a voice or a solo melody",
    &[Instrument, Role, Notes],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. V, Transparence of accompaniment, pp. 120-121: a singer is \
                more easily overpowered by long sustained notes than by short detached ones, and \
                sustained harmony in the voice's own register covers it, women's voices above \
                all",
        ),
    },
);

const MSK_02: Rule = rule(
    RuleId::Msk02,
    "Heavy doublings under a voice",
    "under a voice, strings are doubled by woodwind and brass, brass by woodwind, one harmonic \
        part is given to two clarinets, two oboes or two horns in unison, the timpani or other \
        percussion roll, or the double basses hold long notes",
    &[Instrument, Role, Notes, Articulation],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. V, Transparence of accompaniment, pp. 120-121: such \
                combinations are liable to drown the singer, and long sustained notes in the \
                double basses produce a peculiar throbbing effect with the voice",
        ),
    },
);

const MSK_03: Rule = rule(
    RuleId::Msk03,
    "Weak register exposed",
    "a melody lies in a register of factor 0.5 or less under accompaniment of weight 1 or more in \
        the same register band",
    &[Instrument, Role, Notes, Weights, RegisterFactors],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, wood-wind remarks, p. 20: in their low and middle compass \
                the piccolo and the small clarinet are so much weaker that they are of little \
                service there; Ch. II, p. 48: the clarinet is weak in the middle compass",
            forsyth: "the flute's low register is easily covered, and double basses blur it; the \
                fourth horn tends to drop out of sight in the bottom fifth of its compass",
        ),
    },
);

const MSK_04: Rule = rule(
    RuleId::Msk04,
    "Harmonic basis too close to the melody",
    "a woodwind or brass harmonic basis lies within an octave of a woodwind or brass melody, at an \
        equal or higher dynamic",
    &[Instrument, Role, Notes, Dynamic],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Tone quality as a harmonic force, pp. 114-115: woodwind \
                and brass differ little in timbre, so a harmonic basis in them generally remains \
                an octave removed from the melodic design and should be of inferior dynamic power",
        ),
    },
);

// Voicing.

const VOI_01: Rule = rule(
    RuleId::Voi01,
    "Three-part woodwind chord in three colours",
    "three woodwind instruments of different kinds each take one note of a three-part chord",
    &[Instrument, Harmony],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Four-part and three-part harmony, p. 74: chords in \
                three-part harmony are given to two instruments of one timbre and a third of \
                another, but never to three different timbres",
        ),
    },
);

const VOI_02: Rule = rule(
    RuleId::Voi02,
    "Close four-part chord in four colours",
    "four woodwind instruments of different kinds play a four-part chord in close position",
    &[Instrument, Harmony],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Four-part and three-part harmony, p. 74: four different \
                timbres in close four-part harmony are to be avoided, as their registers will not \
                correspond",
        ),
    },
);

const VOI_03: Rule = rule(
    RuleId::Voi03,
    "Enclosure inside one colour",
    "a part is enclosed between two instruments of the same kind",
    &[Instrument, Harmony],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Four-part and three-part harmony, p. 74: a tone quality \
                that is enclosed must be enclosed between two different timbres",
        ),
    },
);

const VOI_04: Rule = rule(
    RuleId::Voi04,
    "Doubling changes within a passage",
    "from chord to chord some chord notes are doubled and others not, with no melodic reason",
    &[Instrument, Harmony],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Wood-wind harmony, p. 72: instruments forming chords are \
                used the same way throughout a passage, doubled or not, unless one harmonic part \
                is to stand out",
        ),
    },
);

const VOI_05: Rule = rule(
    RuleId::Voi05,
    "Discord in one colour",
    "a second, fourth, fifth or seventh lies between two players of one woodwind instrument; \
        stricter for the oboe",
    &[Instrument, Harmony],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Wood-wind harmony, p. 72: concords, not discords, are \
                given to instruments of the same kind, above all to the penetrating oboe, unless \
                the discord is to be emphasised",
        ),
    },
);

const VOI_06: Rule = rule(
    RuleId::Voi06,
    "Non-corresponding registers",
    "one chord puts a note in a weak register (factor 0.5 or less) next to one in a strong \
        register (factor 1.25 or more)",
    &[Instrument, Notes, RegisterFactors],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, pp. 72-73: corresponding registers should coincide; the \
                soft, weak register of one instrument should not be coupled with the powerful, \
                piercing range of another",
        ),
    },
);

const VOI_07: Rule = rule(
    RuleId::Voi07,
    "Horn pairs broken",
    "in filling-up parts the third horn lies below the second, or the second or the fourth horn \
        takes the top of the chord",
    &[Part, Notes],
    Severity::ByCase {
        usual: Level::Hint,
        raised: Level::Warning,
        case: "the fourth horn is sent above its comfortable range",
    },
    Provenance::Cited {
        source: cite!(
            forsyth_second_scan: "pp. 122-123: in ordinary filling-up parts the horns are used \
                as two pairs, Nos. 1 and 3 playing the upper parts and Nos. 2 and 4 the lower, \
                the notes of a chord dove-tailed; the lower pair can ascend to high G, and the \
                extreme notes are approached with discretion",
        ),
    },
);

const VOI_08: Rule = rule(
    RuleId::Voi08,
    "Clarinets inside horns, or horns inside bassoons",
    "clarinets are enclosed between horns, or horns between bassoons",
    &[Instrument, Harmony],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Harmony in combined groups, p. 90: the enclosure of \
                clarinets between horns, or of horns between bassoons",
        ),
    },
);

const VOI_09: Rule = rule(
    RuleId::Voi09,
    "Woodwind below the trumpets in a combined chord",
    "woodwind chord notes lie below the trumpets while the trumpets hold the top",
    &[Instrument, Harmony],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Harmony in combined groups, p. 92: flutes, oboes and \
                clarinets are better used to form the harmonic part above the trumpets",
        ),
    },
);

const VOI_10: Rule = rule(
    RuleId::Voi10,
    "Uneven string divisi",
    "in a sustained chord some upper string parts are divided and others not, or the cellos and \
        basses stay undivided under fully divided upper strings at an equal dynamic",
    &[Part, Harmony],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, String harmony, p. 70: harmony with every upper string \
                part divided the same way balances; under divided upper strings, cellos and \
                basses playing non divisi are a trifle heavy",
        ),
    },
);

const VOI_11: Rule = rule(
    RuleId::Voi11,
    "Divisi in four or more at forte",
    "one section is divided four or more ways at f or ff",
    &[Part, Dynamic],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, Stringed instruments, p. 7: division into four and more \
                parts is rare, but may be used in piano passages, as it greatly reduces the \
                volume of tone of the strings",
        ),
    },
);

const VOI_12: Rule = rule(
    RuleId::Voi12,
    "Sustained multi-stop string chords",
    "one string part plays three or four notes at once, longer than a short chord or at p; only \
        the upper two notes can be held",
    &[Instrument, Notes],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, String harmony, p. 69: chords of three or four notes can \
                only be played rapidly on the strings; the two upper notes can be sustained",
        ),
    },
);

// Playability.

const PLY_01: Rule = rule(
    RuleId::Ply01,
    "Wind breath",
    "a wind part plays without a rest for longer than 8 bars of moderate 4/4; one held note \
        outlasts the instrument's breath; the flute's lowest register runs more than a few notes \
        without a breath",
    &[Instrument, Notes, Tempo],
    WARNING,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, p. 19: wood-wind players cannot manage extremely long \
                sustained passages, as they are compelled to take breath, and must be given a \
                little rest from time to time",
            widor: "p. 127: a medium-register saxophone note cannot be held longer than about \
                ten bars of moderate 4/4, about forty seconds being a soloist's maximum at piano",
            forsyth: "in the flute's lowest notes the player must breathe after every few notes";
            "the phrase length is an estimate"
        ),
    },
);

const PLY_02: Rule = rule(
    RuleId::Ply02,
    "Brass endurance",
    "a brass part plays long loud or high passages with no rest, or repeated notes outside the \
        middle register",
    &[Instrument, Notes],
    WARNING,
    Provenance::Cited {
        source: cite!(
            widor: "p. 198: the brass player's lips tire under the pressure of high notes",
            forsyth_second_scan: "p. 98: repeated trumpet notes are best placed in the middle \
                register and broken up to give the player time to breathe",
        ),
    },
);

const PLY_03: Rule = rule(
    RuleId::Ply03,
    "Bow length",
    "one slurred pp group lasts longer than about 4 bars of moderate time, or about half that for \
        a slur of many notes, or a forte slur is long",
    &[Instrument, Notes, Dynamic, Tempo],
    HINT,
    Provenance::Cited {
        source: cite!(
            forsyth_second_scan: "p. 343: pianissimo held notes or slurs of a few long notes \
                last up to about 4 bars of moderate time, about half that when the slur includes \
                a large number of notes; long forte slurs are an absolute impossibility",
        ),
    },
);

const PLY_04: Rule = rule(
    RuleId::Ply04,
    "Mute change time",
    "a mute is put on with less free time than the instrument's mute change needs (about two bars \
        of moderate time for strings), or taken off with less than slightly under that",
    &[Instrument, Part, Articulation, Tempo],
    WARNING,
    Provenance::Cited {
        source: cite!(
            forsyth_second_scan: "p. 374: a couple of bars at least of moderate time must be \
                allowed for fixing the mute, a slightly shorter time for taking it off",
        ),
    },
);

const PLY_05: Rule = rule(
    RuleId::Ply05,
    "Pizzicato and arco switch",
    "a string part changes from pizzicato to arco with less than about half a second free",
    &[Instrument, Articulation, Tempo],
    WARNING,
    Provenance::Cited {
        source: cite!(
            forsyth_second_scan: "p. 367: about half a second must be allowed to bring the bow \
                back after pizzicato; very little time is needed from arco to pizzicato",
        ),
    },
);

const PLY_06: Rule = rule(
    RuleId::Ply06,
    "Pizzicato speed",
    "a pizzicato passage is faster than it could be bowed, or a double-bass pizzicato is nearly \
        as fast as a violin's",
    &[Instrument, Articulation, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, Pizzicato, p. 27: pizzicato cannot be as rapid as bowed \
                playing, and the double basses pluck much more slowly than the violins",
        ),
    },
);

const PLY_07: Rule = rule(
    RuleId::Ply07,
    "Harp pedal setting",
    "a harp note lies outside the current pedal setting with no pedal change, two spellings of one \
        letter sound at once, or a note needs a double sharp or flat",
    &[HarpPedals, Notes],
    ERROR,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, Harp, p. 28: the harp is essentially a diatonic instrument",
            forsyth_second_scan: "pp. 463-464: the harp is an instrument of harmonic formulae; \
                once set it is nearly as much bound to its formula as the old harp was, and each \
                pedal acts on all the strings of the same name, raising them a semitone or a \
                whole tone",
        ),
    },
);

const PLY_08: Rule = rule(
    RuleId::Ply08,
    "Harp pedal changes",
    "one foot changes more than one pedal per beat at moderate tempo (the left foot works D, C and \
        B, the right E, F, G and A), or a pedal changes while its string must sound",
    &[HarpPedals, Notes, Tempo],
    WARNING,
    Provenance::Cited {
        source: cite!(
            forsyth_second_scan: "p. 464: the seven pedals are arranged three on the player's \
                left, B, C and D, and four on the right, E, F, G and A";
            "one change per foot per beat is an estimate"
        ),
    },
);

const PLY_09: Rule = rule(
    RuleId::Ply09,
    "Harp chord shape",
    "a harp chord gives one hand more than four notes, or puts the hands too far apart",
    &[Instrument, Notes],
    Severity::ByCase {
        usual: Level::Warning,
        raised: Level::Error,
        case: "the chord has more than eight notes, more than both hands can play",
    },
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, Harp, p. 28: only four notes at the most can be played by \
                each hand, so the notes of a chord are written close together, with not too great \
                a space between one hand and the other",
        ),
    },
);

const PLY_10: Rule = rule(
    RuleId::Ply10,
    "Harp fast figures low",
    "a harp plays rapid figures in its low or middle register",
    &[Instrument, Notes, Tempo],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, Harp, p. 28: in quick modulations the lingering strings mix \
                one chord with the next, so rapid figures are clear only in the upper register, \
                where the strings are shorter",
        ),
    },
);

const PLY_11: Rule = rule(
    RuleId::Ply11,
    "Timpani retuning",
    "a timpani note lies on no tuned drum, a drum is retuned with less rest than its retuning \
        needs (two or three bars of moderate 4/4 per whole tone on hand-tuned drums, more for a \
        larger change), or a passage has more pitches than drums",
    &[TimpaniTuning, Notes, Tempo, Meter],
    Severity::ByCase {
        usual: Level::Warning,
        raised: Level::Error,
        case: "no drum holds the pitch and no time is left to retune one",
    },
    Provenance::Cited {
        source: cite!(
            forsyth_second_scan: "p. 46: the drummer needs two or three bars at least of 4/4 \
                moderato to change a drum by a whole tone, and proportionately longer for a bigger \
                change",
            rimsky_korsakov: "Ch. I, Kettle-drums, p. 30: with three drums the player can retune \
                at least one of them during a pause of some length",
        ),
    },
);

const PLY_12: Rule = rule(
    RuleId::Ply12,
    "Timpani thirds",
    "two timpani a third apart are struck together",
    &[Instrument, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            forsyth_second_scan: "p. 45: two drums a third apart only seem to get in each \
                other's way through the confusion of their upper partials; fifths give the drum \
                effect",
        ),
    },
);

const PLY_13: Rule = rule(
    RuleId::Ply13,
    "Horn tessitura jumps",
    "one horn part changes suddenly and completely from a high to a low tessitura",
    &[Part, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            forsyth_second_scan: "p. 122: allow the player reasonable time and he can adjust \
                his embouchure; give him a sudden complete change of tessitura and he is done",
        ),
    },
);

const PLY_14: Rule = rule(
    RuleId::Ply14,
    "Fast repeated notes and tonguing",
    "fast repeated notes need double tonguing on an oboe, clarinet or bassoon, where only the \
        flute double-tongues among the woodwinds, or on a low brass instrument, where only \
        trumpets and cornets do among the brass",
    &[Instrument, Notes, Tempo],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, p. 18: repetition of a single note by double tonguing is \
                only possible on the flute among the wood-wind; Ch. I, brass remarks, p. 25: \
                double tonguing among the brass",
        ),
    },
);

const PLY_15: Rule = rule(
    RuleId::Ply15,
    "Clarinet octave leaps",
    "a clarinet leaps rapidly between octaves",
    &[Instrument, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, p. 18: on account of its construction the clarinet is not \
                well adapted to sudden leaps from one octave to another; these skips are easier \
                on flutes, oboes and bassoons",
        ),
    },
);

// Texture.

const TEX_01: Rule = rule(
    RuleId::Tex01,
    "Characteristic colours overused",
    "a celesta, glockenspiel, xylophone, castanets or piano sounds in most bars of a piece",
    &[Instrument, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Economy in orchestral colour, p. 118: the celesta, \
                glockenspiel and xylophone are too characteristic in timbre to be employed over \
                frequently, and the same may be said of the piano and castanets";
            "most bars, more than half, is an estimate"
        ),
    },
);

const TEX_02: Rule = rule(
    RuleId::Tex02,
    "Percussion all together, or never resting",
    "all the percussion plays at once, or the brass and percussion never rest",
    &[Instrument, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Economy in orchestral colour, p. 118: the percussion is \
                seldom employed, and practically never all together; the trombones, trumpets and \
                tuba are occasionally tacet for long periods, and a silent group gains fresh \
                interest on its return",
        ),
    },
);

const TEX_03: Rule = rule(
    RuleId::Tex03,
    "Tutti at pp",
    "the full tutti plays pp or p",
    &[Instrument, Dynamic],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Full Tutti, p. 102: the tutti is used essentially in \
                forte and fortissimo, rarely in pianissimo and piano",
        ),
    },
);

const TEX_04: Rule = rule(
    RuleId::Tex04,
    "Echo or imitation against register",
    "an echo is given to another family, such as a woodwind echoing the strings, or an imitation \
        in a higher register to a lower instrument",
    &[Instrument, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Repetition of phrases, imitation, echo, p. 110: imitation \
                follows the law of register, an answer in the upper register going to an \
                instrument of higher range; an echo is weaker than the first instrument but \
                related to it",
        ),
    },
);

const TEX_05: Rule = rule(
    RuleId::Tex05,
    "Crescendo order",
    "a long crescendo brings in the brass before the woodwind and strings, or a diminuendo drops \
        the strings first",
    &[Instrument, Dynamic],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Crescendo and diminuendo, p. 112: a long crescendo adds \
                the groups in the order strings, woodwind, brass, and a diminuendo removes them \
                in reverse",
        ),
    },
);

const TEX_06: Rule = rule(
    RuleId::Tex06,
    "Hand-over gap",
    "a line passes from one instrument to another with no common note at the join",
    &[Instrument, Notes],
    HINT,
    Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Transference of passages and phrases, pp. 107-108: the last \
                note of one instrument's part coincides with the first note of the next",
            forsyth_second_scan: "p. 227: the tone-colour of each chord is just overlapped on to \
                that of the next, so as to prevent any sort of gap",
        ),
    },
);

/// Every rule, one per [`RuleId`], in the order of [`RuleId::ALL`].
pub static RULES: [Rule; RuleId::ALL.len()] = [
    RNG_01, RNG_02, RNG_03, RNG_04, SPC_01, SPC_02, SPC_03, SPC_04, SPC_05, SPC_06, SPC_07, SPC_08,
    DBL_01, DBL_02, DBL_03, DBL_04, DBL_05, DBL_06, DBL_07, DBL_08, DBL_09, BAL_01, BAL_02, BAL_03,
    BAL_04, BAL_05, BAL_06, BAL_07, BAL_08, BAL_09, MSK_01, MSK_02, MSK_03, MSK_04, VOI_01, VOI_02,
    VOI_03, VOI_04, VOI_05, VOI_06, VOI_07, VOI_08, VOI_09, VOI_10, VOI_11, VOI_12, PLY_01, PLY_02,
    PLY_03, PLY_04, PLY_05, PLY_06, PLY_07, PLY_08, PLY_09, PLY_10, PLY_11, PLY_12, PLY_13, PLY_14,
    PLY_15, TEX_01, TEX_02, TEX_03, TEX_04, TEX_05, TEX_06,
];

// Compile-time check: the rule at each place of `RULES` has the id at the same place of
// `RuleId::ALL`, so the catalogue holds every rule once and a lookup by place cannot return
// another rule.
const _: () = {
    let mut i = 0;
    while i < RULES.len() {
        assert!(
            RULES[i].id() as usize == i,
            "RULES holds each rule at its id's place"
        );
        i += 1;
    }
};

/// The record of rule `id`.
pub(in crate::orchestration) fn rule_record(id: RuleId) -> &'static Rule {
    // The check above places each rule at its id's index, so the index is in bounds.
    &RULES[id as usize]
}

// Thresholds.

/// RNG-03: how long a brass part may hold or repeat notes above its comfortable range, two bars
/// of moderate 4/4.
pub const SUSTAINED_HIGH_BRASS: Threshold<Span> = Threshold {
    rules: &[RuleId::Rng03],
    value: Span::ModerateBeats(8),
    provenance: Provenance::Estimate {
        reason: reason!(
            "Widor (",
            widor,
            ") p. 198 warns against sustained extreme high notes on the brass but gives no length",
        ),
    },
};

/// RNG-04: above this pitch a violin or woodwind melody wants a doubling an octave below.
pub const UNDOUBLED_HIGH_MELODY: Threshold<SoundingPitch> = Threshold {
    rules: &[RuleId::Rng04],
    value: pitch(90),
    provenance: Provenance::Estimate {
        reason: reason!(
            "Rimsky-Korsakov (",
            rimsky_korsakov,
            ") Ch. II, p. 44, doubles a violin melody an octave below from about the middle of \
                his fifth octave, C6-B6 when his octave n is read as scientific octave n + 1, \
                which three passages of his text support",
        ),
    },
};

/// RNG-04: below this pitch a melody wants a doubling an octave above.
pub const UNDOUBLED_LOW_MELODY: Threshold<SoundingPitch> = Threshold {
    rules: &[RuleId::Rng04],
    value: pitch(42),
    provenance: Provenance::Estimate {
        reason: reason!(
            "Rimsky-Korsakov (",
            rimsky_korsakov,
            ") Ch. II, p. 50, doubles a melody in the extreme bottom an octave higher; the middle \
                of his first octave, C2-B2 when his octave n is read as scientific octave n + 1, \
                which three passages of his text support",
        ),
    },
};

/// SPC-02: below this pitch the lower note of a dissonance between two parts sounds muddy.
pub const LOW_DISSONANCE: Threshold<SoundingPitch> = Threshold {
    rules: &[RuleId::Spc02],
    value: pitch(50),
    provenance: Provenance::Cited {
        source: cite!(
            rogers_big_band_voicings: "below the middle line of the bass staff, D3, keep the \
                intervals consonant",
        ),
    },
};

/// SPC-03: the most semitones a sustained bass should lie below the next part up, an octave.
pub const BASS_SPACING: Threshold<u8> = Threshold {
    rules: &[RuleId::Spc03],
    value: 12,
    provenance: Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. III, Distribution of notes in chords, p. 67: the bass should \
                rarely lie at a greater distance than an octave from the part directly above it",
        ),
    },
};

/// SPC-04: the most semitones between two adjacent upper parts of a sustained chord, an octave.
pub const MIDDLE_GAP: Threshold<u8> = Threshold {
    rules: &[RuleId::Spc04],
    value: 12,
    provenance: Provenance::Estimate {
        reason: reason!(
            "Rimsky-Korsakov (",
            rimsky_korsakov,
            ") Ch. III, pp. 68 and 78, allows no void in the intermediate parts but names no \
                interval; an octave, as he allows between the bass and the part above it",
        ),
    },
};

/// BAL-01: a harmonic part weighing less than this share of the strongest part other than the
/// melody is too weak.
pub const WEAK_CHORD_MEMBER: Threshold<f32> = Threshold {
    rules: &[RuleId::Bal01],
    value: 0.5,
    provenance: Provenance::Estimate {
        reason: reason!(
            "Rimsky-Korsakov (",
            rimsky_korsakov,
            ") Ch. III, p. 69, asks for equally balanced harmonic parts and gives no number",
        ),
    },
};

/// BAL-03: the melody should weigh at least this many times the accompaniment near it.
pub const MELODY_MARGIN: Threshold<f32> = Threshold {
    rules: &[RuleId::Bal03],
    value: 1.5,
    provenance: Provenance::Estimate {
        reason: reason!(
            "Rimsky-Korsakov (",
            rimsky_korsakov,
            ") Ch. IV, pp. 114-115, gives the harmonic basis inferior dynamic power and no number",
        ),
    },
};

/// BAL-08: the most weight an accompaniment under a solo string should have.
pub const SOLO_STRING_ACCOMPANIMENT: Threshold<f32> = Threshold {
    rules: &[RuleId::Bal08],
    value: 2.0,
    provenance: Provenance::Estimate {
        reason: reason!(
            "Rimsky-Korsakov (",
            rimsky_korsakov,
            ") Ch. IV, p. 105, asks for a light, transparent accompaniment and gives no number; \
                two woodwind players, the weight of one string section at forte",
        ),
    },
};

/// BAL-09: the fewest first violins that balance winds in pairs.
pub const FEWEST_FIRST_VIOLINS: Threshold<u8> = Threshold {
    rules: &[RuleId::Bal09],
    value: 8,
    provenance: Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, Stringed instruments, p. 6: fewer than 8 first violins is a \
                mistake, as the balance between strings and wind is completely destroyed",
        ),
    },
};

/// VOI-11: a section divided this many ways or more is too thin at forte.
pub const LOUD_DIVISI: Threshold<u8> = Threshold {
    rules: &[RuleId::Voi11],
    value: 4,
    provenance: Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. I, Stringed instruments, p. 7: division into four and more \
                parts is rare, but may be used in piano passages, as it greatly reduces the \
                volume of tone",
        ),
    },
};

/// PLY-01: how long a wind part may play without a rest, eight bars of moderate 4/4.
pub const WIND_PHRASE: Threshold<Span> = Threshold {
    rules: &[RuleId::Ply01],
    value: Span::ModerateBeats(32),
    provenance: Provenance::Estimate {
        reason: reason!(
            "Rimsky-Korsakov (",
            rimsky_korsakov,
            ") Ch. I, p. 19, asks for rests to breathe and Widor (",
            widor,
            ") p. 127 times one held saxophone note, but no source gives a phrase length without \
                a rest",
        ),
    },
};

/// PLY-03: how long a pianissimo slur may last on one bow, four bars of moderate time.
pub const SOFT_SLUR: Threshold<Span> = Threshold {
    rules: &[RuleId::Ply03],
    value: Span::ModerateBeats(16),
    provenance: Provenance::Cited {
        source: cite!(
            forsyth_second_scan: "p. 343: pianissimo held notes or slurs of a few long notes \
                last up to about 4 bars of moderate time",
        ),
    },
};

/// PLY-03: how long a pianissimo slur of many notes may last on one bow, half a soft slur.
pub const MANY_NOTE_SLUR: Threshold<Span> = Threshold {
    rules: &[RuleId::Ply03],
    value: Span::ModerateBeats(8),
    provenance: Provenance::Cited {
        source: cite!(
            forsyth_second_scan: "p. 343: when a pianissimo slur includes a large number of \
                notes, such as scales or figures, its maximum of about 4 bars of moderate time \
                must be about halved",
        ),
    },
};

/// TEX-01: a characteristic colour sounding in more than this share of a piece's bars is
/// overused.
pub const CHARACTERISTIC_COLOUR_SHARE: Threshold<f32> = Threshold {
    rules: &[RuleId::Tex01],
    value: 0.5,
    provenance: Provenance::Estimate {
        reason: reason!(
            "Rimsky-Korsakov (",
            rimsky_korsakov,
            ") Ch. IV, p. 118, warns against employing such colours over frequently and gives no \
                share; most bars, read as more than half",
        ),
    },
};

// Doubling.

/// The B♭ and A clarinets, which the handbooks call simply the clarinet.
const CLARINETS: &[InstrumentType] = &[ClarinetBb, ClarinetA];

/// The C and B♭ trumpets, which the handbooks call simply the trumpet.
const TRUMPETS: &[InstrumentType] = &[TrumpetBb, TrumpetC];

/// The tenor and bass trombones.
const TROMBONES: &[InstrumentType] = &[TromboneTenor, TromboneBass];

/// The four bowed strings.
const STRINGS: &[InstrumentType] = &[Violin, Viola, Cello, DoubleBass];

/// A member covering `ids`, uncounted.
const fn any(ids: &'static [InstrumentType]) -> Member {
    Member {
        selector: Selector::Ids(ids),
        players: None,
    }
}

/// A member of `count` players of `ids`.
const fn players(count: u8, ids: &'static [InstrumentType]) -> Member {
    Member {
        selector: Selector::Ids(ids),
        players: Some(PlayerRange::new(count, count)),
    }
}

/// A member covering the lines `selector` selects, uncounted.
const fn lines(selector: Selector) -> Member {
    Member {
        selector,
        players: None,
    }
}

/// A doubling of any line.
const fn doubling(
    members: &'static [Member],
    spacings: &'static [Spacing],
    effect: &'static str,
    prevails: Prevails,
    provenance: Provenance,
) -> NamedDoubling {
    NamedDoubling {
        members,
        spacings,
        builds_bass: false,
        effect,
        prevails,
        provenance,
    }
}

/// A doubling that builds the bass.
const fn bass(
    members: &'static [Member],
    spacings: &'static [Spacing],
    effect: &'static str,
    prevails: Prevails,
    provenance: Provenance,
) -> NamedDoubling {
    NamedDoubling {
        builds_bass: true,
        ..doubling(members, spacings, effect, prevails, provenance)
    }
}

const UNISON: &[Spacing] = &[Spacing::Unison];
const OCTAVE: &[Spacing] = &[Spacing::Octaves(1)];

const FIRST: Prevails = Prevails::Members(&[0]);
const SECOND: Prevails = Prevails::Members(&[1]);

/// Rimsky-Korsakov on the unisons of strings and wind.
const STRINGS_AND_WIND: Provenance = Provenance::Cited {
    source: cite!(
        rimsky_korsakov: "Ch. II, Combination of strings and wind, p. 58: the natural unisons of \
            strings and wood-wind are violins with flute, oboe or clarinet, violas with oboe, \
            English horn, clarinet or bassoon, cellos with clarinet, bass clarinet or bassoon, \
            and double basses with bass clarinet, bassoon or double bassoon; when the two are of \
            equal power the strings predominate",
    ),
};

/// Rimsky-Korsakov on the unisons of strings and brass.
const STRINGS_AND_BRASS: Provenance = Provenance::Cited {
    source: cite!(
        rimsky_korsakov: "Ch. II, Combination of strings and brass, p. 61: when a brass and a \
            stringed instrument progress in unison each can be heard separately; the best pairs \
            are those whose registers correspond, violin with trumpet, viola with horn, cellos \
            or double basses with trombones or tuba for heavy massive effects",
    ),
};

/// Rimsky-Korsakov on the unisons of wood-wind and brass.
const WIND_AND_BRASS: Provenance = Provenance::Cited {
    source: cite!(
        rimsky_korsakov: "Ch. II, Combination of wind and brass in unison, pp. 56-57: in a \
            unison of brass and wood-wind the brass tone predominates, and the wood-wind softens \
            it and gives it a finer legato",
    ),
};

/// Rimsky-Korsakov on two wood-wind instruments in unison.
const WOODWIND_UNISON: Provenance = Provenance::Cited {
    source: cite!(
        rimsky_korsakov: "Ch. II, Combination in unison, pp. 47-48: the colour of each unison of \
            two or more wood-wind instruments and which predominates in the low and the high \
            register",
    ),
};

/// Rimsky-Korsakov on the instruments of one branch in octaves.
const SAME_BRANCH: Provenance = Provenance::Cited {
    source: cite!(
        rimsky_korsakov: "Ch. II, Combination in octaves, p. 50: instruments of the same branch \
            playing in octaves, bassoon over double bassoon, clarinet over bass clarinet, oboe \
            over English horn, small clarinet over clarinet and flute over alto flute, always \
            produce a good effect",
    ),
};

/// Rimsky-Korsakov on strings with strings.
const STRINGS_WITH_STRINGS: Provenance = Provenance::Cited {
    source: cite!(
        rimsky_korsakov: "Ch. II, Grouping in unison, pp. 39-40, and Stringed instruments \
            doubling in octaves, pp. 40-44: the colour of each unison and octave of the string \
            sections",
    ),
};

/// Rimsky-Korsakov on mixed brass in octaves.
const BRASS_OCTAVES: Provenance = Provenance::Cited {
    source: cite!(
        rimsky_korsakov: "Ch. II, Brass in unison, in octaves, thirds and sixths, pp. 55-56: \
            mixed brass in octaves works in the normal order of register, trumpet over two \
            horns, trumpet over trombone, trombone over tuba, two trumpets over two trombones, \
            two horns over tuba; horns over trombones are not so reliable",
    ),
};

/// Every doubling the handbooks name: strings and wind, strings and brass, wood-wind and brass,
/// the colour doublings of tuned percussion, keyboards and harp, wood-wind unisons and octaves of
/// one branch, strings with strings, wood-wind with brass in octaves and chords, brass with brass,
/// the three-group unisons of a tutti, and the ways of building the bass.
pub static NAMED_DOUBLINGS: [NamedDoubling; 78] = [
    // Strings with wood-wind.
    doubling(
        &[any(&[Violin]), any(&[Flute])],
        UNISON,
        "among the best and most natural unisons of strings and wind; the flute adds resonance \
            and the strings soften it",
        FIRST,
        STRINGS_AND_WIND,
    ),
    doubling(
        &[lines(Selector::Solo(&[Violin])), any(&[Flute])],
        UNISON,
        "great purity and abundance of tone in the high register, the solo timbre kept",
        FIRST,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. IV, Soli in the strings, p. 106: a solo violin doubled in \
                    unison by a flute attains great purity and abundance of tone",
            ),
        },
    ),
    doubling(
        &[any(&[Violin]), any(&[Oboe])],
        UNISON,
        "a natural pair; one oboe added to the first and second violins only thickens them, the \
            woodwind timbre being lost",
        FIRST,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 34: violins I and II with \
                    one oboe, the wood-wind timbre being lost; Ch. II, Combination of strings \
                    and wind, p. 58: violins with oboe are a natural unison, the strings \
                    predominating when the two are of equal power",
            ),
        },
    ),
    doubling(
        &[any(&[Violin]), any(&[ClarinetBb, ClarinetA, ClarinetEb])],
        UNISON,
        "a natural pair",
        FIRST,
        STRINGS_AND_WIND,
    ),
    doubling(
        &[any(&[Viola]), any(CLARINETS)],
        UNISON,
        "a natural pair and a point of contact between the groups: the viola's timbre recalls \
            the clarinet's lowest compass",
        FIRST,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 34: the viola's timbre is \
                    vaguely compared to the lowest compass of the clarinet; Ch. II, Combination \
                    of strings and wind, p. 58: violas with clarinet are a natural unison",
            ),
        },
    ),
    doubling(
        &[any(&[Viola]), any(&[Oboe, EnglishHorn, Bassoon])],
        UNISON,
        "natural pairs",
        FIRST,
        STRINGS_AND_WIND,
    ),
    doubling(
        &[any(&[Cello]), any(&[Bassoon])],
        UNISON,
        "a natural pair; one bassoon added to the cellos and double basses only thickens them",
        FIRST,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 34: cellos and double \
                    basses with one bassoon; Ch. II, Combination of strings and wind, p. 58: \
                    cellos with bassoon are a natural unison",
            ),
        },
    ),
    doubling(
        &[any(&[Cello]), any(&[ClarinetBb, ClarinetA, BassClarinet])],
        UNISON,
        "natural pairs",
        FIRST,
        STRINGS_AND_WIND,
    ),
    doubling(
        &[
            any(&[DoubleBass]),
            any(&[BassClarinet, Bassoon, Contrabassoon]),
        ],
        UNISON,
        "natural pairs",
        FIRST,
        STRINGS_AND_WIND,
    ),
    doubling(
        &[players(2, &[Flute]), players(2, &[Oboe]), any(&[Violin])],
        UNISON,
        "the woodwind absorbs one string section: a sweet, coherent quality, the woodwind timbre \
            still predominating",
        Prevails::Members(&[0, 1]),
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 34: 2 flutes and 2 oboes \
                    with the first violins, 2 oboes and 2 clarinets with the violas, 2 clarinets \
                    and 2 bassoons with the cellos give a sweet coherent quality, the wood-wind \
                    timbre still predominating",
            ),
        },
    ),
    doubling(
        &[players(2, &[Oboe]), players(2, CLARINETS), any(&[Viola])],
        UNISON,
        "the woodwind absorbs the violas, its timbre still predominating",
        Prevails::Members(&[0, 1]),
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 34: 2 oboes and 2 \
                    clarinets with the violas, the wood-wind timbre still predominating",
            ),
        },
    ),
    doubling(
        &[players(2, CLARINETS), players(2, &[Bassoon]), any(&[Cello])],
        UNISON,
        "the woodwind absorbs the cellos, its timbre still predominating",
        Prevails::Members(&[0, 1]),
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 34: 2 clarinets and 2 \
                    bassoons with the cellos, the wood-wind timbre still predominating",
            ),
        },
    ),
    doubling(
        &[any(&[Viola]), any(&[Cello]), players(1, CLARINETS)],
        UNISON,
        "one clarinet added to the violas and cellos only thickens them",
        Prevails::Members(&[0, 1]),
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 34: violas and cellos with \
                    one clarinet; one wind instrument added to the strings only thickens them",
            ),
        },
    ),
    doubling(
        &[
            lines(Selector::Played {
                ids: STRINGS,
                articulations: &[ConSordino],
            }),
            lines(Selector::Family(Family::Woodwinds)),
        ],
        UNISON,
        "muted strings do not combine so well with woodwind: the two qualities stay apart",
        Prevails::Apart,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 34: muted strings do not \
                    combine so well with wood-wind, as the two tone qualities remain distinct",
            ),
        },
    ),
    // Strings with brass.
    doubling(
        &[any(&[HornF]), any(&[Cello])],
        UNISON,
        "a beautifully blended, soft quality of tone",
        Prevails::Blend,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination of strings and brass, p. 61: the \
                    combination of horns and cellos produces a beautifully blended, soft quality \
                    of tone",
            ),
        },
    ),
    doubling(
        &[any(&[HornF]), lines(Selector::Divided(&[Viola, Cello]))],
        &[Spacing::Unison, Spacing::Chord],
        "the splendid effect of horns doubled by divided violas or cellos",
        Prevails::Blend,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. III, Harmony in combined groups, p. 95: the splendid \
                    effect of horns doubled by divided violas or cellos",
            ),
        },
    ),
    doubling(
        &[any(&[Violin]), any(TRUMPETS)],
        UNISON,
        "the best string and brass pair for the top, but each is heard on its own",
        Prevails::Apart,
        STRINGS_AND_BRASS,
    ),
    doubling(
        &[any(&[Viola]), any(&[HornF])],
        UNISON,
        "the best string and brass pair for the middle, but each is heard on its own",
        Prevails::Apart,
        STRINGS_AND_BRASS,
    ),
    doubling(
        &[
            any(&[Cello, DoubleBass]),
            any(&[TromboneTenor, TromboneBass, Tuba]),
        ],
        UNISON,
        "for heavy, massive effects; each is heard on its own",
        Prevails::Apart,
        STRINGS_AND_BRASS,
    ),
    // Wood-wind with brass.
    doubling(
        &[any(TRUMPETS), any(&[ClarinetBb, ClarinetA, Oboe, Flute])],
        UNISON,
        "the brass tone dominates; the woodwind softens it and gives a finer legato than the \
            brass alone",
        FIRST,
        WIND_AND_BRASS,
    ),
    doubling(
        &[any(&[HornF]), any(&[ClarinetBb, ClarinetA, Bassoon])],
        UNISON,
        "the horn dominates, softened by the woodwind",
        FIRST,
        WIND_AND_BRASS,
    ),
    doubling(
        &[any(&[TromboneTenor, Tuba]), any(&[Bassoon])],
        UNISON,
        "the brass dominates, softened by the bassoon",
        FIRST,
        WIND_AND_BRASS,
    ),
    doubling(
        &[
            lines(Selector::Played {
                ids: &[HornF, TrumpetBb, TrumpetC],
                articulations: &[Stopped, ConSordino],
            }),
            any(&[Oboe, EnglishHorn]),
        ],
        &[Spacing::Unison, Spacing::Chord],
        "a magnificent tone: stopped notes resemble the oboe and the English horn",
        Prevails::Blend,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. III, Harmony in combined groups, p. 89: stopped or muted \
                    horns and trumpets with oboes and English horn give a magnificent tone, \
                    stopped notes resembling the oboe and English horn",
            ),
        },
    ),
    doubling(
        &[
            lines(Selector::Played {
                ids: &[HornF],
                articulations: &[Stopped],
            }),
            any(CLARINETS),
        ],
        &[Spacing::Chord],
        "a beautiful dark tone from stopped middle horn notes and low clarinets; with bassoons \
            instead the effect loses part of its character",
        Prevails::Blend,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. III, Harmony in combined groups, p. 89: a beautiful dark \
                    tone is derived from the combination of middle notes of stopped horns and low \
                    notes of the clarinets",
            ),
        },
    ),
    doubling(
        &[
            players(2, &[Oboe, ClarinetBb, ClarinetA, Flute]),
            players(1, &[HornF]),
        ],
        OCTAVE,
        "two woodwinds an octave above one horn take the place of a trumpet above it",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination of wind and brass in octaves, p. 57: \
                    wood-wind often replaces a trumpet above a horn, one horn below 2 oboes, 2 \
                    clarinets or 2 flutes",
            ),
        },
    ),
    doubling(
        &[
            Member {
                selector: Selector::Ids(&[Oboe, ClarinetBb, ClarinetA, Flute]),
                players: Some(PlayerRange::new(3, 4)),
            },
            players(2, &[HornF]),
        ],
        OCTAVE,
        "two horns below need three or four woodwinds above, above all in forte",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination of wind and brass in octaves, p. 57: with \
                    two horns in the lower octave three or four wind instruments are necessary \
                    above, especially in forte",
            ),
        },
    ),
    doubling(
        &[
            Member {
                selector: Selector::Ids(&[Flute, Oboe, ClarinetBb, ClarinetA]),
                players: Some(PlayerRange::new(2, 4)),
            },
            any(TRUMPETS),
        ],
        OCTAVE,
        "three or four woodwinds double a trumpet in the upper octave; in the top register two \
            flutes suffice",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination of wind and brass in octaves, p. 57: to \
                    double a trumpet in the upper octave three or four wind instruments are \
                    required, but in the top register two flutes will suffice",
            ),
        },
    ),
    doubling(
        &[
            lines(Selector::Family(Family::Brass)),
            lines(Selector::Family(Family::Woodwinds)),
        ],
        &[Spacing::Chord],
        "a chord for full brass doubled by the same chord for full woodwind in pairs gives a \
            magnificent and uniform tone",
        Prevails::Blend,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. III, Harmony in combined groups, p. 88: a chord for full \
                    brass doubled by the same chord for full wood-wind in pairs produces a \
                    magnificent and uniform tone",
            ),
        },
    ),
    // Tuned percussion, keyboards and harp.
    doubling(
        &[any(&[Glockenspiel]), any(&[Flute, Piccolo, Celesta, Harp])],
        &[
            Spacing::Unison,
            Spacing::Octaves(1),
            Spacing::Octaves(2),
            Spacing::Octaves(3),
        ],
        "the usual colour doublings of a melody, the glockenspiel in unison or one to three \
            octaves above",
        FIRST,
        Provenance::Cited {
            source: cite!(
                vsl_academy_glockenspiel: "it doubles a melody in unison or one to three octaves \
                    above; usual partners are flute, piccolo, celesta and harp",
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 33: the glockenspiel will \
                    easily prevail";
                "that it prevails in these doublings is an estimate"
            ),
        },
    ),
    doubling(
        &[any(&[Xylophone]), any(&[Celesta, Glockenspiel])],
        UNISON,
        "the xylophone dominates",
        FIRST,
        Provenance::Cited {
            source: cite!(
                vsl_academy_xylophone: "it is doubled with celesta and glockenspiel, the \
                    xylophone dominating",
            ),
        },
    ),
    doubling(
        &[
            any(&[Xylophone]),
            lines(Selector::Played {
                ids: &[Violin, Viola],
                articulations: &[Pizzicato],
            }),
        ],
        &[Spacing::Unison, Spacing::Octaves(1)],
        "sharpens the pizzicato and accents the top notes of a melody",
        FIRST,
        Provenance::Cited {
            source: cite!(
                vsl_academy_xylophone: "it doubles violins and violas, sharpening their \
                    pizzicato, and accents the top notes of a melody",
            ),
        },
    ),
    doubling(
        &[
            any(&[Celesta]),
            any(&[
                Flute,
                Harp,
                PianoConcertGrand,
                PianoStudioGrand,
                PianoUpright,
            ]),
        ],
        UNISON,
        "duplicates a theme",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                wikipedia_celesta: "it often duplicates a theme in the flute, harp or piano",
            ),
        },
    ),
    doubling(
        &[
            any(&[Harp]),
            lines(Selector::Played {
                ids: STRINGS,
                articulations: &[Pizzicato],
            }),
        ],
        UNISON,
        "the harp and pizzicato strings form one plucked group",
        Prevails::Blend,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. IV, Tutti pizzicato, p. 103: the strings pizzicato, \
                    reinforced occasionally by the harp and piano, constitute a particular kind \
                    of tutti",
            ),
        },
    ),
    doubling(
        &[
            lines(Selector::Played {
                ids: &[Harp],
                articulations: &[Harmonics],
            }),
            any(&[Flute, ClarinetBb, ClarinetA]),
        ],
        UNISON,
        "a magical colour in the middle register",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                berlioz: "harp harmonics with flutes and clarinets in the middle register",
            ),
        },
    ),
    // Wood-wind with wood-wind.
    doubling(
        &[any(&[Flute]), any(&[Oboe])],
        UNISON,
        "fuller than the flute, sweeter than the oboe",
        Prevails::ByRegister(&[0, 1]),
        WOODWIND_UNISON,
    ),
    doubling(
        &[any(&[Flute]), any(CLARINETS)],
        UNISON,
        "fuller than the flute, duller than the clarinet",
        Prevails::ByRegister(&[0, 1]),
        WOODWIND_UNISON,
    ),
    doubling(
        &[any(&[Oboe]), any(CLARINETS)],
        UNISON,
        "fuller than either: the dark, nasal oboe prevails low, the bright clarinet high; the \
            closest woodwind stand-in for a trumpet phrase that lies too high",
        Prevails::ByRegister(&[0, 1]),
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination in unison, p. 47: oboe and clarinet give a \
                    fuller quality than either, the dark, nasal oboe prevailing in the low \
                    register and the bright clarinet in the high; Ch. IV: a phrase too high for \
                    the trumpets may be entrusted to the oboes and clarinets in unison, this \
                    combination possessing the closest resemblance to the trumpet tone both in \
                    character and power",
            ),
        },
    ),
    doubling(
        &[any(&[Flute]), any(&[Oboe]), any(CLARINETS)],
        UNISON,
        "very full: the flute prevails low, the oboe in the middle, the clarinet high",
        Prevails::ByRegister(&[0, 1, 2]),
        WOODWIND_UNISON,
    ),
    doubling(
        &[any(&[Bassoon]), any(CLARINETS)],
        UNISON,
        "very full: the gloomy clarinet prevails low, the bassoon high",
        Prevails::ByRegister(&[1, 0]),
        WOODWIND_UNISON,
    ),
    doubling(
        &[any(&[Bassoon]), any(&[Oboe, Flute])],
        UNISON,
        "seldom found outside a tutti: more resonance, no new colour; the low flute or oboe \
            prevails at the bottom, the high bassoon at the top",
        Prevails::ByRegister(&[1, 0]),
        WOODWIND_UNISON,
    ),
    doubling(
        &[any(&[Flute]), any(&[Oboe]), any(CLARINETS), any(&[Bassoon])],
        UNISON,
        "a rich quality, difficult to define; rare",
        Prevails::Unstated,
        WOODWIND_UNISON,
    ),
    doubling(
        &[any(&[Bassoon]), any(&[Contrabassoon])],
        OCTAVE,
        "one branch in octaves, always good",
        Prevails::Unstated,
        SAME_BRANCH,
    ),
    doubling(
        &[any(CLARINETS), any(&[BassClarinet])],
        OCTAVE,
        "one branch in octaves, always good",
        Prevails::Unstated,
        SAME_BRANCH,
    ),
    doubling(
        &[any(&[Oboe]), any(&[EnglishHorn])],
        OCTAVE,
        "one branch in octaves, always good",
        Prevails::Unstated,
        SAME_BRANCH,
    ),
    doubling(
        &[any(&[ClarinetEb]), any(CLARINETS)],
        OCTAVE,
        "one branch in octaves, always good",
        Prevails::Unstated,
        SAME_BRANCH,
    ),
    doubling(
        &[any(&[Flute]), any(&[AltoFlute])],
        OCTAVE,
        "one branch in octaves, always good",
        Prevails::Unstated,
        SAME_BRANCH,
    ),
    doubling(
        &[any(&[Piccolo]), any(&[Flute, Oboe, ClarinetBb, ClarinetA])],
        OCTAVE,
        "the normal support of the piccolo, which is weak low",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination in octaves, p. 50: the piccolo is doubled \
                    an octave lower by a flute, an oboe or a clarinet; piccolo over flute is one \
                    branch in octaves, always good",
            ),
        },
    ),
    doubling(
        &[any(&[Piccolo]), any(&[Flute, Oboe, ClarinetBb, ClarinetA])],
        &[Spacing::Octaves(2)],
        "the piccolo two octaves above a woodwind phrase, the middle octave left out",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                forsyth: "the piccolo sometimes doubles a wood-wind phrase at the sixteenth, \
                    with the middle octave left out",
            ),
        },
    ),
    doubling(
        &[
            any(&[Piccolo]),
            any(&[Flute]),
            any(&[Oboe, ClarinetBb, ClarinetA]),
            any(&[Bassoon]),
        ],
        OCTAVE,
        "a melody spread over three or four octaves of woodwind in its natural order, such as \
            piccolo, two flutes, two oboes with a clarinet, and a bassoon",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Doubling in two, three and four octaves, p. 51: in \
                    three and four octaves the wood-wind keeps its natural order",
            ),
        },
    ),
    doubling(
        &[
            any(&[Bassoon, ClarinetBb, ClarinetA, BassClarinet]),
            any(&[Contrabassoon]),
        ],
        OCTAVE,
        "a melody in the extreme bottom doubled an octave higher",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination in octaves, p. 50: the double bassoon is \
                    doubled an octave higher by the bassoon, clarinet or bass clarinet",
            ),
        },
    ),
    // Strings with strings.
    doubling(
        &[any(&[Violin]), any(&[Violin])],
        UNISON,
        "first and second violins in unison: no change of colour, more power; a woodwind \
            doubling added cannot predominate",
        Prevails::Blend,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Violin]), any(&[Viola])],
        UNISON,
        "rich and full, the violins predominating",
        FIRST,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Viola]), any(&[Cello])],
        UNISON,
        "a rich, full resonance, the cello predominating",
        SECOND,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Violin]), any(&[Cello])],
        UNISON,
        "fuller, the cello tone prevailing",
        SECOND,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Violin]), any(&[Violin]), any(&[Viola]), any(&[Cello])],
        UNISON,
        "first and second violins, violas and cellos in unison, possible only in the alto and \
            tenor register: very tense and powerful in forte, extremely full and rich in piano",
        Prevails::Blend,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Cello]), any(&[DoubleBass])],
        UNISON,
        "a rich, full resonance, now and then for very low phrases",
        Prevails::Unstated,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Violin]), any(&[Violin])],
        OCTAVE,
        "first violins an octave above the seconds, very common above all very high: it secures \
            expression, fullness of tone and firmness of timbre",
        Prevails::Unstated,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Violin]), any(&[Viola])],
        OCTAVE,
        "common, above all when the lower octave lies below the violin's G string",
        Prevails::Unstated,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Viola]), any(&[Cello])],
        OCTAVE,
        "for when the violins are busy",
        Prevails::Unstated,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Violin]), any(&[Cello])],
        OCTAVE,
        "for expressive passages with the cellos on their A or D string; more resonant than \
            violas over cellos",
        Prevails::Unstated,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Violin]), any(&[Viola, Cello])],
        OCTAVE,
        "first and second violins in unison an octave above violas and cellos in unison: a \
            beautiful quality of tone, somewhat severe, for melodies in the middle range",
        Prevails::Unstated,
        STRINGS_WITH_STRINGS,
    ),
    doubling(
        &[any(&[Violin]), any(&[Violin]), any(&[Viola, Cello])],
        OCTAVE,
        "first violins, second violins and violas or cellos in double octaves: a very tense \
            cantabile, best in forte",
        Prevails::Unstated,
        STRINGS_WITH_STRINGS,
    ),
    // Brass with brass.
    doubling(
        &[any(TRUMPETS), players(2, &[HornF])],
        OCTAVE,
        "mixed brass in octaves in the normal order of register",
        Prevails::Unstated,
        BRASS_OCTAVES,
    ),
    doubling(
        &[any(TRUMPETS), any(TROMBONES)],
        OCTAVE,
        "mixed brass in octaves in the normal order of register",
        Prevails::Unstated,
        BRASS_OCTAVES,
    ),
    doubling(
        &[any(TROMBONES), any(&[Tuba])],
        OCTAVE,
        "mixed brass in octaves in the normal order of register",
        Prevails::Unstated,
        BRASS_OCTAVES,
    ),
    doubling(
        &[players(2, &[HornF]), players(1, TROMBONES)],
        OCTAVE,
        "horns above a trombone, not so reliable; only in octaves",
        Prevails::Unstated,
        BRASS_OCTAVES,
    ),
    doubling(
        &[
            any(&[HornF]),
            any(&[TrumpetBb, TrumpetC, TromboneTenor, TromboneBass]),
        ],
        &[Spacing::Chord],
        "horns beside the same chord in trumpets or trombones soften their penetrating timbre",
        Prevails::Blend,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. III, Duplication in the brass, pp. 85-86: horns added to \
                    the same chord in trumpets or trombones moderate their penetrating timbre",
            ),
        },
    ),
    // The three groups in a tutti.
    doubling(
        &[
            any(&[Violin]),
            any(&[Oboe, Flute, ClarinetBb, ClarinetA]),
            any(TRUMPETS),
        ],
        UNISON,
        "the most natural unison of the three groups for a melody in a tutti",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination of the three groups, p. 61: violins, oboe \
                    or flute or clarinet, and trumpet",
            ),
        },
    ),
    doubling(
        &[
            any(&[Viola, Cello]),
            any(&[ClarinetBb, ClarinetA, EnglishHorn]),
            any(&[HornF]),
        ],
        UNISON,
        "the natural unison of the three groups for a middle melody in a tutti",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination of the three groups, p. 61: violas or \
                    cellos, clarinet or English horn, and horn",
            ),
        },
    ),
    // The bass.
    bass(
        &[any(&[Cello]), any(&[DoubleBass])],
        OCTAVE,
        "the usual orchestral bass; the double-bass part may be simplified",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Stringed instruments doubling in octaves, p. 43: \
                    cellos and double basses in octaves are the usual construction of the bass, \
                    the double-bass part sometimes simplified",
            ),
        },
    ),
    bass(
        &[any(&[Cello, Bassoon]), any(&[DoubleBass])],
        UNISON,
        "the double bass rarely plays an independent part except pedal notes and usually doubles \
            the cellos or a bassoon",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Stringed instruments, p. 9: the double bass rarely \
                    plays an independent part, usually moving with the cellos or doubling the \
                    bassoon, except for pedal notes",
            ),
        },
    ),
    bass(
        &[any(&[Viola]), any(&[DoubleBass])],
        OCTAVE,
        "seldom, when the cellos are busy",
        Prevails::Unstated,
        STRINGS_WITH_STRINGS,
    ),
    bass(
        &[any(&[Viola]), any(&[DoubleBass])],
        &[Spacing::Octaves(2)],
        "the violas two octaves above the double basses reinforce their partials",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                vsl_academy_double_bass: "violas often play parallel to the double basses two \
                    octaves higher, reinforcing their partials",
            ),
        },
    ),
    bass(
        &[any(&[DoubleBass]), any(&[Contrabassoon])],
        UNISON,
        "depth: the contrabassoon doubles the double basses",
        Prevails::Unstated,
        STRINGS_AND_WIND,
    ),
    bass(
        &[any(&[TromboneTenor, TromboneBass]), any(&[Tuba])],
        OCTAVE,
        "the bass of a trombone chord: the third trombone or the bass trombone in octaves with \
            the tuba",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. III, Harmony in the brass, p. 83: the third trombone and \
                    the tuba in octaves as the bass",
                forsyth: "the bass trombone and the tuba in octaves",
            ),
        },
    ),
    bass(
        &[players(2, &[HornF]), any(&[Tuba])],
        OCTAVE,
        "two horns in unison an octave above the tuba under three trombones: a beautiful full \
            resonance",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. III, Harmony in the brass, p. 83: two horns and tuba in \
                    octaves under three trombones give a beautiful full resonance",
            ),
        },
    ),
    bass(
        &[any(&[HornF]), any(&[Bassoon])],
        OCTAVE,
        "a bassoon doubles the bass of a four-horn chord an octave below; trombone and tuba are \
            too resonant for this",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. III, pp. 83 and 90: the bass of a chord for four horns is \
                    doubled by a bassoon an octave below, the too resonant trombone and tuba \
                    seldom being used",
            ),
        },
    ),
    bass(
        &[
            any(&[Cello, DoubleBass]),
            players(2, &[Bassoon]),
            players(3, TROMBONES),
            any(&[Tuba]),
        ],
        UNISON,
        "the heavy bass unison of the three groups",
        Prevails::Unstated,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination of the three groups, p. 61: cellos and \
                    double basses, two bassoons, three trombones and tuba",
            ),
        },
    ),
];

/// What the handbooks warn not to double, each with the rules that check it.
pub static DO_NOT_DOUBLE: [Avoid; 12] = [
    Avoid {
        what: "double an expressive solo line",
        why: "doubling restricts the variety of colour and expression",
        rules: &[RuleId::Dbl07],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination in unison, p. 48: phrases demanding \
                    diversity of expression should be entrusted to solo instruments of simple \
                    timbre",
            ),
        },
    },
    Avoid {
        what: "duplicate the whole woodwind only to balance an oversized string body",
        why: "a medium string body balances the usual woodwind",
        rules: &[],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Stringed instruments, p. 6: 20 or 24 first violins \
                    overpower the customary wood-wind, and it is advisable to rely on a \
                    medium-sized body of strings; Ch. II, p. 49",
            ),
        },
    },
    Avoid {
        what: "divide the first and the second violins in octaves in a small orchestra",
        why: "it halves the players on each line and loses resonance",
        rules: &[RuleId::Dbl06],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Stringed instruments doubling in octaves, p. 41",
            ),
        },
    },
    Avoid {
        what: "put the divided halves of one viola, cello or double-bass section in octaves",
        why: "the parts lie on strings that do not correspond and unity of tone is impaired; the \
            violins are the exception",
        rules: &[RuleId::Dbl05],
        provenance: Provenance::Cited {
            source: cite!(rimsky_korsakov: "Ch. II, Note II, p. 44"),
        },
    },
    Avoid {
        what: "put two winds of one colour in octaves without strings doubling them",
        why: "playing in different registers, they do not correspond",
        rules: &[RuleId::Dbl03],
        provenance: Provenance::Cited {
            source: cite!(rimsky_korsakov: "Ch. II, Combination in octaves, pp. 49-50"),
        },
    },
    Avoid {
        what: "invert the woodwind order in a doubling, such as a bassoon above a clarinet or a \
            flute below an oboe",
        why: "a far-fetched, unnatural tone",
        rules: &[RuleId::Dbl02],
        provenance: Provenance::Cited {
            source: cite!(rimsky_korsakov: "Ch. II, p. 47"),
        },
    },
    Avoid {
        what: "double a trombone an octave above with woodwind",
        why: "trumpets are more suitable",
        rules: &[RuleId::Dbl04],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination of wind and brass in octaves, p. 57",
            ),
        },
    },
    Avoid {
        what: "double the bass upward or the tenor in wide spacing, double the bass of an \
            inverted seventh chord in an upper part, or write consecutive octaves between upper \
            parts",
        why: "the rules of harmony carry over into orchestration",
        rules: &[RuleId::Spc06, RuleId::Spc07, RuleId::Spc08],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. III, Number of harmonic parts, pp. 64-65",
            ),
        },
    },
    Avoid {
        what: "under a voice, double strings with woodwind and brass or brass with woodwind, or \
            give one harmonic part to two clarinets, two oboes or two horns in unison",
        why: "such combinations are liable to drown the singer",
        rules: &[RuleId::Msk02],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. V, Transparence of accompaniment, pp. 120-121",
            ),
        },
    },
    Avoid {
        what: "replace a missing celesta by the glockenspiel",
        why: "an upright piano is the better substitute",
        rules: &[],
        provenance: Provenance::Cited {
            source: cite!(rimsky_korsakov: "Ch. I, Piano and Celesta, p. 32"),
        },
    },
    Avoid {
        what: "use compound timbres all the time",
        why: "constant mixtures give a dull, neutral texture",
        rules: &[],
        provenance: Provenance::Cited {
            source: cite!(rimsky_korsakov: "Ch. I, Comparison of resonance, p. 35"),
        },
    },
    Avoid {
        what: "pair strings and brass in unison expecting a blend",
        why: "each can be heard separately",
        rules: &[RuleId::Dbl09],
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination of strings and brass, p. 61",
            ),
        },
    },
];

/// The natural order of the woodwinds from the top, with the auxiliary instruments in their
/// branches.
pub const WOODWIND_ORDER: WoodwindOrder = WoodwindOrder {
    branches: [
        &[Piccolo, Flute, AltoFlute, BassFlute],
        &[Oboe, OboeDamore, EnglishHorn],
        &[ClarinetEb, ClarinetBb, ClarinetA, BassClarinet],
        &[Bassoon, Contrabassoon],
    ],
    provenance: Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. II, p. 47: the normal order of the wood-wind, which produces the \
                most natural resonance, is flutes, oboes, clarinets, bassoons; p. 50: piccolo \
                and alto flute with the flutes, English horn with the oboes, small and bass \
                clarinets with the clarinets, double bassoon with the bassoons";
            "the bass flute with the flutes and the oboe d'amore with the oboes is an estimate"
        ),
    },
};

// Compile-time checks of the doubling tables.
const _: () = {
    let mut i = 0;
    while i < NAMED_DOUBLINGS.len() {
        assert!(
            NAMED_DOUBLINGS[i].check().is_ok(),
            "a named doubling is invalid"
        );
        i += 1;
    }
    let mut i = 0;
    while i < DO_NOT_DOUBLE.len() {
        assert!(DO_NOT_DOUBLE[i].check().is_ok(), "a warning is invalid");
        i += 1;
    }
    assert!(
        WOODWIND_ORDER.check().is_ok(),
        "the woodwind order is invalid"
    );
};

// Weights, register factors and low-interval limits.

/// Where the register bands come from.
pub(in crate::orchestration) const REGISTER_BANDS: Provenance = Provenance::Estimate {
    reason: reason!(
        "Rimsky-Korsakov (",
        rimsky_korsakov,
        ") numbers octaves in prose and names the strings' registers (Ch. II, pp. 36-38); his \
            octave n is read as scientific octave n + 1, which three passages of his text support",
    ),
};

/// How strongly each line sounds at each dynamic band, the most specific rows first: a solo or a
/// pizzicato string matches its own row before the arco section's.
pub static WEIGHTS: [WeightRow; 17] = [
    WeightRow {
        selector: Selector::Solo(STRINGS),
        soft: Some(0.5),
        medium: Some(0.5),
        loud: Some(0.5),
        provenance: Provenance::Estimate {
            reason: reason!(
                "Rimsky-Korsakov (",
                rimsky_korsakov,
                ") Ch. IV, Soli in the strings, p. 105, calls the solo tone comparatively weak \
                    and gives no number",
            ),
        },
    },
    WeightRow {
        selector: Selector::Played {
            ids: STRINGS,
            articulations: &[Pizzicato, ColLegno, ColLegnoBattuto],
        },
        soft: Some(0.5),
        medium: Some(0.5),
        loud: Some(0.5),
        provenance: Provenance::Estimate {
            reason: reason!(
                "Rimsky-Korsakov (",
                rimsky_korsakov,
                ") Ch. I, Comparison of resonance, pp. 33-34: pizzicato and col legno strings \
                    are easily overpowered by the sustaining groups; no number",
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(STRINGS),
        soft: Some(1.0),
        medium: Some(1.4),
        loud: Some(2.0),
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 33: in a medium orchestra \
                    the first violins equal 2 flutes, or 1 oboe and 1 clarinet, in forte, and 1 \
                    flute in piano";
                "the other sections taken to weigh as the first violins, the mf weight as the \
                    geometric middle, and the weight of a larger or smaller section in proportion \
                    to its players are estimates"
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[Flute, Oboe, ClarinetBb, ClarinetA, Bassoon]),
        soft: Some(1.0),
        medium: Some(1.0),
        loud: Some(1.0),
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 33: 1 horn equals 2 \
                    clarinets, 2 oboes, 2 flutes or 2 bassoons in forte, and in piano all wind \
                    instruments are of fairly equal balance; one of them is the unit",
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[
            EnglishHorn,
            OboeDamore,
            ClarinetEb,
            BassClarinet,
            AltoFlute,
            BassFlute,
            Piccolo,
            Contrabassoon,
        ]),
        soft: Some(1.0),
        medium: Some(1.0),
        loud: Some(1.0),
        provenance: Provenance::Estimate {
            reason: reason!(
                "the same group as the flute, oboe, clarinet and bassoon, which Rimsky-Korsakov (",
                rimsky_korsakov,
                ") Ch. I, p. 33, makes the unit; he gives no separate figure, and the register \
                    factors adjust their weak and strong registers",
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[
            SaxophoneSoprano,
            SaxophoneAlto,
            SaxophoneTenor,
            SaxophoneBaritone,
        ]),
        soft: Some(1.0),
        medium: Some(1.4),
        loud: Some(2.0),
        provenance: Provenance::Estimate {
            reason: "no source that was read gives a ratio; placed with the horn",
        },
    },
    WeightRow {
        selector: Selector::Ids(&[HornF]),
        soft: Some(1.0),
        medium: Some(1.4),
        loud: Some(2.0),
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 33: 1 horn equals 2 \
                    woodwinds in forte, and all wind instruments are of fairly equal balance in \
                    piano";
                "the mf weight is the geometric middle of the soft and loud ones"
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[WagnerTubaTenor, WagnerTubaBass, Euphonium, Flugelhorn]),
        soft: Some(1.0),
        medium: Some(1.4),
        loud: Some(2.0),
        provenance: Provenance::Estimate {
            reason: "conical brass close to the horn; no source that was read gives a ratio",
        },
    },
    WeightRow {
        selector: Selector::Ids(&[TrumpetBb, TrumpetC, TrumpetPiccolo]),
        soft: Some(1.0),
        medium: Some(2.0),
        loud: Some(4.0),
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 33: 1 trumpet equals 1 \
                    trombone, 1 tuba or 2 horns in forte, and all wind instruments are of fairly \
                    equal balance in piano";
                "the piccolo trumpet taken to weigh as the trumpet and the mf weight as the \
                    geometric middle are estimates"
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[Cornet]),
        soft: Some(1.0),
        medium: Some(1.7),
        loud: Some(3.0),
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, brass remarks, pp. 22-24: cornets have not quite the \
                    same force as trumpets and are softer and weaker";
                "the numbers, between the horn and the trumpet in forte, are an estimate"
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[TromboneTenor, TromboneBass, Tuba, Cimbasso]),
        soft: Some(1.0),
        medium: Some(2.0),
        loud: Some(4.0),
        provenance: Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Comparison of resonance, p. 33: 1 trumpet equals 1 \
                    trombone or 1 tuba in forte, and all wind instruments are of fairly equal \
                    balance in piano";
                "the cimbasso taken to weigh as the tuba and the mf weight as the geometric \
                    middle are estimates"
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[Harp]),
        soft: Some(0.5),
        medium: Some(0.5),
        loud: Some(0.5),
        provenance: Provenance::Estimate {
            reason: reason!(
                "Rimsky-Korsakov (",
                rimsky_korsakov,
                ") Ch. I, Harp, p. 29, needs three or four harps against a full orchestra in \
                    forte and calls it never a very powerful instrument; no number",
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[Celesta]),
        soft: Some(0.5),
        medium: Some(0.5),
        loud: None,
        provenance: Provenance::Estimate {
            reason: reason!(
                "Rimsky-Korsakov (",
                rimsky_korsakov,
                ") Ch. I, pp. 33-34: the celesta is easily overpowered by the sustaining groups; \
                    no number, and none for forte",
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[PianoConcertGrand, PianoStudioGrand, PianoUpright]),
        soft: Some(0.5),
        medium: Some(0.5),
        loud: None,
        provenance: Provenance::Estimate {
            reason: reason!(
                "Rimsky-Korsakov (",
                rimsky_korsakov,
                ") Ch. I, pp. 33-34: the piano played softly is easily overpowered by the \
                    sustaining groups; no number, and none for the piano played loud",
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[Glockenspiel, Xylophone, TubularBells, Crotales]),
        soft: Some(2.0),
        medium: Some(3.0),
        loud: Some(4.0),
        provenance: Provenance::Estimate {
            reason: reason!(
                "Rimsky-Korsakov (",
                rimsky_korsakov,
                ") Ch. I, pp. 33-34: the glockenspiel, bells and xylophone will easily prevail; \
                    no number",
            ),
        },
    },
    WeightRow {
        selector: Selector::Ids(&[Timpani]),
        soft: Some(1.0),
        medium: Some(2.0),
        loud: Some(4.0),
        provenance: Provenance::Estimate {
            reason: reason!(
                "Rimsky-Korsakov (",
                rimsky_korsakov,
                ") Ch. I, pp. 33-34: the kettle-drums will easily prevail; set equal to the \
                    trumpet",
            ),
        },
    },
    WeightRow {
        selector: Selector::Family(Family::Choir),
        soft: Some(1.0),
        medium: Some(1.0),
        loud: Some(1.5),
        provenance: Provenance::Estimate {
            reason: "no source that was read gives a ratio for voices; how an accompaniment \
                covers a voice is the masking rules' concern",
        },
    },
];

/// One step of notated dynamic as a weight factor: a part marked one step louder weighs twice as
/// much, one step softer half.
pub const DYNAMIC_STEP: Threshold<f32> = Threshold {
    rules: &[RuleId::Bal02, RuleId::Bal04],
    value: 2.0,
    provenance: Provenance::Estimate {
        reason: reason!(
            "Rimsky-Korsakov (",
            rimsky_korsakov,
            ") Ch. III, pp. 84-85, marks single horns one degree louder in place of doubling them, \
                so one step is worth about a factor of 2 in his arithmetic",
        ),
    },
};

/// MSK-03 and VOI-06: a register of this factor or less is weak.
pub const WEAK_REGISTER: Threshold<f32> = Threshold {
    rules: &[RuleId::Msk03, RuleId::Voi06],
    value: 0.5,
    provenance: Provenance::Estimate {
        reason: "the factor the register factors give a register that the handbooks call weak \
            or easily covered",
    },
};

/// VOI-06: a register of this factor or more is strong.
pub const STRONG_REGISTER: Threshold<f32> = Threshold {
    rules: &[RuleId::Voi06],
    value: 1.25,
    provenance: Provenance::Estimate {
        reason: "the smallest factor the register factors give a register that the handbooks \
            call prevailing or powerful",
    },
};

/// MSK-03: an accompaniment of this weight or more covers a melody in a weak register.
pub const MASKING_ACCOMPANIMENT: Threshold<f32> = Threshold {
    rules: &[RuleId::Msk03],
    value: 1.0,
    provenance: Provenance::Estimate {
        reason: "one woodwind player in the middle register, the unit of the weights",
    },
};

/// The percussion that, at fortissimo, overpowers any tutti.
pub const HEAVY_PERCUSSION: Combination = Combination {
    members: &[
        Selector::Ids(&[BassDrum]),
        Selector::Ids(&[ClashCymbals]),
        Selector::Ids(&[TamTam]),
        Selector::Played {
            ids: &[SnareDrum],
            articulations: &[Roll],
        },
    ],
    effect: "the bass drum, cymbals, tam-tam and a snare-drum roll together at fortissimo \
        overpower any orchestral tutti",
    provenance: Provenance::Cited {
        source: cite!(
            rimsky_korsakov: "Ch. IV, Use of percussion instruments, p. 117: the bass drum, \
                cymbals, gong and a tremolo on the side drum, played fortissimo, are sufficient \
                to overpower any orchestral tutti",
        ),
    },
};

/// A register factor.
const fn factor(
    instrument: InstrumentType,
    low: i64,
    high: i64,
    factor: f32,
    applies: Applies,
    provenance: Provenance,
) -> RegisterFactor {
    RegisterFactor {
        instrument,
        register: PitchRange::new(low, high),
        factor,
        applies,
        provenance,
    }
}

/// How registers change the weights. The factors are estimates that encode what the handbooks say
/// in words; each row cites the statement it encodes.
pub static REGISTER_FACTORS: [RegisterFactor; 13] = [
    factor(
        Flute,
        59,
        67,
        0.5,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                forsyth: "the flute's low register, C4 to about A4, is thick and heavy and easily \
                    covered, and double basses blur it",
                widor: "in combination the flute only begins to tell from G or A upwards";
                "the factor and the span, B3 to G4, are estimates"
            ),
        },
    ),
    factor(
        Flute,
        79,
        98,
        1.5,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                forsyth: "from G5 up the flute is bright and telling";
                "the factor is an estimate"
            ),
        },
    ),
    factor(
        AltoFlute,
        55,
        64,
        1.0,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                forsyth: "the alto flute's bottom sixth is remarkably strong",
                wikipedia_alto_flute: "a greater dynamic presence in the bottom octave and a \
                    half";
                "the factor stays 1, unlike the flute's weak low register, because the register \
                    is still light in absolute terms; an estimate"
            ),
        },
    ),
    factor(
        Piccolo,
        74,
        96,
        0.5,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, wood-wind remarks, p. 20: in its low and middle range \
                    the piccolo's tone is so much weaker than the flute's that it is of little \
                    service there",
                vsl_academy_piccolo: "registers D5-G5, G#5-C7 and C#7-C8";
                "the factor is an estimate"
            ),
        },
    ),
    factor(
        Piccolo,
        97,
        108,
        2.0,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, wood-wind remarks, p. 20: the piccolo's highest compass \
                    is extraordinarily powerful",
                vsl_academy_piccolo: "the upper register C#7-C8, piercing in forte";
                "the factor is an estimate"
            ),
        },
    ),
    factor(
        ClarinetEb,
        55,
        73,
        0.5,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, wood-wind remarks, p. 20: in its low and middle range \
                    the small clarinet's tone is so much weaker than the clarinet's that it is of \
                    little service there",
                wikipedia_cs_klarinet: "the clarinet's chalumeau and throat registers, written \
                    E3 to B♭4, which sound G3 to D♭5 on the E♭ clarinet";
                "the factor is an estimate"
            ),
        },
    ),
    factor(
        ClarinetBb,
        63,
        68,
        0.5,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination in unison, p. 48: the clarinet is weak in \
                    the middle compass",
                forsyth: "the throat notes are the very worst part of the instrument, the weakest \
                    in quality, almost inaudible in pp",
                wikipedia_cs_klarinet: "the weak throat register, written F4 to B♭4, which \
                    sounds E♭4 to A♭4 on the B♭ clarinet",
                wikipedia_register_music: "the clarinet's throat register";
                "the factor is an estimate"
            ),
        },
    ),
    factor(
        ClarinetA,
        62,
        67,
        0.5,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination in unison, p. 48: the clarinet is weak in \
                    the middle compass",
                wikipedia_cs_klarinet: "the weak throat register, written F4 to B♭4, which \
                    sounds D4 to G4 on the A clarinet";
                "the factor is an estimate"
            ),
        },
    ),
    factor(
        HornF,
        35,
        42,
        0.5,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                forsyth: "the fourth horn tends to drop out of sight in the bottom fifth of its \
                    compass";
                "the factor and the span, the fifth above the playable bottom, are estimates"
            ),
        },
    ),
    factor(
        Contrabassoon,
        22,
        28,
        1.5,
        Applies::At(DynamicBand::Soft),
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, wood-wind remarks, p. 20: the very deep notes of the \
                    double bassoon are remarkably thick and dense, very powerful in piano \
                    passages",
                vsl_academy_contrabassoon: "the lower register B♭0 to E1";
                "the factor is an estimate"
            ),
        },
    ),
    factor(
        Oboe,
        58,
        65,
        1.25,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination in unison, p. 47: the dark, nasal tone of \
                    the oboe prevails over the clarinet in the low register",
                forsyth_second_scan: "the oboe's low fifth is coarse and over-reedy";
                "the factor and the span, the low fifth B♭3 to F4, are estimates"
            ),
        },
    ),
    factor(
        Bassoon,
        59,
        75,
        1.25,
        Applies::Always,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. II, Combination in unison, p. 48: the sickly quality of \
                    the bassoon prevails over the clarinet in the higher register",
                vsl_academy_bassoon: "the upper register B3 to E♭5";
                "the factor is an estimate"
            ),
        },
    ),
    factor(
        Harp,
        23,
        71,
        0.5,
        Applies::InFastFigures,
        Provenance::Cited {
            source: cite!(
                rimsky_korsakov: "Ch. I, Harp, p. 28: in the middle and lower octaves the strings \
                    ring on, so more or less rapid figures are clear only in the upper register, \
                    where the strings are shorter";
                "the factor and the span, below his upper octave C5, are estimates"
            ),
        },
    ),
];

/// The low-interval limits, by semitones from the minor second to the major ninth.
pub const LOW_INTERVAL_LIMITS: IntervalLimits = IntervalLimits {
    rows: &[
        lowest(1, 52),
        lowest(2, 51),
        lowest(3, 48),
        lowest(4, 46),
        lowest(5, 45),
        lowest(6, 46),
        lowest(7, 34),
        lowest(8, 41),
        lowest(9, 41),
        lowest(10, 41),
        lowest(11, 41),
        IntervalLimit {
            semitones: 12,
            limit: Limit::Unlimited,
        },
        lowest(13, 40),
        lowest(14, 39),
    ],
    provenance: Provenance::Estimate {
        reason: reason!(
            "the commonly taught arranging guideline, checked against Rimsky-Korsakov (",
            rimsky_korsakov,
            ") Ch. IV, p. 106, for whom the lowest octaves, below MIDI 48, sound bad in close \
                intervals, and Rogers (",
            rogers_big_band_voicings,
            "), who keeps intervals below D3 consonant; the charts of Hoffmann (",
            hoffmann_low_interval_limits,
            ") and Sleepfreaks (",
            sleepfreaks_low_interval_limit,
            ") are images that were not read, and published charts disagree, notably on the \
                tritone and the fifth",
        ),
    },
};

/// The limit of the interval of `semitones`: its lower note no lower than `note`.
const fn lowest(semitones: u8, note: i64) -> IntervalLimit {
    IntervalLimit {
        semitones,
        limit: Limit::Lowest(pitch(note)),
    }
}

// Compile-time checks of the weight, register and limit tables: every row checks out, and an
// instrument's registers do not overlap, so a note has one factor.
const _: () = {
    let mut i = 0;
    while i < WEIGHTS.len() {
        assert!(WEIGHTS[i].check().is_ok(), "a weight row is invalid");
        i += 1;
    }
    let mut i = 0;
    while i < REGISTER_FACTORS.len() {
        let row = REGISTER_FACTORS[i];
        assert!(row.check().is_ok(), "a register factor is invalid");
        let mut j = i + 1;
        while j < REGISTER_FACTORS.len() {
            let other = REGISTER_FACTORS[j];
            assert!(
                row.instrument as u8 != other.instrument as u8
                    || row.register.high().get() < other.register.low().get()
                    || other.register.high().get() < row.register.low().get(),
                "two registers of one instrument overlap"
            );
            j += 1;
        }
        i += 1;
    }
    assert!(
        HEAVY_PERCUSSION.check().is_ok(),
        "the heavy percussion is invalid"
    );
    assert!(
        LOW_INTERVAL_LIMITS.check().is_ok(),
        "the low-interval limits are invalid"
    );
};
