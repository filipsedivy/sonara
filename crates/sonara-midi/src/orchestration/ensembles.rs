//! Ensemble templates: the parts of an orchestra or a smaller group in the order its score prints
//! them, with their players, the sizes of the string sections and where each part sits in the
//! stereo image.
//!
//! A template is a recipe, not project data. It names instrument types only, never a sample
//! library, plugin or preset: choosing a sound for each part and creating the tracks, folders and
//! buses is the application's work. Each template says where its roster comes from.
//!
//! A part always plays an [`InstrumentType`], so every consumer can look its record up. Parts a
//! score calls for that no type covers are left out rather than invented, and the template's
//! provenance names them: the late-Romantic orchestra's bass trumpet, contrabass trombone,
//! cowbells and hammer, the film orchestra's synthesizer layer, and the big band's guitar and drum
//! kit. Where a near type stands in for an instrument of the period, such as the horn in F for the
//! natural horn, the part plays it and its note says so.

use thiserror::Error;

use super::data;
use super::ensemble_profile::PlayerRange;
use super::id::{InstrumentType, is_snake_case, same_text};
use super::instrument::{Provenance, is_stated};

/// A typed identifier of an ensemble template, such as the Romantic orchestra.
///
/// The string id ([`EnsembleId::id`]) is the external form and never changes; the variants are in
/// the order of [`EnsembleId::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum EnsembleId {
    /// The Classical orchestra of Haydn and Mozart (`classical_orchestra`).
    ClassicalOrchestra,
    /// Beethoven's orchestra (`beethoven_orchestra`).
    BeethovenOrchestra,
    /// The Romantic orchestra of Brahms and Tchaikovsky (`romantic_orchestra`).
    RomanticOrchestra,
    /// The large late-Romantic orchestra of Wagner, Mahler and Strauss
    /// (`late_romantic_orchestra`).
    LateRomanticOrchestra,
    /// A modern film and scoring orchestra (`film_orchestra`).
    FilmOrchestra,
    /// A chamber orchestra (`chamber_orchestra`).
    ChamberOrchestra,
    /// A string orchestra (`string_orchestra`).
    StringOrchestra,
    /// A big band (`big_band`).
    BigBand,
    /// A chorus and orchestra with vocal soloists (`choir_and_orchestra`).
    ChoirAndOrchestra,
    /// A string quartet (`string_quartet`).
    StringQuartet,
    /// A wind quintet (`wind_quintet`).
    WindQuintet,
    /// A brass quintet (`brass_quintet`).
    BrassQuintet,
    /// A piano trio (`piano_trio`).
    PianoTrio,
}

impl EnsembleId {
    /// Every ensemble template: the orchestras from the Classical to the film orchestra, the
    /// chamber and string orchestras, the big band, chorus and orchestra, then the small chamber
    /// groups.
    pub const ALL: [Self; 13] = [
        Self::ClassicalOrchestra,
        Self::BeethovenOrchestra,
        Self::RomanticOrchestra,
        Self::LateRomanticOrchestra,
        Self::FilmOrchestra,
        Self::ChamberOrchestra,
        Self::StringOrchestra,
        Self::BigBand,
        Self::ChoirAndOrchestra,
        Self::StringQuartet,
        Self::WindQuintet,
        Self::BrassQuintet,
        Self::PianoTrio,
    ];

    /// The stable identifier, for example `romantic_orchestra`: lower-case `snake_case`, never
    /// renamed.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::ClassicalOrchestra => "classical_orchestra",
            Self::BeethovenOrchestra => "beethoven_orchestra",
            Self::RomanticOrchestra => "romantic_orchestra",
            Self::LateRomanticOrchestra => "late_romantic_orchestra",
            Self::FilmOrchestra => "film_orchestra",
            Self::ChamberOrchestra => "chamber_orchestra",
            Self::StringOrchestra => "string_orchestra",
            Self::BigBand => "big_band",
            Self::ChoirAndOrchestra => "choir_and_orchestra",
            Self::StringQuartet => "string_quartet",
            Self::WindQuintet => "wind_quintet",
            Self::BrassQuintet => "brass_quintet",
            Self::PianoTrio => "piano_trio",
        }
    }

    /// The template with identifier `id`, or `None` when no template has it. The match is exact:
    /// a display name (`Romantic orchestra`) or any other spelling is not an id.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|ensemble| ensemble.id() == id)
    }

    /// The template's record.
    #[must_use]
    pub fn get(self) -> &'static Ensemble {
        data::ensemble(self)
    }
}

// Compile-time checks: `ALL` lists every variant once in declaration order, and the ids are unique
// lower-case `snake_case`, so `from_id` and the record lookup cannot confuse two templates.
const _: () = {
    let all = EnsembleId::ALL;
    let mut i = 0;
    while i < all.len() {
        assert!(
            all[i] as usize == i,
            "EnsembleId::ALL lists every variant once, in declaration order"
        );
        assert!(
            is_snake_case(all[i].id()),
            "an ensemble id is non-empty lower-case snake_case"
        );
        let mut j = i + 1;
        while j < all.len() {
            assert!(
                !same_text(all[i].id(), all[j].id()),
                "two ensembles share an id"
            );
            j += 1;
        }
        i += 1;
    }
};

/// One of the five string sections of an orchestra.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StringSection {
    /// The first violins.
    ViolinsI,
    /// The second violins.
    ViolinsII,
    /// The violas.
    Violas,
    /// The cellos.
    Cellos,
    /// The double basses.
    Basses,
}

impl StringSection {
    /// The five sections, in score order: the order of [`StringSize::counts`].
    pub const ALL: [Self; 5] = [
        Self::ViolinsI,
        Self::ViolinsII,
        Self::Violas,
        Self::Cellos,
        Self::Basses,
    ];

    /// The instrument the section plays.
    #[must_use]
    pub const fn instrument_type(self) -> InstrumentType {
        match self {
            Self::ViolinsI | Self::ViolinsII => InstrumentType::Violin,
            Self::Violas => InstrumentType::Viola,
            Self::Cellos => InstrumentType::Cello,
            Self::Basses => InstrumentType::DoubleBass,
        }
    }
}

/// A size of the string body: the players of each section, such as Rimsky-Korsakov's full
/// orchestra of 16, 14, 12, 10 and 8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StringSize {
    /// The stable identifier within an ensemble, lower-case `snake_case`, such as `full`.
    pub slug: &'static str,
    /// The display name, such as "Full".
    pub name: &'static str,
    /// The players of each section, in the order of [`StringSection::ALL`]: first violins,
    /// second violins, violas, cellos, double basses. Every section has at least one.
    pub counts: [u8; 5],
    /// Where the counts come from.
    pub provenance: Provenance,
}

impl StringSize {
    /// The players of `section`.
    #[must_use]
    pub const fn count(&self, section: StringSection) -> u8 {
        let [violins_1, violins_2, violas, cellos, basses] = self.counts;
        match section {
            StringSection::ViolinsI => violins_1,
            StringSection::ViolinsII => violins_2,
            StringSection::Violas => violas,
            StringSection::Cellos => cellos,
            StringSection::Basses => basses,
        }
    }

    /// The players of the whole string body.
    #[must_use]
    pub const fn players(&self) -> u16 {
        let [violins_1, violins_2, violas, cellos, basses] = self.counts;
        violins_1 as u16 + violins_2 as u16 + violas as u16 + cellos as u16 + basses as u16
    }
}

/// How the strings of an orchestra sit, from the conductor's left to right. The other sections
/// sit the same in both.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Seating {
    /// First and second violins together on the left, then violas and cellos, the basses behind
    /// the cellos: the layout most orchestras have used since the 1920s.
    American,
    /// First and second violins on opposite sides, cellos and violas between them: the layout of
    /// the 18th and 19th centuries, which composers up to Mahler wrote for.
    Antiphonal,
}

impl Seating {
    /// Both seatings.
    pub const ALL: [Self; 2] = [Self::American, Self::Antiphonal];
}

/// Where a part sits in the stereo image, as a track pan from −1 (hard left) to +1 (hard right),
/// seen from the audience, for each [`Seating`].
///
/// A source recorded with the players in their places carries the stage image in its samples, so
/// whether to apply the pan is the caller's decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pan {
    /// The pan in the American seating.
    pub american: f32,
    /// The pan in the antiphonal seating.
    pub antiphonal: f32,
}

impl Pan {
    /// The centre, in both seatings.
    pub const CENTRE: Self = Self::same(0.0);

    /// The pan `pan` in both seatings.
    #[must_use]
    pub const fn same(pan: f32) -> Self {
        Self {
            american: pan,
            antiphonal: pan,
        }
    }

    /// The pan in `seating`; `None` for an ensemble without a choice of seating, whose parts sit
    /// the same in both.
    #[must_use]
    pub const fn at(self, seating: Option<Seating>) -> f32 {
        match seating {
            Some(Seating::Antiphonal) => self.antiphonal,
            Some(Seating::American) | None => self.american,
        }
    }

    /// Whether both values lie in −1..=+1 (and neither is NaN).
    const fn is_within(self) -> bool {
        is_pan(self.american) && is_pan(self.antiphonal)
    }

    /// Whether the two seatings place the part differently.
    const fn depends_on_seating(self) -> bool {
        // Bit equality: the data writes one literal for both seatings when the place does not
        // depend on the seating.
        self.american.to_bits() != self.antiphonal.to_bits()
    }
}

/// Whether `pan` lies in −1..=+1; `false` for NaN.
const fn is_pan(pan: f32) -> bool {
    pan >= -1.0 && pan <= 1.0
}

/// Who plays a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Players {
    /// Players of its own: one for a wind part, a range such as 16 to 20 singers for a choir
    /// section. At least one.
    Own(PlayerRange),
    /// A string section, whose players the chosen [`StringSize`] gives.
    Section(StringSection),
    /// No players of its own: the player of the part with slug `by` changes instrument to play
    /// it, so the two never sound at once. That part has exactly one player of its own and
    /// plays another instrument.
    Doubling {
        /// The slug of the player's own part.
        by: &'static str,
    },
}

impl Players {
    /// The number of people on the part with the string body `size`: the range of an own part,
    /// the section's count, or none for a doubling. `None` for a section without a size.
    #[must_use]
    pub const fn count(self, size: Option<&StringSize>) -> Option<PlayerRange> {
        match self {
            Self::Own(range) => Some(range),
            Self::Section(section) => match size {
                Some(size) => {
                    let count = size.count(section);
                    Some(PlayerRange::new(count, count))
                }
                None => None,
            },
            Self::Doubling { .. } => Some(PlayerRange::NOT_USED),
        }
    }
}

/// One part of an ensemble: a track's worth of music for one instrument.
///
/// The fields are checked when the part is placed in an ensemble ([`Ensemble::try_new`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    /// The stable identifier within the ensemble, lower-case `snake_case`, such as `horn_3` or
    /// `violins_1`.
    pub slug: &'static str,
    /// The display name, such as "Horn 3" or "Piccolo (Fl. 3)": a doubled instrument names its
    /// player's part.
    pub name: &'static str,
    /// The short name later pages of a score print, such as "Hn. 3".
    pub short: &'static str,
    /// The instrument the part plays.
    pub instrument: InstrumentType,
    /// Other instruments of the same family a work may use for the part instead, such as the
    /// clarinet in A for the clarinet in B♭; empty when none.
    pub alternatives: &'static [InstrumentType],
    /// Who plays the part.
    pub players: Players,
    /// Whether every work of the kind needs the part; an optional part is used by only some, or
    /// is an extra.
    pub required: bool,
    /// Where the part sits.
    pub pan: Pan,
    /// What the template's sources add about the part, such as "finale only" or which
    /// instrument of the period a type stands in for; empty when nothing.
    pub note: &'static str,
}

/// How an ensemble orders its parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PartOrder {
    /// The order of a full orchestral score, that of [`InstrumentType::ALL`]: woodwinds, brass,
    /// percussion, harp, keyboards, voices, strings. A doubled instrument may also sit directly
    /// below its player's part, as a piccolo below the third flute, or below another doubling of
    /// the same player placed so; a doubling placed anywhere else keeps the order of its type,
    /// as the Wagner tubas below the eighth horn do.
    FullScore,
    /// The order the ensemble's own scores print, which differs from a full score: the rhythm
    /// section at the bottom of a big band, the trumpets above the horn in a brass quintet, the
    /// piano at the bottom of a piano trio.
    Edition,
}

/// The values of an ensemble template, checked by [`Ensemble::try_new`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnsembleSpec {
    /// The display name, such as "Romantic orchestra".
    pub name: &'static str,
    /// One sentence on what the template holds and which works it follows.
    pub description: &'static str,
    /// How the parts are ordered.
    pub order: PartOrder,
    /// The seatings a user may choose, the default first; empty when the ensemble has no choice,
    /// and then every part sits the same in both.
    pub seatings: &'static [Seating],
    /// The sizes of the string body a user may choose, the default first; empty when the ensemble
    /// has no string section.
    pub string_sizes: &'static [StringSize],
    /// The parts, in the order the score prints them.
    pub parts: &'static [Part],
    /// Where the roster, its sizes and its places come from, and what is estimated.
    pub provenance: &'static [Provenance],
}

/// Why an ensemble template is invalid. A variant that names a part or a string size carries its
/// slug.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum EnsembleError {
    /// The name or the description is empty or blank.
    #[error("the ensemble's name or description is empty or blank")]
    EmptyName,
    /// The ensemble lists no provenance, or a provenance of the ensemble or of a string size is
    /// empty or blank.
    #[error("a provenance is missing, empty or blank")]
    EmptyProvenance,
    /// A string size has a slug that is not lower-case `snake_case` or that another size has, an
    /// empty or blank name, or a section with no players.
    #[error("string size {0:?} is invalid")]
    InvalidStringSize(&'static str),
    /// The ensemble lists string sizes but no string section.
    #[error("the ensemble lists string sizes but no string section")]
    UnusedStringSizes,
    /// A seating is listed twice.
    #[error("a seating is listed twice")]
    DuplicateSeating,
    /// A part's slug is not lower-case `snake_case`.
    #[error("part slug {0:?} is not lower-case snake_case")]
    InvalidPartSlug(&'static str),
    /// A part's name or short name is empty or blank.
    #[error("part {0:?} has an empty or blank name")]
    EmptyPartName(&'static str),
    /// Two parts share a slug or a name; the variant names the later one.
    #[error("part {0:?} repeats the slug or the name of another part")]
    DuplicatePart(&'static str),
    /// A part lists its own instrument, an instrument of another family or one instrument twice
    /// as an alternative.
    #[error("part {0:?} lists an invalid alternative")]
    InvalidAlternative(&'static str),
    /// A part has players of its own, but none at the low end.
    #[error("part {0:?} has no players of its own")]
    NoPlayers(&'static str),
    /// A string-section part plays another instrument than its section's, repeats a section, or
    /// belongs to an ensemble without string sizes.
    #[error("string section part {0:?} is invalid")]
    InvalidSection(&'static str),
    /// A doubling names no part of the ensemble, a part without exactly one player of its own, or
    /// a part of the same instrument; or it is required while its player's part is optional.
    #[error("doubling part {0:?} is invalid")]
    InvalidDoubling(&'static str),
    /// A pan lies outside −1..=+1 or is NaN.
    #[error("part {0:?} has a pan outside -1 to +1")]
    PanOutOfRange(&'static str),
    /// A part of an ensemble without a choice of seating sits differently in the two seatings.
    #[error("part {0:?} depends on a seating the ensemble does not offer")]
    PanDependsOnSeating(&'static str),
    /// The ensemble has no required part.
    #[error("the ensemble has no required part")]
    NoRequiredPart,
    /// A part of a full-score ensemble comes before an instrument that a full score prints above
    /// it ([`PartOrder::FullScore`]).
    #[error("part {0:?} is out of score order")]
    OutOfScoreOrder(&'static str),
}

/// An ensemble template: its parts in score order, the sizes of its string body, its seatings and
/// where they come from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ensemble {
    id: EnsembleId,
    spec: EnsembleSpec,
}

impl Ensemble {
    /// Builds the template `id` after checking the invariants of `spec`.
    ///
    /// # Errors
    /// [`EnsembleError`], the first failing check in this order:
    /// [`EmptyName`](EnsembleError::EmptyName) for a blank name or description;
    /// [`EmptyProvenance`](EnsembleError::EmptyProvenance) for no provenance or a blank one, of
    /// the ensemble or of a string size; then, size by size,
    /// [`InvalidStringSize`](EnsembleError::InvalidStringSize) for a slug that is not
    /// `snake_case` or repeats an earlier size's, a blank name or a section without players;
    /// [`DuplicateSeating`](EnsembleError::DuplicateSeating) for a seating listed twice; then,
    /// part by part in list order, [`InvalidPartSlug`](EnsembleError::InvalidPartSlug),
    /// [`EmptyPartName`](EnsembleError::EmptyPartName),
    /// [`DuplicatePart`](EnsembleError::DuplicatePart) for a slug or name an earlier part has,
    /// [`InvalidAlternative`](EnsembleError::InvalidAlternative),
    /// [`NoPlayers`](EnsembleError::NoPlayers), [`InvalidSection`](EnsembleError::InvalidSection),
    /// [`InvalidDoubling`](EnsembleError::InvalidDoubling),
    /// [`PanOutOfRange`](EnsembleError::PanOutOfRange) and
    /// [`PanDependsOnSeating`](EnsembleError::PanDependsOnSeating); last
    /// [`NoRequiredPart`](EnsembleError::NoRequiredPart),
    /// [`UnusedStringSizes`](EnsembleError::UnusedStringSizes) for sizes without a section, and,
    /// for a full-score ensemble, [`OutOfScoreOrder`](EnsembleError::OutOfScoreOrder).
    pub const fn try_new(id: EnsembleId, spec: EnsembleSpec) -> Result<Self, EnsembleError> {
        if is_blank(spec.name) || is_blank(spec.description) {
            return Err(EnsembleError::EmptyName);
        }
        if !provenances_are_stated(&spec) {
            return Err(EnsembleError::EmptyProvenance);
        }
        if let Err(error) = check_string_sizes(spec.string_sizes) {
            return Err(error);
        }
        if has_duplicate_seating(spec.seatings) {
            return Err(EnsembleError::DuplicateSeating);
        }
        let parts = spec.parts;
        let mut has_required = false;
        let mut has_section = false;
        let mut i = 0;
        while i < parts.len() {
            if let Err(error) = check_part(&spec, i) {
                return Err(error);
            }
            has_required |= parts[i].required;
            has_section |= matches!(parts[i].players, Players::Section(_));
            i += 1;
        }
        if !has_required {
            return Err(EnsembleError::NoRequiredPart);
        }
        if !has_section && !spec.string_sizes.is_empty() {
            return Err(EnsembleError::UnusedStringSizes);
        }
        if matches!(spec.order, PartOrder::FullScore)
            && let Some(part) = out_of_score_order(parts)
        {
            return Err(EnsembleError::OutOfScoreOrder(part));
        }
        Ok(Self { id, spec })
    }

    /// Like [`Ensemble::try_new`]; an invalid template in a `const` item is a compile error.
    ///
    /// # Panics
    /// When [`Ensemble::try_new`] would fail.
    #[must_use]
    pub const fn new(id: EnsembleId, spec: EnsembleSpec) -> Self {
        match Self::try_new(id, spec) {
            Ok(ensemble) => ensemble,
            Err(_) => panic!("invalid ensemble template"),
        }
    }

    /// The template's id.
    #[must_use]
    pub const fn id(&self) -> EnsembleId {
        self.id
    }

    /// The checked values of the template, as [`Ensemble::try_new`] received them.
    #[must_use]
    pub const fn spec(&self) -> EnsembleSpec {
        self.spec
    }

    /// The display name, such as "Romantic orchestra". Not an identifier.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.spec.name
    }

    /// One sentence on what the template holds and which works it follows.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        self.spec.description
    }

    /// How the parts are ordered.
    #[must_use]
    pub const fn order(&self) -> PartOrder {
        self.spec.order
    }

    /// The seatings a user may choose, the default first; empty when the parts sit the same in
    /// both.
    #[must_use]
    pub const fn seatings(&self) -> &'static [Seating] {
        self.spec.seatings
    }

    /// The default seating, or `None` when the ensemble offers no choice.
    #[must_use]
    pub const fn default_seating(&self) -> Option<Seating> {
        match self.spec.seatings.first() {
            Some(seating) => Some(*seating),
            None => None,
        }
    }

    /// The sizes of the string body a user may choose, the default first; empty when the
    /// ensemble has no string section.
    #[must_use]
    pub const fn string_sizes(&self) -> &'static [StringSize] {
        self.spec.string_sizes
    }

    /// The default size of the string body, or `None` when the ensemble has no string section.
    #[must_use]
    pub const fn default_string_size(&self) -> Option<&'static StringSize> {
        self.spec.string_sizes.first()
    }

    /// The string size with slug `slug`, or `None`. The match is exact.
    #[must_use]
    pub fn string_size(&self, slug: &str) -> Option<&'static StringSize> {
        self.spec.string_sizes.iter().find(|size| size.slug == slug)
    }

    /// The parts, in the order the score prints them.
    #[must_use]
    pub const fn parts(&self) -> &'static [Part] {
        self.spec.parts
    }

    /// The part with slug `slug`, or `None`. The match is exact: a display name is not a slug.
    #[must_use]
    pub fn part(&self, slug: &str) -> Option<&'static Part> {
        self.spec.parts.iter().find(|part| part.slug == slug)
    }

    /// The part whose player also plays the doubling `part`, or `None` when `part` is not a
    /// doubling of this ensemble.
    #[must_use]
    pub fn player_of(&self, part: &Part) -> Option<&'static Part> {
        match part.players {
            Players::Doubling { by } => self.part(by),
            Players::Own(_) | Players::Section(_) => None,
        }
    }

    /// Where the roster, its sizes and its places come from, and what is estimated.
    #[must_use]
    pub const fn provenance(&self) -> &'static [Provenance] {
        self.spec.provenance
    }
}

/// Whether `text` is empty or holds only ASCII whitespace.
const fn is_blank(text: &str) -> bool {
    text.trim_ascii().is_empty()
}

/// Whether the ensemble lists a provenance and every one, its own and its string sizes', is
/// stated.
const fn provenances_are_stated(spec: &EnsembleSpec) -> bool {
    if spec.provenance.is_empty() {
        return false;
    }
    let mut i = 0;
    while i < spec.provenance.len() {
        if !is_stated(spec.provenance[i]) {
            return false;
        }
        i += 1;
    }
    let mut i = 0;
    while i < spec.string_sizes.len() {
        if !is_stated(spec.string_sizes[i].provenance) {
            return false;
        }
        i += 1;
    }
    true
}

/// Checks each string size: a `snake_case` slug no earlier size has, a name, and players in every
/// section.
const fn check_string_sizes(sizes: &[StringSize]) -> Result<(), EnsembleError> {
    let mut i = 0;
    while i < sizes.len() {
        let size = sizes[i];
        let mut valid = is_snake_case(size.slug) && !is_blank(size.name);
        let mut s = 0;
        while s < size.counts.len() {
            valid &= size.counts[s] > 0;
            s += 1;
        }
        let mut j = 0;
        while j < i {
            valid &= !same_text(sizes[j].slug, size.slug);
            j += 1;
        }
        if !valid {
            return Err(EnsembleError::InvalidStringSize(size.slug));
        }
        i += 1;
    }
    Ok(())
}

/// Whether `seatings` lists a seating twice.
const fn has_duplicate_seating(seatings: &[Seating]) -> bool {
    let mut i = 0;
    while i < seatings.len() {
        let mut j = i + 1;
        while j < seatings.len() {
            if seatings[i] as u8 == seatings[j] as u8 {
                return true;
            }
            j += 1;
        }
        i += 1;
    }
    false
}

/// Checks part `index` of `spec` on its own and against the other parts.
const fn check_part(spec: &EnsembleSpec, index: usize) -> Result<(), EnsembleError> {
    let parts = spec.parts;
    let part = parts[index];
    if !is_snake_case(part.slug) {
        return Err(EnsembleError::InvalidPartSlug(part.slug));
    }
    if is_blank(part.name) || is_blank(part.short) {
        return Err(EnsembleError::EmptyPartName(part.slug));
    }
    let mut j = 0;
    while j < index {
        if same_text(parts[j].slug, part.slug) || same_text(parts[j].name, part.name) {
            return Err(EnsembleError::DuplicatePart(part.slug));
        }
        j += 1;
    }
    if !alternatives_are_valid(part) {
        return Err(EnsembleError::InvalidAlternative(part.slug));
    }
    match part.players {
        Players::Own(range) => {
            if range.min() == 0 {
                return Err(EnsembleError::NoPlayers(part.slug));
            }
        }
        Players::Section(section) => {
            if !section_is_valid(spec, index, section) {
                return Err(EnsembleError::InvalidSection(part.slug));
            }
        }
        Players::Doubling { by } => {
            if !doubling_is_valid(parts, part, by) {
                return Err(EnsembleError::InvalidDoubling(part.slug));
            }
        }
    }
    if !part.pan.is_within() {
        return Err(EnsembleError::PanOutOfRange(part.slug));
    }
    if spec.seatings.is_empty() && part.pan.depends_on_seating() {
        return Err(EnsembleError::PanDependsOnSeating(part.slug));
    }
    Ok(())
}

/// Whether `part`'s alternatives are other instruments of its family, each listed once.
const fn alternatives_are_valid(part: Part) -> bool {
    let alternatives = part.alternatives;
    let mut i = 0;
    while i < alternatives.len() {
        let alternative = alternatives[i];
        if alternative as u8 == part.instrument as u8
            || alternative.family() as u8 != part.instrument.family() as u8
        {
            return false;
        }
        let mut j = i + 1;
        while j < alternatives.len() {
            if alternatives[j] as u8 == alternative as u8 {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Whether part `index`, a part of `section`, plays the section's instrument, is the only part of
/// that section, and belongs to an ensemble with string sizes.
const fn section_is_valid(spec: &EnsembleSpec, index: usize, section: StringSection) -> bool {
    let parts = spec.parts;
    if spec.string_sizes.is_empty()
        || parts[index].instrument as u8 != section.instrument_type() as u8
    {
        return false;
    }
    let mut j = 0;
    while j < parts.len() {
        if j != index
            && let Players::Section(other) = parts[j].players
            && other as u8 == section as u8
        {
            return false;
        }
        j += 1;
    }
    true
}

/// Whether the doubling `part` names a part with exactly one player of its own and another
/// instrument, which is required when `part` is.
const fn doubling_is_valid(parts: &[Part], part: Part, by: &str) -> bool {
    let mut j = 0;
    while j < parts.len() {
        let player = parts[j];
        if same_text(player.slug, by) {
            let one_player = match player.players {
                Players::Own(range) => range.min() == 1 && range.max() == 1,
                Players::Section(_) | Players::Doubling { .. } => false,
            };
            return one_player
                && player.instrument as u8 != part.instrument as u8
                && (player.required || !part.required);
        }
        j += 1;
    }
    false
}

/// The slug of the first part that breaks full-score order, or `None`.
///
/// Instrument types never decrease from part to part, except that a doubling may sit directly
/// below its player's part, or below another doubling placed so, whatever its type; the next part
/// is then compared with the player's part.
const fn out_of_score_order(parts: &[Part]) -> Option<&'static str> {
    // The type of the last part in score order and the slug of the player whose doublings may
    // follow directly.
    let mut previous: Option<InstrumentType> = None;
    let mut player: Option<&'static str> = None;
    let mut i = 0;
    while i < parts.len() {
        let part = parts[i];
        let below_its_player = match (part.players, player) {
            (Players::Doubling { by }, Some(player)) => same_text(by, player),
            _ => false,
        };
        if !below_its_player {
            if let Some(previous) = previous
                && (part.instrument as u8) < (previous as u8)
            {
                return Some(part.slug);
            }
            previous = Some(part.instrument);
            player = Some(part.slug);
        }
        i += 1;
    }
    None
}
