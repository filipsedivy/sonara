//! The rule catalogue, one record per [`RuleId`], and the thresholds its checks compare with.
//!
//! Each rule cites the handbook passage it comes from; the numbers a check compares with are named
//! thresholds with their own provenance, most of them estimates that turn a source's words into a
//! number. How serious a finding is follows how firmly the source states the rule, an estimate
//! throughout.

use super::sources::{cite, reason};
use crate::orchestration::instrument::Provenance;
use crate::orchestration::pitch::SoundingPitch;
use crate::orchestration::playability::Span;
use crate::orchestration::rules::RuleInput::{
    Articulation, Dynamic, Harmony, HarpPedals, Instrument, Meter, Notes, Part, RegisterFactors,
    Role, Tempo, TimpaniTuning, Weights,
};
use crate::orchestration::rules::{Level, Rule, RuleId, RuleInput, RuleSpec, Severity, Threshold};

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
