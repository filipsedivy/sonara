//! The orchestration findings: each rule a project's notes are checked against gives its finding
//! on a project built to break it and none on one that keeps it, with the rule's id as the code,
//! its severity, its place and its explanation; tracks without a part give none.
//!
//! The passages are those of the rule predicates' own contract tests
//! (`crates/sonara-midi/tests/check.rs`), written there by hand from each rule's check text and
//! thresholds (TST-10), here laid out as the tracks and clips of a project. The project runs at
//! crotchet = 120 in 4/4, so a beat lasts 500 ms.

use std::sync::Arc;

use sonara_app::quality::orchestration::check_orchestration;
use sonara_app::quality::{Finding, Severity, check_project};
use sonara_core::midi::MidiNote;
use sonara_core::model::clip::{Clip, MidiClip, MidiClipData};
use sonara_core::model::part::Part;
use sonara_core::model::project::ProjectData;
use sonara_core::model::track::{Track, TrackData, TrackKind};
use sonara_core::model::{JsonMap, Valid};
use sonara_midi::orchestration::InstrumentType as I;
use sonara_midi::orchestration::rules::RuleId;
use sonara_midi::performance::check::{CHECKED_RULES, CheckError};

/// A note at mezzo-forte (velocity 80).
fn note(id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, 80, 0).expect("a valid note")
}

/// A note at `velocity`.
fn at(velocity: i64, id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, velocity, 0).expect("a valid note")
}

/// A note played with `articulation`.
fn played(articulation: &str, id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    note(id, pitch, start, length)
        .with_articulation(Some(articulation.to_owned()))
        .expect("a vocabulary id")
}

/// `count` notes of `pitch`, `step` beats apart and `length` long, from beat `start`; ids
/// `{prefix}0`, `{prefix}1`, ...
fn run(prefix: &str, pitch: i64, start: f64, step: f64, length: f64, count: u32) -> Vec<MidiNote> {
    (0..count)
        .map(|index| {
            note(
                &format!("{prefix}{index}"),
                pitch,
                start + f64::from(index) * step,
                length,
            )
        })
        .collect()
}

/// A line of notes `pitches`, one per beat from beat 0, a beat long; ids `{prefix}0`, ...
fn line_of(prefix: &str, pitches: &[i64]) -> Vec<MidiNote> {
    pitches
        .iter()
        .zip(0_u32..)
        .map(|(&pitch, index)| note(&format!("{prefix}{index}"), pitch, f64::from(index), 1.0))
        .collect()
}

/// A MIDI clip `id` at timeline beat `start`, `length` beats long, holding `notes`.
fn clip_data(id: &str, start: f64, length: f64, notes: Vec<MidiNote>) -> MidiClipData {
    MidiClipData {
        id: id.to_owned(),
        name: id.to_owned(),
        start,
        length,
        notes: Arc::new(notes),
        controls: Arc::new(Vec::new()),
        content_offset: 0.0,
        loop_length: None,
        muted: false,
        color: None,
        extra: JsonMap::new(),
    }
}

fn midi(data: MidiClipData) -> Clip {
    Clip::Midi(MidiClip::new(data).expect("a valid clip"))
}

/// A clip at timeline beat 0 long enough for `notes`.
fn clip(id: &str, notes: Vec<MidiNote>) -> Clip {
    let end = notes.iter().map(MidiNote::end).fold(1.0, f64::max);
    midi(clip_data(id, 0.0, end, notes))
}

/// An instrument track `trk_{slug}` named `slug` with `clips`, playing a part of `instrument`
/// when one is given.
fn track(slug: &str, instrument: Option<&str>, clips: Vec<Clip>) -> Track {
    let mut data = TrackData::new(format!("trk_{slug}"), slug, TrackKind::Instrument);
    data.part = instrument.map(|id| Part::new(id).expect("a vocabulary id"));
    data.clips = clips;
    Valid::new(data).expect("a valid track")
}

fn project(tracks: Vec<Track>) -> ProjectData {
    let mut data = ProjectData::new("prj_1", "Orchestration");
    data.tracks = tracks;
    data
}

/// One part of a fixture: the track's slug, the instrument of its part and its notes, all in
/// one clip `clp_{slug}` at beat 0.
type Spec = (&'static str, I, Vec<MidiNote>);

/// A project with one track per spec.
fn score(specs: Vec<Spec>) -> ProjectData {
    project(
        specs
            .into_iter()
            .map(|(slug, instrument, notes)| {
                track(
                    slug,
                    Some(instrument.id()),
                    vec![clip(&format!("clp_{slug}"), notes)],
                )
            })
            .collect(),
    )
}

/// The findings of `project`.
fn findings(project: &ProjectData) -> Vec<Finding> {
    check_orchestration(project)
        .expect("a valid project")
        .findings
}

/// The findings of `project` with `code`.
fn with_code(project: &ProjectData, code: &str) -> Vec<Finding> {
    findings(project)
        .into_iter()
        .filter(|finding| finding.code == code)
        .collect()
}

fn ids(finding: &Finding) -> Vec<&str> {
    finding.note_ids.iter().map(String::as_str).collect()
}

/// What a rule's fixture expects of its first finding.
struct Expected {
    severity: Severity,
    /// The slug of the track the finding is placed on.
    track: &'static str,
    /// The notes of the first finding, or `None` to leave them unchecked.
    notes: Option<&'static [&'static str]>,
}

const fn expect(
    severity: Severity,
    track: &'static str,
    notes: Option<&'static [&'static str]>,
) -> Expected {
    Expected {
        severity,
        track,
        notes,
    }
}

/// A fixture: the parts that break the rule, what the first finding is, and parts that keep it.
struct Fixture {
    fires: Vec<Spec>,
    expected: Expected,
    quiet: Vec<Spec>,
}

/// The fixture of `rule`, or `None` for a rule that has none.
#[expect(
    clippy::too_many_lines,
    reason = "one table of twenty hand-written passages, each next to its quiet twin, reads \
              better whole than split into twenty functions"
)]
fn fixture(rule: RuleId) -> Option<Fixture> {
    use Severity::{Error, Recommendation, Warning};
    let fixture = match rule {
        // The violin plays G3-A7 (55-105).
        RuleId::Rng01 => Fixture {
            fires: vec![(
                "violins",
                I::Violin,
                vec![
                    note("low", 54, 0.0, 1.0),
                    note("ok", 60, 1.0, 1.0),
                    note("high_1", 106, 2.0, 1.0),
                    note("high_2", 107, 3.0, 1.0),
                ],
            )],
            expected: expect(Error, "violins", Some(&["low"])),
            quiet: vec![(
                "violins",
                I::Violin,
                vec![note("g", 55, 0.0, 1.0), note("a", 105, 1.0, 1.0)],
            )],
        },
        // The horn is comfortable from F2 to C5 (41-72).
        RuleId::Rng02 => Fixture {
            fires: vec![(
                "horn",
                I::HornF,
                vec![note("low", 38, 0.0, 1.0), note("ok", 60, 1.0, 1.0)],
            )],
            expected: expect(Warning, "horn", Some(&["low"])),
            quiet: vec![(
                "horn",
                I::HornF,
                vec![note("f2", 41, 0.0, 1.0), note("c5", 72, 1.0, 1.0)],
            )],
        },
        // Nine crotchets on C6 above the trumpet's comfortable B♭5 last 4.5 s, more than two bars
        // of moderate 4/4; eight last exactly two bars.
        RuleId::Rng03 => Fixture {
            fires: vec![("trumpet", I::TrumpetBb, run("c", 84, 0.0, 1.0, 1.0, 9))],
            expected: expect(Warning, "trumpet", None),
            quiet: vec![("trumpet", I::TrumpetBb, run("c", 84, 0.0, 1.0, 1.0, 8))],
        },
        // A minor third is clear from C3 (48); A2-C3 lies below, C3-E♭3 at the limit.
        RuleId::Spc01 => Fixture {
            fires: vec![(
                "cellos",
                I::Cello,
                vec![note("a2", 45, 0.0, 2.0), note("c3", 48, 0.0, 2.0)],
            )],
            expected: expect(Recommendation, "cellos", Some(&["a2", "c3"])),
            quiet: vec![(
                "cellos",
                I::Cello,
                vec![note("c3", 48, 0.0, 1.0), note("eb3", 51, 0.0, 1.0)],
            )],
        },
        // A fourth from G2 below D3 between a cello and a bassoon; a fifth is a consonance.
        RuleId::Spc02 => Fixture {
            fires: vec![
                ("cellos", I::Cello, vec![note("g2", 43, 0.0, 1.0)]),
                ("bassoon", I::Bassoon, vec![note("c3", 48, 0.0, 1.0)]),
            ],
            expected: expect(Recommendation, "cellos", Some(&["g2", "c3"])),
            quiet: vec![
                ("cellos", I::Cello, vec![note("g2", 43, 0.0, 1.0)]),
                ("bassoon", I::Bassoon, vec![note("d3", 50, 0.0, 1.0)]),
            ],
        },
        // A bassoon an octave above a clarinet for four notes; in the natural order, below it.
        RuleId::Dbl02 => Fixture {
            fires: vec![
                ("clarinet", I::ClarinetBb, line_of("cl", &[50, 52, 53, 55])),
                ("bassoon", I::Bassoon, line_of("bn", &[62, 64, 65, 67])),
            ],
            expected: expect(Recommendation, "clarinet", None),
            quiet: vec![
                ("clarinet", I::ClarinetBb, line_of("cl", &[62, 64, 65, 67])),
                ("bassoon", I::Bassoon, line_of("bn", &[50, 52, 53, 55])),
            ],
        },
        // Two flutes in octaves, then with the violins doubling them.
        RuleId::Dbl03 => Fixture {
            fires: vec![
                ("flute_1", I::Flute, line_of("a", &[84, 86, 88, 89])),
                ("flute_2", I::Flute, line_of("b", &[72, 74, 76, 77])),
            ],
            expected: expect(Recommendation, "flute_1", None),
            quiet: vec![
                ("flute_1", I::Flute, line_of("a", &[84, 86, 88, 89])),
                ("flute_2", I::Flute, line_of("b", &[72, 74, 76, 77])),
                ("violins", I::Violin, line_of("v", &[72, 74, 76, 77])),
            ],
        },
        // A flute an octave above a trombone; two octaves above it.
        RuleId::Dbl04 => Fixture {
            fires: vec![
                ("flute", I::Flute, line_of("f", &[60, 62, 64, 65])),
                (
                    "trombone",
                    I::TromboneTenor,
                    line_of("t", &[48, 50, 52, 53]),
                ),
            ],
            expected: expect(Warning, "flute", None),
            quiet: vec![
                ("flute", I::Flute, line_of("f", &[72, 74, 76, 77])),
                (
                    "trombone",
                    I::TromboneTenor,
                    line_of("t", &[48, 50, 52, 53]),
                ),
            ],
        },
        // Violins and a horn in unison; with an oboe joining them.
        RuleId::Dbl09 => Fixture {
            fires: vec![
                ("violins", I::Violin, line_of("v", &[60, 62, 64, 65, 67])),
                ("horn", I::HornF, line_of("h", &[60, 62, 64, 65, 67])),
            ],
            expected: expect(Recommendation, "violins", None),
            quiet: vec![
                ("violins", I::Violin, line_of("v", &[60, 62, 64, 65, 67])),
                ("horn", I::HornF, line_of("h", &[60, 62, 64, 65, 67])),
                ("oboe", I::Oboe, line_of("o", &[60, 62, 64, 65, 67])),
            ],
        },
        // A triple stop held a crotchet (500 ms); a quick one of a quaver.
        RuleId::Voi12 => Fixture {
            fires: vec![(
                "violins",
                I::Violin,
                vec![
                    note("g", 55, 0.0, 1.0),
                    note("d", 62, 0.0, 1.0),
                    note("b", 71, 0.0, 1.0),
                ],
            )],
            expected: expect(Warning, "violins", Some(&["g", "d", "b"])),
            quiet: vec![(
                "violins",
                I::Violin,
                vec![
                    note("g", 55, 0.0, 0.5),
                    note("d", 62, 0.0, 0.5),
                    note("b", 71, 0.0, 0.5),
                ],
            )],
        },
        // 34 crotchets without a rest last 17 s, more than 8 bars of moderate 4/4; a quaver rest
        // after 32 of them breaks the phrase.
        RuleId::Ply01 => Fixture {
            fires: vec![("flute", I::Flute, run("f", 72, 0.0, 1.0, 1.0, 34))],
            expected: expect(Warning, "flute", None),
            quiet: vec![(
                "flute",
                I::Flute,
                run("a", 72, 0.0, 1.0, 1.0, 32)
                    .into_iter()
                    .chain(run("b", 72, 32.5, 1.0, 1.0, 32))
                    .collect(),
            )],
        },
        // Four repeated A2s below the horn's middle register; three.
        RuleId::Ply02 => Fixture {
            fires: vec![("horn", I::HornF, run("r", 45, 0.0, 1.0, 1.0, 4))],
            expected: expect(Warning, "horn", Some(&["r0", "r1", "r2", "r3"])),
            quiet: vec![("horn", I::HornF, run("r", 45, 0.0, 1.0, 1.0, 3))],
        },
        // The string mute put on with one second free, where it needs two bars (4 s); with six.
        RuleId::Ply04 => Fixture {
            fires: vec![(
                "violins",
                I::Violin,
                vec![
                    note("open", 60, 0.0, 4.0),
                    played("con_sordino", "muted", 60, 6.0, 1.0),
                ],
            )],
            expected: expect(Warning, "violins", Some(&["open", "muted"])),
            quiet: vec![(
                "violins",
                I::Violin,
                vec![
                    note("open", 60, 0.0, 4.0),
                    played("con_sordino", "muted", 60, 12.0, 1.0),
                ],
            )],
        },
        // From pizzicato to the bow in a quaver (250 ms), less than half a second; in a crotchet.
        RuleId::Ply05 => Fixture {
            fires: vec![(
                "violins",
                I::Violin,
                vec![
                    played("pizzicato", "pizz", 60, 0.0, 0.5),
                    note("arco", 62, 0.75, 1.0),
                ],
            )],
            expected: expect(Warning, "violins", Some(&["pizz", "arco"])),
            quiet: vec![(
                "violins",
                I::Violin,
                vec![
                    played("pizzicato", "pizz", 60, 0.0, 1.0),
                    note("arco", 62, 1.0, 1.0),
                ],
            )],
        },
        // Four plucks 100 ms apart, faster than the violin plucks; semiquavers (125 ms).
        RuleId::Ply06 => Fixture {
            fires: vec![(
                "violins",
                I::Violin,
                (0..4)
                    .map(|i| played("pizzicato", &format!("p{i}"), 60, f64::from(i) * 0.2, 0.2))
                    .collect(),
            )],
            expected: expect(Recommendation, "violins", Some(&["p0", "p1", "p2", "p3"])),
            quiet: vec![(
                "violins",
                I::Violin,
                (0..8)
                    .map(|i| played("pizzicato", &format!("p{i}"), 60, f64::from(i) * 0.25, 0.25))
                    .collect(),
            )],
        },
        // Timpani struck a third apart; a fifth apart.
        RuleId::Ply12 => Fixture {
            fires: vec![(
                "timpani",
                I::Timpani,
                vec![note("a2", 45, 0.0, 1.0), note("c3", 48, 0.0, 1.0)],
            )],
            expected: expect(Recommendation, "timpani", Some(&["a2", "c3"])),
            quiet: vec![(
                "timpani",
                I::Timpani,
                vec![note("a2", 45, 0.0, 1.0), note("e3", 52, 0.0, 1.0)],
            )],
        },
        // Four oboe A4s 100 ms apart, faster than single tonguing; semiquavers.
        RuleId::Ply14 => Fixture {
            fires: vec![("oboe", I::Oboe, run("o", 69, 0.0, 0.2, 0.2, 4))],
            expected: expect(Recommendation, "oboe", Some(&["o0", "o1", "o2", "o3"])),
            quiet: vec![("oboe", I::Oboe, run("o", 69, 0.0, 0.25, 0.25, 8))],
        },
        // Rapid octave leaps on the clarinet; a seventh.
        RuleId::Ply15 => Fixture {
            fires: vec![(
                "clarinet",
                I::ClarinetBb,
                vec![
                    note("a", 60, 0.0, 0.25),
                    note("b", 72, 0.25, 0.25),
                    note("c", 60, 0.5, 0.25),
                    note("d", 62, 2.0, 1.0),
                ],
            )],
            expected: expect(Recommendation, "clarinet", Some(&["a", "b", "c"])),
            quiet: vec![(
                "clarinet",
                I::ClarinetBb,
                vec![note("a", 60, 0.0, 0.25), note("b", 71, 0.25, 0.25)],
            )],
        },
        // A celesta in three of four bars of violins; in two.
        RuleId::Tex01 => Fixture {
            fires: vec![
                ("violins", I::Violin, vec![note("v", 60, 0.0, 16.0)]),
                (
                    "celesta",
                    I::Celesta,
                    vec![
                        note("a", 72, 0.0, 1.0),
                        note("b", 72, 4.0, 1.0),
                        note("c", 72, 8.0, 1.0),
                    ],
                ),
            ],
            expected: expect(Recommendation, "celesta", Some(&["a", "b", "c"])),
            quiet: vec![
                ("violins", I::Violin, vec![note("v", 60, 0.0, 16.0)]),
                (
                    "celesta",
                    I::Celesta,
                    vec![note("a", 72, 0.0, 4.0), note("b", 72, 8.0, 4.0)],
                ),
            ],
        },
        // Three percussion parts sounding at once; two.
        RuleId::Tex02 => Fixture {
            fires: vec![
                ("timpani", I::Timpani, vec![note("t", 45, 0.0, 2.0)]),
                ("bass_drum", I::BassDrum, vec![note("b", 36, 1.0, 2.0)]),
                ("snare", I::SnareDrum, vec![note("s", 38, 1.5, 1.0)]),
            ],
            expected: expect(Recommendation, "timpani", Some(&["t", "b", "s"])),
            quiet: vec![
                ("timpani", I::Timpani, vec![note("t", 45, 0.0, 2.0)]),
                ("bass_drum", I::BassDrum, vec![note("b", 36, 0.0, 2.0)]),
            ],
        },
        _ => return None,
    };
    Some(fixture)
}

#[test]
fn every_checked_rule_has_a_fixture() {
    // A rule checked from the notes but without a fixture here would go untested.
    for rule in CHECKED_RULES {
        assert!(fixture(rule).is_some(), "{} has no fixture", rule.code());
    }
}

#[test]
fn each_rule_fires_with_its_code_severity_and_place() {
    for rule in CHECKED_RULES {
        let fixture = fixture(rule).expect("a fixture");
        let code = rule.code();
        let found = with_code(&score(fixture.fires), code);
        let first = found
            .first()
            .unwrap_or_else(|| panic!("{code} gives no finding"));
        let expected = &fixture.expected;
        assert_eq!(first.severity, expected.severity, "{code}");
        assert_eq!(
            first.track_id.as_deref(),
            Some(format!("trk_{}", expected.track).as_str()),
            "{code}"
        );
        if let Some(notes) = expected.notes {
            assert_eq!(ids(first), notes, "{code}");
        }
        assert!(
            first.message.starts_with(rule.get().title()),
            "{code}: {}",
            first.message
        );
        assert!(
            first
                .explanation
                .starts_with(&format!("{code} flags this when ")),
            "{code}: {}",
            first.explanation
        );
    }
}

#[test]
fn each_rule_stays_quiet_where_it_is_kept() {
    for rule in CHECKED_RULES {
        let fixture = fixture(rule).expect("a fixture");
        let code = rule.code();
        assert_eq!(with_code(&score(fixture.quiet), code), [], "{code}");
    }
}

#[test]
fn a_project_without_parts_gives_no_finding() {
    // The notes that put a violin part out of range, on tracks that state no part: one named
    // after the instrument, one with a part an unknown instrument plays (reported when the
    // project opens), and a folder.
    let notes = || vec![note("low", 30, 0.0, 1.0), note("high", 120, 1.0, 1.0)];
    let unnamed = project(vec![
        track("violin", None, vec![clip("clp_a", notes())]),
        track("unknown", Some("violin_xl"), vec![clip("clp_b", notes())]),
        Valid::new(TrackData::new("trk_folder", "Strings", TrackKind::Folder)).expect("valid"),
    ]);
    let report = check_orchestration(&unnamed).expect("a valid project");
    assert_eq!(report.findings, []);
    assert!(report.is_sound());
    assert_eq!(findings(&project(Vec::new())), []);
    // A part without notes has nothing to check either.
    let silent = project(vec![track("violins", Some("violin"), Vec::new())]);
    assert_eq!(findings(&silent), []);
}

#[test]
fn the_message_names_the_tracks_and_bars_and_the_explanation_its_basis() {
    let data = score(vec![(
        "violins",
        I::Violin,
        vec![note("low", 54, 4.0, 1.0), note("high", 106, 8.0, 5.0)],
    )]);
    let found = with_code(&data, "RNG-01");
    let messages: Vec<&str> = found.iter().map(|f| f.message.as_str()).collect();
    assert_eq!(
        messages,
        [
            "Note outside the playable range on 'violins', bar 2: 1 note below the playable \
             range of the Violin, G3 (55) to A7 (105).",
            "Note outside the playable range on 'violins', bars 3-4: 1 note above the playable \
             range of the Violin, G3 (55) to A7 (105).",
        ]
    );
    assert_eq!(found[0].clip_id.as_deref(), Some("clp_violins"));
    // The range rule rests on an estimate; the rule of sustained high brass on a source.
    assert!(
        found[0]
            .explanation
            .starts_with("RNG-01 flags this when a note lies below the lowest or above the highest playable note of its instrument. An estimate: "),
        "{}",
        found[0].explanation
    );
    let brass = score(vec![(
        "trumpet",
        I::TrumpetBb,
        run("c", 84, 0.0, 1.0, 1.0, 9),
    )]);
    let found = with_code(&brass, "RNG-03");
    assert!(
        found[0].explanation.contains(". Source: "),
        "{}",
        found[0].explanation
    );
    assert!(
        found[0].message.contains("bars 1-3"),
        "{}",
        found[0].message
    );
}

#[test]
fn a_finding_across_tracks_names_them_all_and_sits_on_the_first() {
    let data = score(vec![
        ("flute_1", I::Flute, line_of("a", &[84, 86, 88, 89])),
        ("flute_2", I::Flute, line_of("b", &[72, 74, 76, 77])),
    ]);
    let found = with_code(&data, "DBL-03");
    assert_eq!(found.len(), 1);
    assert!(
        found[0].message.starts_with(
            "Same-colour winds in octaves without strings on 'flute_1' and 'flute_2', bar 1: "
        ),
        "{}",
        found[0].message
    );
    assert_eq!(found[0].track_id.as_deref(), Some("trk_flute_1"));
    // The notes of both tracks, in note order; two clips, so no one clip.
    assert_eq!(
        ids(&found[0]),
        ["b0", "a0", "b1", "a1", "b2", "a2", "b3", "a3"]
    );
    assert_eq!(found[0].clip_id, None);
}

#[test]
fn notes_meet_where_the_timeline_plays_them() {
    // The cello's G2 is the second beat of bar 2 of its clip, which starts at beat 0; the
    // bassoon's C3 is the first note of its clip, which starts at beat 5: they sound together.
    let cello = |start| {
        track(
            "cellos",
            Some("cello"),
            vec![midi(clip_data(
                "clp_vc",
                start,
                8.0,
                vec![note("g2", 43, 5.0, 1.0)],
            ))],
        )
    };
    let bassoon = track(
        "bassoon",
        Some("bassoon"),
        vec![midi(clip_data(
            "clp_bn",
            5.0,
            4.0,
            vec![note("c3", 48, 0.0, 1.0)],
        ))],
    );
    let together = project(vec![cello(0.0), bassoon.clone()]);
    let found = with_code(&together, "SPC-02");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(ids(&found[0]), ["g2", "c3"]);
    assert!(
        found[0].message.contains(", bar 2: "),
        "{}",
        found[0].message
    );
    // A beat later on the timeline, the cello's note no longer meets the bassoon's.
    let apart = project(vec![cello(1.0), bassoon]);
    assert_eq!(with_code(&apart, "SPC-02"), []);
}

#[test]
fn a_looped_clip_repeats_and_a_muted_clip_is_not_played() {
    // Three crotchets on C6 looped three times are nine on the timeline: more than two bars of
    // high brass. Each note is named once.
    let mut looped = clip_data("clp_loop", 0.0, 9.0, run("c", 84, 0.0, 1.0, 1.0, 3));
    looped.loop_length = Some(3.0);
    let data = project(vec![track(
        "trumpet",
        Some("trumpet_bb"),
        vec![midi(looped)],
    )]);
    let found = with_code(&data, "RNG-03");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(ids(&found[0]), ["c0", "c1", "c2"]);
    assert_eq!(found[0].clip_id.as_deref(), Some("clp_loop"));
    // A muted clip of notes below the violin's range plays nothing.
    let mut muted = clip_data("clp_muted", 0.0, 4.0, vec![note("low", 30, 0.0, 1.0)]);
    muted.muted = true;
    let data = project(vec![track("violins", Some("violin"), vec![midi(muted)])]);
    assert_eq!(findings(&data), []);
}

#[test]
fn notes_outside_the_clip_are_not_played_and_a_note_is_cut_at_its_end() {
    // A triple stop held a crotchet in a clip a quaver long sounds a quaver: a quick chord.
    let chord = vec![
        note("g", 55, 0.0, 1.0),
        note("d", 62, 0.0, 1.0),
        note("b", 71, 0.0, 1.0),
    ];
    let cut = project(vec![track(
        "violins",
        Some("violin"),
        vec![midi(clip_data("clp_cut", 0.0, 0.5, chord.clone()))],
    )]);
    assert_eq!(with_code(&cut, "VOI-12"), []);
    let whole = project(vec![track(
        "violins",
        Some("violin"),
        vec![midi(clip_data("clp_whole", 0.0, 1.0, chord))],
    )]);
    assert_eq!(with_code(&whole, "VOI-12").len(), 1);
    // A note before the clip's content offset and one after its end are not played.
    let mut offset = clip_data(
        "clp_offset",
        0.0,
        2.0,
        vec![
            note("before", 30, 0.0, 1.0),
            note("ok", 60, 1.0, 1.0),
            note("after", 120, 3.0, 1.0),
        ],
    );
    offset.content_offset = 1.0;
    let data = project(vec![track("violins", Some("violin"), vec![midi(offset)])]);
    assert_eq!(findings(&data), []);
}

#[test]
fn findings_come_most_serious_first_and_repeat() {
    // An oboe repeating a note faster than it tongues (a hint), then a horn below its range (an
    // error) and outside its comfortable range (a warning).
    let data = score(vec![
        ("oboe", I::Oboe, run("o", 69, 0.0, 0.2, 0.2, 4)),
        (
            "horn",
            I::HornF,
            vec![note("off", 30, 8.0, 1.0), note("low", 38, 9.0, 1.0)],
        ),
    ]);
    let report = check_orchestration(&data).expect("a valid project");
    let severities: Vec<Severity> = report.findings.iter().map(|f| f.severity).collect();
    assert!(
        severities.windows(2).all(|pair| pair[0] <= pair[1]),
        "{severities:?}"
    );
    let codes: Vec<&str> = report.findings.iter().map(|f| f.code).collect();
    assert_eq!(codes, ["RNG-01", "RNG-02", "PLY-14"]);
    assert!(!report.is_sound());
    assert_eq!(report.to_json()["errors"], 1);
    // TST-04: the same project gives the same report.
    assert_eq!(check_orchestration(&data), Ok(report));
}

#[test]
fn an_invalid_note_is_refused_and_named() {
    // TST-03: a velocity of 0 written into the field directly, which the model never accepts.
    let mut silent = note("silent", 60, 0.0, 1.0);
    silent.velocity = 0;
    let data = score(vec![("violins", I::Violin, vec![silent])]);
    assert_eq!(
        check_orchestration(&data),
        Err(CheckError::InvalidNote("silent".to_owned()))
    );
}

#[test]
fn the_project_checks_keep_their_codes() {
    // The orchestration findings are a check of their own: the project checks of a score that
    // breaks a range rule give none of the rule codes.
    let data = score(vec![(
        "violins",
        I::Violin,
        vec![note("low", 54, 0.0, 1.0), at(90, "ok", 60, 1.0, 1.0)],
    )]);
    let codes: Vec<&str> = check_project(&data)
        .findings
        .iter()
        .map(|f| f.code)
        .collect();
    assert!(
        codes.iter().all(|code| RuleId::from_code(code).is_none()),
        "{codes:?}"
    );
    assert_eq!(with_code(&data, "RNG-01").len(), 1);
}
