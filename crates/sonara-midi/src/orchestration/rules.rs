//! Orchestration rules as data: a catalogue of checkable rules for doubling, balance, spacing,
//! voicing and playability, and the thresholds their checks compare with.
//!
//! - [`RULES`] holds the 67 rules, keyed by [`RuleId`] (`RNG-01` to `TEX-06`), each with what it
//!   checks, the inputs it needs, how serious a finding is and where the rule comes from.
//! - [`NAMED_DOUBLINGS`] lists the doublings the handbooks name and the colour they give,
//!   [`DO_NOT_DOUBLE`] what not to double, and [`WOODWIND_ORDER`] the natural order of the
//!   woodwinds from the top.
//! - Named thresholds, such as [`SUSTAINED_HIGH_BRASS`], hold the numbers the checks compare
//!   with, each with its own provenance and the rules that use it.
//!
//! The rules describe the music, not a sample library or the application: the predicates that
//! check a project are code elsewhere, and this module holds what they read. Every rule says
//! whether a source states it (`Cited`) or not (`Estimate`); most numbers are estimates that encode
//! what the handbooks say in words. The statics are checked at compile time, so invalid data does
//! not build; the row types offer the same checks (`check`) for data built at run time.
//!
//! Nothing here runs on a real-time thread (RT-01).

use thiserror::Error;

use super::articulation::Articulation;
use super::data;
use super::ensemble_profile::PlayerRange;
use super::id::InstrumentType;
use super::instrument::{Family, Provenance, is_stated};

pub use super::data::rules::{
    BASS_SPACING, CHARACTERISTIC_COLOUR_SHARE, DO_NOT_DOUBLE, FEWEST_FIRST_VIOLINS, LOUD_DIVISI,
    LOW_DISSONANCE, MANY_NOTE_SLUR, MELODY_MARGIN, MIDDLE_GAP, NAMED_DOUBLINGS, RULES, SOFT_SLUR,
    SOLO_STRING_ACCOMPANIMENT, SUSTAINED_HIGH_BRASS, UNDOUBLED_HIGH_MELODY, UNDOUBLED_LOW_MELODY,
    WEAK_CHORD_MEMBER, WIND_PHRASE, WOODWIND_ORDER,
};

/// The eight groups of rules, by what they check. A rule's code starts with its group's prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuleGroup {
    /// Notes against the instrument's ranges (`RNG`).
    Range,
    /// The spacing of chords and the low-interval limits (`SPC`).
    Spacing,
    /// Doubling a line in unison or octaves (`DBL`).
    Doubling,
    /// The balance of parts against each other (`BAL`).
    Balance,
    /// One part covering another, above all a voice or a solo (`MSK`).
    Masking,
    /// How chords are laid out among the instruments (`VOI`).
    Voicing,
    /// What a player can play: breath, bow, mutes, pedals, retuning (`PLY`).
    Playability,
    /// The texture of a whole passage or piece (`TEX`).
    Texture,
}

impl RuleGroup {
    /// The eight groups, in the order of their rules in [`RuleId::ALL`].
    pub const ALL: [Self; 8] = [
        Self::Range,
        Self::Spacing,
        Self::Doubling,
        Self::Balance,
        Self::Masking,
        Self::Voicing,
        Self::Playability,
        Self::Texture,
    ];

    /// The prefix of the group's rule codes, such as `RNG`.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Range => "RNG",
            Self::Spacing => "SPC",
            Self::Doubling => "DBL",
            Self::Balance => "BAL",
            Self::Masking => "MSK",
            Self::Voicing => "VOI",
            Self::Playability => "PLY",
            Self::Texture => "TEX",
        }
    }

    /// The number of rules in the group.
    #[must_use]
    pub const fn rule_count(self) -> u8 {
        match self {
            Self::Range | Self::Masking => 4,
            Self::Spacing => 8,
            Self::Doubling | Self::Balance => 9,
            Self::Voicing => 12,
            Self::Playability => 15,
            Self::Texture => 6,
        }
    }

    /// The place in [`RuleId::ALL`] of the group's first rule.
    const fn first(self) -> u8 {
        let mut first = 0;
        let mut i = 0;
        while i < Self::ALL.len() {
            if Self::ALL[i] as u8 == self as u8 {
                return first;
            }
            first += Self::ALL[i].rule_count();
            i += 1;
        }
        first
    }
}

/// The identifier of a rule. [`RuleId::code`] is the external form, such as `RNG-01`, and never
/// changes; the variants are named after it and listed in [`RuleId::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum RuleId {
    /// `RNG-01`: a note outside the playable range.
    Rng01,
    /// `RNG-02`: a note outside the comfortable range.
    Rng02,
    /// `RNG-03`: sustained extreme high brass.
    Rng03,
    /// `RNG-04`: an extreme-register melody left undoubled.
    Rng04,
    /// `SPC-01`: the low-interval limit.
    Spc01,
    /// `SPC-02`: dissonance in the low bass.
    Spc02,
    /// `SPC-03`: the bass too far below the harmony.
    Spc03,
    /// `SPC-04`: a hole in the middle of a chord.
    Spc04,
    /// `SPC-05`: spacing against the harmonic series.
    Spc05,
    /// `SPC-06`: the bass doubled upward or mixed into the upper parts.
    Spc06,
    /// `SPC-07`: consecutive octaves between upper parts.
    Spc07,
    /// `SPC-08`: the bass of an inverted seventh chord doubled.
    Spc08,
    /// `DBL-01`: a unison pairing the tables do not name.
    Dbl01,
    /// `DBL-02`: the woodwind order inverted in a doubling.
    Dbl02,
    /// `DBL-03`: same-colour winds in octaves without strings.
    Dbl03,
    /// `DBL-04`: a woodwind octave above a trombone.
    Dbl04,
    /// `DBL-05`: divided low strings in octaves.
    Dbl05,
    /// `DBL-06`: violins divided in octaves with too few players.
    Dbl06,
    /// `DBL-07`: an expressive solo doubled.
    Dbl07,
    /// `DBL-08`: a double-bass phrase without support.
    Dbl08,
    /// `DBL-09`: strings and brass in unison expected to blend.
    Dbl09,
    /// `BAL-01`: unequal chord members.
    Bal01,
    /// `BAL-02`: single horns against trumpets or trombones.
    Bal02,
    /// `BAL-03`: the melody not on top of the weight.
    Bal03,
    /// `BAL-04`: woodwinds under-weighted against brass.
    Bal04,
    /// `BAL-05`: a non-sustaining melody against a sustained forte.
    Bal05,
    /// `BAL-06`: heavy percussion at fortissimo.
    Bal06,
    /// `BAL-07`: a family without its own bass.
    Bal07,
    /// `BAL-08`: a solo string over heavy accompaniment.
    Bal08,
    /// `BAL-09`: a string body too small for the winds.
    Bal09,
    /// `MSK-01`: accompaniment in the voice's register.
    Msk01,
    /// `MSK-02`: heavy doublings under a voice.
    Msk02,
    /// `MSK-03`: a weak register exposed.
    Msk03,
    /// `MSK-04`: a harmonic basis too close to the melody.
    Msk04,
    /// `VOI-01`: a three-part woodwind chord in three colours.
    Voi01,
    /// `VOI-02`: a close four-part chord in four colours.
    Voi02,
    /// `VOI-03`: an enclosure inside one colour.
    Voi03,
    /// `VOI-04`: doubling that changes within a passage.
    Voi04,
    /// `VOI-05`: a discord in one colour.
    Voi05,
    /// `VOI-06`: non-corresponding registers in one chord.
    Voi06,
    /// `VOI-07`: horn pairs broken.
    Voi07,
    /// `VOI-08`: clarinets inside horns, or horns inside bassoons.
    Voi08,
    /// `VOI-09`: woodwinds below the trumpets in a combined chord.
    Voi09,
    /// `VOI-10`: uneven string divisi.
    Voi10,
    /// `VOI-11`: divisi in four or more at forte.
    Voi11,
    /// `VOI-12`: sustained multi-stop string chords.
    Voi12,
    /// `PLY-01`: wind breath.
    Ply01,
    /// `PLY-02`: brass endurance.
    Ply02,
    /// `PLY-03`: bow length.
    Ply03,
    /// `PLY-04`: time for a mute change.
    Ply04,
    /// `PLY-05`: the switch from pizzicato to arco.
    Ply05,
    /// `PLY-06`: pizzicato speed.
    Ply06,
    /// `PLY-07`: the harp pedal setting.
    Ply07,
    /// `PLY-08`: harp pedal changes.
    Ply08,
    /// `PLY-09`: the shape of a harp chord.
    Ply09,
    /// `PLY-10`: fast harp figures low.
    Ply10,
    /// `PLY-11`: timpani retuning.
    Ply11,
    /// `PLY-12`: timpani thirds.
    Ply12,
    /// `PLY-13`: horn tessitura jumps.
    Ply13,
    /// `PLY-14`: fast repeated notes and tonguing.
    Ply14,
    /// `PLY-15`: clarinet octave leaps.
    Ply15,
    /// `TEX-01`: characteristic colours overused.
    Tex01,
    /// `TEX-02`: percussion all together, or never resting.
    Tex02,
    /// `TEX-03`: a tutti at pianissimo.
    Tex03,
    /// `TEX-04`: an echo or imitation against the register.
    Tex04,
    /// `TEX-05`: the order of an orchestral crescendo.
    Tex05,
    /// `TEX-06`: a gap at a hand-over.
    Tex06,
}

impl RuleId {
    /// Every rule, grouped by [`RuleGroup`] in its order and numbered from 1 within each group.
    pub const ALL: [Self; 67] = [
        Self::Rng01,
        Self::Rng02,
        Self::Rng03,
        Self::Rng04,
        Self::Spc01,
        Self::Spc02,
        Self::Spc03,
        Self::Spc04,
        Self::Spc05,
        Self::Spc06,
        Self::Spc07,
        Self::Spc08,
        Self::Dbl01,
        Self::Dbl02,
        Self::Dbl03,
        Self::Dbl04,
        Self::Dbl05,
        Self::Dbl06,
        Self::Dbl07,
        Self::Dbl08,
        Self::Dbl09,
        Self::Bal01,
        Self::Bal02,
        Self::Bal03,
        Self::Bal04,
        Self::Bal05,
        Self::Bal06,
        Self::Bal07,
        Self::Bal08,
        Self::Bal09,
        Self::Msk01,
        Self::Msk02,
        Self::Msk03,
        Self::Msk04,
        Self::Voi01,
        Self::Voi02,
        Self::Voi03,
        Self::Voi04,
        Self::Voi05,
        Self::Voi06,
        Self::Voi07,
        Self::Voi08,
        Self::Voi09,
        Self::Voi10,
        Self::Voi11,
        Self::Voi12,
        Self::Ply01,
        Self::Ply02,
        Self::Ply03,
        Self::Ply04,
        Self::Ply05,
        Self::Ply06,
        Self::Ply07,
        Self::Ply08,
        Self::Ply09,
        Self::Ply10,
        Self::Ply11,
        Self::Ply12,
        Self::Ply13,
        Self::Ply14,
        Self::Ply15,
        Self::Tex01,
        Self::Tex02,
        Self::Tex03,
        Self::Tex04,
        Self::Tex05,
        Self::Tex06,
    ];

    /// The stable code, such as `RNG-01`: the group's prefix, a hyphen and the two-digit number.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Rng01 => "RNG-01",
            Self::Rng02 => "RNG-02",
            Self::Rng03 => "RNG-03",
            Self::Rng04 => "RNG-04",
            Self::Spc01 => "SPC-01",
            Self::Spc02 => "SPC-02",
            Self::Spc03 => "SPC-03",
            Self::Spc04 => "SPC-04",
            Self::Spc05 => "SPC-05",
            Self::Spc06 => "SPC-06",
            Self::Spc07 => "SPC-07",
            Self::Spc08 => "SPC-08",
            Self::Dbl01 => "DBL-01",
            Self::Dbl02 => "DBL-02",
            Self::Dbl03 => "DBL-03",
            Self::Dbl04 => "DBL-04",
            Self::Dbl05 => "DBL-05",
            Self::Dbl06 => "DBL-06",
            Self::Dbl07 => "DBL-07",
            Self::Dbl08 => "DBL-08",
            Self::Dbl09 => "DBL-09",
            Self::Bal01 => "BAL-01",
            Self::Bal02 => "BAL-02",
            Self::Bal03 => "BAL-03",
            Self::Bal04 => "BAL-04",
            Self::Bal05 => "BAL-05",
            Self::Bal06 => "BAL-06",
            Self::Bal07 => "BAL-07",
            Self::Bal08 => "BAL-08",
            Self::Bal09 => "BAL-09",
            Self::Msk01 => "MSK-01",
            Self::Msk02 => "MSK-02",
            Self::Msk03 => "MSK-03",
            Self::Msk04 => "MSK-04",
            Self::Voi01 => "VOI-01",
            Self::Voi02 => "VOI-02",
            Self::Voi03 => "VOI-03",
            Self::Voi04 => "VOI-04",
            Self::Voi05 => "VOI-05",
            Self::Voi06 => "VOI-06",
            Self::Voi07 => "VOI-07",
            Self::Voi08 => "VOI-08",
            Self::Voi09 => "VOI-09",
            Self::Voi10 => "VOI-10",
            Self::Voi11 => "VOI-11",
            Self::Voi12 => "VOI-12",
            Self::Ply01 => "PLY-01",
            Self::Ply02 => "PLY-02",
            Self::Ply03 => "PLY-03",
            Self::Ply04 => "PLY-04",
            Self::Ply05 => "PLY-05",
            Self::Ply06 => "PLY-06",
            Self::Ply07 => "PLY-07",
            Self::Ply08 => "PLY-08",
            Self::Ply09 => "PLY-09",
            Self::Ply10 => "PLY-10",
            Self::Ply11 => "PLY-11",
            Self::Ply12 => "PLY-12",
            Self::Ply13 => "PLY-13",
            Self::Ply14 => "PLY-14",
            Self::Ply15 => "PLY-15",
            Self::Tex01 => "TEX-01",
            Self::Tex02 => "TEX-02",
            Self::Tex03 => "TEX-03",
            Self::Tex04 => "TEX-04",
            Self::Tex05 => "TEX-05",
            Self::Tex06 => "TEX-06",
        }
    }

    /// The rule with code `code`, or `None`. The match is exact: `rng-01` or `RNG-1` is not a
    /// code.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|rule| rule.code() == code)
    }

    /// The rule's group.
    #[must_use]
    pub const fn group(self) -> RuleGroup {
        let index = self as u8;
        let mut i = 0;
        while i + 1 < RuleGroup::ALL.len() {
            if index < RuleGroup::ALL[i + 1].first() {
                return RuleGroup::ALL[i];
            }
            i += 1;
        }
        RuleGroup::Texture
    }

    /// The rule's number within its group, from 1.
    #[must_use]
    pub const fn number(self) -> u8 {
        self as u8 - self.group().first() + 1
    }

    /// The rule's record.
    #[must_use]
    pub fn get(self) -> &'static Rule {
        data::rule_record(self)
    }
}

// Compile-time checks: the groups' sizes add up to the catalogue, `ALL` lists every variant once in
// declaration order, and each code is the group's prefix, a hyphen and the rule's two-digit number,
// so the codes are unique and `from_code` cannot confuse two rules.
const _: () = {
    let mut total = 0;
    let mut g = 0;
    while g < RuleGroup::ALL.len() {
        total += RuleGroup::ALL[g].rule_count() as usize;
        g += 1;
    }
    assert!(
        total == RuleId::ALL.len(),
        "the groups hold every rule once"
    );
    let mut i = 0;
    while i < RuleId::ALL.len() {
        let rule = RuleId::ALL[i];
        assert!(
            rule as usize == i,
            "RuleId::ALL lists every variant once, in declaration order"
        );
        assert!(
            rule.number() >= 1 && rule.number() <= rule.group().rule_count(),
            "a rule's number lies within its group"
        );
        assert!(
            is_code(rule.code(), rule.group().prefix(), rule.number()),
            "a rule code is its group's prefix, a hyphen and its two-digit number"
        );
        i += 1;
    }
};

/// Whether `code` is `prefix`, a hyphen and `number` in two digits.
const fn is_code(code: &str, prefix: &str, number: u8) -> bool {
    let (code, prefix) = (code.as_bytes(), prefix.as_bytes());
    if number > 99 || code.len() != prefix.len() + 3 {
        return false;
    }
    let mut i = 0;
    while i < prefix.len() {
        if code[i] != prefix[i] {
            return false;
        }
        i += 1;
    }
    let at = prefix.len();
    code[at] == b'-' && code[at + 1] == b'0' + number / 10 && code[at + 2] == b'0' + number % 10
}

/// How serious a finding is, from the mildest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Level {
    /// Style or colour advice that a composer may override on purpose.
    Hint,
    /// Playable, but a handbook says it fails: balance, masking or endurance.
    Warning,
    /// Unplayable or out of range.
    Error,
}

impl Level {
    /// The slug, such as `warning`.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Hint => "hint",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// A band of notated dynamics, the resolution at which the sources compare loudness.
///
/// The sources name the soft end (pp to p) and the loud end (f to ff), and the weights add mf
/// between them. Where mp falls is an estimate: it lies between p and mf and takes the middle band.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DynamicBand {
    /// pp and p.
    Soft,
    /// mp and mf.
    Medium,
    /// f and ff.
    Loud,
}

impl DynamicBand {
    /// The three bands, from the softest.
    pub const ALL: [Self; 3] = [Self::Soft, Self::Medium, Self::Loud];
}

/// How a rule grades its findings.
///
/// Which level a source's rule deserves is an estimate throughout: a judgement of how firmly the
/// source states it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Severity {
    /// The same level in every case.
    Fixed(Level),
    /// `soft` in the soft and middle bands, `loud` at f and ff: the sources name the risk at forte.
    ByDynamic {
        /// The level from pp to mf.
        soft: Level,
        /// The level at f and ff; stronger than `soft`.
        loud: Level,
    },
    /// `usual`, raised to `raised` in the case `case` describes, which the rule's check tells
    /// apart.
    ByCase {
        /// The level in the usual case.
        usual: Level,
        /// The level in the case `case` describes; stronger than `usual`.
        raised: Level,
        /// The case that raises the level, in words.
        case: &'static str,
    },
}

impl Severity {
    /// The level of a finding in `band`; `in_case` says whether the finding is the case a
    /// [`Severity::ByCase`] raises, and is ignored otherwise.
    #[must_use]
    pub const fn level(self, band: DynamicBand, in_case: bool) -> Level {
        match self {
            Self::Fixed(level) => level,
            Self::ByDynamic { soft, loud } => match band {
                DynamicBand::Loud => loud,
                DynamicBand::Soft | DynamicBand::Medium => soft,
            },
            Self::ByCase { usual, raised, .. } => {
                if in_case {
                    raised
                } else {
                    usual
                }
            }
        }
    }

    /// The strongest level the rule can give.
    #[must_use]
    pub const fn highest(self) -> Level {
        match self {
            Self::Fixed(level) => level,
            Self::ByDynamic { loud, .. } => loud,
            Self::ByCase { raised, .. } => raised,
        }
    }

    /// Whether a graded severity rises from its usual to its raised level and names its case.
    const fn is_valid(self) -> bool {
        match self {
            Self::Fixed(_) => true,
            Self::ByDynamic { soft, loud } => (soft as u8) < (loud as u8),
            Self::ByCase {
                usual,
                raised,
                case,
            } => (usual as u8) < (raised as u8) && !is_blank(case),
        }
    }
}

/// What a rule needs to know about the music to check it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleInput {
    /// The instrument type each part plays.
    Instrument,
    /// The notes: sounding pitch, start, length and velocity.
    Notes,
    /// The notated dynamic of each part or passage.
    Dynamic,
    /// The tempo.
    Tempo,
    /// The metre.
    Meter,
    /// The part each line belongs to: first or second violins, which horn, solo or section, the
    /// players, divisi and mutes.
    Part,
    /// The role of each line: melody, counter-melody, harmony, bass, rhythm, colour or pedal.
    Role,
    /// The chord at each moment and which part holds which chord member.
    Harmony,
    /// The articulation of each note or passage.
    Articulation,
    /// The weights of the parts: how strongly each sounds against the others.
    Weights,
    /// The register factors: how a register changes an instrument's weight.
    RegisterFactors,
    /// The setting of the harp's pedals.
    HarpPedals,
    /// The pitch each timpano is tuned to.
    TimpaniTuning,
}

/// The values of a rule, checked by [`Rule::try_new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleSpec {
    /// A short name, such as "Note outside the playable range".
    pub title: &'static str,
    /// What the check looks for, in words, with the numbers it compares.
    pub check: &'static str,
    /// What the check needs to know, each listed once.
    pub inputs: &'static [RuleInput],
    /// How serious a finding is.
    pub severity: Severity,
    /// Where the rule comes from, and which of its numbers are estimated.
    pub provenance: Provenance,
}

/// Why a rule, or a row of the tables the rules read, is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RuleError {
    /// A title, check, case, effect or warning is empty or blank.
    #[error("a text is empty or blank")]
    BlankText,
    /// A provenance is empty or blank.
    #[error("a provenance is empty or blank")]
    EmptyProvenance,
    /// A rule lists no input.
    #[error("the rule lists no input")]
    NoInputs,
    /// A rule lists an input twice.
    #[error("the rule lists an input twice")]
    DuplicateInput,
    /// A graded severity does not rise from its usual level to its raised one.
    #[error("the severity does not rise from its usual level")]
    InvalidSeverity,
    /// A selector names no instrument or one instrument twice, names a family no instrument has,
    /// or plays no articulation or one articulation twice.
    #[error("a selector is invalid")]
    InvalidSelector,
    /// A doubling has fewer than two members, a member counted with no players, no spacing, a
    /// spacing twice or of no octaves, or names a prevailing member it does not have.
    #[error("a doubling is invalid")]
    InvalidDoubling,
    /// The woodwind order has an empty branch, an instrument that is not a woodwind, or one
    /// instrument twice.
    #[error("the woodwind order is invalid")]
    InvalidOrder,
}

/// A rule of the catalogue: what it checks, the inputs it needs, how serious a finding is and
/// where the rule comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    id: RuleId,
    spec: RuleSpec,
}

impl Rule {
    /// Builds the rule `id` after checking `spec`.
    ///
    /// # Errors
    /// [`RuleError`], the first failing check in this order:
    /// [`BlankText`](RuleError::BlankText) for a blank title or check;
    /// [`NoInputs`](RuleError::NoInputs); [`DuplicateInput`](RuleError::DuplicateInput);
    /// [`InvalidSeverity`](RuleError::InvalidSeverity) for a graded severity that does not rise or
    /// a blank case; [`EmptyProvenance`](RuleError::EmptyProvenance).
    pub const fn try_new(id: RuleId, spec: RuleSpec) -> Result<Self, RuleError> {
        if is_blank(spec.title) || is_blank(spec.check) {
            return Err(RuleError::BlankText);
        }
        if spec.inputs.is_empty() {
            return Err(RuleError::NoInputs);
        }
        if has_duplicate_input(spec.inputs) {
            return Err(RuleError::DuplicateInput);
        }
        if !spec.severity.is_valid() {
            return Err(RuleError::InvalidSeverity);
        }
        if !is_stated(spec.provenance) {
            return Err(RuleError::EmptyProvenance);
        }
        Ok(Self { id, spec })
    }

    /// Like [`Rule::try_new`]; an invalid rule in a `const` item is a compile error.
    ///
    /// # Panics
    /// When [`Rule::try_new`] would fail.
    #[must_use]
    pub const fn new(id: RuleId, spec: RuleSpec) -> Self {
        match Self::try_new(id, spec) {
            Ok(rule) => rule,
            Err(_) => panic!("invalid rule"),
        }
    }

    /// The rule's id.
    #[must_use]
    pub const fn id(&self) -> RuleId {
        self.id
    }

    /// The rule's code, such as `RNG-01`.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.id.code()
    }

    /// The checked values of the rule, as [`Rule::try_new`] received them.
    #[must_use]
    pub const fn spec(&self) -> RuleSpec {
        self.spec
    }

    /// A short name, such as "Note outside the playable range".
    #[must_use]
    pub const fn title(&self) -> &'static str {
        self.spec.title
    }

    /// What the check looks for, in words, with the numbers it compares.
    #[must_use]
    pub const fn check(&self) -> &'static str {
        self.spec.check
    }

    /// What the check needs to know.
    #[must_use]
    pub const fn inputs(&self) -> &'static [RuleInput] {
        self.spec.inputs
    }

    /// How serious a finding is.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.spec.severity
    }

    /// Where the rule comes from, and which of its numbers are estimated.
    #[must_use]
    pub const fn provenance(&self) -> Provenance {
        self.spec.provenance
    }
}

/// Whether `inputs` lists an input twice.
const fn has_duplicate_input(inputs: &[RuleInput]) -> bool {
    let mut i = 0;
    while i < inputs.len() {
        let mut j = i + 1;
        while j < inputs.len() {
            if inputs[i] as u8 == inputs[j] as u8 {
                return true;
            }
            j += 1;
        }
        i += 1;
    }
    false
}

/// How many play a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Forces {
    /// The usual forces: a section of strings or voices, one wind or percussion player.
    Usual,
    /// One player of an instrument usually played by a section, such as a solo violin.
    Solo,
    /// A section divided into several parts.
    Divided,
}

/// One line as a selector sees it: the instrument, how it is played and by how many.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Playing {
    /// The instrument.
    pub instrument: InstrumentType,
    /// The articulation, or `None` for the instrument's ordinary sound.
    pub articulation: Option<Articulation>,
    /// How many play the line.
    pub forces: Forces,
}

impl Playing {
    /// `instrument` played in its ordinary way by its usual forces.
    #[must_use]
    pub const fn ordinary(instrument: InstrumentType) -> Self {
        Self {
            instrument,
            articulation: None,
            forces: Forces::Usual,
        }
    }
}

/// Which lines a row of a table is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selector {
    /// Any of these instruments, however played.
    Ids(&'static [InstrumentType]),
    /// Any instrument of the family.
    Family(Family),
    /// Any of these instruments playing one of these articulations.
    Played {
        /// The instruments.
        ids: &'static [InstrumentType],
        /// The articulations.
        articulations: &'static [Articulation],
    },
    /// One player of one of these instruments ([`Forces::Solo`]).
    Solo(&'static [InstrumentType]),
    /// A divided section of one of these instruments ([`Forces::Divided`]).
    Divided(&'static [InstrumentType]),
}

impl Selector {
    /// Whether the selector covers the line `playing`.
    #[must_use]
    pub fn matches(self, playing: Playing) -> bool {
        match self {
            Self::Ids(ids) => ids.contains(&playing.instrument),
            Self::Family(family) => playing.instrument.family() == family,
            Self::Played { ids, articulations } => {
                ids.contains(&playing.instrument)
                    && playing
                        .articulation
                        .is_some_and(|articulation| articulations.contains(&articulation))
            }
            Self::Solo(ids) => playing.forces == Forces::Solo && ids.contains(&playing.instrument),
            Self::Divided(ids) => {
                playing.forces == Forces::Divided && ids.contains(&playing.instrument)
            }
        }
    }

    /// The instruments the selector names; empty for a [`Selector::Family`].
    #[must_use]
    pub const fn ids(self) -> &'static [InstrumentType] {
        match self {
            Self::Ids(ids) | Self::Solo(ids) | Self::Divided(ids) | Self::Played { ids, .. } => ids,
            Self::Family(_) => &[],
        }
    }

    /// Checks the selector.
    ///
    /// # Errors
    /// [`RuleError::InvalidSelector`] when it names no instrument or one twice, names
    /// [`Family::Other`], which no instrument has, or plays no articulation or one twice.
    pub const fn check(self) -> Result<(), RuleError> {
        let valid = match self {
            Self::Family(family) => !matches!(family, Family::Other),
            Self::Played { ids, articulations } => {
                is_type_set(ids) && is_articulation_set(articulations)
            }
            Self::Ids(ids) | Self::Solo(ids) | Self::Divided(ids) => is_type_set(ids),
        };
        if valid {
            Ok(())
        } else {
            Err(RuleError::InvalidSelector)
        }
    }
}

/// Whether `ids` is non-empty and names each instrument once.
const fn is_type_set(ids: &[InstrumentType]) -> bool {
    if ids.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < ids.len() {
        let mut j = i + 1;
        while j < ids.len() {
            if ids[i] as u8 == ids[j] as u8 {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Whether `articulations` is non-empty and names each articulation once.
const fn is_articulation_set(articulations: &[Articulation]) -> bool {
    if articulations.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < articulations.len() {
        let mut j = i + 1;
        while j < articulations.len() {
            if articulations[i] as u8 == articulations[j] as u8 {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

/// One member of a doubling: the lines it covers and, where the source counts them, how many play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    /// The lines.
    pub selector: Selector,
    /// How many players or parts the source counts, such as two oboes; `None` when it does not
    /// count them.
    pub players: Option<PlayerRange>,
}

/// How the members of a doubling lie against each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Spacing {
    /// On the same pitches.
    Unison,
    /// Each member this many octaves above the next one in the list: the members are listed from
    /// the top.
    Octaves(u8),
    /// As the notes of a chord rather than one line.
    Chord,
}

/// Which members of a doubling the ear hears most.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prevails {
    /// The members at these places in the list, which sound as one colour over the rest.
    Members(&'static [u8]),
    /// The members blend into one colour.
    Blend,
    /// Each member is heard on its own; they do not blend.
    Apart,
    /// It depends on the register: the members at these places prevail from the lowest register
    /// to the highest.
    ByRegister(&'static [u8]),
    /// The sources do not say.
    Unstated,
}

/// A doubling a source names, with the colour or effect it gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NamedDoubling {
    /// The members, at least two; from the top when they lie in octaves.
    pub members: &'static [Member],
    /// The spacings the source names, each once.
    pub spacings: &'static [Spacing],
    /// Whether the doubling builds the bass, such as the cellos and double basses in octaves;
    /// otherwise it doubles any line.
    pub builds_bass: bool,
    /// The colour or effect, in words.
    pub effect: &'static str,
    /// Which members the ear hears most.
    pub prevails: Prevails,
    /// Where the doubling comes from.
    pub provenance: Provenance,
}

impl NamedDoubling {
    /// Checks the row.
    ///
    /// # Errors
    /// [`RuleError::InvalidSelector`] for an invalid member selector;
    /// [`RuleError::InvalidDoubling`] for fewer than two members, a member counted with no players,
    /// no spacing, a spacing listed twice or of no octaves, or a prevailing member it does not
    /// have; [`RuleError::BlankText`] for a blank effect; [`RuleError::EmptyProvenance`].
    pub const fn check(&self) -> Result<(), RuleError> {
        if self.members.len() < 2 {
            return Err(RuleError::InvalidDoubling);
        }
        let mut i = 0;
        while i < self.members.len() {
            let member = self.members[i];
            if let Err(error) = member.selector.check() {
                return Err(error);
            }
            if let Some(players) = member.players
                && players.min() == 0
            {
                return Err(RuleError::InvalidDoubling);
            }
            i += 1;
        }
        if !are_spacings_valid(self.spacings) || !self.prevails_is_valid() {
            return Err(RuleError::InvalidDoubling);
        }
        if is_blank(self.effect) {
            return Err(RuleError::BlankText);
        }
        if !is_stated(self.provenance) {
            return Err(RuleError::EmptyProvenance);
        }
        Ok(())
    }

    /// Whether every member the row says prevails is one of its members, named at least once
    /// (twice for a register-dependent answer).
    const fn prevails_is_valid(&self) -> bool {
        let (indices, fewest) = match self.prevails {
            Prevails::Members(indices) => (indices, 1),
            Prevails::ByRegister(indices) => (indices, 2),
            Prevails::Blend | Prevails::Apart | Prevails::Unstated => return true,
        };
        if indices.len() < fewest {
            return false;
        }
        let mut i = 0;
        while i < indices.len() {
            if indices[i] as usize >= self.members.len() {
                return false;
            }
            i += 1;
        }
        true
    }
}

/// Whether `spacings` is non-empty, lists each spacing once and has no octave spacing of zero.
const fn are_spacings_valid(spacings: &[Spacing]) -> bool {
    if spacings.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < spacings.len() {
        if let Spacing::Octaves(0) = spacings[i] {
            return false;
        }
        let mut j = i + 1;
        while j < spacings.len() {
            if same_spacing(spacings[i], spacings[j]) {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Whether two spacings are the same.
const fn same_spacing(a: Spacing, b: Spacing) -> bool {
    match (a, b) {
        (Spacing::Unison, Spacing::Unison) | (Spacing::Chord, Spacing::Chord) => true,
        (Spacing::Octaves(a), Spacing::Octaves(b)) => a == b,
        _ => false,
    }
}

/// Something a source warns not to double, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Avoid {
    /// What not to do.
    pub what: &'static str,
    /// Why, in words.
    pub why: &'static str,
    /// The rules that check it; empty when no rule does.
    pub rules: &'static [RuleId],
    /// Where the warning comes from.
    pub provenance: Provenance,
}

impl Avoid {
    /// Checks the row.
    ///
    /// # Errors
    /// [`RuleError::BlankText`] for a blank text; [`RuleError::EmptyProvenance`].
    pub const fn check(&self) -> Result<(), RuleError> {
        if is_blank(self.what) || is_blank(self.why) {
            return Err(RuleError::BlankText);
        }
        if !is_stated(self.provenance) {
            return Err(RuleError::EmptyProvenance);
        }
        Ok(())
    }
}

/// The natural order of the woodwinds from the top: flutes, oboes, clarinets, bassoons. A doubling
/// that puts a lower branch above a higher one sounds unnatural.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WoodwindOrder {
    /// The four branches, from the top, each with the instruments that belong to it.
    pub branches: [&'static [InstrumentType]; 4],
    /// Where the order comes from.
    pub provenance: Provenance,
}

impl WoodwindOrder {
    /// The place of `instrument`'s branch, 0 for the flutes at the top; `None` for an instrument
    /// in no branch, such as a saxophone.
    #[must_use]
    pub fn rank(&self, instrument: InstrumentType) -> Option<usize> {
        self.branches
            .iter()
            .position(|branch| branch.contains(&instrument))
    }

    /// Checks the order.
    ///
    /// # Errors
    /// [`RuleError::InvalidOrder`] for an empty branch, an instrument that is not a woodwind, or
    /// an instrument listed twice; [`RuleError::EmptyProvenance`].
    pub const fn check(&self) -> Result<(), RuleError> {
        let mut seen = [false; InstrumentType::ALL.len()];
        let mut b = 0;
        while b < self.branches.len() {
            let branch = self.branches[b];
            if branch.is_empty() {
                return Err(RuleError::InvalidOrder);
            }
            let mut i = 0;
            while i < branch.len() {
                let instrument = branch[i];
                if !matches!(instrument.family(), Family::Woodwinds) || seen[instrument as usize] {
                    return Err(RuleError::InvalidOrder);
                }
                seen[instrument as usize] = true;
                i += 1;
            }
            b += 1;
        }
        if !is_stated(self.provenance) {
            return Err(RuleError::EmptyProvenance);
        }
        Ok(())
    }
}

/// A number a rule's check compares with, such as two bars for sustained high brass, with the rules
/// that use it and where it comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Threshold<T> {
    /// The rules whose checks compare with it.
    pub rules: &'static [RuleId],
    /// The number.
    pub value: T,
    /// Where it comes from. Most thresholds are estimates that turn a source's words into a
    /// number.
    pub provenance: Provenance,
}

/// Whether `text` is empty or holds only ASCII whitespace.
const fn is_blank(text: &str) -> bool {
    text.trim_ascii().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_ids_number_within_their_groups() {
        // TST-10: the first and last rule of each group, written from the catalogue.
        let cases = [
            (RuleId::Rng01, RuleGroup::Range, 1),
            (RuleId::Rng04, RuleGroup::Range, 4),
            (RuleId::Spc01, RuleGroup::Spacing, 1),
            (RuleId::Spc08, RuleGroup::Spacing, 8),
            (RuleId::Dbl09, RuleGroup::Doubling, 9),
            (RuleId::Bal01, RuleGroup::Balance, 1),
            (RuleId::Msk04, RuleGroup::Masking, 4),
            (RuleId::Voi12, RuleGroup::Voicing, 12),
            (RuleId::Ply01, RuleGroup::Playability, 1),
            (RuleId::Ply15, RuleGroup::Playability, 15),
            (RuleId::Tex06, RuleGroup::Texture, 6),
        ];
        for (rule, group, number) in cases {
            assert_eq!(rule.group(), group, "{rule:?}");
            assert_eq!(rule.number(), number, "{rule:?}");
        }
    }

    #[test]
    fn codes_are_checked_digit_by_digit() {
        assert!(is_code("RNG-01", "RNG", 1));
        assert!(is_code("PLY-15", "PLY", 15));
        assert!(!is_code("RNG-1", "RNG", 1));
        assert!(!is_code("RNG-02", "RNG", 1));
        assert!(!is_code("RNG_01", "RNG", 1));
        assert!(!is_code("SPC-01", "RNG", 1));
        assert!(!is_code("RNG-001", "RNG", 1));
    }

    #[test]
    fn severities_grade_by_band_and_case() {
        let fixed = Severity::Fixed(Level::Error);
        let by_dynamic = Severity::ByDynamic {
            soft: Level::Hint,
            loud: Level::Warning,
        };
        let by_case = Severity::ByCase {
            usual: Level::Warning,
            raised: Level::Error,
            case: "more than eight notes",
        };
        for band in DynamicBand::ALL {
            assert_eq!(fixed.level(band, true), Level::Error);
            assert_eq!(by_case.level(band, false), Level::Warning);
            assert_eq!(by_case.level(band, true), Level::Error);
        }
        // The middle band takes the soft level; the case flag does not move a dynamic grade.
        assert_eq!(by_dynamic.level(DynamicBand::Soft, true), Level::Hint);
        assert_eq!(by_dynamic.level(DynamicBand::Medium, false), Level::Hint);
        assert_eq!(by_dynamic.level(DynamicBand::Loud, false), Level::Warning);
        assert_eq!(fixed.highest(), Level::Error);
        assert_eq!(by_dynamic.highest(), Level::Warning);
        assert_eq!(by_case.highest(), Level::Error);
        assert!(Level::Hint < Level::Warning && Level::Warning < Level::Error);
    }

    #[test]
    fn graded_severities_must_rise() {
        assert!(Severity::Fixed(Level::Hint).is_valid());
        assert!(
            !Severity::ByDynamic {
                soft: Level::Warning,
                loud: Level::Warning,
            }
            .is_valid()
        );
        assert!(
            !Severity::ByDynamic {
                soft: Level::Warning,
                loud: Level::Hint,
            }
            .is_valid()
        );
        assert!(
            !Severity::ByCase {
                usual: Level::Hint,
                raised: Level::Warning,
                case: " ",
            }
            .is_valid()
        );
        assert!(
            !Severity::ByCase {
                usual: Level::Error,
                raised: Level::Warning,
                case: "a case",
            }
            .is_valid()
        );
    }

    #[test]
    fn spacings_are_checked() {
        assert!(are_spacings_valid(&[Spacing::Unison, Spacing::Octaves(1)]));
        assert!(are_spacings_valid(&[
            Spacing::Octaves(1),
            Spacing::Octaves(2)
        ]));
        assert!(!are_spacings_valid(&[]));
        assert!(!are_spacings_valid(&[Spacing::Octaves(0)]));
        assert!(!are_spacings_valid(&[Spacing::Chord, Spacing::Chord]));
        assert!(!are_spacings_valid(&[
            Spacing::Octaves(2),
            Spacing::Octaves(2)
        ]));
    }
}
