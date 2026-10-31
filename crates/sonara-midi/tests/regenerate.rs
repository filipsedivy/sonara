//! Localized regeneration (SON-P11): region rules, candidate validation, preservation, undo and
//! staleness. The generators here are deterministic test doubles; the crate ships none.

use std::sync::Arc;

use sonara_core::commands::Command;
use sonara_core::commands::clip::CreateMidiClip;
use sonara_core::commands::project::AddSection;
use sonara_core::commands::track::CreateTrack;
use sonara_core::error::ErrorCode;
use sonara_core::ids::SequentialIds;
use sonara_core::midi::MidiNote;
use sonara_core::model::clip::Clip;
use sonara_core::model::project::Project;
use sonara_core::model::track::TrackKind;
use sonara_core::plugins::builtin_registry;
use sonara_core::session::{ProjectLocation, ProjectSession};
use sonara_midi::regenerate::{
    CandidateNote, Constraints, GenerationError, GenerationInput, RegenError, RegenerationRequest,
    RegionGenerator, RegionTarget, UNDO_LABEL, apply, propose, regenerate,
};

/// Returns the same notes whatever the input.
struct Fixed(Vec<CandidateNote>);

impl RegionGenerator for Fixed {
    fn generate(&self, _: &GenerationInput<'_>) -> Result<Vec<CandidateNote>, GenerationError> {
        Ok(self.0.clone())
    }
}

struct Failing;

impl RegionGenerator for Failing {
    fn generate(&self, _: &GenerationInput<'_>) -> Result<Vec<CandidateNote>, GenerationError> {
        Err(GenerationError("model unavailable".to_owned()))
    }
}

/// One note per region beat at `pitch`, so the output depends on the region and the seed.
struct Pulse {
    pitch: i64,
}

impl RegionGenerator for Pulse {
    fn generate(&self, input: &GenerationInput<'_>) -> Result<Vec<CandidateNote>, GenerationError> {
        let seed = i64::try_from(input.seed.unwrap_or(0)).expect("small seed");
        let mut out = Vec::new();
        let mut beat = input.region.start;
        while beat < input.region.end - 1e-9 {
            out.push(cand(self.pitch + seed, beat, 1.0));
            beat += 1.0;
        }
        Ok(out)
    }
}

fn cand(pitch: i64, start: f64, length: f64) -> CandidateNote {
    CandidateNote {
        pitch,
        start,
        length,
        velocity: 90,
        channel: 0,
    }
}

fn note(id: &str, pitch: i64, start: f64, length: f64) -> MidiNote {
    MidiNote::new(id, pitch, start, length, 100, 0).expect("valid note")
}

struct Fixture {
    session: ProjectSession,
    track: String,
    clip: String,
    other_clip: String,
}

/// Track `lead` holds a clip at beats 4..20 with a note every beat (`n0`…`n15`) plus a long
/// note `long` from beat 0 (content) to 6; track `bass` holds a clip over the same time.
fn fixture() -> Fixture {
    let mut ids = SequentialIds::new(5000);
    let project = Project::create(&mut ids, "Song", 48_000, 120.0).expect("project");
    let master = project.master.id.clone();
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
    let mut add_track = |name: &str, id: &str| {
        let command = CreateTrack {
            name: name.to_owned(),
            kind: TrackKind::Instrument,
            index: None,
            color: None,
            output_id: master.clone(),
            instrument_plugin: None,
            instrument_preset: None,
            parent_id: None,
            track_id: id.to_owned(),
            instrument_id: format!("{id}_inst"),
            part: None,
        };
        session.execute(&command, false, "test").expect("track");
    };
    add_track("lead", "trk_lead");
    add_track("bass", "trk_bass");
    let mut lead_notes: Vec<MidiNote> = (0..16)
        .map(|i| note(&format!("n{i}"), 60, f64::from(i), 1.0))
        .collect();
    lead_notes.push(note("long", 72, 0.0, 6.0));
    let mut add_clip = |track: &str, id: &str, notes: Vec<MidiNote>| {
        let command = CreateMidiClip {
            track_id: track.to_owned(),
            start: 4.0,
            length: 16.0,
            name: id.to_owned(),
            notes,
            loop_length: None,
            color: None,
            clip_id: id.to_owned(),
        };
        session.execute(&command, false, "test").expect("clip");
    };
    add_clip("trk_lead", "clp_lead", lead_notes);
    add_clip("trk_bass", "clp_bass", vec![note("b0", 36, 2.0, 4.0)]);
    Fixture {
        session,
        track: "trk_lead".to_owned(),
        clip: "clp_lead".to_owned(),
        other_clip: "clp_bass".to_owned(),
    }
}

impl Fixture {
    fn notes(&self, clip_id: &str) -> Vec<MidiNote> {
        let project = self.session.require_project().expect("project");
        match project.clip(clip_id).expect("clip").1 {
            Clip::Midi(clip) => clip.notes.as_ref().clone(),
            Clip::Audio(_) => panic!("audio clip"),
        }
    }

    fn request(&self, start: f64, end: f64) -> RegenerationRequest {
        RegenerationRequest {
            clip_id: self.clip.clone(),
            target: RegionTarget::Beats { start, end },
            constraints: Constraints::default(),
            seed: None,
        }
    }
}

fn ids() -> SequentialIds {
    SequentialIds::new(90_000)
}

fn ids_of(notes: &[MidiNote]) -> Vec<String> {
    let mut v: Vec<String> = notes.iter().map(|n| n.id.clone()).collect();
    v.sort();
    v
}

/// Region 8..12 timeline beats is content beats 4..8 of the lead clip (clip starts at 4).
#[test]
fn replaces_only_notes_that_start_in_the_half_open_region() {
    let mut f = fixture();
    let before = f.notes("clp_lead");
    let request = f.request(8.0, 12.0);
    let report = regenerate(
        &mut f.session,
        &request,
        &Pulse { pitch: 65 },
        &mut ids(),
        "test",
    )
    .expect("regenerated");
    // n4..n7 start in [4, 8); n8 starts exactly at the end and stays, n3 stays.
    assert_eq!(report.removed, ["n4", "n5", "n6", "n7"]);
    assert_eq!(report.added.len(), 4);
    assert_eq!(report.preserved, before.len() - 4);
    let after = f.notes("clp_lead");
    for kept in before.iter().filter(|n| !report.removed.contains(&n.id)) {
        assert!(after.contains(kept), "{} changed", kept.id);
    }
    let new: Vec<&MidiNote> = after
        .iter()
        .filter(|n| report.added.contains(&n.id))
        .collect();
    assert!(
        new.iter()
            .all(|n| n.pitch == 65 && (4.0..8.0).contains(&n.start))
    );
}

#[test]
fn a_note_crossing_the_start_is_kept_untruncated_and_a_note_crossing_the_end_is_replaced() {
    let mut f = fixture();
    // `long` starts at content 0 and sounds to 6; region content [4, 8) starts inside it.
    let plan = propose(
        &f.session,
        &f.request(8.0, 12.0),
        &Pulse { pitch: 65 },
        &mut ids(),
    )
    .expect("plan");
    assert_eq!(plan.crossing, ["long"]);
    assert!(plan.removed.iter().all(|n| n.id != "long"));
    // Region content [0, 5): `long` starts inside, ends at 6 > 5, and is replaced as a whole.
    let plan = propose(
        &f.session,
        &f.request(4.0, 9.0),
        &Pulse { pitch: 65 },
        &mut ids(),
    )
    .expect("plan");
    assert!(plan.removed.iter().any(|n| n.id == "long"));
    assert_eq!(plan.crossing, Vec::<String>::new());
    apply(&mut f.session, &plan, "test").expect("applied");
    assert!(f.notes("clp_lead").iter().all(|n| n.id != "long"));
}

#[test]
fn other_tracks_and_untouched_notes_keep_their_identity() {
    let mut f = fixture();
    let bass_before = f.notes(&f.other_clip);
    let project_before = f.session.require_project().expect("project").clone();
    let request = f.request(8.0, 12.0);
    regenerate(
        &mut f.session,
        &request,
        &Pulse { pitch: 65 },
        &mut ids(),
        "test",
    )
    .expect("regenerated");
    assert_eq!(f.notes(&f.other_clip), bass_before);
    let project = f.session.require_project().expect("project");
    assert_eq!(project.tracks.len(), project_before.tracks.len());
    assert_eq!(project.tracks[1], project_before.tracks[1]);
    assert_eq!(project.master, project_before.master);
    assert_eq!(project.sections, project_before.sections);
}

#[test]
fn one_undo_restores_the_clip_exactly() {
    let mut f = fixture();
    let before = f.notes("clp_lead");
    let version = f.session.version();
    let request = f.request(8.0, 12.0);
    let report = regenerate(
        &mut f.session,
        &request,
        &Pulse { pitch: 65 },
        &mut ids(),
        "test",
    )
    .expect("regenerated");
    assert_eq!(report.command.label, UNDO_LABEL);
    assert_eq!(f.session.version(), version + 1);
    f.session.undo("test").expect("undo");
    assert_eq!(f.notes("clp_lead"), before);
    f.session.redo("test").expect("redo");
    assert_eq!(f.notes("clp_lead").len(), before.len());
}

#[test]
fn a_section_target_uses_the_section_span() {
    let mut f = fixture();
    let section = AddSection {
        name: "Verse".to_owned(),
        start: 8.0,
        length: 4.0,
        color: None,
        section_id: "sec_v".to_owned(),
    };
    f.session.execute(&section, false, "test").expect("section");
    let mut request = f.request(0.0, 1.0);
    request.target = RegionTarget::Section("sec_v".to_owned());
    let plan = propose(&f.session, &request, &Pulse { pitch: 65 }, &mut ids()).expect("plan");
    assert_eq!((plan.region.start, plan.region.end), (4.0, 8.0));
    request.target = RegionTarget::Section("sec_missing".to_owned());
    let error = propose(&f.session, &request, &Pulse { pitch: 65 }, &mut ids()).expect_err("no");
    assert!(matches!(error, RegenError::NotFound(_)));
    assert_eq!(error.code(), ErrorCode::NotFound);
}

#[test]
fn bad_requests_are_rejected_before_the_generator_runs() {
    let f = fixture();
    let generator = Failing;
    let invalid = |request: &RegenerationRequest| {
        let error = propose(&f.session, request, &generator, &mut ids()).expect_err("rejected");
        assert!(matches!(error, RegenError::InvalidRequest(_)), "{error}");
    };
    invalid(&f.request(8.0, 8.0));
    invalid(&f.request(12.0, 8.0));
    invalid(&f.request(f64::NAN, 8.0));
    invalid(&f.request(2.0, 8.0)); // starts before the clip
    invalid(&f.request(8.0, 21.0)); // ends after the clip
    let mut request = f.request(8.0, 12.0);
    request.constraints.pitch_range = Some((80, 60));
    invalid(&request);
    request.constraints = Constraints {
        max_notes: Some(0),
        ..Constraints::default()
    };
    invalid(&request);

    let mut missing = f.request(8.0, 12.0);
    missing.clip_id = "clp_missing".to_owned();
    let error = propose(&f.session, &missing, &generator, &mut ids()).expect_err("missing");
    assert!(matches!(error, RegenError::NotFound(_)));
}

#[test]
fn generator_failure_leaves_the_project_untouched() {
    let mut f = fixture();
    let version = f.session.version();
    let before = f.notes("clp_lead");
    let request = f.request(8.0, 12.0);
    let error =
        regenerate(&mut f.session, &request, &Failing, &mut ids(), "test").expect_err("fails");
    assert!(matches!(error, RegenError::Generation(_)));
    assert_eq!(f.session.version(), version);
    assert_eq!(f.notes("clp_lead"), before);
}

#[test]
fn invalid_candidates_are_rejected_with_every_issue_and_no_mutation() {
    let mut f = fixture();
    let before = f.notes("clp_lead");
    let version = f.session.version();
    let mut request = f.request(8.0, 12.0); // content [4, 8)
    request.constraints.pitch_range = Some((50, 70));
    let cases: Vec<(Vec<CandidateNote>, &str)> = vec![
        (vec![], "no notes"),
        (vec![cand(128, 5.0, 1.0)], "pitch"),
        (vec![cand(60, 5.0, 0.0)], "length"),
        (vec![cand(60, 3.0, 1.0)], "outside the region"), // before the region
        (vec![cand(60, 8.0, 1.0)], "outside the region"), // at the end boundary
        (vec![cand(60, 7.5, 1.0)], "outside the region"), // sounds past the end
        (vec![cand(75, 5.0, 1.0)], "outside 50..=70"),
        (
            vec![CandidateNote {
                velocity: 0,
                ..cand(60, 5.0, 1.0)
            }],
            "velocity",
        ),
    ];
    for (candidate, expected) in cases {
        let error = regenerate(
            &mut f.session,
            &request,
            &Fixed(candidate),
            &mut ids(),
            "test",
        )
        .expect_err("rejected");
        let RegenError::Rejected(issues) = &error else {
            panic!("expected Rejected, got {error}");
        };
        assert!(
            issues.iter().any(|i| i.contains(expected)),
            "{issues:?} lacks {expected}"
        );
    }
    assert_eq!(f.session.version(), version);
    assert_eq!(f.notes("clp_lead"), before);

    request.constraints = Constraints {
        max_notes: Some(1),
        ..Constraints::default()
    };
    let two = Fixed(vec![cand(60, 4.0, 1.0), cand(62, 5.0, 1.0)]);
    let error = propose(&f.session, &request, &two, &mut ids()).expect_err("too many");
    assert!(matches!(error, RegenError::Rejected(_)));
}

#[test]
fn new_ids_are_unique_and_differ_from_existing_notes() {
    let mut f = fixture();
    let mut clashing = SequentialIds::new(0);
    let mut seen = std::collections::HashSet::new();
    let plan = propose(
        &f.session,
        &f.request(8.0, 12.0),
        &Pulse { pitch: 65 },
        &mut clashing,
    )
    .expect("plan");
    let existing = f.notes("clp_lead");
    for added in &plan.added {
        assert!(seen.insert(added.id.clone()), "duplicate id {}", added.id);
        assert!(existing.iter().all(|n| n.id != added.id));
    }
    apply(&mut f.session, &plan, "test").expect("applied");
    assert_eq!(
        ids_of(&f.notes("clp_lead")).len(),
        existing.len() - plan.removed.len() + plan.added.len()
    );
}

#[test]
fn a_stale_plan_is_rejected_and_cannot_be_applied_twice() {
    let mut f = fixture();
    let plan = propose(
        &f.session,
        &f.request(8.0, 12.0),
        &Pulse { pitch: 65 },
        &mut ids(),
    )
    .expect("plan");
    // An unrelated edit while the generator was running.
    let extra = CreateMidiClip {
        track_id: f.track.clone(),
        start: 30.0,
        length: 4.0,
        name: "later".to_owned(),
        notes: vec![],
        loop_length: None,
        color: None,
        clip_id: "clp_later".to_owned(),
    };
    f.session.execute(&extra, false, "test").expect("edit");
    let before = f.notes("clp_lead");
    let error = apply(&mut f.session, &plan, "test").expect_err("stale");
    assert!(matches!(error, RegenError::Stale { .. }));
    assert_eq!(error.code(), ErrorCode::Conflict);
    assert_eq!(f.notes("clp_lead"), before);

    let fresh = propose(
        &f.session,
        &f.request(8.0, 12.0),
        &Pulse { pitch: 65 },
        &mut ids(),
    )
    .expect("plan");
    apply(&mut f.session, &fresh, "test").expect("first");
    let after_first = f.notes("clp_lead");
    let error = apply(&mut f.session, &fresh, "test").expect_err("repeat");
    assert!(matches!(error, RegenError::Stale { .. }));
    assert_eq!(f.notes("clp_lead"), after_first);
}

#[test]
fn a_deleted_clip_makes_the_plan_fail_without_change() {
    let mut f = fixture();
    let plan = propose(
        &f.session,
        &f.request(8.0, 12.0),
        &Pulse { pitch: 65 },
        &mut ids(),
    )
    .expect("plan");
    let delete = sonara_core::commands::clip::DeleteClips {
        clip_ids: vec![f.clip.clone()],
    };
    f.session.execute(&delete, false, "test").expect("delete");
    assert!(matches!(
        apply(&mut f.session, &plan, "test"),
        Err(RegenError::Stale { .. })
    ));
    let error = propose(
        &f.session,
        &f.request(8.0, 12.0),
        &Pulse { pitch: 65 },
        &mut ids(),
    )
    .expect_err("gone");
    assert!(matches!(error, RegenError::NotFound(_)));
}

#[test]
fn the_same_seed_gives_the_same_notes_and_a_different_seed_differs() {
    let f = fixture();
    let make = |seed| {
        let mut request = f.request(8.0, 12.0);
        request.seed = Some(seed);
        let plan = propose(&f.session, &request, &Pulse { pitch: 60 }, &mut ids()).expect("plan");
        plan.added
            .iter()
            .map(|n| (n.pitch, n.start.to_bits()))
            .collect::<Vec<_>>()
    };
    assert_eq!(make(1), make(1));
    assert_ne!(make(1), make(2));
}

#[test]
fn a_new_note_over_a_preserved_note_of_the_same_pitch_only_warns() {
    let mut f = fixture();
    // Region content [6, 8); preserved `n5` (pitch 60, 5..6) does not overlap, but `long`
    // (pitch 72, 0..6) ends at 6, so a pitch 72 note at 6 does not either. Start earlier.
    let request = f.request(10.0, 12.0);
    let overlapping = Fixed(vec![cand(60, 6.0, 1.0), cand(72, 6.0, 1.0)]);
    let plan = propose(&f.session, &request, &overlapping, &mut ids()).expect("plan");
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);

    // Region content [5, 8) keeps `long`'s crossing tail; a pitch 72 note at 5 overlaps it.
    let request = f.request(9.0, 12.0);
    let plan = propose(
        &f.session,
        &request,
        &Fixed(vec![cand(72, 5.0, 1.0)]),
        &mut ids(),
    )
    .expect("plan");
    assert_eq!(plan.warnings.len(), 1);
    let report = apply(&mut f.session, &plan, "test").expect("warnings do not block");
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn audio_and_looping_clips_are_not_regenerated() {
    let mut f = fixture();
    let looped = CreateMidiClip {
        track_id: f.track.clone(),
        start: 40.0,
        length: 8.0,
        name: "loop".to_owned(),
        notes: vec![note("l0", 60, 0.0, 1.0)],
        loop_length: Some(2.0),
        color: None,
        clip_id: "clp_loop".to_owned(),
    };
    f.session.execute(&looped, false, "test").expect("clip");
    let mut request = f.request(40.0, 44.0);
    request.clip_id = "clp_loop".to_owned();
    let error = propose(&f.session, &request, &Pulse { pitch: 60 }, &mut ids()).expect_err("loop");
    assert!(matches!(error, RegenError::InvalidRequest(_)));
}

#[test]
fn context_includes_preserved_notes_and_other_tracks_but_not_the_region() {
    struct Probe;
    impl RegionGenerator for Probe {
        fn generate(
            &self,
            input: &GenerationInput<'_>,
        ) -> Result<Vec<CandidateNote>, GenerationError> {
            assert!(
                input
                    .preserved
                    .iter()
                    .all(|n| !(4.0..8.0).contains(&n.start))
            );
            assert!(input.preserved.iter().any(|n| n.id == "long"));
            // The bass note b0 sounds at content 2..6, overlapping the region start at 4.
            assert_eq!(input.other_tracks.len(), 1);
            assert_eq!(input.other_tracks[0].track_id, "trk_bass");
            assert_eq!(input.track_id, "trk_lead");
            Ok(vec![cand(60, 4.0, 1.0)])
        }
    }
    let f = fixture();
    propose(&f.session, &f.request(8.0, 12.0), &Probe, &mut ids()).expect("plan");
}

#[test]
fn the_plan_command_is_a_valid_project_command() {
    let f = fixture();
    let plan = propose(
        &f.session,
        &f.request(8.0, 12.0),
        &Pulse { pitch: 65 },
        &mut ids(),
    )
    .expect("plan");
    let command = plan.command();
    assert_eq!(command.name(), "Transaction");
    let simulated = f.session.simulate(&command).expect("simulate");
    assert_eq!(simulated.tracks.len(), 2);
    // Simulating does not commit.
    assert_eq!(f.notes("clp_lead").len(), 17);
}

/// SON-P11: a proposal that is dropped changes nothing (the user rejects it by not applying it),
/// and an applied one survives save and open with every note outside the region unchanged.
#[test]
fn a_rejected_proposal_changes_nothing_and_an_applied_one_survives_save_and_open() {
    use sonara_project::store::ProjectStore;

    let mut f = fixture();
    let before = f.notes("clp_lead");
    let version = f.session.version();
    let request = f.request(8.0, 12.0);
    let plan = propose(&f.session, &request, &Pulse { pitch: 65 }, &mut ids()).expect("plan");
    drop(plan);
    assert_eq!(f.session.version(), version);
    assert_eq!(f.notes("clp_lead"), before);

    regenerate(
        &mut f.session,
        &request,
        &Pulse { pitch: 65 },
        &mut ids(),
        "test",
    )
    .expect("applied");
    let after = f.notes("clp_lead");
    assert_ne!(after, before);

    let dir = std::env::temp_dir().join(format!("sonara-regen-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temporary directory");
    let store = ProjectStore::new(dir.join("workspaces"));
    let target = dir.join("song.sonara");
    let project = f.session.require_project().expect("project").clone();
    store.save(&project, &dir, &target, None).expect("saved");
    let reopened = store.open(&target, false).expect("opened");
    let on_disk = match reopened.project.clip("clp_lead").expect("clip").1 {
        Clip::Midi(clip) => clip.notes.as_ref().clone(),
        Clip::Audio(_) => panic!("audio clip"),
    };
    assert_eq!(on_disk, after);
    // The region is beats 8 to 12 on the timeline; the clip starts at beat 4, so content beats 4 to 8.
    let untouched = |notes: &[MidiNote]| -> Vec<MidiNote> {
        notes
            .iter()
            .filter(|n| n.start < 4.0 || n.start >= 8.0)
            .cloned()
            .collect()
    };
    assert_eq!(untouched(&on_disk), untouched(&before));
    match reopened.project.clip("clp_bass").expect("other clip").1 {
        Clip::Midi(clip) => assert_eq!(clip.notes.as_ref().clone(), f.notes("clp_bass")),
        Clip::Audio(_) => panic!("audio clip"),
    }
    std::fs::remove_dir_all(&dir).expect("cleanup");
}
