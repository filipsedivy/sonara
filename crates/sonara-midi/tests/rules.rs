//! Contract tests of the orchestration rules through the public API.

use sonara_midi::orchestration::Provenance;
use sonara_midi::orchestration::rules::{
    Applies, Combination, DYNAMIC_STEP, HEAVY_PERCUSSION, IntervalLimit, IntervalLimits,
    LOW_INTERVAL_LIMITS, Limit, MASKING_ACCOMPANIMENT, REGISTER_FACTORS, RegisterBand,
    RegisterFactor, STRONG_REGISTER, WEAK_REGISTER, WEIGHTS, WeightRow, register_factor,
    section_weight, weight_row,
};
use sonara_midi::orchestration::rules::{
    Avoid, CARRIERS, Carriers, DO_NOT_DOUBLE, Forces, Member, NAMED_DOUBLINGS, NamedDoubling,
    Playing, Prevails, Role, Selector, Spacing, WOODWIND_ORDER, WoodwindOrder, carriers,
};
use sonara_midi::orchestration::rules::{
    BASS_SPACING, CHARACTERISTIC_COLOUR_SHARE, DynamicBand, FEWEST_FIRST_VIOLINS, LOUD_DIVISI,
    LOW_DISSONANCE, Level, MANY_NOTE_SLUR, MELODY_MARGIN, MIDDLE_GAP, RULES, Rule, RuleError,
    RuleGroup, RuleId, RuleInput, RuleSpec, SOFT_SLUR, SOLO_STRING_ACCOMPANIMENT,
    SUSTAINED_HIGH_BRASS, Severity, UNDOUBLED_HIGH_MELODY, UNDOUBLED_LOW_MELODY, WEAK_CHORD_MEMBER,
    WIND_PHRASE,
};
use sonara_midi::orchestration::{
    Articulation, Family, InstrumentType, MODERATO_BPM, PitchModel, PitchRange, PlayerRange,
    SoundingPitch, Span, StringSection,
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

/// One threshold as its name, rules, value written as text by `text`, and provenance.
macro_rules! threshold {
    ($name:ident, $text:expr) => {
        (
            stringify!($name),
            $name.rules,
            $text($name.value),
            $name.provenance,
        )
    };
}

/// Every threshold as its name, rules, value written as text, and provenance.
fn thresholds() -> Vec<(&'static str, &'static [RuleId], String, Provenance)> {
    let span = |span: Span| match span {
        Span::ModerateBeats(beats) => format!("{beats} beats"),
        Span::Millis(millis) => format!("{millis} ms"),
    };
    let pitch = |pitch: SoundingPitch| pitch.get().to_string();
    let count = |count: u8| count.to_string();
    let number = |number: f32| number.to_string();
    vec![
        threshold!(SUSTAINED_HIGH_BRASS, span),
        threshold!(UNDOUBLED_HIGH_MELODY, pitch),
        threshold!(UNDOUBLED_LOW_MELODY, pitch),
        threshold!(LOW_DISSONANCE, pitch),
        threshold!(BASS_SPACING, count),
        threshold!(MIDDLE_GAP, count),
        threshold!(WEAK_CHORD_MEMBER, number),
        threshold!(MELODY_MARGIN, number),
        threshold!(SOLO_STRING_ACCOMPANIMENT, number),
        threshold!(FEWEST_FIRST_VIOLINS, count),
        threshold!(LOUD_DIVISI, count),
        threshold!(WIND_PHRASE, span),
        threshold!(SOFT_SLUR, span),
        threshold!(MANY_NOTE_SLUR, span),
        threshold!(CHARACTERISTIC_COLOUR_SHARE, number),
        threshold!(DYNAMIC_STEP, number),
        threshold!(WEAK_REGISTER, number),
        threshold!(STRONG_REGISTER, number),
        threshold!(MASKING_ACCOMPANIMENT, number),
    ]
}

#[test]
fn thresholds_hold_the_catalogue_numbers() {
    // TST-10: the values and sources written from the catalogue: two bars of moderate 4/4 are
    // eight beats, eight bars thirty-two; MIDI 90, 42 and D3 = 50; an octave is 12 semitones.
    let expected: [(&str, &[RuleId], &str, bool); 19] = [
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
        ("DYNAMIC_STEP", &[RuleId::Bal02, RuleId::Bal04], "2", false),
        (
            "WEAK_REGISTER",
            &[RuleId::Msk03, RuleId::Voi06],
            "0.5",
            false,
        ),
        ("STRONG_REGISTER", &[RuleId::Voi06], "1.25", false),
        ("MASKING_ACCOMPANIMENT", &[RuleId::Msk03], "1", false),
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
        (RuleId::Msk03, "factor 0.5 or less"),
        (RuleId::Msk03, "weight 1 or more"),
        (RuleId::Voi06, "factor 0.5 or less"),
        (RuleId::Voi06, "factor 1.25 or more"),
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
        .filter_map(|instrument| match *instrument.get().pitch() {
            PitchModel::Pitched(pitched) => {
                let range = pitched.playable.range;
                #[expect(clippy::cast_precision_loss, reason = "MIDI notes are 0-127")]
                Some((range.low().get() + range.high().get()) as f64 / 2.0)
            }
            PitchModel::Unpitched => None,
        })
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
    // TST-07: a selector of a played articulation, in a doubling, a weight row or the heavy
    // percussion, names one each instrument lists, and each articulation is played by one of its
    // instruments.
    let selectors = NAMED_DOUBLINGS
        .iter()
        .flat_map(|row| row.members.iter().map(|member| member.selector))
        .chain(WEIGHTS.iter().map(|row| row.selector))
        .chain(HEAVY_PERCUSSION.members.iter().copied());
    for (i, selector) in selectors.enumerate() {
        {
            let Selector::Played { ids, articulations } = selector else {
                continue;
            };
            for instrument in ids {
                let listed = instrument.get().articulations();
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
                        instrument.get().articulations().contains(articulation)
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

/// Whether two weights agree; the data writes one- or two-digit decimals.
fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

fn same_weights(a: [Option<f32>; 3], b: [Option<f32>; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| match (a, b) {
        (Some(a), Some(b)) => close(*a, b),
        (None, None) => true,
        _ => false,
    })
}

fn bands(row: &WeightRow) -> [Option<f32>; 3] {
    DynamicBand::ALL.map(|band| row.at(band))
}

const UNIT: [Option<f32>; 3] = [Some(1.0), Some(1.0), Some(1.0)];
const HORN: [Option<f32>; 3] = [Some(1.0), Some(1.4), Some(2.0)];
const TRUMPET: [Option<f32>; 3] = [Some(1.0), Some(2.0), Some(4.0)];
const CORNET: [Option<f32>; 3] = [Some(1.0), Some(1.7), Some(3.0)];
const LIGHT: [Option<f32>; 3] = [Some(0.5), Some(0.5), Some(0.5)];
const SOFT_ONLY: [Option<f32>; 3] = [Some(0.5), Some(0.5), None];
const BELLS: [Option<f32>; 3] = [Some(2.0), Some(3.0), Some(4.0)];
const VOICE: [Option<f32>; 3] = [Some(1.0), Some(1.0), Some(1.5)];

/// Every instrument with a weight and its weights at pp-p, mf and f-ff, written from the balance
/// table: Rimsky-Korsakov's ratios (Ch. I, p. 33), one woodwind the unit, a horn 2 and a trumpet,
/// trombone or tuba 4 in forte, one string section 2 in forte and 1 in piano, every wind 1 in
/// piano; the other rows are the table's estimates.
fn expected_weights() -> Vec<(InstrumentType, [Option<f32>; 3])> {
    use InstrumentType as T;
    vec![
        (T::Piccolo, UNIT),
        (T::Flute, UNIT),
        (T::AltoFlute, UNIT),
        (T::BassFlute, UNIT),
        (T::Oboe, UNIT),
        (T::OboeDamore, UNIT),
        (T::EnglishHorn, UNIT),
        (T::ClarinetEb, UNIT),
        (T::ClarinetBb, UNIT),
        (T::ClarinetA, UNIT),
        (T::BassClarinet, UNIT),
        (T::SaxophoneSoprano, HORN),
        (T::SaxophoneAlto, HORN),
        (T::SaxophoneTenor, HORN),
        (T::SaxophoneBaritone, HORN),
        (T::Bassoon, UNIT),
        (T::Contrabassoon, UNIT),
        (T::HornF, HORN),
        (T::WagnerTubaTenor, HORN),
        (T::WagnerTubaBass, HORN),
        (T::TrumpetPiccolo, TRUMPET),
        (T::TrumpetC, TRUMPET),
        (T::TrumpetBb, TRUMPET),
        (T::Cornet, CORNET),
        (T::Flugelhorn, HORN),
        (T::TromboneTenor, TRUMPET),
        (T::TromboneBass, TRUMPET),
        (T::Euphonium, HORN),
        (T::Tuba, TRUMPET),
        (T::Cimbasso, TRUMPET),
        (T::Timpani, TRUMPET),
        (T::Glockenspiel, BELLS),
        (T::Xylophone, BELLS),
        (T::TubularBells, BELLS),
        (T::Crotales, BELLS),
        (T::Harp, LIGHT),
        (T::Celesta, SOFT_ONLY),
        (T::PianoConcertGrand, SOFT_ONLY),
        (T::PianoStudioGrand, SOFT_ONLY),
        (T::PianoUpright, SOFT_ONLY),
        (T::VoiceSoprano, VOICE),
        (T::VoiceMezzoSoprano, VOICE),
        (T::VoiceAlto, VOICE),
        (T::VoiceTenor, VOICE),
        (T::VoiceBaritone, VOICE),
        (T::VoiceBass, VOICE),
        (T::ChoirSoprano, VOICE),
        (T::ChoirAlto, VOICE),
        (T::ChoirTenor, VOICE),
        (T::ChoirBass, VOICE),
        (T::Violin, HORN),
        (T::Viola, HORN),
        (T::Cello, HORN),
        (T::DoubleBass, HORN),
    ]
}

#[test]
fn weights_match_the_balance_table() {
    // TST-10: every instrument played in its ordinary way finds the weights of the table, and
    // instruments without a row have no weight.
    let expected = expected_weights();
    for instrument in InstrumentType::ALL {
        let found = weight_row(Playing::ordinary(instrument)).map(bands);
        let wanted = expected
            .iter()
            .find(|(listed, _)| *listed == instrument)
            .map(|(_, weights)| *weights);
        match (found, wanted) {
            (Some(found), Some(wanted)) => {
                assert!(same_weights(found, wanted), "{instrument:?}: {found:?}");
            }
            (None, None) => {}
            _ => panic!("{instrument:?}: {found:?} against {wanted:?}"),
        }
    }
    // A solo string, and a pizzicato or col legno section, weigh half a woodwind; tremolo is
    // still the arco section.
    let violin = Playing::ordinary(InstrumentType::Violin);
    let played = |articulation| Playing {
        articulation: Some(articulation),
        ..violin
    };
    let solo = Playing {
        forces: Forces::Solo,
        ..violin
    };
    for playing in [
        solo,
        played(Articulation::Pizzicato),
        played(Articulation::ColLegno),
    ] {
        let row = weight_row(playing).expect("a row");
        assert!(same_weights(bands(row), LIGHT), "{playing:?}");
    }
    let tremolo = weight_row(played(Articulation::Tremolo)).expect("a row");
    assert!(same_weights(bands(tremolo), HORN));
}

#[test]
fn every_sustaining_instrument_has_a_weight_and_rows_never_weaken_louder() {
    // TST-07: every woodwind, brass, string and voice part weighs something at every band, and
    // no row weighs less at a louder band.
    for instrument in InstrumentType::ALL {
        let sustains = matches!(
            instrument.family(),
            Family::Woodwinds | Family::Brass | Family::Strings | Family::Choir
        );
        if sustains {
            let row = weight_row(Playing::ordinary(instrument)).expect("a weight");
            assert!(bands(row).iter().all(Option::is_some), "{instrument:?}");
        }
    }
    for row in &WEIGHTS {
        assert_eq!(row.check(), Ok(()));
        assert!(is_traceable(row.provenance));
        let given: Vec<f32> = bands(row).into_iter().flatten().collect();
        assert!(given.windows(2).all(|pair| pair[0] <= pair[1]), "{row:?}");
    }
}

#[test]
fn section_weights_grow_with_the_players() {
    // TST-10: the weights describe the medium orchestra's sections, 12-10-8-6-4 players.
    assert!(close(section_weight(2.0, StringSection::ViolinsI, 12), 2.0));
    assert!(close(
        section_weight(2.0, StringSection::ViolinsI, 16),
        16.0 / 6.0
    ));
    assert!(close(
        section_weight(2.0, StringSection::ViolinsII, 10),
        2.0
    ));
    assert!(close(section_weight(1.0, StringSection::Violas, 4), 0.5));
    assert!(close(section_weight(1.0, StringSection::Cellos, 3), 0.5));
    assert!(close(section_weight(2.0, StringSection::Basses, 8), 4.0));
    assert!(close(section_weight(2.0, StringSection::Basses, 0), 0.0));
}

#[test]
fn heavy_percussion_is_four_instruments_at_fortissimo() {
    use InstrumentType::{BassDrum, ClashCymbals, SnareDrum, SuspendedCymbal, TamTam};
    // TST-10: the bass drum, cymbals, gong and a side-drum roll (Rimsky-Korsakov, Ch. IV, p. 117).
    let covers = |instrument, articulation| {
        HEAVY_PERCUSSION.members.iter().any(|member| {
            member.matches(Playing {
                articulation,
                ..Playing::ordinary(instrument)
            })
        })
    };
    assert_eq!(HEAVY_PERCUSSION.members.len(), 4);
    assert!(covers(BassDrum, None));
    assert!(covers(ClashCymbals, None));
    assert!(covers(TamTam, None));
    assert!(covers(SnareDrum, Some(Articulation::Roll)));
    assert!(!covers(SnareDrum, None), "only the snare drum's roll");
    assert!(!covers(SuspendedCymbal, None));
    assert_eq!(HEAVY_PERCUSSION.check(), Ok(()));
    assert!(is_traceable(HEAVY_PERCUSSION.provenance));
}

#[test]
fn register_factors_match_the_table() {
    use InstrumentType as T;
    // TST-10: the factor table, with spans read off the records' ranges and the registers the
    // sources name: the flute's low B3-G4 and bright top from G5, the alto flute's strong bottom
    // sixth, the piccolo's weak low and middle and powerful top from C#7, the small clarinet's
    // chalumeau and throat, the clarinets' throat, the horn's bottom fifth, the contrabassoon's
    // deep notes in piano, the oboe's low fifth, the bassoon's upper register, and the harp below
    // C5 in fast figures.
    let expected = [
        (T::Flute, 59, 67, 0.5, Applies::Always),
        (T::Flute, 79, 98, 1.5, Applies::Always),
        (T::AltoFlute, 55, 64, 1.0, Applies::Always),
        (T::Piccolo, 74, 96, 0.5, Applies::Always),
        (T::Piccolo, 97, 108, 2.0, Applies::Always),
        (T::ClarinetEb, 55, 73, 0.5, Applies::Always),
        (T::ClarinetBb, 63, 68, 0.5, Applies::Always),
        (T::ClarinetA, 62, 67, 0.5, Applies::Always),
        (T::HornF, 35, 42, 0.5, Applies::Always),
        (
            T::Contrabassoon,
            22,
            28,
            1.5,
            Applies::At(DynamicBand::Soft),
        ),
        (T::Oboe, 58, 65, 1.25, Applies::Always),
        (T::Bassoon, 59, 75, 1.25, Applies::Always),
        (T::Harp, 23, 71, 0.5, Applies::InFastFigures),
    ];
    assert_eq!(REGISTER_FACTORS.len(), expected.len());
    for (row, (instrument, low, high, factor, applies)) in REGISTER_FACTORS.iter().zip(expected) {
        assert_eq!(row.instrument, instrument);
        assert_eq!(row.register, PitchRange::new(low, high), "{instrument:?}");
        assert!(close(row.factor, factor), "{instrument:?}");
        assert_eq!(row.applies, applies, "{instrument:?}");
        assert_eq!(row.check(), Ok(()));
        assert!(is_traceable(row.provenance), "{instrument:?}");
    }
}

#[test]
fn register_factors_lie_inside_the_playable_range() {
    // TST-07: a register is one the instrument plays.
    for row in &REGISTER_FACTORS {
        let PitchModel::Pitched(pitched) = *row.instrument.get().pitch() else {
            panic!("{:?} is pitched", row.instrument);
        };
        assert!(
            pitched.playable.range.encloses(row.register),
            "{:?}",
            row.instrument
        );
    }
}

fn sounding(note: i64) -> SoundingPitch {
    SoundingPitch::new(note).expect("a MIDI note")
}

#[test]
fn register_factor_lookup_applies_each_condition() {
    use InstrumentType::{Contrabassoon, Flute, Harp, Violin};
    let factor = |instrument, note, band, fast| {
        register_factor(instrument, sounding(note), band, fast).map(|row| row.factor)
    };
    let medium = DynamicBand::Medium;
    // TST-10: a factor applies inside its register and under its condition; elsewhere none (1).
    assert_eq!(factor(Flute, 59, medium, false), Some(0.5));
    assert_eq!(factor(Flute, 67, medium, false), Some(0.5));
    assert_eq!(factor(Flute, 68, medium, false), None);
    assert_eq!(factor(Flute, 79, medium, false), Some(1.5));
    assert_eq!(
        factor(Contrabassoon, 25, DynamicBand::Soft, false),
        Some(1.5)
    );
    assert_eq!(factor(Contrabassoon, 25, DynamicBand::Loud, false), None);
    assert_eq!(factor(Contrabassoon, 29, DynamicBand::Soft, false), None);
    assert_eq!(factor(Harp, 50, medium, true), Some(0.5));
    assert_eq!(factor(Harp, 50, medium, false), None);
    assert_eq!(factor(Harp, 72, medium, true), None);
    assert_eq!(factor(Violin, 60, medium, false), None);
}

#[test]
fn register_bands_split_midi_into_octaves_from_c2() {
    // TST-10: the band boundaries are the Cs from C2 (36) to C7 (96).
    let cases = [
        (0, RegisterBand::Sub),
        (35, RegisterBand::Sub),
        (36, RegisterBand::Bass),
        (47, RegisterBand::Bass),
        (48, RegisterBand::Tenor),
        (60, RegisterBand::Alto),
        (71, RegisterBand::Alto),
        (72, RegisterBand::Soprano),
        (84, RegisterBand::High),
        (95, RegisterBand::High),
        (96, RegisterBand::Top),
        (127, RegisterBand::Top),
    ];
    for (note, band) in cases {
        assert_eq!(RegisterBand::of(sounding(note)), band, "{note}");
    }
    // TST-07: the ranges cover MIDI in order without a gap, and each holds exactly its notes.
    let mut next = 0;
    for band in RegisterBand::ALL {
        let range = band.range();
        assert_eq!(range.low().get(), next, "{band:?}");
        next = range.high().get() + 1;
    }
    assert_eq!(next, 128);
    for note in 0..=127 {
        let pitch = sounding(note);
        assert!(RegisterBand::of(pitch).range().contains(pitch), "{note}");
    }
    assert!(is_traceable(RegisterBand::PROVENANCE));
}

#[test]
fn low_interval_limits_match_the_table() {
    // TST-10: the lowest lower note per interval, from the minor second (E3 = 52) to the major
    // ninth (E♭2 = 39); the octave has none.
    let expected = [
        (1, Some(52)),
        (2, Some(51)),
        (3, Some(48)),
        (4, Some(46)),
        (5, Some(45)),
        (6, Some(46)),
        (7, Some(34)),
        (8, Some(41)),
        (9, Some(41)),
        (10, Some(41)),
        (11, Some(41)),
        (12, None),
        (13, Some(40)),
        (14, Some(39)),
    ];
    assert_eq!(LOW_INTERVAL_LIMITS.rows.len(), expected.len());
    for (semitones, note) in expected {
        let limit = match note {
            Some(note) => Limit::Lowest(sounding(note)),
            None => Limit::Unlimited,
        };
        assert_eq!(
            LOW_INTERVAL_LIMITS.of(semitones),
            Some(limit),
            "{semitones}"
        );
    }
    // TST-03: a unison and an interval wider than a ninth are not in the table.
    assert_eq!(LOW_INTERVAL_LIMITS.of(0), None);
    assert_eq!(LOW_INTERVAL_LIMITS.of(15), None);
    assert_eq!(LOW_INTERVAL_LIMITS.check(), Ok(()));
    assert!(is_estimate(LOW_INTERVAL_LIMITS.provenance));
    // The table is looser than the low-bass dissonance rule: below D3 it still admits a fourth.
    assert!(45 < LOW_DISSONANCE.value.get());
}

const SECTION_WEIGHTS: WeightRow = WeightRow {
    selector: Selector::Ids(&[InstrumentType::Violin]),
    soft: Some(1.0),
    medium: None,
    loud: Some(2.0),
    provenance: FIXTURE,
};

#[test]
fn weight_validator_rejects_each_broken_row() {
    // TST-03: one fixture per failing check.
    assert_eq!(SECTION_WEIGHTS.check(), Ok(()));
    let weights = [
        (
            WeightRow {
                soft: None,
                loud: None,
                ..SECTION_WEIGHTS
            },
            RuleError::InvalidWeight,
        ),
        (
            WeightRow {
                soft: Some(0.0),
                ..SECTION_WEIGHTS
            },
            RuleError::InvalidWeight,
        ),
        (
            WeightRow {
                medium: Some(-1.0),
                ..SECTION_WEIGHTS
            },
            RuleError::InvalidWeight,
        ),
        (
            WeightRow {
                loud: Some(f32::NAN),
                ..SECTION_WEIGHTS
            },
            RuleError::InvalidWeight,
        ),
        (
            WeightRow {
                loud: Some(f32::INFINITY),
                ..SECTION_WEIGHTS
            },
            RuleError::InvalidWeight,
        ),
        (
            WeightRow {
                selector: Selector::Family(Family::Other),
                ..SECTION_WEIGHTS
            },
            RuleError::InvalidSelector,
        ),
        (
            WeightRow {
                provenance: Provenance::Estimate { reason: " " },
                ..SECTION_WEIGHTS
            },
            RuleError::EmptyProvenance,
        ),
    ];
    for (row, error) in weights {
        assert_eq!(row.check(), Err(error), "{row:?}");
    }
}

#[test]
fn factor_validator_rejects_each_broken_row() {
    // TST-03: a factor must be a positive finite number, with a provenance.
    let factor = RegisterFactor {
        instrument: InstrumentType::Flute,
        register: PitchRange::new(59, 67),
        factor: 0.5,
        applies: Applies::Always,
        provenance: FIXTURE,
    };
    assert_eq!(factor.check(), Ok(()));
    for broken in [0.0, -0.5, f32::NAN, f32::INFINITY] {
        assert_eq!(
            RegisterFactor {
                factor: broken,
                ..factor
            }
            .check(),
            Err(RuleError::InvalidFactor),
            "{broken}"
        );
    }
    assert_eq!(
        RegisterFactor {
            provenance: Provenance::Cited { source: "" },
            ..factor
        }
        .check(),
        Err(RuleError::EmptyProvenance)
    );
}

const MINOR_SECOND: IntervalLimit = IntervalLimit {
    semitones: 1,
    limit: Limit::Unlimited,
};

const MINOR_THIRD: IntervalLimit = IntervalLimit {
    semitones: 3,
    limit: Limit::Unlimited,
};

#[test]
fn limit_and_combination_validators_reject_each_broken_table() {
    // TST-03: the limits must run from one semitone upward without a gap.
    let limits = IntervalLimits {
        rows: &[MINOR_SECOND],
        provenance: FIXTURE,
    };
    assert_eq!(limits.check(), Ok(()));
    for rows in [
        &[][..],
        &[MINOR_THIRD],
        &[MINOR_SECOND, MINOR_THIRD],
        &[MINOR_SECOND, MINOR_SECOND],
    ] {
        assert_eq!(
            IntervalLimits { rows, ..limits }.check(),
            Err(RuleError::InvalidLimits),
            "{rows:?}"
        );
    }
    assert_eq!(
        IntervalLimits {
            provenance: Provenance::Estimate { reason: "" },
            ..limits
        }
        .check(),
        Err(RuleError::EmptyProvenance)
    );

    let combination = Combination {
        members: &[
            Selector::Ids(&[InstrumentType::BassDrum]),
            Selector::Ids(&[InstrumentType::TamTam]),
        ],
        effect: "loud",
        provenance: FIXTURE,
    };
    assert_eq!(combination.check(), Ok(()));
    let broken = [
        (
            Combination {
                members: &[Selector::Ids(&[InstrumentType::BassDrum])],
                ..combination
            },
            RuleError::InvalidSelector,
        ),
        (
            Combination {
                members: &[
                    Selector::Ids(&[InstrumentType::BassDrum]),
                    Selector::Ids(&[]),
                ],
                ..combination
            },
            RuleError::InvalidSelector,
        ),
        (
            Combination {
                effect: "",
                ..combination
            },
            RuleError::BlankText,
        ),
        (
            Combination {
                provenance: Provenance::Cited { source: " " },
                ..combination
            },
            RuleError::EmptyProvenance,
        ),
    ];
    for (row, error) in broken {
        assert_eq!(row.check(), Err(error), "{row:?}");
    }
}

/// A row of the function table as the test writes it: role, bands, carriers and colours.
type ExpectedCarriers = (
    Role,
    &'static [RegisterBand],
    &'static [InstrumentType],
    &'static [InstrumentType],
);

#[test]
fn the_function_table_names_the_handbook_carriers() {
    // TST-10: the melody and bass rows written out from Rimsky-Korsakov, Ch. II and III, in the
    // order a writer reaches for them: strings, woodwinds, brass, then the colours.
    use InstrumentType::{
        BassClarinet, Bassoon, Celesta, Cello, ClarinetA, ClarinetBb, ClarinetEb, Contrabassoon,
        DoubleBass, EnglishHorn, Euphonium, Flute, Glockenspiel, Harp, HornF, Oboe, Piccolo,
        Timpani, TromboneBass, TromboneTenor, TrumpetBb, TrumpetC, Tuba, Viola, Violin, Xylophone,
    };
    use RegisterBand::{Alto, Bass, High, Soprano, Sub, Tenor, Top};
    let expected: [ExpectedCarriers; 5] = [
        (
            Role::Melody,
            &[High, Top],
            &[Violin, Piccolo, Flute, ClarinetEb, TrumpetC, TrumpetBb],
            &[Glockenspiel, Xylophone, Celesta],
        ),
        (
            Role::Melody,
            &[Alto, Soprano],
            &[
                Violin,
                Flute,
                Oboe,
                ClarinetBb,
                ClarinetA,
                EnglishHorn,
                HornF,
                TrumpetBb,
                TrumpetC,
            ],
            &[],
        ),
        (
            Role::Melody,
            &[Tenor],
            &[
                Cello,
                Viola,
                Bassoon,
                BassClarinet,
                EnglishHorn,
                ClarinetBb,
                ClarinetA,
                HornF,
                TromboneTenor,
                Euphonium,
            ],
            &[],
        ),
        (
            Role::Melody,
            &[Sub, Bass],
            &[
                Cello,
                DoubleBass,
                Bassoon,
                Contrabassoon,
                Tuba,
                TromboneBass,
            ],
            &[],
        ),
        (
            Role::Bass,
            &[Sub, Bass],
            &[
                Cello,
                DoubleBass,
                Bassoon,
                Contrabassoon,
                BassClarinet,
                Tuba,
                TromboneBass,
            ],
            &[Timpani, Harp],
        ),
    ];
    assert_eq!(CARRIERS.len(), expected.len());
    for (row, (role, bands, instruments, colours)) in CARRIERS.iter().zip(expected) {
        assert_eq!(row.role, role);
        assert_eq!(row.bands, bands, "{role:?}");
        assert_eq!(row.instruments, instruments, "{role:?} {bands:?}");
        assert_eq!(row.colours, colours, "{role:?} {bands:?}");
    }
}

#[test]
fn every_function_row_is_valid_traceable_and_ordered_by_family() {
    let rank = |family: Family| match family {
        Family::Strings => 0,
        Family::Woodwinds => 1,
        Family::Brass => 2,
        _ => 3,
    };
    for row in &CARRIERS {
        assert_eq!(row.check(), Ok(()), "{row:?}");
        assert!(is_traceable(row.provenance), "{row:?}");
        // The strings come first, then the woodwinds, the brass and the others.
        assert!(
            row.instruments
                .is_sorted_by_key(|instrument| rank(instrument.family())),
            "{:?} {:?}",
            row.role,
            row.bands
        );
        // A carrier of a pitched line is pitched.
        for instrument in row.instruments.iter().chain(row.colours) {
            assert!(
                matches!(instrument.get().pitch(), PitchModel::Pitched(_)),
                "{instrument:?}"
            );
        }
    }
}

#[test]
fn the_melody_has_a_row_in_every_band_and_the_bass_only_low() {
    for band in RegisterBand::ALL {
        let melody = carriers(Role::Melody, band).expect("every band has melody carriers");
        assert!(melody.covers(band));
        assert_eq!(melody.role, Role::Melody);
    }
    // The first carrier of a melody is a violin from the alto band up, a cello below it.
    assert_eq!(
        carriers(Role::Melody, RegisterBand::Soprano).map(|row| row.instruments[0]),
        Some(InstrumentType::Violin)
    );
    assert_eq!(
        carriers(Role::Melody, RegisterBand::Tenor).map(|row| row.instruments[0]),
        Some(InstrumentType::Cello)
    );
    assert!(carriers(Role::Bass, RegisterBand::Bass).is_some());
    assert!(carriers(Role::Bass, RegisterBand::Sub).is_some());
    // TST-03: a function and band without a row give `None`, never another row.
    assert_eq!(carriers(Role::Bass, RegisterBand::Tenor), None);
    assert_eq!(carriers(Role::Harmony, RegisterBand::Alto), None);
}

#[test]
fn carriers_validator_rejects_each_broken_row() {
    // TST-03: one fixture per failing check.
    let row = Carriers {
        role: Role::Melody,
        bands: &[RegisterBand::Alto, RegisterBand::Soprano],
        instruments: &[InstrumentType::Violin, InstrumentType::Flute],
        colours: &[InstrumentType::Celesta],
        provenance: FIXTURE,
    };
    assert_eq!(row.check(), Ok(()));
    let broken = [
        Carriers { bands: &[], ..row },
        Carriers {
            bands: &[RegisterBand::Alto, RegisterBand::Alto],
            ..row
        },
        Carriers {
            instruments: &[],
            ..row
        },
        Carriers {
            instruments: &[InstrumentType::Violin, InstrumentType::Violin],
            ..row
        },
        Carriers {
            colours: &[InstrumentType::Flute],
            ..row
        },
    ];
    for broken in broken {
        assert_eq!(
            broken.check(),
            Err(RuleError::InvalidCarriers),
            "{broken:?}"
        );
    }
    assert_eq!(
        Carriers {
            provenance: Provenance::Estimate { reason: "" },
            ..row
        }
        .check(),
        Err(RuleError::EmptyProvenance)
    );
}
