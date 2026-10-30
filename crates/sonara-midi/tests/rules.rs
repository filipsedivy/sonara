//! Contract tests of the orchestration rules through the public API.

use sonara_midi::orchestration::Provenance;
use sonara_midi::orchestration::rules::{
    BASS_SPACING, CHARACTERISTIC_COLOUR_SHARE, DynamicBand, FEWEST_FIRST_VIOLINS, LOUD_DIVISI,
    LOW_DISSONANCE, Level, MANY_NOTE_SLUR, MELODY_MARGIN, MIDDLE_GAP, RULES, Rule, RuleError,
    RuleGroup, RuleId, RuleInput, RuleSpec, SOFT_SLUR, SOLO_STRING_ACCOMPANIMENT,
    SUSTAINED_HIGH_BRASS, Severity, UNDOUBLED_HIGH_MELODY, UNDOUBLED_LOW_MELODY, WEAK_CHORD_MEMBER,
    WIND_PHRASE,
};
use sonara_midi::orchestration::{MODERATO_BPM, Span};

/// Whether a reader can trace `provenance`: a cited source that names a URL or a page, or an
/// estimate with a reason.
fn is_traceable(provenance: Provenance) -> bool {
    match provenance {
        Provenance::Cited { source } => {
            !source.trim().is_empty()
                && (source.contains("https://")
                    || source.contains("http://")
                    || source.contains("p. "))
        }
        Provenance::Estimate { reason } => !reason.trim().is_empty(),
    }
}

fn is_estimate(provenance: Provenance) -> bool {
    matches!(provenance, Provenance::Estimate { .. })
}

/// Every rule's code and grade, written out from the catalogue independently of the enum: `E`
/// error, `W` warning, `H` hint, `H/W loud` a hint raised to a warning at f and ff, and `x/y case`
/// a grade raised in a case the check tells apart.
const CATALOGUE: [(&str, &str); 67] = [
    ("RNG-01", "E"),
    ("RNG-02", "W"),
    ("RNG-03", "W"),
    ("RNG-04", "H"),
    ("SPC-01", "H/W case"),
    ("SPC-02", "H/W case"),
    ("SPC-03", "H"),
    ("SPC-04", "H/W loud"),
    ("SPC-05", "H"),
    ("SPC-06", "H"),
    ("SPC-07", "W"),
    ("SPC-08", "H"),
    ("DBL-01", "H"),
    ("DBL-02", "H"),
    ("DBL-03", "H"),
    ("DBL-04", "W"),
    ("DBL-05", "W"),
    ("DBL-06", "W"),
    ("DBL-07", "H"),
    ("DBL-08", "W"),
    ("DBL-09", "H"),
    ("BAL-01", "H/W loud"),
    ("BAL-02", "W"),
    ("BAL-03", "W"),
    ("BAL-04", "H"),
    ("BAL-05", "W"),
    ("BAL-06", "H"),
    ("BAL-07", "H"),
    ("BAL-08", "W"),
    ("BAL-09", "H"),
    ("MSK-01", "W"),
    ("MSK-02", "W"),
    ("MSK-03", "W"),
    ("MSK-04", "H"),
    ("VOI-01", "H"),
    ("VOI-02", "H"),
    ("VOI-03", "H"),
    ("VOI-04", "H"),
    ("VOI-05", "H"),
    ("VOI-06", "H"),
    ("VOI-07", "H/W case"),
    ("VOI-08", "H"),
    ("VOI-09", "H"),
    ("VOI-10", "H"),
    ("VOI-11", "W"),
    ("VOI-12", "W"),
    ("PLY-01", "W"),
    ("PLY-02", "W"),
    ("PLY-03", "H"),
    ("PLY-04", "W"),
    ("PLY-05", "W"),
    ("PLY-06", "H"),
    ("PLY-07", "E"),
    ("PLY-08", "W"),
    ("PLY-09", "W/E case"),
    ("PLY-10", "H"),
    ("PLY-11", "W/E case"),
    ("PLY-12", "H"),
    ("PLY-13", "H"),
    ("PLY-14", "H"),
    ("PLY-15", "H"),
    ("TEX-01", "H"),
    ("TEX-02", "H"),
    ("TEX-03", "H"),
    ("TEX-04", "H"),
    ("TEX-05", "H"),
    ("TEX-06", "H"),
];

fn letter(level: Level) -> &'static str {
    match level {
        Level::Hint => "H",
        Level::Warning => "W",
        Level::Error => "E",
    }
}

fn grade(severity: Severity) -> String {
    match severity {
        Severity::Fixed(level) => letter(level).to_owned(),
        Severity::ByDynamic { soft, loud } => format!("{}/{} loud", letter(soft), letter(loud)),
        Severity::ByCase { usual, raised, .. } => {
            format!("{}/{} case", letter(usual), letter(raised))
        }
    }
}

#[test]
fn the_catalogue_lists_the_67_rules_in_code_order() {
    assert_eq!(RULES.len(), CATALOGUE.len());
    assert_eq!(RuleId::ALL.len(), CATALOGUE.len());
    for ((rule, id), (code, expected)) in RULES.iter().zip(RuleId::ALL).zip(CATALOGUE) {
        assert_eq!(rule.id(), id);
        assert_eq!(rule.code(), code);
        assert_eq!(id.code(), code);
        assert_eq!(RuleId::from_code(code), Some(id));
        assert!(
            std::ptr::eq(id.get(), rule),
            "{code}: get returns the record"
        );
        assert_eq!(grade(rule.severity()), expected, "{code}");
        assert!(
            code.starts_with(id.group().prefix()),
            "{code}: the prefix is the group's"
        );
    }
}

#[test]
fn rule_groups_hold_the_catalogue_counts() {
    // The counts of the catalogue: RNG 4, SPC 8, DBL 9, BAL 9, MSK 4, VOI 12, PLY 15, TEX 6.
    let expected = [
        ("RNG", 4),
        ("SPC", 8),
        ("DBL", 9),
        ("BAL", 9),
        ("MSK", 4),
        ("VOI", 12),
        ("PLY", 15),
        ("TEX", 6),
    ];
    for (group, (prefix, count)) in RuleGroup::ALL.into_iter().zip(expected) {
        assert_eq!(group.prefix(), prefix);
        assert_eq!(group.rule_count(), count, "{prefix}");
        let members: Vec<RuleId> = RuleId::ALL
            .into_iter()
            .filter(|rule| rule.group() == group)
            .collect();
        assert_eq!(members.len(), usize::from(count), "{prefix}");
        // Numbered 1, 2, ... in order within the group.
        for (number, rule) in (1..).zip(&members) {
            assert_eq!(rule.number(), number, "{rule:?}");
        }
    }
}

#[test]
fn rule_lookup_is_strict() {
    // TST-03: other spellings are not codes.
    for code in [
        "rng-01", "RNG-1", "RNG01", "RNG-00", "RNG-05", "TEX-07", " RNG-01", "",
    ] {
        assert_eq!(RuleId::from_code(code), None, "{code:?}");
    }
}

#[test]
fn every_rule_is_described_and_traceable() {
    let mut estimates = Vec::new();
    for rule in &RULES {
        let code = rule.code();
        assert!(!rule.title().trim().is_empty(), "{code}");
        assert!(!rule.check().trim().is_empty(), "{code}");
        assert!(!rule.inputs().is_empty(), "{code}");
        assert!(is_traceable(rule.provenance()), "{code}");
        if is_estimate(rule.provenance()) {
            estimates.push(code);
        }
        if let Severity::ByCase { case, .. } = rule.severity() {
            assert!(!case.trim().is_empty(), "{code}");
        }
    }
    // Only the two range rules rest on the records' own ranges rather than on a handbook.
    assert_eq!(estimates, ["RNG-01", "RNG-02"]);
}

#[test]
fn range_rules_grade_errors_and_warnings_as_range_checks_do() {
    // TST-10: a note outside the playable range is an error, outside the comfortable range a
    // warning, at every dynamic.
    for band in DynamicBand::ALL {
        assert_eq!(
            RuleId::Rng01.get().severity().level(band, false),
            Level::Error
        );
        assert_eq!(
            RuleId::Rng02.get().severity().level(band, false),
            Level::Warning
        );
    }
    // A low interval is a hint softly in strings, a warning when loud or in brass.
    let spc01 = RuleId::Spc01.get().severity();
    assert_eq!(spc01.level(DynamicBand::Soft, false), Level::Hint);
    assert_eq!(spc01.level(DynamicBand::Soft, true), Level::Warning);
    // A hole in the middle is a warning only at f and ff.
    let spc04 = RuleId::Spc04.get().severity();
    assert_eq!(spc04.level(DynamicBand::Medium, false), Level::Hint);
    assert_eq!(spc04.level(DynamicBand::Loud, false), Level::Warning);
}

#[test]
fn rules_list_the_inputs_their_checks_name() {
    use RuleInput::{
        Dynamic, Harmony, HarpPedals, Instrument, Part, RegisterFactors, TimpaniTuning, Weights,
    };
    // TST-10: the inputs the catalogue names for these rules.
    let cases: [(RuleId, &[RuleInput]); 8] = [
        (RuleId::Rng01, &[Instrument]),
        (RuleId::Spc04, &[Harmony, Dynamic]),
        (RuleId::Dbl05, &[Part]),
        (RuleId::Bal01, &[Weights, RegisterFactors, Dynamic, Harmony]),
        (RuleId::Voi06, &[RegisterFactors]),
        (RuleId::Ply07, &[HarpPedals]),
        (RuleId::Ply11, &[TimpaniTuning]),
        (RuleId::Bal09, &[Part]),
    ];
    for (id, needed) in cases {
        let inputs = id.get().inputs();
        for input in needed {
            assert!(inputs.contains(input), "{id:?} needs {input:?}");
        }
    }
    // The catalogue reads harp pedals and timpani tuning only in the harp and timpani rules.
    for rule in &RULES {
        if rule.inputs().contains(&HarpPedals) {
            assert!(
                matches!(rule.id(), RuleId::Ply07 | RuleId::Ply08),
                "{}",
                rule.code()
            );
        }
        if rule.inputs().contains(&TimpaniTuning) {
            assert_eq!(rule.id(), RuleId::Ply11);
        }
    }
}

/// Every threshold as its name, rules, value written as text, and whether it is cited.
fn thresholds() -> Vec<(&'static str, &'static [RuleId], String, Provenance)> {
    let span = |span: Span| match span {
        Span::ModerateBeats(beats) => format!("{beats} beats"),
        Span::Millis(millis) => format!("{millis} ms"),
    };
    vec![
        (
            "SUSTAINED_HIGH_BRASS",
            SUSTAINED_HIGH_BRASS.rules,
            span(SUSTAINED_HIGH_BRASS.value),
            SUSTAINED_HIGH_BRASS.provenance,
        ),
        (
            "UNDOUBLED_HIGH_MELODY",
            UNDOUBLED_HIGH_MELODY.rules,
            UNDOUBLED_HIGH_MELODY.value.get().to_string(),
            UNDOUBLED_HIGH_MELODY.provenance,
        ),
        (
            "UNDOUBLED_LOW_MELODY",
            UNDOUBLED_LOW_MELODY.rules,
            UNDOUBLED_LOW_MELODY.value.get().to_string(),
            UNDOUBLED_LOW_MELODY.provenance,
        ),
        (
            "LOW_DISSONANCE",
            LOW_DISSONANCE.rules,
            LOW_DISSONANCE.value.get().to_string(),
            LOW_DISSONANCE.provenance,
        ),
        (
            "BASS_SPACING",
            BASS_SPACING.rules,
            BASS_SPACING.value.to_string(),
            BASS_SPACING.provenance,
        ),
        (
            "MIDDLE_GAP",
            MIDDLE_GAP.rules,
            MIDDLE_GAP.value.to_string(),
            MIDDLE_GAP.provenance,
        ),
        (
            "WEAK_CHORD_MEMBER",
            WEAK_CHORD_MEMBER.rules,
            WEAK_CHORD_MEMBER.value.to_string(),
            WEAK_CHORD_MEMBER.provenance,
        ),
        (
            "MELODY_MARGIN",
            MELODY_MARGIN.rules,
            MELODY_MARGIN.value.to_string(),
            MELODY_MARGIN.provenance,
        ),
        (
            "SOLO_STRING_ACCOMPANIMENT",
            SOLO_STRING_ACCOMPANIMENT.rules,
            SOLO_STRING_ACCOMPANIMENT.value.to_string(),
            SOLO_STRING_ACCOMPANIMENT.provenance,
        ),
        (
            "FEWEST_FIRST_VIOLINS",
            FEWEST_FIRST_VIOLINS.rules,
            FEWEST_FIRST_VIOLINS.value.to_string(),
            FEWEST_FIRST_VIOLINS.provenance,
        ),
        (
            "LOUD_DIVISI",
            LOUD_DIVISI.rules,
            LOUD_DIVISI.value.to_string(),
            LOUD_DIVISI.provenance,
        ),
        (
            "WIND_PHRASE",
            WIND_PHRASE.rules,
            span(WIND_PHRASE.value),
            WIND_PHRASE.provenance,
        ),
        (
            "SOFT_SLUR",
            SOFT_SLUR.rules,
            span(SOFT_SLUR.value),
            SOFT_SLUR.provenance,
        ),
        (
            "MANY_NOTE_SLUR",
            MANY_NOTE_SLUR.rules,
            span(MANY_NOTE_SLUR.value),
            MANY_NOTE_SLUR.provenance,
        ),
        (
            "CHARACTERISTIC_COLOUR_SHARE",
            CHARACTERISTIC_COLOUR_SHARE.rules,
            CHARACTERISTIC_COLOUR_SHARE.value.to_string(),
            CHARACTERISTIC_COLOUR_SHARE.provenance,
        ),
    ]
}

#[test]
fn thresholds_hold_the_catalogue_numbers() {
    // TST-10: the values and sources written from the catalogue: two bars of moderate 4/4 are
    // eight beats, eight bars thirty-two; MIDI 90, 42 and D3 = 50; an octave is 12 semitones.
    let expected: [(&str, &[RuleId], &str, bool); 15] = [
        ("SUSTAINED_HIGH_BRASS", &[RuleId::Rng03], "8 beats", false),
        ("UNDOUBLED_HIGH_MELODY", &[RuleId::Rng04], "90", false),
        ("UNDOUBLED_LOW_MELODY", &[RuleId::Rng04], "42", false),
        ("LOW_DISSONANCE", &[RuleId::Spc02], "50", true),
        ("BASS_SPACING", &[RuleId::Spc03], "12", true),
        ("MIDDLE_GAP", &[RuleId::Spc04], "12", false),
        ("WEAK_CHORD_MEMBER", &[RuleId::Bal01], "0.5", false),
        ("MELODY_MARGIN", &[RuleId::Bal03], "1.5", false),
        ("SOLO_STRING_ACCOMPANIMENT", &[RuleId::Bal08], "2", false),
        ("FEWEST_FIRST_VIOLINS", &[RuleId::Bal09], "8", true),
        ("LOUD_DIVISI", &[RuleId::Voi11], "4", true),
        ("WIND_PHRASE", &[RuleId::Ply01], "32 beats", false),
        ("SOFT_SLUR", &[RuleId::Ply03], "16 beats", true),
        ("MANY_NOTE_SLUR", &[RuleId::Ply03], "8 beats", true),
        (
            "CHARACTERISTIC_COLOUR_SHARE",
            &[RuleId::Tex01],
            "0.5",
            false,
        ),
    ];
    let actual = thresholds();
    assert_eq!(actual.len(), expected.len());
    for ((name, rules, value, provenance), (e_name, e_rules, e_value, e_cited)) in
        actual.into_iter().zip(expected)
    {
        assert_eq!(name, e_name);
        assert_eq!(rules, e_rules, "{name}");
        assert_eq!(value, e_value, "{name}");
        assert_eq!(!is_estimate(provenance), e_cited, "{name}");
        assert!(is_traceable(provenance), "{name}");
    }
    // Two bars of moderate 4/4 take four seconds at the moderate tempo.
    assert_eq!(MODERATO_BPM, 120);
    assert_eq!(SUSTAINED_HIGH_BRASS.value.millis(), 4_000);
}

#[test]
fn an_estimated_threshold_never_backs_more_than_a_warning() {
    // TST-07: a check that compares with an estimated number reports at most a warning, so an
    // estimate is never presented as an error.
    for (name, rules, _, provenance) in thresholds() {
        assert!(!rules.is_empty(), "{name}");
        if is_estimate(provenance) {
            for rule in rules {
                assert!(
                    rule.get().severity().highest() <= Level::Warning,
                    "{name} backs {}",
                    rule.code()
                );
            }
        }
    }
}

#[test]
fn check_texts_state_their_thresholds() {
    // DES-01: the words a reader sees name the same number the check compares with.
    let cases = [
        (RuleId::Rng03, "two bars of moderate 4/4"),
        (RuleId::Rng04, "MIDI 90"),
        (RuleId::Rng04, "MIDI 42"),
        (RuleId::Spc02, "MIDI 50"),
        (RuleId::Spc03, "more than an octave"),
        (RuleId::Spc04, "more than an octave"),
        (RuleId::Bal01, "less than half"),
        (RuleId::Bal03, "1.5 times"),
        (RuleId::Bal08, "more than 2 units"),
        (RuleId::Bal09, "fewer than 8 first violins"),
        (RuleId::Voi11, "four or more ways"),
        (RuleId::Ply01, "8 bars of moderate 4/4"),
        (RuleId::Ply03, "4 bars of moderate time"),
        (RuleId::Tex01, "most bars"),
    ];
    for (id, words) in cases {
        assert!(id.get().check().contains(words), "{}: {words}", id.code());
    }
    assert_eq!(SUSTAINED_HIGH_BRASS.value, Span::ModerateBeats(2 * 4));
    assert_eq!(WIND_PHRASE.value, Span::ModerateBeats(8 * 4));
    assert_eq!(SOFT_SLUR.value, Span::ModerateBeats(4 * 4));
    assert_eq!(BASS_SPACING.value, 12);
    assert_eq!(MIDDLE_GAP.value, 12);
}

fn spec() -> RuleSpec {
    RuleSpec {
        title: "A rule",
        check: "what it looks for",
        inputs: &[RuleInput::Instrument, RuleInput::Notes],
        severity: Severity::Fixed(Level::Hint),
        provenance: Provenance::Estimate {
            reason: "test fixture",
        },
    }
}

#[test]
fn validator_accepts_a_valid_rule_and_keeps_its_values() {
    let rule = Rule::try_new(RuleId::Tex06, spec()).expect("valid");
    assert_eq!(rule.id(), RuleId::Tex06);
    assert_eq!(rule.spec(), spec());
    assert_eq!(rule.title(), "A rule");
    assert_eq!(rule.check(), "what it looks for");
    assert_eq!(rule.severity(), Severity::Fixed(Level::Hint));
    // Each shipped rule rebuilds from its own values.
    for shipped in &RULES {
        assert_eq!(Rule::try_new(shipped.id(), shipped.spec()), Ok(*shipped));
    }
}

#[test]
fn validator_rejects_each_broken_value() {
    // TST-03: one fixture per error, each breaking one check of a valid spec.
    let cases = [
        (
            RuleSpec {
                title: " ",
                ..spec()
            },
            RuleError::BlankText,
        ),
        (
            RuleSpec {
                check: "",
                ..spec()
            },
            RuleError::BlankText,
        ),
        (
            RuleSpec {
                inputs: &[],
                ..spec()
            },
            RuleError::NoInputs,
        ),
        (
            RuleSpec {
                inputs: &[RuleInput::Notes, RuleInput::Tempo, RuleInput::Notes],
                ..spec()
            },
            RuleError::DuplicateInput,
        ),
        (
            RuleSpec {
                severity: Severity::ByDynamic {
                    soft: Level::Warning,
                    loud: Level::Warning,
                },
                ..spec()
            },
            RuleError::InvalidSeverity,
        ),
        (
            RuleSpec {
                severity: Severity::ByCase {
                    usual: Level::Warning,
                    raised: Level::Hint,
                    case: "a case",
                },
                ..spec()
            },
            RuleError::InvalidSeverity,
        ),
        (
            RuleSpec {
                severity: Severity::ByCase {
                    usual: Level::Hint,
                    raised: Level::Warning,
                    case: "",
                },
                ..spec()
            },
            RuleError::InvalidSeverity,
        ),
        (
            RuleSpec {
                provenance: Provenance::Cited { source: " " },
                ..spec()
            },
            RuleError::EmptyProvenance,
        ),
        (
            RuleSpec {
                provenance: Provenance::Estimate { reason: "" },
                ..spec()
            },
            RuleError::EmptyProvenance,
        ),
    ];
    for (spec, error) in cases {
        assert_eq!(Rule::try_new(RuleId::Rng01, spec), Err(error), "{spec:?}");
    }
}
