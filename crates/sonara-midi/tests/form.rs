//! Song-form generation (SON-P10): the plan, its timing on the bar grid, and applying and
//! regenerating through project commands.

use std::collections::HashSet;
use std::sync::Arc;

use sonara_core::error::ErrorCode;
use sonara_core::ids::SequentialIds;
use sonara_core::model::markers::Section;
use sonara_core::model::project::Project;
use sonara_core::plugins::builtin_registry;
use sonara_core::session::{ProjectLocation, ProjectSession};
use sonara_core::time::{TimeSignatureMap, TimeSignaturePoint};
use sonara_midi::form::{
    FormConfig, FormTemplate, Level, SectionChange, SectionRole, SongForm, apply_song_form,
    plan_form, regenerate_in_plan, regenerate_section,
};

fn four_four() -> TimeSignatureMap {
    TimeSignatureMap::default()
}

fn plan(config: &FormConfig, signatures: &TimeSignatureMap) -> SongForm {
    plan_form(config, signatures, &mut SequentialIds::new(0)).expect("valid configuration")
}

fn roles(form: &SongForm) -> Vec<SectionRole> {
    form.sections().iter().map(|s| s.role).collect()
}

fn labels(form: &SongForm) -> Vec<&str> {
    form.sections().iter().map(|s| s.label.as_str()).collect()
}

fn session() -> ProjectSession {
    let mut ids = SequentialIds::new(5000);
    let project = Project::create(&mut ids, "Song", 48_000, 120.0).expect("project");
    let mut session = ProjectSession::new(Arc::new(builtin_registry()));
    session.load(
        project,
        ProjectLocation {
            path: None,
            workspace: std::env::temp_dir(),
            packaged: false,
        },
        true,
    );
    session
}

fn project_sections(session: &ProjectSession) -> Vec<Section> {
    session.require_project().expect("project").sections.clone()
}

#[test]
fn every_template_has_its_recorded_roles_and_bars() {
    use SectionRole::{Breakdown, Bridge, Chorus, Intro, Outro, PreChorus, Verse};
    let expected: [(FormTemplate, Vec<SectionRole>, u32); 4] = [
        (
            FormTemplate::VerseChorus,
            vec![Intro, Verse, Chorus, Verse, Chorus, Outro],
            40,
        ),
        (
            FormTemplate::PopPreChorus,
            vec![
                Intro, Verse, PreChorus, Chorus, Verse, PreChorus, Chorus, Bridge, Chorus, Outro,
            ],
            64,
        ),
        (FormTemplate::Aaba, vec![Verse, Verse, Bridge, Verse], 32),
        (
            FormTemplate::Dance,
            vec![
                Intro, Verse, Breakdown, Chorus, Verse, Breakdown, Chorus, Outro,
            ],
            96,
        ),
    ];
    for (template, expected_roles, bars) in expected {
        let form = plan(&FormConfig::new(template), &four_four());
        assert_eq!(roles(&form), expected_roles, "{}", template.name());
        assert_eq!(form.total_bars(), bars, "{}", template.name());
        assert_eq!(form.template(), template);
    }
}

#[test]
fn labels_number_only_repeated_roles() {
    let form = plan(&FormConfig::new(FormTemplate::VerseChorus), &four_four());
    assert_eq!(
        labels(&form),
        [
            "Intro", "Verse 1", "Chorus 1", "Verse 2", "Chorus 2", "Outro"
        ]
    );
    let form = plan(&FormConfig::new(FormTemplate::PopPreChorus), &four_four());
    assert_eq!(
        labels(&form),
        [
            "Intro",
            "Verse 1",
            "Pre-Chorus 1",
            "Chorus 1",
            "Verse 2",
            "Pre-Chorus 2",
            "Chorus 2",
            "Bridge",
            "Chorus 3",
            "Outro"
        ]
    );
}

#[test]
fn sections_are_contiguous_with_unique_ids_in_4_4() {
    let form = plan(&FormConfig::new(FormTemplate::VerseChorus), &four_four());
    let ids: HashSet<&str> = form.sections().iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids.len(), form.sections().len());
    assert_eq!(form.sections()[0].id, "sec_000000000001");
    let mut beat = 0.0;
    for s in form.sections() {
        assert!((s.start_beat - beat).abs() < 1e-12, "{}", s.label);
        assert!((s.length_beats - f64::from(s.bars) * 4.0).abs() < 1e-12);
        beat = s.end_beat();
    }
    assert!((form.end_beat() - 160.0).abs() < 1e-12);
    assert!(form.start_beat().abs() < 1e-12);
}

#[test]
fn lengths_follow_the_meter_including_a_change_inside_the_song() {
    let three_four =
        TimeSignatureMap::new(vec![TimeSignaturePoint::new(0, 3, 4).expect("3/4")]).expect("map");
    let form = plan(&FormConfig::new(FormTemplate::Aaba), &three_four);
    assert!((form.end_beat() - 96.0).abs() < 1e-12, "32 bars of 3 beats");

    // 4/4 for the first 12 bars, then 7/8 (3.5 beats per bar).
    let mixed = TimeSignatureMap::new(vec![
        TimeSignaturePoint::new(0, 4, 4).expect("4/4"),
        TimeSignaturePoint::new(12, 7, 8).expect("7/8"),
    ])
    .expect("map");
    let form = plan(&FormConfig::new(FormTemplate::Aaba), &mixed);
    let starts: Vec<f64> = form.sections().iter().map(|s| s.start_beat).collect();
    // Bars 0-7 | 8-15 (crosses the change at bar 12) | 16-23 | 24-31.
    assert_eq!(starts, [0.0, 32.0, 62.0, 90.0]);
    assert!((form.sections()[1].length_beats - 30.0).abs() < 1e-12);
    for pair in form.sections().windows(2) {
        assert!((pair[0].end_beat() - pair[1].start_beat).abs() < 1e-12);
    }
}

#[test]
fn start_bar_options_and_overrides_are_applied() {
    let mut config = FormConfig::new(FormTemplate::VerseChorus);
    config.start_bar = 2;
    config.omit_intro = true;
    config.omit_outro = true;
    config.bars.insert(SectionRole::Chorus, 16);
    let form = plan(&config, &four_four());
    assert_eq!(
        roles(&form),
        [
            SectionRole::Verse,
            SectionRole::Chorus,
            SectionRole::Verse,
            SectionRole::Chorus
        ]
    );
    let bars: Vec<u32> = form.sections().iter().map(|s| s.bars).collect();
    assert_eq!(bars, [8, 16, 8, 16]);
    assert!((form.start_beat() - 8.0).abs() < 1e-12);
    assert_eq!(form.total_bars(), 48);
}

#[test]
fn defaults_put_the_chorus_above_the_verse() {
    let form = plan(&FormConfig::new(FormTemplate::PopPreChorus), &four_four());
    let level = |role| {
        form.sections()
            .iter()
            .find(|s| s.role == role)
            .map(|s| (s.energy, s.density))
            .expect("role present")
    };
    assert!(level(SectionRole::Chorus).0 > level(SectionRole::PreChorus).0);
    assert!(level(SectionRole::PreChorus).0 > level(SectionRole::Verse).0);
    assert!(level(SectionRole::Verse).0 > level(SectionRole::Intro).0);
    let chorus = form.sections()[3].id.clone();
    assert_eq!(form.contrast_with_previous(&chorus), Some(30));
    assert_eq!(form.contrast_with_previous(&form.sections()[0].id), Some(0));
    assert_eq!(form.contrast_with_previous("sec_missing"), None);
}

#[test]
fn planning_is_deterministic() {
    let config = FormConfig::new(FormTemplate::Dance);
    assert_eq!(plan(&config, &four_four()), plan(&config, &four_four()));
}

#[test]
fn invalid_configurations_are_rejected_without_using_ids() {
    let signatures = four_four();
    let mut cases: Vec<(FormConfig, String)> = Vec::new();
    let mut negative = FormConfig::new(FormTemplate::Aaba);
    negative.start_bar = -1;
    cases.push((negative, "start bar must be >= 0".to_owned()));
    let mut no_intro = FormConfig::new(FormTemplate::Aaba);
    no_intro.omit_intro = true;
    cases.push((no_intro, "the aaba form has no intro".to_owned()));
    let mut no_outro = FormConfig::new(FormTemplate::Aaba);
    no_outro.omit_outro = true;
    cases.push((no_outro, "the aaba form has no outro".to_owned()));
    let mut absent = FormConfig::new(FormTemplate::Aaba);
    absent.bars.insert(SectionRole::Chorus, 8);
    cases.push((
        absent,
        "the form has no chorus section to resize".to_owned(),
    ));
    let mut omitted = FormConfig::new(FormTemplate::VerseChorus);
    omitted.omit_intro = true;
    omitted.bars.insert(SectionRole::Intro, 8);
    cases.push((
        omitted,
        "the form has no intro section to resize".to_owned(),
    ));
    for bars in [0, 65] {
        let mut config = FormConfig::new(FormTemplate::Aaba);
        config.bars.insert(SectionRole::Verse, bars);
        cases.push((
            config,
            format!("verse length must be between 1 and 64 bars, got {bars}"),
        ));
    }
    for (config, message) in cases {
        let mut ids = SequentialIds::new(0);
        let err = plan_form(&config, &signatures, &mut ids).expect_err(&message);
        assert_eq!(err.code, ErrorCode::InvalidArgument, "{message}");
        assert_eq!(err.message, message);
        let mut fresh = SequentialIds::new(0);
        assert_eq!(
            sonara_core::ids::IdGenerator::new_id(&mut ids, "sec"),
            sonara_core::ids::IdGenerator::new_id(&mut fresh, "sec"),
            "a rejected configuration consumes no IDs"
        );
    }
}

#[test]
fn template_names_round_trip_and_unknown_names_are_rejected() {
    for template in FormTemplate::ALL {
        assert_eq!(
            template.name().parse::<FormTemplate>().expect("known"),
            template
        );
    }
    let err = "rondo".parse::<FormTemplate>().expect_err("unknown");
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(
        err.message
            .starts_with("unknown song form \"rondo\"; expected one of verse_chorus")
    );
}

#[test]
fn levels_are_validated() {
    assert_eq!(Level::new(100).expect("max").percent(), 100);
    let err = Level::new(101).expect_err("too high");
    assert_eq!(err.code, ErrorCode::InvalidArgument);
}

#[test]
fn applying_creates_the_sections_in_one_undo_step() {
    let mut session = session();
    let form = plan(&FormConfig::new(FormTemplate::VerseChorus), &four_four());
    let result = apply_song_form(&mut session, &form, "test").expect("applied");
    assert!(result.committed);
    let sections = project_sections(&session);
    assert_eq!(sections, form.to_sections().expect("sections"));
    assert_eq!(sections.len(), 6);
    assert_eq!(session.history().labels().0, ["Generate Song Form"]);

    session.undo("test").expect("undo");
    assert_eq!(project_sections(&session).len(), 0);
}

#[test]
fn applying_to_a_project_with_sections_is_a_conflict_and_changes_nothing() {
    let mut session = session();
    let form = plan(&FormConfig::new(FormTemplate::Aaba), &four_four());
    apply_song_form(&mut session, &form, "test").expect("applied");
    let before = project_sections(&session);
    let version = session.version();
    let other = plan_form(
        &FormConfig::new(FormTemplate::Dance),
        &four_four(),
        &mut SequentialIds::new(100),
    )
    .expect("plan");
    let err = apply_song_form(&mut session, &other, "test").expect_err("conflict");
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(project_sections(&session), before);
    assert_eq!(session.version(), version);
}

#[test]
fn a_repeated_id_from_the_generator_is_rejected() {
    struct Constant;
    impl sonara_core::ids::IdGenerator for Constant {
        fn new_id(&mut self, _prefix: &str) -> String {
            "sec_000000000001".to_owned()
        }
    }
    let err = plan_form(
        &FormConfig::new(FormTemplate::Aaba),
        &four_four(),
        &mut Constant,
    )
    .expect_err("duplicate IDs");
    assert_eq!(err.code, ErrorCode::Internal);
}

#[test]
fn applying_without_a_project_fails() {
    let mut session = ProjectSession::new(Arc::new(builtin_registry()));
    let form = plan(&FormConfig::new(FormTemplate::Aaba), &four_four());
    let err = apply_song_form(&mut session, &form, "test").expect_err("no project");
    assert_eq!(err.code, ErrorCode::NoProject);
}

#[test]
fn the_form_follows_the_project_meter() {
    use sonara_core::commands::project::SetTimeSignature as Set;
    let mut session = session();
    session
        .execute(
            &Set {
                numerator: 3,
                denominator: 4,
                bar: 0,
            },
            false,
            "test",
        )
        .expect("meter");
    let signatures = session
        .require_project()
        .expect("project")
        .time_signatures
        .clone();
    let form = plan(&FormConfig::new(FormTemplate::Aaba), &signatures);
    apply_song_form(&mut session, &form, "test").expect("applied");
    let last = project_sections(&session).pop().expect("sections");
    assert!((last.end() - 96.0).abs() < 1e-12);
}

#[test]
fn regenerating_one_section_leaves_every_other_section_untouched() {
    let mut session = session();
    let form = plan(&FormConfig::new(FormTemplate::VerseChorus), &four_four());
    apply_song_form(&mut session, &form, "test").expect("applied");
    let before = project_sections(&session);
    let target = form.sections()[3].id.clone(); // Verse 2
    let change = SectionChange {
        role: Some(SectionRole::Bridge),
        ..SectionChange::default()
    };
    let (next, result) =
        regenerate_section(&mut session, &form, &target, &change, "test").expect("regenerated");
    assert!(result.committed);

    let after = project_sections(&session);
    assert_eq!(after.len(), before.len());
    for (old, new) in before.iter().zip(&after) {
        if old.id == target {
            assert_eq!(new.name, "Bridge");
            assert_eq!(
                (new.id.as_str(), new.start, new.length, &new.color),
                (old.id.as_str(), old.start, old.length, &old.color),
                "ID, start and length of the target are kept"
            );
        } else {
            assert_eq!(new, old, "section {} must be untouched", old.id);
        }
    }
    let section = next.section(&target).expect("target");
    assert_eq!(section.role, SectionRole::Bridge);
    assert_eq!(section.energy, SectionRole::Bridge.default_energy());
    for (old, new) in form.sections().iter().zip(next.sections()) {
        if old.id != target {
            assert_eq!(old, new);
        }
    }
    assert_eq!(session.history().labels().0.len(), 2);
}

#[test]
fn regenerating_with_levels_only_keeps_the_role_and_label() {
    let form = plan(&FormConfig::new(FormTemplate::VerseChorus), &four_four());
    let target = form.sections()[2].id.clone();
    let change = SectionChange {
        energy: Some(Level::new(100).expect("level")),
        density: Some(Level::new(55).expect("level")),
        ..SectionChange::default()
    };
    let next = regenerate_in_plan(&form, &target, &change).expect("regenerated");
    let section = next.section(&target).expect("target");
    assert_eq!(
        (section.role, section.label.as_str()),
        (SectionRole::Chorus, "Chorus 1")
    );
    assert_eq!(
        (section.energy.percent(), section.density.percent()),
        (100, 55)
    );
}

#[test]
fn a_regenerated_label_never_duplicates_another_section() {
    let form = plan(&FormConfig::new(FormTemplate::VerseChorus), &four_four());
    // Turn Chorus 1 into a verse: "Verse 1" and "Verse 2" exist, so it becomes "Verse 3".
    let target = form.sections()[2].id.clone();
    let change = SectionChange {
        role: Some(SectionRole::Verse),
        ..SectionChange::default()
    };
    let next = regenerate_in_plan(&form, &target, &change).expect("regenerated");
    assert_eq!(next.section(&target).expect("target").label, "Verse 3");
    // A role not present yet gets the bare name.
    let change = SectionChange {
        role: Some(SectionRole::Breakdown),
        ..SectionChange::default()
    };
    let next = regenerate_in_plan(&form, &target, &change).expect("regenerated");
    assert_eq!(next.section(&target).expect("target").label, "Breakdown");
}

#[test]
fn regenerating_an_unknown_section_fails_and_changes_nothing() {
    let mut session = session();
    let form = plan(&FormConfig::new(FormTemplate::Aaba), &four_four());
    apply_song_form(&mut session, &form, "test").expect("applied");
    let before = project_sections(&session);
    let version = session.version();
    let change = SectionChange {
        role: Some(SectionRole::Chorus),
        ..SectionChange::default()
    };
    let err =
        regenerate_section(&mut session, &form, "sec_nope", &change, "test").expect_err("unknown");
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(project_sections(&session), before);
    assert_eq!(session.version(), version);
}

#[test]
fn regenerating_a_section_edited_since_planning_is_a_conflict() {
    use sonara_core::commands::project::UpdateSection;
    let mut session = session();
    let form = plan(&FormConfig::new(FormTemplate::Aaba), &four_four());
    apply_song_form(&mut session, &form, "test").expect("applied");
    let target = form.sections()[1].id.clone();
    session
        .execute(
            &UpdateSection {
                section_id: target.clone(),
                name: None,
                start: None,
                length: Some(4.0),
                color: None,
            },
            false,
            "test",
        )
        .expect("edit");
    let before = project_sections(&session);
    let change = SectionChange {
        role: Some(SectionRole::Chorus),
        ..SectionChange::default()
    };
    let err =
        regenerate_section(&mut session, &form, &target, &change, "test").expect_err("conflict");
    assert_eq!(err.code, ErrorCode::Conflict);
    assert_eq!(project_sections(&session), before);
}

/// SON-P10: a song form is represented in the project model, so it survives saving and opening
/// the project, and regenerating one section changes only that section on disk too.
#[test]
fn a_form_and_a_regenerated_section_survive_save_and_open() {
    use sonara_project::store::ProjectStore;

    let mut session = session();
    let form = plan(&FormConfig::new(FormTemplate::VerseChorus), &four_four());
    apply_song_form(&mut session, &form, "test").expect("applied");
    let before = project_sections(&session);
    assert_eq!(before.len(), 6);

    let dir = std::env::temp_dir().join(format!("sonara-form-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temporary directory");
    let store = ProjectStore::new(dir.join("workspaces"));
    let target = dir.join("song.sonara");
    let project = session.require_project().expect("project").clone();
    store.save(&project, &dir, &target, None).expect("saved");
    let reopened = store.open(&target, false).expect("opened");
    assert_eq!(reopened.project.sections, before);

    // Regenerate the second section's levels: every other section is unchanged after a round trip.
    let target_id = before[1].id.clone();
    regenerate_section(
        &mut session,
        &form,
        &target_id,
        &SectionChange {
            role: None,
            energy: Some(Level::new(10).expect("a level")),
            density: None,
        },
        "test",
    )
    .expect("regenerated");
    let project = session.require_project().expect("project").clone();
    let target2 = dir.join("song2.sonara");
    store
        .save(&project, &dir, &target2, None)
        .expect("saved again");
    let after = store
        .open(&target2, false)
        .expect("opened again")
        .project
        .sections
        .clone();
    for (old, new) in before.iter().zip(&after) {
        if old.id != target_id {
            assert_eq!(old, new, "section {} changed", old.id);
        }
    }
    std::fs::remove_dir_all(&dir).expect("cleanup");
}
