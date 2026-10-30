//! Contract tests of the orchestration rules through the public API.

use sonara_midi::orchestration::Provenance;
use sonara_midi::orchestration::rules::{
    Avoid, DO_NOT_DOUBLE, Forces, Member, NAMED_DOUBLINGS, NamedDoubling, Playing, Prevails,
    Selector, Spacing, WOODWIND_ORDER, WoodwindOrder,
};
use sonara_midi::orchestration::rules::{
    BASS_SPACING, CHARACTERISTIC_COLOUR_SHARE, DynamicBand, FEWEST_FIRST_VIOLINS, LOUD_DIVISI,
    LOW_DISSONANCE, Level, MANY_NOTE_SLUR, MELODY_MARGIN, MIDDLE_GAP, RULES, Rule, RuleError,
    RuleGroup, RuleId, RuleInput, RuleSpec, SOFT_SLUR, SOLO_STRING_ACCOMPANIMENT,
    SUSTAINED_HIGH_BRASS, Severity, UNDOUBLED_HIGH_MELODY, UNDOUBLED_LOW_MELODY, WEAK_CHORD_MEMBER,
    WIND_PHRASE,
};
use sonara_midi::orchestration::{
    Articulation, Family, InstrumentType, MODERATO_BPM, PitchModel, PlayerRange, Span,
};

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

/// The instruments a selector can cover: its ids, or every type of its family.
fn covered(selector: Selector) -> Vec<InstrumentType> {
    match selector {
        Selector::Family(family) => InstrumentType::ALL
            .into_iter()
            .filter(|instrument| instrument.family() == family)
            .collect(),
        _ => selector.ids().to_vec(),
    }
}

/// The named unison of `a` and `b` played in their ordinary way, as DBL-01 looks it up: a row of
/// two members, one covering each.
fn named_unison(a: InstrumentType, b: InstrumentType) -> Option<&'static NamedDoubling> {
    NAMED_DOUBLINGS.iter().find(|row| {
        let [first, second] = row.members else {
            return false;
        };
        let covers =
            |member: &Member, instrument| member.selector.matches(Playing::ordinary(instrument));
        row.spacings.contains(&Spacing::Unison)
            && !row.builds_bass
            && ((covers(first, a) && covers(second, b)) || (covers(first, b) && covers(second, a)))
    })
}

/// Which instrument of a two-member row prevails, as the instrument itself.
fn prevailing(row: &NamedDoubling, a: InstrumentType) -> &'static str {
    match row.prevails {
        Prevails::Members(&[index]) => {
            let member = row.members[usize::from(index)];
            if member.selector.matches(Playing::ordinary(a)) {
                "first"
            } else {
                "second"
            }
        }
        Prevails::Members(_) => "several",
        Prevails::Blend => "blend",
        Prevails::Apart => "apart",
        Prevails::ByRegister(_) => "by register",
        Prevails::Unstated => "unstated",
    }
}

#[test]
fn named_unisons_match_the_handbook_pairs() {
    use InstrumentType::{
        Bassoon, Cello, ClarinetBb, DoubleBass, EnglishHorn, Flute, Harp, HornF, Oboe, Piccolo,
        TromboneTenor, TrumpetBb, Tuba, Viola, Violin,
    };
    // TST-10: the pairs Rimsky-Korsakov names (Ch. II, pp. 47-61), each with who prevails as he
    // says it, the first instrument written first: strings prevail over one wind, cellos over
    // violas and violins, brass over woodwind; horn and cello blend; strings and brass stay apart;
    // woodwind pairs depend on the register.
    let cases = [
        (Violin, Flute, "first"),
        (Violin, Oboe, "first"),
        (Viola, ClarinetBb, "first"),
        (Viola, EnglishHorn, "first"),
        (Cello, Bassoon, "first"),
        (DoubleBass, Bassoon, "first"),
        (Viola, Cello, "second"),
        (Violin, Cello, "second"),
        (Violin, Viola, "first"),
        (HornF, Cello, "blend"),
        (Violin, TrumpetBb, "apart"),
        (Viola, HornF, "apart"),
        (Cello, TromboneTenor, "apart"),
        (TrumpetBb, Oboe, "first"),
        (HornF, Bassoon, "first"),
        (Tuba, Bassoon, "first"),
        (Flute, Oboe, "by register"),
        (Oboe, ClarinetBb, "by register"),
        (Bassoon, ClarinetBb, "by register"),
    ];
    for (a, b, expected) in cases {
        let row = named_unison(a, b).unwrap_or_else(|| panic!("{a:?} + {b:?} is named"));
        assert_eq!(prevailing(row, a), expected, "{a:?} + {b:?}");
        // The pair is found in either order.
        assert!(std::ptr::eq(named_unison(b, a).expect("named"), row));
    }
    // TST-03: pairs no handbook names give no row.
    for (a, b) in [(Tuba, Piccolo), (Harp, Tuba), (Piccolo, DoubleBass)] {
        assert!(named_unison(a, b).is_none(), "{a:?} + {b:?}");
    }
}

#[test]
fn every_named_doubling_is_valid_and_traceable() {
    assert_eq!(NAMED_DOUBLINGS.len(), 78);
    for (i, row) in NAMED_DOUBLINGS.iter().enumerate() {
        assert_eq!(row.check(), Ok(()), "row {i}");
        assert!(is_traceable(row.provenance), "row {i}");
        assert!(!row.effect.trim().is_empty(), "row {i}");
    }
    // The bass tables: the cello and double bass in octaves is the usual bass.
    let basses: Vec<&NamedDoubling> = NAMED_DOUBLINGS
        .iter()
        .filter(|row| row.builds_bass)
        .collect();
    assert_eq!(basses.len(), 9);
    assert!(basses.iter().any(|row| {
        row.spacings == [Spacing::Octaves(1)]
            && row.members[0].selector == Selector::Ids(&[InstrumentType::Cello])
            && row.members[1].selector == Selector::Ids(&[InstrumentType::DoubleBass])
    }));
}

/// The mean centre of the playable ranges of the pitched instruments a selector covers, in MIDI
/// notes.
fn centre_of(selector: Selector) -> f64 {
    let centres: Vec<f64> = covered(selector)
        .into_iter()
        .filter_map(
            |instrument| match instrument.get().map(|record| *record.pitch()) {
                Some(PitchModel::Pitched(pitched)) => {
                    let range = pitched.playable.range;
                    #[expect(clippy::cast_precision_loss, reason = "MIDI notes are 0-127")]
                    Some((range.low().get() + range.high().get()) as f64 / 2.0)
                }
                _ => None,
            },
        )
        .collect();
    assert!(
        !centres.is_empty(),
        "{selector:?} covers a pitched instrument"
    );
    #[expect(clippy::cast_precision_loss, reason = "a handful of instruments")]
    let count = centres.len() as f64;
    centres.iter().sum::<f64>() / count
}

#[test]
fn octave_doublings_list_their_members_from_the_top() {
    // TST-07: in an octave doubling each member lies above the next. Instruments of one compass,
    // such as a horn over a bassoon, can play either part, so the check allows the upper member's
    // instruments to centre up to a fifth below the next member's; a member listed below its
    // partner centres an octave or more lower. Its woodwinds never come from a lower branch than
    // the next member's (DBL-02).
    for (i, row) in NAMED_DOUBLINGS.iter().enumerate() {
        if !row
            .spacings
            .iter()
            .any(|spacing| matches!(spacing, Spacing::Octaves(_)))
        {
            continue;
        }
        for pair in row.members.windows(2) {
            let (upper, lower) = (pair[0].selector, pair[1].selector);
            assert!(
                centre_of(upper) + 7.0 >= centre_of(lower),
                "row {i}: {upper:?} over {lower:?}"
            );
            let ranks = |selector| -> Vec<usize> {
                covered(selector)
                    .into_iter()
                    .filter_map(|instrument| WOODWIND_ORDER.rank(instrument))
                    .collect()
            };
            if let (Some(lowest_branch_above), Some(highest_branch_below)) = (
                ranks(upper).into_iter().max(),
                ranks(lower).into_iter().min(),
            ) {
                assert!(
                    lowest_branch_above <= highest_branch_below,
                    "row {i}: {upper:?} over {lower:?}"
                );
            }
        }
    }
}

#[test]
fn played_selectors_name_articulations_their_instruments_play() {
    // TST-07: a doubling of a played articulation names one each instrument lists, and each
    // articulation is played by one of its instruments.
    for (i, row) in NAMED_DOUBLINGS.iter().enumerate() {
        for member in row.members {
            let Selector::Played { ids, articulations } = member.selector else {
                continue;
            };
            for instrument in ids {
                let listed = instrument.get().expect("record").articulations();
                assert!(
                    articulations
                        .iter()
                        .any(|articulation| listed.contains(articulation)),
                    "row {i}: {instrument:?}"
                );
            }
            for articulation in articulations {
                assert!(
                    ids.iter().any(|instrument| {
                        instrument
                            .get()
                            .expect("record")
                            .articulations()
                            .contains(articulation)
                    }),
                    "row {i}: {articulation:?}"
                );
            }
        }
    }
}

#[test]
fn selectors_match_by_instrument_articulation_and_forces() {
    use InstrumentType::{Cello, Flute, HornF, Violin};
    let ordinary = Playing::ordinary(Violin);
    let pizzicato = Playing {
        articulation: Some(Articulation::Pizzicato),
        ..ordinary
    };
    let solo = Playing {
        forces: Forces::Solo,
        ..ordinary
    };
    let divided = Playing {
        forces: Forces::Divided,
        ..ordinary
    };
    let ids = Selector::Ids(&[Violin, Cello]);
    let played = Selector::Played {
        ids: &[Violin],
        articulations: &[Articulation::Pizzicato],
    };
    // TST-10: each row is the expected answer for ordinary, pizzicato, solo and divided violins.
    let cases = [
        (ids, [true, true, true, true]),
        (Selector::Family(Family::Strings), [true, true, true, true]),
        (
            Selector::Family(Family::Woodwinds),
            [false, false, false, false],
        ),
        (played, [false, true, false, false]),
        (Selector::Solo(&[Violin]), [false, false, true, false]),
        (
            Selector::Divided(&[Violin, Cello]),
            [false, false, false, true],
        ),
        (Selector::Ids(&[Flute, HornF]), [false, false, false, false]),
    ];
    for (selector, expected) in cases {
        let found = [ordinary, pizzicato, solo, divided].map(|playing| selector.matches(playing));
        assert_eq!(found, expected, "{selector:?}");
    }
    assert_eq!(played.ids(), [Violin]);
    assert_eq!(Selector::Family(Family::Brass).ids(), []);
}

#[test]
fn the_warnings_name_the_rules_that_check_them() {
    // TST-10: the rules written from the catalogue next to each warning; three have no rule.
    let expected: [&[RuleId]; 12] = [
        &[RuleId::Dbl07],
        &[],
        &[RuleId::Dbl06],
        &[RuleId::Dbl05],
        &[RuleId::Dbl03],
        &[RuleId::Dbl02],
        &[RuleId::Dbl04],
        &[RuleId::Spc06, RuleId::Spc07, RuleId::Spc08],
        &[RuleId::Msk02],
        &[],
        &[],
        &[RuleId::Dbl09],
    ];
    assert_eq!(DO_NOT_DOUBLE.len(), expected.len());
    for (row, rules) in DO_NOT_DOUBLE.iter().zip(expected) {
        assert_eq!(row.rules, rules, "{}", row.what);
        assert_eq!(row.check(), Ok(()), "{}", row.what);
        assert!(is_traceable(row.provenance), "{}", row.what);
    }
}

#[test]
fn the_woodwind_order_ranks_each_branch() {
    use InstrumentType::{
        AltoFlute, BassClarinet, BassFlute, Bassoon, ClarinetA, ClarinetBb, ClarinetEb,
        Contrabassoon, EnglishHorn, Flute, Oboe, OboeDamore, Piccolo, SaxophoneAlto,
        SaxophoneBaritone, SaxophoneSoprano, SaxophoneTenor,
    };
    // TST-10: flutes, oboes, clarinets, bassoons from the top; the saxophones have no branch.
    let expected = [
        (Piccolo, Some(0)),
        (Flute, Some(0)),
        (AltoFlute, Some(0)),
        (BassFlute, Some(0)),
        (Oboe, Some(1)),
        (OboeDamore, Some(1)),
        (EnglishHorn, Some(1)),
        (ClarinetEb, Some(2)),
        (ClarinetBb, Some(2)),
        (ClarinetA, Some(2)),
        (BassClarinet, Some(2)),
        (SaxophoneSoprano, None),
        (SaxophoneAlto, None),
        (SaxophoneTenor, None),
        (SaxophoneBaritone, None),
        (Bassoon, Some(3)),
        (Contrabassoon, Some(3)),
    ];
    let woodwinds: Vec<InstrumentType> = InstrumentType::ALL
        .into_iter()
        .filter(|instrument| instrument.family() == Family::Woodwinds)
        .collect();
    assert_eq!(woodwinds, expected.map(|(instrument, _)| instrument));
    for (instrument, rank) in expected {
        assert_eq!(WOODWIND_ORDER.rank(instrument), rank, "{instrument:?}");
    }
    assert_eq!(WOODWIND_ORDER.rank(InstrumentType::Violin), None);
    assert_eq!(WOODWIND_ORDER.check(), Ok(()));
    assert!(is_traceable(WOODWIND_ORDER.provenance));
}

const FIXTURE: Provenance = Provenance::Estimate {
    reason: "test fixture",
};

const VIOLINS: Member = Member {
    selector: Selector::Ids(&[InstrumentType::Violin]),
    players: None,
};

const TWO_FLUTES: Member = Member {
    selector: Selector::Ids(&[InstrumentType::Flute]),
    players: Some(PlayerRange::new(2, 2)),
};

const NO_FLUTES: Member = Member {
    selector: Selector::Ids(&[InstrumentType::Flute]),
    players: Some(PlayerRange::NOT_USED),
};

const NOBODY: Member = Member {
    selector: Selector::Ids(&[]),
    players: None,
};

fn pair() -> NamedDoubling {
    NamedDoubling {
        members: &[VIOLINS, TWO_FLUTES],
        spacings: &[Spacing::Unison, Spacing::Octaves(1)],
        builds_bass: false,
        effect: "a colour",
        prevails: Prevails::Members(&[0]),
        provenance: FIXTURE,
    }
}

#[test]
fn doubling_validator_rejects_each_broken_row() {
    // TST-03: one fixture per failing check of each row type.
    assert_eq!(pair().check(), Ok(()));
    let doublings = [
        (
            NamedDoubling {
                members: &[VIOLINS],
                ..pair()
            },
            RuleError::InvalidDoubling,
        ),
        (
            NamedDoubling {
                members: &[VIOLINS, NO_FLUTES],
                ..pair()
            },
            RuleError::InvalidDoubling,
        ),
        (
            NamedDoubling {
                members: &[VIOLINS, NOBODY],
                ..pair()
            },
            RuleError::InvalidSelector,
        ),
        (
            NamedDoubling {
                spacings: &[],
                ..pair()
            },
            RuleError::InvalidDoubling,
        ),
        (
            NamedDoubling {
                spacings: &[Spacing::Octaves(0)],
                ..pair()
            },
            RuleError::InvalidDoubling,
        ),
        (
            NamedDoubling {
                spacings: &[Spacing::Unison, Spacing::Unison],
                ..pair()
            },
            RuleError::InvalidDoubling,
        ),
        (
            NamedDoubling {
                prevails: Prevails::Members(&[2]),
                ..pair()
            },
            RuleError::InvalidDoubling,
        ),
        (
            NamedDoubling {
                prevails: Prevails::Members(&[]),
                ..pair()
            },
            RuleError::InvalidDoubling,
        ),
        (
            NamedDoubling {
                prevails: Prevails::ByRegister(&[0]),
                ..pair()
            },
            RuleError::InvalidDoubling,
        ),
        (
            NamedDoubling {
                effect: " ",
                ..pair()
            },
            RuleError::BlankText,
        ),
        (
            NamedDoubling {
                provenance: Provenance::Cited { source: "" },
                ..pair()
            },
            RuleError::EmptyProvenance,
        ),
    ];
    for (row, error) in doublings {
        assert_eq!(row.check(), Err(error), "{row:?}");
    }
}

#[test]
fn selector_validator_rejects_each_broken_selector() {
    use InstrumentType::Violin;
    // TST-03: no instrument, one twice, a family no instrument has, no articulation or one twice.
    let selectors = [
        Selector::Ids(&[]),
        Selector::Solo(&[Violin, Violin]),
        Selector::Divided(&[]),
        Selector::Family(Family::Other),
        Selector::Played {
            ids: &[Violin],
            articulations: &[],
        },
        Selector::Played {
            ids: &[Violin],
            articulations: &[Articulation::Pizzicato, Articulation::Pizzicato],
        },
        Selector::Played {
            ids: &[],
            articulations: &[Articulation::Pizzicato],
        },
    ];
    for selector in selectors {
        assert_eq!(
            selector.check(),
            Err(RuleError::InvalidSelector),
            "{selector:?}"
        );
    }
    assert_eq!(Selector::Family(Family::Choir).check(), Ok(()));
}

#[test]
fn warning_validator_rejects_blank_rows() {
    let avoid = Avoid {
        what: "do this",
        why: "because",
        rules: &[],
        provenance: FIXTURE,
    };
    assert_eq!(avoid.check(), Ok(()));
    assert_eq!(
        Avoid { what: "", ..avoid }.check(),
        Err(RuleError::BlankText)
    );
    assert_eq!(
        Avoid { why: " ", ..avoid }.check(),
        Err(RuleError::BlankText)
    );
    assert_eq!(
        Avoid {
            provenance: Provenance::Estimate { reason: "" },
            ..avoid
        }
        .check(),
        Err(RuleError::EmptyProvenance)
    );
}

#[test]
fn woodwind_order_validator_rejects_broken_branches() {
    use InstrumentType::{Flute, Violin};
    let order = WoodwindOrder {
        branches: [
            &[Flute],
            &[InstrumentType::Oboe],
            &[InstrumentType::ClarinetBb],
            &[InstrumentType::Bassoon],
        ],
        provenance: FIXTURE,
    };
    assert_eq!(order.check(), Ok(()));
    let broken = [
        [
            &[][..],
            &[InstrumentType::Oboe],
            &[InstrumentType::ClarinetBb],
            &[InstrumentType::Bassoon],
        ],
        [
            &[Flute],
            &[InstrumentType::Oboe],
            &[Violin],
            &[InstrumentType::Bassoon],
        ],
        [
            &[Flute],
            &[InstrumentType::Oboe],
            &[Flute],
            &[InstrumentType::Bassoon],
        ],
    ];
    for branches in broken {
        assert_eq!(
            WoodwindOrder { branches, ..order }.check(),
            Err(RuleError::InvalidOrder),
            "{branches:?}"
        );
    }
    assert_eq!(
        WoodwindOrder {
            provenance: Provenance::Cited { source: " " },
            ..order
        }
        .check(),
        Err(RuleError::EmptyProvenance)
    );
}
