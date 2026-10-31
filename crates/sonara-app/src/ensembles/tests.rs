//! Ensemble plans over the shipped templates and hand-built catalog snapshots (TST-08): no library
//! on disk, no project. The layouts are pinned against rosters written out from the templates'
//! scores (TST-10), the sounds against the degradation chain itself, and every refusal by its
//! error (TST-03).

use std::path::PathBuf;

use sonara_midi::orchestration::{
    Articulation, EnsembleId, Family, InstrumentType, Mute, PitchRange, Request, Seating, Vibrato,
};

use super::{
    EnsemblePlan, EnsemblePlanError, EnsembleRequest, PartChoice, PartPlan, PartSelection,
    Placement, PlannedKind, bus_name, folder_name, plan,
};
use crate::instruments::{
    Availability, Builtin, ChainPreferences, Claim, ClaimSource, EnsembleClaim, EnsembleKind,
    FamilyClaim, InstrumentClaim, InstrumentId, InstrumentPreset, LibraryId, PresetKind,
    SoundRequest, SoundSource, Step, Verdict, VerdictState, choose_sound,
};

// Fixtures.

fn library() -> LibraryId {
    LibraryId::parse("lib").expect("a valid library id")
}

fn profile<T>(value: T) -> Claim<T> {
    Claim {
        value,
        source: ClaimSource::Profile,
    }
}

/// A playable patch preset `lib/<path without its extension>` that the profile says plays
/// `instrument` in the size `kind`, as is, with nothing else claimed and no facts read.
fn preset(path: &str, instrument: InstrumentType, kind: EnsembleKind) -> InstrumentPreset {
    let library = library();
    let stem = path.rsplit_once('.').map_or(path, |(stem, _)| stem);
    InstrumentPreset {
        id: InstrumentId::new(&library, path),
        display_name: stem.to_owned(),
        family: FamilyClaim::Profile(instrument.family()),
        library,
        patch_path: PathBuf::from("/libraries/lib").join(path),
        kind: PresetKind::Patch,
        articulations: Vec::new(),
        license: None,
        availability: Availability::Available,
        instrument: Some(InstrumentClaim {
            instrument,
            source: ClaimSource::Profile,
        }),
        ensemble: Some(profile(EnsembleClaim {
            kind,
            players: None,
        })),
        articulation_map: None,
        keymap: None,
        key_offset: None,
        dynamics: None,
        legato: None,
        in_situ: None,
        license_claim: None,
        verdict: Some(profile(Verdict {
            state: VerdictState::AsIs,
            lost: Vec::new(),
        })),
        facts: None,
    }
}

/// `preset` with the library's word on whether it was recorded with the players in their places.
fn recorded_in_situ(mut preset: InstrumentPreset, in_situ: bool) -> InstrumentPreset {
    preset.in_situ = Some(Claim {
        value: in_situ,
        source: ClaimSource::Declared,
    });
    preset
}

fn sampler(path: &str) -> SoundSource {
    SoundSource::Sampler {
        preset: InstrumentId::new(&library(), path),
    }
}

fn request(ensemble: EnsembleId) -> EnsembleRequest {
    EnsembleRequest::new(ensemble)
}

fn only(ensemble: EnsembleId, slugs: &[&str]) -> EnsembleRequest {
    EnsembleRequest {
        parts: PartSelection::Only(slugs.iter().map(|slug| PartChoice::new(*slug)).collect()),
        ..request(ensemble)
    }
}

/// Every part of `ensemble`, in the order its template lists them.
fn every_part(ensemble: EnsembleId) -> EnsembleRequest {
    let slugs: Vec<&str> = ensemble
        .get()
        .parts()
        .iter()
        .map(|part| part.slug)
        .collect();
    only(ensemble, &slugs)
}

fn planned(presets: &[InstrumentPreset], request: &EnsembleRequest) -> EnsemblePlan {
    plan(presets, request).expect("a valid request")
}

fn part<'a>(plan: &'a EnsemblePlan, slug: &str) -> &'a PartPlan {
    plan.parts()
        .find(|part| part.part.slug == slug)
        .unwrap_or_else(|| panic!("{slug} is planned"))
}

/// The plan's tracks in the notation of the rosters below: `bus <name>`, `folder <name>` and the
/// part slugs, each part prefixed by its folder's name when it has one.
fn layout(plan: &EnsemblePlan) -> Vec<String> {
    plan.tracks
        .iter()
        .map(|track| match &track.kind {
            PlannedKind::Bus(_) => format!("bus {}", track.name),
            PlannedKind::Folder(_) => format!("folder {}", track.name),
            PlannedKind::Part(part) => match track.parent {
                Some(folder) => format!("{}/{}", plan.tracks[folder].name, part.part.slug),
                None => part.part.slug.to_owned(),
            },
        })
        .collect()
}

const STRINGS: [&str; 5] = ["violins_1", "violins_2", "violas", "cellos", "basses"];

/// The required parts of every template, grouped by family folder in the order a default plan
/// builds them, written out from the rosters of the templates' scores (the parts each marks as
/// needed by every work of its kind), independently of the plan code.
#[expect(
    clippy::type_complexity,
    reason = "a literal table of templates, folders and parts reads best as nested slices"
)]
const REQUIRED: [(EnsembleId, &[(&str, &[&str])]); 13] = [
    (
        EnsembleId::ClassicalOrchestra,
        &[
            (
                "Woodwinds",
                &["flute_1", "oboe_1", "oboe_2", "bassoon_1", "bassoon_2"],
            ),
            ("Brass", &["horn_1", "horn_2"]),
            ("Strings", &STRINGS),
        ],
    ),
    (
        EnsembleId::BeethovenOrchestra,
        &[
            (
                "Woodwinds",
                &[
                    "flute_1",
                    "flute_2",
                    "oboe_1",
                    "oboe_2",
                    "clarinet_1",
                    "clarinet_2",
                    "bassoon_1",
                    "bassoon_2",
                ],
            ),
            ("Brass", &["horn_1", "horn_2", "trumpet_1", "trumpet_2"]),
            ("Percussion", &["timpani"]),
            ("Strings", &STRINGS),
        ],
    ),
    (
        EnsembleId::RomanticOrchestra,
        &[
            (
                "Woodwinds",
                &[
                    "flute_1",
                    "flute_2",
                    "oboe_1",
                    "oboe_2",
                    "clarinet_1",
                    "clarinet_2",
                    "bassoon_1",
                    "bassoon_2",
                ],
            ),
            (
                "Brass",
                &[
                    "horn_1",
                    "horn_2",
                    "horn_3",
                    "horn_4",
                    "trumpet_1",
                    "trumpet_2",
                    "trombone_1",
                    "trombone_2",
                    "trombone_3",
                ],
            ),
            ("Percussion", &["timpani"]),
            ("Strings", &STRINGS),
        ],
    ),
    (
        EnsembleId::LateRomanticOrchestra,
        &[
            (
                "Woodwinds",
                &[
                    "piccolo",
                    "flute_1",
                    "flute_2",
                    "flute_3",
                    "oboe_1",
                    "oboe_2",
                    "oboe_3",
                    "english_horn",
                    "clarinet_eb",
                    "clarinet_1",
                    "clarinet_2",
                    "bass_clarinet",
                    "bassoon_1",
                    "bassoon_2",
                    "bassoon_3",
                    "contrabassoon",
                ],
            ),
            (
                "Brass",
                &[
                    "horn_1",
                    "horn_2",
                    "horn_3",
                    "horn_4",
                    "horn_5",
                    "horn_6",
                    "horn_7",
                    "horn_8",
                    "trumpet_1",
                    "trumpet_2",
                    "trumpet_3",
                    "trumpet_4",
                    "trombone_1",
                    "trombone_2",
                    "trombone_3",
                    "tuba",
                ],
            ),
            (
                "Percussion",
                &[
                    "timpani_1",
                    "bass_drum",
                    "snare_drum",
                    "cymbals",
                    "suspended_cymbal",
                    "tam_tam",
                    "triangle",
                    "glockenspiel",
                    "xylophone",
                    "tubular_bells",
                ],
            ),
            ("Plucked", &["harp_1", "harp_2"]),
            ("Strings", &STRINGS),
        ],
    ),
    (
        EnsembleId::FilmOrchestra,
        &[
            (
                "Woodwinds",
                &[
                    "flute_1",
                    "flute_2",
                    "oboe_1",
                    "oboe_2",
                    "clarinet_1",
                    "clarinet_2",
                    "bassoon_1",
                    "bassoon_2",
                ],
            ),
            (
                "Brass",
                &[
                    "horn_1",
                    "horn_2",
                    "horn_3",
                    "horn_4",
                    "horn_5",
                    "horn_6",
                    "trumpet_1",
                    "trumpet_2",
                    "trumpet_3",
                    "trombone_1",
                    "trombone_2",
                    "bass_trombone_1",
                    "tuba",
                ],
            ),
            (
                "Percussion",
                &[
                    "timpani",
                    "bass_drum",
                    "snare_drum",
                    "cymbals",
                    "suspended_cymbal",
                    "tam_tam",
                ],
            ),
            ("Plucked", &["harp"]),
            ("Strings", &STRINGS),
        ],
    ),
    (
        EnsembleId::ChamberOrchestra,
        &[
            (
                "Woodwinds",
                &[
                    "flute_1",
                    "flute_2",
                    "oboe_1",
                    "oboe_2",
                    "bassoon_1",
                    "bassoon_2",
                ],
            ),
            ("Brass", &["horn_1", "horn_2"]),
            ("Strings", &STRINGS),
        ],
    ),
    (
        EnsembleId::StringOrchestra,
        &[("Strings", &["violins_1", "violins_2", "violas", "cellos"])],
    ),
    (
        EnsembleId::BigBand,
        &[
            (
                "Woodwinds",
                &[
                    "alto_sax_1",
                    "alto_sax_2",
                    "tenor_sax_1",
                    "tenor_sax_2",
                    "baritone_sax",
                ],
            ),
            (
                "Brass",
                &[
                    "trumpet_1",
                    "trumpet_2",
                    "trumpet_3",
                    "trumpet_4",
                    "trombone_1",
                    "trombone_2",
                    "trombone_3",
                    "bass_trombone",
                ],
            ),
            ("Keyboards", &["piano"]),
            ("Strings", &["bass"]),
        ],
    ),
    (
        EnsembleId::ChoirAndOrchestra,
        &[
            (
                "Woodwinds",
                &[
                    "piccolo",
                    "flute_1",
                    "flute_2",
                    "oboe_1",
                    "oboe_2",
                    "clarinet_1",
                    "clarinet_2",
                    "bassoon_1",
                    "bassoon_2",
                ],
            ),
            (
                "Brass",
                &[
                    "horn_1",
                    "horn_2",
                    "horn_3",
                    "horn_4",
                    "trumpet_1",
                    "trumpet_2",
                    "trombone_1",
                    "trombone_2",
                    "trombone_3",
                    "tuba",
                ],
            ),
            ("Percussion", &["timpani"]),
            ("Plucked", &["harp_1"]),
            (
                "Voices",
                &[
                    "soprano_solo",
                    "baritone_solo",
                    "choir_sopranos",
                    "choir_altos",
                    "choir_tenors",
                    "choir_basses",
                ],
            ),
            ("Strings", &STRINGS),
        ],
    ),
    (
        EnsembleId::StringQuartet,
        &[("Strings", &["violin_1", "violin_2", "viola", "cello"])],
    ),
    (
        EnsembleId::WindQuintet,
        // The quintet's own scores print the horn between the clarinet and the bassoon; the
        // Woodwinds folder holds all four woodwinds, so the horn's folder follows it.
        &[
            ("Woodwinds", &["flute", "oboe", "clarinet", "bassoon"]),
            ("Brass", &["horn"]),
        ],
    ),
    (
        EnsembleId::BrassQuintet,
        &[(
            "Brass",
            &["trumpet_1", "trumpet_2", "horn", "trombone", "tuba"],
        )],
    ),
    (
        EnsembleId::PianoTrio,
        // The piano is printed below the strings, so its folder comes second.
        &[("Strings", &["violin", "cello"]), ("Keyboards", &["piano"])],
    ),
];

// The parts and their order.

#[test]
fn every_template_has_its_required_parts_in_family_folders_with_buses_first() {
    assert_eq!(
        REQUIRED.map(|(id, _)| id),
        EnsembleId::ALL,
        "one row per template"
    );
    for (id, folders) in REQUIRED {
        let plan = planned(&[], &request(id));
        let mut expected: Vec<String> = folders
            .iter()
            .map(|(folder, _)| format!("bus {folder} Bus"))
            .collect();
        for (folder, slugs) in folders {
            expected.push(format!("folder {folder}"));
            expected.extend(slugs.iter().map(|slug| format!("{folder}/{slug}")));
        }
        assert_eq!(layout(&plan), expected, "{}", id.id());
        // Each part feeds the bus of its own family.
        for track in &plan.tracks {
            if let PlannedKind::Part(part) = &track.kind {
                let bus = track.output.map(|bus| &plan.tracks[bus].kind);
                assert_eq!(
                    bus,
                    Some(&PlannedKind::Bus(part.part.instrument.family())),
                    "{}: {}",
                    id.id(),
                    part.part.slug
                );
            }
        }
    }
}

#[test]
fn without_folders_the_parts_keep_the_template_order() {
    for id in EnsembleId::ALL {
        let template: Vec<&str> = id.get().parts().iter().map(|part| part.slug).collect();
        let plan = planned(
            &[],
            &EnsembleRequest {
                folders: false,
                buses: false,
                ..every_part(id)
            },
        );
        assert_eq!(layout(&plan), template, "{}", id.id());
        assert!(
            plan.tracks
                .iter()
                .all(|track| track.parent.is_none() && track.output.is_none()),
            "{}: nothing to belong to or feed but the master",
            id.id()
        );
    }

    // Buses without folders still come first, and the parts keep the edition's order.
    let quintet = planned(
        &[],
        &EnsembleRequest {
            folders: false,
            ..request(EnsembleId::WindQuintet)
        },
    );
    assert_eq!(
        layout(&quintet),
        [
            "bus Woodwinds Bus",
            "bus Brass Bus",
            "flute",
            "oboe",
            "clarinet",
            "horn",
            "bassoon"
        ]
    );
    let outputs: Vec<Option<usize>> = quintet.tracks.iter().map(|track| track.output).collect();
    assert_eq!(
        outputs,
        [None, None, Some(0), Some(0), Some(0), Some(1), Some(0)]
    );
}

#[test]
fn a_selection_keeps_the_template_order_and_folders_without_buses_feed_the_master() {
    // Asked for in reverse, planned in score order; the doubling sits below its player.
    let plan = planned(
        &[],
        &EnsembleRequest {
            buses: false,
            ..only(
                EnsembleId::RomanticOrchestra,
                &["cellos", "harp", "piccolo", "flute_3", "horn_1"],
            )
        },
    );
    assert_eq!(
        layout(&plan),
        [
            "folder Woodwinds",
            "Woodwinds/flute_3",
            "Woodwinds/piccolo",
            "folder Brass",
            "Brass/horn_1",
            "folder Plucked",
            "Plucked/harp",
            "folder Strings",
            "Strings/cellos",
        ]
    );
    assert!(plan.tracks.iter().all(|track| track.output.is_none()));
}

#[test]
fn every_track_refers_only_to_earlier_tracks_of_its_family() {
    for id in EnsembleId::ALL {
        for (folders, buses) in [(true, true), (true, false), (false, true), (false, false)] {
            let plan = planned(
                &[],
                &EnsembleRequest {
                    folders,
                    buses,
                    ..every_part(id)
                },
            );
            assert_eq!(plan.parts().count(), id.get().parts().len());
            for (index, track) in plan.tracks.iter().enumerate() {
                let family = match &track.kind {
                    PlannedKind::Part(part) => part.part.instrument.family(),
                    PlannedKind::Folder(family) | PlannedKind::Bus(family) => *family,
                };
                if let Some(parent) = track.parent {
                    assert!(parent < index);
                    assert_eq!(plan.tracks[parent].kind, PlannedKind::Folder(family));
                    assert_eq!(plan.tracks[parent].name, folder_name(family));
                }
                if let Some(output) = track.output {
                    assert!(output < index);
                    assert_eq!(plan.tracks[output].kind, PlannedKind::Bus(family));
                    assert_eq!(plan.tracks[output].name, bus_name(family));
                }
                let is_part = matches!(track.kind, PlannedKind::Part(_));
                assert_eq!(track.parent.is_some(), is_part && folders, "{}", id.id());
                assert_eq!(track.output.is_some(), is_part && buses, "{}", id.id());
            }
        }
    }
}

#[test]
fn families_have_one_folder_and_one_bus_name_each() {
    let names: Vec<(&str, &str)> = Family::ALL
        .iter()
        .map(|&family| (folder_name(family), bus_name(family)))
        .collect();
    assert_eq!(
        names,
        [
            ("Strings", "Strings Bus"),
            ("Woodwinds", "Woodwinds Bus"),
            ("Brass", "Brass Bus"),
            ("Percussion", "Percussion Bus"),
            ("Keyboards", "Keyboards Bus"),
            ("Plucked", "Plucked Bus"),
            ("Voices", "Voices Bus"),
            ("Other", "Other Bus"),
        ]
    );
}

// The parts as project data.

#[test]
fn track_parts_state_the_players_and_the_doubler() {
    let romantic = |size: Option<&str>| {
        planned(
            &[],
            &EnsembleRequest {
                string_size: size.map(str::to_owned),
                ..every_part(EnsembleId::RomanticOrchestra)
            },
        )
    };
    let full = romantic(None);
    assert_eq!(full.string_size.map(|size| size.slug), Some("full"));
    let violins = part(&full, "violins_1");
    assert_eq!(violins.size, EnsembleKind::Section);
    assert_eq!(violins.track_part.instrument, "violin");
    assert_eq!(violins.track_part.name.as_deref(), Some("Violins I"));
    assert_eq!(violins.track_part.players, Some(16));
    assert_eq!(violins.track_part.solo, Some(false));
    assert_eq!(violins.track_part.player, None);
    assert_eq!(part(&full, "basses").track_part.players, Some(8));

    // Rimsky-Korsakov's medium orchestra: 12, 10, 8, 6 and 4.
    let medium = romantic(Some("medium"));
    let players: Vec<Option<u8>> = STRINGS
        .iter()
        .map(|slug| part(&medium, slug).track_part.players)
        .collect();
    assert_eq!(players, [Some(12), Some(10), Some(8), Some(6), Some(4)]);

    // The third flute's player plays the piccolo; the part names the player's part.
    let piccolo = part(&full, "piccolo");
    assert_eq!(piccolo.size, EnsembleKind::Solo);
    assert_eq!(piccolo.track_part.instrument, "piccolo");
    assert_eq!(piccolo.track_part.name.as_deref(), Some("Piccolo (Fl. 3)"));
    assert_eq!(piccolo.track_part.players, Some(1));
    assert_eq!(piccolo.track_part.solo, Some(true));
    assert_eq!(piccolo.track_part.player.as_deref(), Some("Flute 3"));
    let horn = part(&full, "horn_3");
    assert_eq!(
        (horn.size, horn.track_part.players, horn.track_part.solo),
        (EnsembleKind::Solo, Some(1), Some(true))
    );

    // A choir section of 16 to 20 singers is a section; the range states no one number.
    let choir = planned(&[], &request(EnsembleId::ChoirAndOrchestra));
    let sopranos = part(&choir, "choir_sopranos");
    assert_eq!(sopranos.size, EnsembleKind::Section);
    assert_eq!(sopranos.track_part.players, None);
    assert_eq!(sopranos.track_part.solo, Some(false));
    // A chamber group's players are soloists.
    let quartet = planned(&[], &request(EnsembleId::StringQuartet));
    assert!(quartet.parts().all(|part| part.size == EnsembleKind::Solo
        && part.track_part.players == Some(1)
        && part.track_part.solo == Some(true)));
}

#[test]
fn every_track_part_is_valid_project_data() {
    // TST-07: what a track stores passes the project schema's part rules.
    for id in EnsembleId::ALL {
        for size in id.get().string_sizes() {
            let plan = planned(
                &[],
                &EnsembleRequest {
                    string_size: Some(size.slug.to_owned()),
                    ..every_part(id)
                },
            );
            for part in plan.parts() {
                assert_eq!(part.track_part.validate(), Ok(()), "{}", part.part.slug);
                assert_eq!(part.track_part.instrument, part.part.instrument.id());
            }
        }
    }
}

// The sounds.

#[test]
fn each_part_takes_the_chains_sound_and_its_messages() {
    let violins = preset(
        "ViolinEns.sfz",
        InstrumentType::Violin,
        EnsembleKind::Section,
    );
    let cello = preset("SoloCello.sfz", InstrumentType::Cello, EnsembleKind::Solo);
    let presets = [violins, cello];
    let pizzicato =
        Request::new(Some(Articulation::Pizzicato), Mute::Open, Vibrato::Ordinary).expect("valid");
    let plan = planned(
        &presets,
        &EnsembleRequest {
            parts: PartSelection::Only(vec![
                PartChoice {
                    articulations: vec![pizzicato],
                    ..PartChoice::new("violins_1")
                },
                PartChoice::new("cellos"),
                PartChoice::new("horn_1"),
            ]),
            ..request(EnsembleId::RomanticOrchestra)
        },
    );

    // The oracle is the chain itself, asked for each part in its size with what it plays.
    let asked = |instrument, kind, articulations: Vec<Request>| {
        choose_sound(
            &presets,
            &SoundRequest {
                instrument,
                ensemble: Some(kind),
                articulations,
                span: None,
            },
            ChainPreferences::default(),
        )
    };
    let violins = part(&plan, "violins_1");
    assert_eq!(
        violins.sound,
        asked(
            InstrumentType::Violin,
            EnsembleKind::Section,
            vec![pizzicato]
        )
    );
    assert_eq!(violins.sound.source, sampler("ViolinEns"));
    assert_eq!(violins.sound.step, Step::Exact);
    assert_eq!(violins.sound.articulations[0].requested, pizzicato);
    let cellos = part(&plan, "cellos");
    assert_eq!(
        cellos.sound,
        asked(InstrumentType::Cello, EnsembleKind::Section, Vec::new())
    );
    assert_eq!(cellos.sound.source, sampler("SoloCello"));
    let horn = part(&plan, "horn_1");
    assert_eq!(
        horn.sound,
        asked(InstrumentType::HornF, EnsembleKind::Solo, Vec::new())
    );
    assert_eq!(
        horn.sound.source,
        SoundSource::Builtin(Builtin::Synth { preset: "Brass" })
    );

    // The messages, part by part in the plan's order.
    let notices: Vec<(&str, Step, &str)> = plan
        .notices()
        .map(|(name, notice)| (name, notice.step, notice.message.as_str()))
        .collect();
    let mut expected: Vec<(&str, Step, &str)> = Vec::new();
    for (name, sound) in [
        ("Horn 1", &horn.sound),
        ("Violins I", &violins.sound),
        ("Cellos", &cellos.sound),
    ] {
        expected.extend(
            sound
                .notices
                .iter()
                .map(|notice| (name, notice.step, notice.message.as_str())),
        );
    }
    assert_eq!(notices, expected);
    assert!(notices.contains(&(
        "Cellos",
        Step::EnsembleSwap,
        "No Cello section is installed; a solo Cello plays the part."
    )));
    assert!(notices.contains(&(
        "Horn 1",
        Step::Builtin,
        "No sampled sound for Horn in F is installed; the built-in Poly Synth preset 'Brass' \
         plays the part, a synthetic approximation."
    )));
}

#[test]
fn a_question_is_carried_and_declining_it_takes_the_other_sound() {
    // A part whose range is given reaches past the neighbour's: the chain asks first.
    let tuba = preset("Tuba.sfz", InstrumentType::Tuba, EnsembleKind::Solo);
    let span = PitchRange::try_new(34, 70).expect("a valid range");
    let film = planned(
        std::slice::from_ref(&tuba),
        &EnsembleRequest {
            parts: PartSelection::Only(vec![PartChoice {
                span: Some(span),
                ..PartChoice::new("cimbasso")
            }]),
            ..request(EnsembleId::FilmOrchestra)
        },
    );
    let cimbasso = part(&film, "cimbasso");
    assert_eq!(cimbasso.sound.step, Step::FamilyNeighbour);
    assert_eq!(
        cimbasso
            .sound
            .confirm
            .as_ref()
            .map(|confirm| confirm.question.as_str()),
        Some(
            "The part reaches A#1 to A#4, beyond Tuba's playable range of A0 to G4. Play it on \
             Tuba with the notes outside that range moved by an octave, or leave Tuba out?"
        )
    );

    // A solo voice that only a choir section plays asks first; the choir was recorded in its
    // place, so the track stays centred until the answer "no" takes the built-in choir pad.
    let choir = recorded_in_situ(
        preset(
            "Choir.sfz",
            InstrumentType::ChoirSoprano,
            EnsembleKind::Section,
        ),
        true,
    );
    let presets = [choir];
    let mut plan = planned(
        &presets,
        &only(EnsembleId::ChoirAndOrchestra, &["soprano_solo"]),
    );
    let PlannedKind::Part(soprano) = &mut plan.tracks[2].kind else {
        panic!("the soprano follows the Voices bus and folder");
    };
    assert_eq!(soprano.part.slug, "soprano_solo");
    assert_eq!(soprano.sound.source, sampler("Choir"));
    assert_eq!(soprano.placement, Placement::InSitu);
    let otherwise = soprano
        .sound
        .confirm
        .as_ref()
        .map(|confirm| (*confirm.otherwise).clone())
        .expect("a question");
    assert!(soprano.decline(&presets));
    assert_eq!(soprano.sound, otherwise);
    assert_eq!(
        soprano.sound.source,
        SoundSource::Builtin(Builtin::Synth {
            preset: "Choir Pad"
        })
    );
    assert_eq!(soprano.placement, Placement::Pan(0.0));
    // A sound that asks nothing has nothing to decline.
    let before = soprano.clone();
    assert!(!soprano.decline(&presets));
    assert_eq!(**soprano, *before);
}

// The pans.

#[test]
fn pans_follow_the_seating() {
    let pans = |plan: &EnsemblePlan, slugs: &[&str]| -> Vec<Placement> {
        slugs
            .iter()
            .map(|slug| part(plan, slug).placement)
            .collect()
    };
    let parts = [
        "violins_1",
        "violins_2",
        "violas",
        "cellos",
        "basses",
        "horn_1",
        "flute_1",
        "oboe_1",
        "trombone_3",
        "timpani",
        "harp",
        "cymbals",
    ];
    // The Romantic orchestra sits in the American seating unless asked otherwise.
    let american = planned(&[], &every_part(EnsembleId::RomanticOrchestra));
    assert_eq!(american.seating, Some(Seating::American));
    assert_eq!(
        pans(&american, &parts),
        [
            -0.55, -0.30, 0.10, 0.35, 0.45, -0.35, -0.15, 0.10, 0.35, -0.10, -0.60, 0.20
        ]
        .map(Placement::Pan)
    );
    let antiphonal = planned(
        &[],
        &EnsembleRequest {
            seating: Some(Seating::Antiphonal),
            ..every_part(EnsembleId::RomanticOrchestra)
        },
    );
    assert_eq!(antiphonal.seating, Some(Seating::Antiphonal));
    assert_eq!(
        pans(&antiphonal, &parts),
        [
            -0.55, 0.55, 0.20, -0.20, -0.35, 0.35, -0.15, 0.10, 0.35, -0.10, -0.60, 0.20
        ]
        .map(Placement::Pan)
    );
    // The Classical orchestra defaults to the antiphonal seating its composers wrote for.
    let classical = planned(&[], &request(EnsembleId::ClassicalOrchestra));
    assert_eq!(classical.seating, Some(Seating::Antiphonal));
    assert_eq!(
        part(&classical, "violins_2").placement,
        Placement::Pan(0.55)
    );
    // The big band sits where its sections put the lead and the solo chairs.
    let band = planned(&[], &request(EnsembleId::BigBand));
    assert_eq!(band.seating, None);
    assert_eq!(
        pans(
            &band,
            &["alto_sax_1", "tenor_sax_1", "trumpet_1", "trumpet_2"]
        ),
        [0.0, -0.50, -0.10, -0.30].map(Placement::Pan)
    );
    // A chamber group stays centred.
    for id in [
        EnsembleId::StringQuartet,
        EnsembleId::WindQuintet,
        EnsembleId::BrassQuintet,
        EnsembleId::PianoTrio,
    ] {
        let plan = planned(&[], &request(id));
        assert_eq!(plan.seating, None);
        assert!(
            plan.parts()
                .all(|part| part.placement == Placement::Pan(0.0) && part.stage_pan == 0.0),
            "{}",
            id.id()
        );
    }
}

#[test]
fn a_source_recorded_in_place_keeps_the_track_centred() {
    let in_place = recorded_in_situ(
        preset(
            "ViolinEns.sfz",
            InstrumentType::Violin,
            EnsembleKind::Section,
        ),
        true,
    );
    let dry = recorded_in_situ(
        preset("CelloEns.sfz", InstrumentType::Cello, EnsembleKind::Section),
        false,
    );
    let unknown = preset("ViolaEns.sfz", InstrumentType::Viola, EnsembleKind::Section);
    let plan = planned(
        &[in_place, dry, unknown],
        &request(EnsembleId::StringOrchestra),
    );
    let placed: Vec<(&str, Placement, f64)> = plan
        .parts()
        .map(|part| (part.part.slug, part.placement, part.stage_pan))
        .collect();
    assert_eq!(
        placed,
        [
            ("violins_1", Placement::InSitu, -0.55),
            ("violins_2", Placement::InSitu, -0.30),
            ("violas", Placement::Pan(0.10), 0.10),
            ("cellos", Placement::Pan(0.35), 0.35),
        ]
    );
}

// Refusals.

#[test]
fn invalid_requests_are_refused_with_their_reason() {
    let cases: [(EnsembleRequest, EnsemblePlanError, &str); 7] = [
        (
            EnsembleRequest {
                seating: Some(Seating::American),
                ..request(EnsembleId::StringQuartet)
            },
            EnsemblePlanError::UnsupportedSeating {
                ensemble: EnsembleId::StringQuartet,
                seating: Seating::American,
            },
            "the ensemble 'string_quartet' does not offer the American seating",
        ),
        (
            EnsembleRequest {
                string_size: Some("huge".to_owned()),
                ..request(EnsembleId::RomanticOrchestra)
            },
            EnsemblePlanError::UnknownStringSize {
                ensemble: EnsembleId::RomanticOrchestra,
                slug: "huge".to_owned(),
            },
            "the ensemble 'romantic_orchestra' has no string size 'huge'",
        ),
        (
            // A template without string sections has no size to choose; the match is exact.
            EnsembleRequest {
                string_size: Some("full".to_owned()),
                ..request(EnsembleId::BigBand)
            },
            EnsemblePlanError::UnknownStringSize {
                ensemble: EnsembleId::BigBand,
                slug: "full".to_owned(),
            },
            "the ensemble 'big_band' has no string size 'full'",
        ),
        (
            only(EnsembleId::RomanticOrchestra, &["horn_1", "horn_9"]),
            EnsemblePlanError::UnknownPart {
                ensemble: EnsembleId::RomanticOrchestra,
                slug: "horn_9".to_owned(),
            },
            "the ensemble 'romantic_orchestra' has no part 'horn_9'",
        ),
        (
            // A display name is not a slug.
            only(EnsembleId::RomanticOrchestra, &["Horn 1"]),
            EnsemblePlanError::UnknownPart {
                ensemble: EnsembleId::RomanticOrchestra,
                slug: "Horn 1".to_owned(),
            },
            "the ensemble 'romantic_orchestra' has no part 'Horn 1'",
        ),
        (
            only(
                EnsembleId::RomanticOrchestra,
                &["horn_1", "flute_1", "horn_1"],
            ),
            EnsemblePlanError::DuplicatePart("horn_1".to_owned()),
            "the part 'horn_1' is selected twice",
        ),
        (
            only(EnsembleId::RomanticOrchestra, &[]),
            EnsemblePlanError::NoParts,
            "no part is selected",
        ),
    ];
    for (request, error, message) in cases {
        assert_eq!(plan(&[], &request), Err(error.clone()), "{message}");
        assert_eq!(error.to_string(), message);
    }

    // The seating is checked before the string size, and both before the parts.
    let everything_wrong = EnsembleRequest {
        seating: Some(Seating::Antiphonal),
        string_size: Some("full".to_owned()),
        ..only(EnsembleId::PianoTrio, &["harp"])
    };
    assert!(matches!(
        plan(&[], &everything_wrong),
        Err(EnsemblePlanError::UnsupportedSeating { .. })
    ));
    let wrong_size_and_part = EnsembleRequest {
        string_size: Some("full".to_owned()),
        ..only(EnsembleId::PianoTrio, &["harp"])
    };
    assert!(matches!(
        plan(&[], &wrong_size_and_part),
        Err(EnsemblePlanError::UnknownStringSize { .. })
    ));
}
