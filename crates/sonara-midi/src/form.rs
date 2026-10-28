//! Song form: a rule-based plan of arrangement sections with explicit musical roles (SON-P10).
//!
//! A [`SongForm`] is built from a named [`FormTemplate`] and a [`FormConfig`]. Every section
//! carries a [`SectionRole`] (verse, chorus, …), a length in bars, and an energy and density
//! [`Level`]. Lengths are bars, converted to beats through the project's
//! [`TimeSignatureMap`], so sections stay bar-aligned across meter changes and follow each other
//! without gaps or overlaps. Generation is deterministic: the same configuration, meter map and
//! ID source always give the same plan.
//!
//! The persisted [`Section`] has only an ID, a name, a start, a length and a colour, and the
//! `.sonara` schema must not change (CTR-01). The role, energy and density therefore live only
//! in the plan, which is not saved with the project. The section name is a display label and
//! never the source of truth for the role.
//!
//! The plan reaches the project only through commands: [`apply_song_form`] adds all sections in
//! one undoable transaction, and [`regenerate_section`] changes one section through a single
//! `UpdateSection` that keeps its ID, start and length, so every other section stays untouched.

use std::collections::{BTreeMap, HashSet};
use std::str::FromStr;

use sonara_core::commands::project::{AddSection, UpdateSection};
use sonara_core::commands::{Command, Transaction};
use sonara_core::error::{DomainError, ErrorCode, Result};
use sonara_core::ids::{self, IdGenerator};
use sonara_core::model::markers::Section;
use sonara_core::session::{CommandResult, ProjectSession};
use sonara_core::time::TimeSignatureMap;

/// Largest accepted length of one section in bars.
pub const MAX_SECTION_BARS: u32 = 64;

/// Tolerance in beats when comparing a planned section with the project's section.
const POSITION_TOLERANCE: f64 = 1e-9;

/// The musical role of a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SectionRole {
    /// Opening, usually sparse.
    Intro,
    /// Narrative section with the lowest energy of the main parts.
    Verse,
    /// Short build into the chorus.
    PreChorus,
    /// The memorable, highest-energy section.
    Chorus,
    /// Contrasting section, often once near the end.
    Bridge,
    /// Section without vocals that carries a solo or a riff.
    Instrumental,
    /// Reduced section that makes room for the next build or drop.
    Breakdown,
    /// Closing section.
    Outro,
}

impl SectionRole {
    /// Every role.
    pub const ALL: [Self; 8] = [
        Self::Intro,
        Self::Verse,
        Self::PreChorus,
        Self::Chorus,
        Self::Bridge,
        Self::Instrumental,
        Self::Breakdown,
        Self::Outro,
    ];

    /// The display stem of the role, for example `"Pre-Chorus"`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Intro => "Intro",
            Self::Verse => "Verse",
            Self::PreChorus => "Pre-Chorus",
            Self::Chorus => "Chorus",
            Self::Bridge => "Bridge",
            Self::Instrumental => "Instrumental",
            Self::Breakdown => "Breakdown",
            Self::Outro => "Outro",
        }
    }

    /// The default energy of the role.
    #[must_use]
    pub const fn default_energy(self) -> Level {
        Level(match self {
            Self::Intro | Self::Outro => 20,
            Self::Verse => 40,
            Self::PreChorus => 60,
            Self::Chorus => 90,
            Self::Bridge => 50,
            Self::Instrumental => 70,
            Self::Breakdown => 30,
        })
    }

    /// The default density (how many layers play) of the role.
    #[must_use]
    pub const fn default_density(self) -> Level {
        Level(match self {
            Self::Intro | Self::Outro => 20,
            Self::Verse | Self::Breakdown => 40,
            Self::PreChorus | Self::Bridge => 60,
            Self::Chorus => 85,
            Self::Instrumental => 70,
        })
    }
}

/// An energy or density in whole percent, 0 to 100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Level(u8);

impl Level {
    /// The largest level.
    pub const MAX: u8 = 100;

    /// A validated level.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InvalidArgument`] when `percent` is above 100.
    pub fn new(percent: u8) -> Result<Self> {
        if percent > Self::MAX {
            return Err(DomainError::invalid(format!(
                "level must be between 0 and {}, got {percent}",
                Self::MAX
            )));
        }
        Ok(Self(percent))
    }

    /// The level in percent.
    #[must_use]
    pub const fn percent(self) -> u8 {
        self.0
    }
}

/// A named song form: the roles and default lengths of its sections, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormTemplate {
    /// `verse_chorus`: intro, two verse–chorus pairs, outro (40 bars).
    VerseChorus,
    /// `pop_pre_chorus`: verse, pre-chorus and chorus twice, then bridge, chorus and outro
    /// (64 bars).
    PopPreChorus,
    /// `aaba`: the 32-bar form with a contrasting bridge.
    Aaba,
    /// `dance`: verse, breakdown and chorus (the drop) twice, framed by intro and outro
    /// (96 bars).
    Dance,
}

impl FormTemplate {
    /// Every template.
    pub const ALL: [Self; 4] = [
        Self::VerseChorus,
        Self::PopPreChorus,
        Self::Aaba,
        Self::Dance,
    ];

    /// The template's name, as [`FromStr`] accepts it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::VerseChorus => "verse_chorus",
            Self::PopPreChorus => "pop_pre_chorus",
            Self::Aaba => "aaba",
            Self::Dance => "dance",
        }
    }

    /// The sections of the template in order, with their default lengths in bars.
    #[must_use]
    pub const fn steps(self) -> &'static [(SectionRole, u32)] {
        use SectionRole::{Breakdown, Bridge, Chorus, Intro, Outro, PreChorus, Verse};
        match self {
            Self::VerseChorus => &[
                (Intro, 4),
                (Verse, 8),
                (Chorus, 8),
                (Verse, 8),
                (Chorus, 8),
                (Outro, 4),
            ],
            Self::PopPreChorus => &[
                (Intro, 4),
                (Verse, 8),
                (PreChorus, 4),
                (Chorus, 8),
                (Verse, 8),
                (PreChorus, 4),
                (Chorus, 8),
                (Bridge, 8),
                (Chorus, 8),
                (Outro, 4),
            ],
            Self::Aaba => &[(Verse, 8), (Verse, 8), (Bridge, 8), (Verse, 8)],
            Self::Dance => &[
                (Intro, 8),
                (Verse, 16),
                (Breakdown, 8),
                (Chorus, 16),
                (Verse, 16),
                (Breakdown, 8),
                (Chorus, 16),
                (Outro, 8),
            ],
        }
    }

    fn has_role(self, role: SectionRole) -> bool {
        self.steps().iter().any(|&(r, _)| r == role)
    }
}

impl FromStr for FormTemplate {
    type Err = DomainError;

    fn from_str(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|template| template.name() == name)
            .ok_or_else(|| {
                let known: Vec<&str> = Self::ALL.iter().map(|t| t.name()).collect();
                DomainError::invalid(format!(
                    "unknown song form {name:?}; expected one of {}",
                    known.join(", ")
                ))
            })
    }
}

/// What to generate: the template and its options.
///
/// Every option is validated by [`plan_form`]; an option that cannot apply is an error, never
/// silently ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormConfig {
    /// The form to generate.
    pub template: FormTemplate,
    /// 0-based bar where the first section starts.
    pub start_bar: i64,
    /// Leave out the intro. An error when the template has none.
    pub omit_intro: bool,
    /// Leave out the outro. An error when the template has none.
    pub omit_outro: bool,
    /// Replaces the length in bars of every section of a role (1 to [`MAX_SECTION_BARS`]). An
    /// error for a role the form does not contain.
    pub bars: BTreeMap<SectionRole, u32>,
}

impl FormConfig {
    /// The template with its defaults: starting at bar 0, every section, template lengths.
    #[must_use]
    pub fn new(template: FormTemplate) -> Self {
        Self {
            template,
            start_bar: 0,
            omit_intro: false,
            omit_outro: false,
            bars: BTreeMap::new(),
        }
    }
}

/// One planned section: the persisted facts plus the role and levels that the project does not
/// store.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PlannedSection {
    /// Stable ID (`sec_…`), also the ID of the project section.
    pub id: String,
    /// The musical role.
    pub role: SectionRole,
    /// Display name, for example `"Verse 2"`.
    pub label: String,
    /// 0-based bar where the section starts.
    pub start_bar: i64,
    /// Length in bars (at least 1).
    pub bars: u32,
    /// Start in beats, from the time signature map.
    pub start_beat: f64,
    /// Length in beats, from the time signature map.
    pub length_beats: f64,
    /// How energetic the section is.
    pub energy: Level,
    /// How many layers play.
    pub density: Level,
}

impl PlannedSection {
    /// End in beats.
    #[must_use]
    pub fn end_beat(&self) -> f64 {
        self.start_beat + self.length_beats
    }

    /// The project section for this plan entry.
    ///
    /// # Errors
    ///
    /// The validation errors of [`Section::new`].
    pub fn to_section(&self) -> Result<Section> {
        Section::new(
            self.id.clone(),
            self.label.clone(),
            self.start_beat,
            self.length_beats,
            None,
        )
    }
}

/// A planned song structure: contiguous sections, in order, from one template.
#[derive(Debug, Clone, PartialEq)]
pub struct SongForm {
    template: FormTemplate,
    sections: Vec<PlannedSection>,
}

impl SongForm {
    /// The template the form came from.
    #[must_use]
    pub fn template(&self) -> FormTemplate {
        self.template
    }

    /// The sections in time order.
    #[must_use]
    pub fn sections(&self) -> &[PlannedSection] {
        &self.sections
    }

    /// The section with `section_id`, if any.
    #[must_use]
    pub fn section(&self, section_id: &str) -> Option<&PlannedSection> {
        self.sections.iter().find(|s| s.id == section_id)
    }

    /// Total length in bars.
    #[must_use]
    pub fn total_bars(&self) -> u32 {
        self.sections.iter().map(|s| s.bars).sum()
    }

    /// Start of the first section in beats.
    #[must_use]
    pub fn start_beat(&self) -> f64 {
        self.sections.first().map_or(0.0, |s| s.start_beat)
    }

    /// End of the last section in beats.
    #[must_use]
    pub fn end_beat(&self) -> f64 {
        self.sections.last().map_or(0.0, PlannedSection::end_beat)
    }

    /// The change in energy from the section before `section_id` (0 for the first section), in
    /// percentage points. `None` when the ID is unknown.
    #[must_use]
    pub fn contrast_with_previous(&self, section_id: &str) -> Option<u8> {
        let index = self.sections.iter().position(|s| s.id == section_id)?;
        let energy = self.sections.get(index)?.energy.percent();
        Some(
            index
                .checked_sub(1)
                .and_then(|i| self.sections.get(i))
                .map_or(0, |previous| energy.abs_diff(previous.energy.percent())),
        )
    }

    /// The persisted sections, ready to add to a project.
    ///
    /// # Errors
    ///
    /// The validation errors of [`Section::new`].
    pub fn to_sections(&self) -> Result<Vec<Section>> {
        self.sections
            .iter()
            .map(PlannedSection::to_section)
            .collect()
    }
}

/// Plans the sections of `config` on the bar grid of `signatures`.
///
/// Section IDs come from `ids` only after the configuration has been validated, so a rejected
/// configuration consumes none.
///
/// # Errors
///
/// [`ErrorCode::InvalidArgument`] for a negative start bar, an omitted intro or outro that the
/// template lacks, a length override outside 1 to [`MAX_SECTION_BARS`] or for a role the form
/// does not contain; [`ErrorCode::Internal`] when `ids` repeats an ID.
pub fn plan_form(
    config: &FormConfig,
    signatures: &TimeSignatureMap,
    ids: &mut dyn IdGenerator,
) -> Result<SongForm> {
    if config.start_bar < 0 {
        return Err(DomainError::invalid("start bar must be >= 0"));
    }
    for (omit, role) in [
        (config.omit_intro, SectionRole::Intro),
        (config.omit_outro, SectionRole::Outro),
    ] {
        if omit && !config.template.has_role(role) {
            return Err(DomainError::invalid(format!(
                "the {} form has no {}",
                config.template.name(),
                role.label().to_lowercase()
            )));
        }
    }
    let steps: Vec<(SectionRole, u32)> = config
        .template
        .steps()
        .iter()
        .copied()
        .filter(|&(role, _)| {
            !(config.omit_intro && role == SectionRole::Intro
                || config.omit_outro && role == SectionRole::Outro)
        })
        .collect();
    for (&role, &bars) in &config.bars {
        if !steps.iter().any(|&(r, _)| r == role) {
            return Err(DomainError::invalid(format!(
                "the form has no {} section to resize",
                role.label().to_lowercase()
            )));
        }
        if !(1..=MAX_SECTION_BARS).contains(&bars) {
            return Err(DomainError::invalid(format!(
                "{} length must be between 1 and {MAX_SECTION_BARS} bars, got {bars}",
                role.label().to_lowercase()
            )));
        }
    }

    let mut occurrences: BTreeMap<SectionRole, u32> = BTreeMap::new();
    for &(role, _) in &steps {
        *occurrences.entry(role).or_default() += 1;
    }
    let mut seen: BTreeMap<SectionRole, u32> = BTreeMap::new();
    let mut sections = Vec::with_capacity(steps.len());
    let mut bar = config.start_bar;
    for (role, default_bars) in steps {
        let bars = config.bars.get(&role).copied().unwrap_or(default_bars);
        let ordinal = seen.entry(role).or_default();
        *ordinal += 1;
        let label = if occurrences.get(&role).copied().unwrap_or(1) > 1 {
            format!("{} {ordinal}", role.label())
        } else {
            role.label().to_owned()
        };
        sections.push(plan_section(
            ids.new_id(ids::SECTION),
            role,
            label,
            bar,
            bars,
            signatures,
            role.default_energy(),
            role.default_density(),
        ));
        bar += i64::from(bars);
    }
    let unique: HashSet<&str> = sections.iter().map(|s| s.id.as_str()).collect();
    if unique.len() != sections.len() {
        return Err(DomainError::new(
            ErrorCode::Internal,
            "the ID generator returned a duplicate section ID",
        ));
    }
    Ok(SongForm {
        template: config.template,
        sections,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "a plain constructor of one value type; a builder would add nothing"
)]
fn plan_section(
    id: String,
    role: SectionRole,
    label: String,
    start_bar: i64,
    bars: u32,
    signatures: &TimeSignatureMap,
    energy: Level,
    density: Level,
) -> PlannedSection {
    let start_beat = signatures.bar_to_beat(bar_f64(start_bar));
    let end_beat = signatures.bar_to_beat(bar_f64(start_bar + i64::from(bars)));
    PlannedSection {
        id,
        role,
        label,
        start_bar,
        bars,
        start_beat,
        length_beats: end_beat - start_beat,
        energy,
        density,
    }
}

#[expect(clippy::cast_precision_loss, reason = "bar counts are far below 2^52")]
fn bar_f64(bar: i64) -> f64 {
    bar as f64
}

/// Adds every section of `form` to the open project as one undoable step.
///
/// # Errors
///
/// [`ErrorCode::NoProject`] without an open project; [`ErrorCode::Conflict`] when the project
/// already has sections (remove them first); the errors of the section commands. On any error
/// the project is unchanged.
pub fn apply_song_form(
    session: &mut ProjectSession,
    form: &SongForm,
    source: &str,
) -> Result<CommandResult> {
    let existing = session.require_project()?.sections.len();
    if existing > 0 {
        return Err(DomainError::new(
            ErrorCode::Conflict,
            format!("the project already has {existing} section(s); remove them first"),
        ));
    }
    let commands: Vec<Box<dyn Command>> = form
        .sections
        .iter()
        .map(|s| {
            Box::new(AddSection {
                name: s.label.clone(),
                start: s.start_beat,
                length: s.length_beats,
                color: None,
                section_id: s.id.clone(),
            }) as Box<dyn Command>
        })
        .collect();
    let transaction = Transaction {
        commands,
        name: "Generate Song Form".to_owned(),
    };
    session.execute(&transaction, false, source)
}

/// A request to regenerate one section. `None` keeps a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SectionChange {
    /// The new role. Unless `energy` or `density` are given, they reset to the role's defaults.
    pub role: Option<SectionRole>,
    /// The new energy.
    pub energy: Option<Level>,
    /// The new density.
    pub density: Option<Level>,
}

/// The form with one section regenerated: same ID, start and length, new role and levels.
///
/// Only the target changes. Its label is the role's name, numbered with the smallest number not
/// used by another section, and the labels of other sections are never rewritten.
///
/// # Errors
///
/// [`ErrorCode::NotFound`] for an unknown `section_id`.
pub fn regenerate_in_plan(
    form: &SongForm,
    section_id: &str,
    change: &SectionChange,
) -> Result<SongForm> {
    let index = form
        .sections
        .iter()
        .position(|s| s.id == section_id)
        .ok_or_else(|| DomainError::not_found("section", section_id))?;
    let mut next = form.clone();
    let others: HashSet<&str> = form
        .sections
        .iter()
        .filter(|s| s.id != section_id)
        .map(|s| s.label.as_str())
        .collect();
    if let Some(target) = next.sections.get_mut(index) {
        let role = change.role.unwrap_or(target.role);
        if role != target.role {
            target.role = role;
            target.energy = role.default_energy();
            target.density = role.default_density();
            target.label = free_label(role, &others);
        }
        if let Some(energy) = change.energy {
            target.energy = energy;
        }
        if let Some(density) = change.density {
            target.density = density;
        }
    }
    Ok(next)
}

/// The role's name, or the first numbered form of it that no other section uses.
fn free_label(role: SectionRole, others: &HashSet<&str>) -> String {
    let bare = role.label();
    let numbered_prefix = format!("{bare} ");
    if !others
        .iter()
        .any(|label| *label == bare || label.starts_with(&numbered_prefix))
    {
        return bare.to_owned();
    }
    // Among `others.len() + 1` candidates at least one is free.
    (1..=others.len() + 1)
        .map(|n| format!("{bare} {n}"))
        .find(|candidate| !others.contains(candidate.as_str()))
        .unwrap_or_else(|| bare.to_owned())
}

/// Regenerates one section of `form` and updates the open project through one `UpdateSection`.
///
/// The project section keeps its ID, start and length; no other section is touched. Returns the
/// new plan and the command result.
///
/// # Errors
///
/// [`ErrorCode::NotFound`] when the plan or the project has no such section;
/// [`ErrorCode::Conflict`] when the project's section no longer has the planned start and
/// length (it was edited since); [`ErrorCode::NoProject`]. On any error the project is
/// unchanged.
pub fn regenerate_section(
    session: &mut ProjectSession,
    form: &SongForm,
    section_id: &str,
    change: &SectionChange,
    source: &str,
) -> Result<(SongForm, CommandResult)> {
    let next = regenerate_in_plan(form, section_id, change)?;
    let planned = next
        .section(section_id)
        .ok_or_else(|| DomainError::not_found("section", section_id))?;
    let current = session
        .require_project()?
        .find_section(section_id)
        .ok_or_else(|| DomainError::not_found("section", section_id))?;
    if (current.start - planned.start_beat).abs() > POSITION_TOLERANCE
        || (current.length - planned.length_beats).abs() > POSITION_TOLERANCE
    {
        return Err(DomainError::new(
            ErrorCode::Conflict,
            format!("section {section_id} was changed since the form was planned"),
        ));
    }
    let command = UpdateSection {
        section_id: section_id.to_owned(),
        name: Some(planned.label.clone()),
        start: None,
        length: None,
        color: None,
    };
    let result = session.execute(&command, false, source)?;
    Ok((next, result))
}
