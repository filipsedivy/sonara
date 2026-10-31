//! The degradation chain over hand-built catalog snapshots (TST-08): no library on disk, no
//! loader. Each step is pinned with the sound plan it makes and the words the user is told
//! (TST-03).

use std::collections::HashSet;
use std::path::PathBuf;

use sonara_core::plugins::builtin_registry;
use sonara_midi::gm::GM_MAP;
use sonara_midi::orchestration::fallback::{Emulation, Manner, Scope};
use sonara_midi::orchestration::{
    Articulation, GmVoice, InstrumentType, Mute, PitchRange, Request, Vibrato,
};

use super::tables::{BUILTINS, Builtin, DRUM_KIT, POLY_SYNTH, PREFERRED};
use super::{
    ArticulationChoice, ChainPreferences, Notice, Reach, SoundPlan, SoundRequest, SoundSource,
    Step, Substitute, choose_sound, nearest_articulation, playable, words,
};
use crate::instruments::manifest::{
    ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, EnsembleClaim,
    EnsembleKind, EntryTarget, KeymapEntry, Verdict, VerdictState,
};
use crate::instruments::model::{
    Availability, Claim, ClaimSource, InstrumentClaim, InstrumentId, InstrumentPreset, LibraryId,
    PresetKind, SetMember,
};
use crate::instruments::profiles;
use crate::instruments::{FamilyClaim, KeyZone, PatchFacts, VelocityRange};

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

fn verdict(state: VerdictState, lost: &[&str]) -> Verdict {
    Verdict {
        state,
        lost: lost.iter().map(|line| (*line).to_owned()).collect(),
    }
}

/// A playable patch preset `lib/<path without its extension>` that the profile says plays
/// `instrument`, as is, with nothing else claimed and no facts read.
fn preset(path: &str, instrument: InstrumentType) -> InstrumentPreset {
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
        ensemble: None,
        articulation_map: None,
        keymap: None,
        key_offset: None,
        dynamics: None,
        legato: None,
        in_situ: None,
        license_claim: None,
        verdict: Some(profile(verdict(VerdictState::AsIs, &[]))),
        facts: None,
    }
}

fn sized(mut preset: InstrumentPreset, kind: EnsembleKind) -> InstrumentPreset {
    preset.ensemble = Some(profile(EnsembleClaim {
        kind,
        players: None,
    }));
    preset
}

fn solo(preset: InstrumentPreset) -> InstrumentPreset {
    sized(preset, EnsembleKind::Solo)
}

fn section(preset: InstrumentPreset) -> InstrumentPreset {
    sized(preset, EnsembleKind::Section)
}

fn mapped(mut preset: InstrumentPreset, entries: Vec<ArticulationEntry>) -> InstrumentPreset {
    preset.articulation_map = Some(profile(ArticulationList::Mapped(entries)));
    preset
}

/// An entry of `name` reached by `via`, open, with the ordinary vibrato.
fn entry(name: ArticulationName, via: ArticulationVia) -> ArticulationEntry {
    ArticulationEntry {
        name,
        via,
        member: None,
        label: None,
        is_default: false,
        mute: Mute::Open,
        vibrato: Vibrato::Ordinary,
        attack_ms: None,
    }
}

fn switch(articulation: Articulation, key: u8) -> ArticulationEntry {
    entry(
        ArticulationName::Vocabulary(articulation),
        ArticulationVia::Keyswitch { key },
    )
}

fn default(entry: ArticulationEntry) -> ArticulationEntry {
    ArticulationEntry {
        is_default: true,
        ..entry
    }
}

/// The whole patch plays `articulation`, its ordinary sound.
fn whole(articulation: Articulation) -> ArticulationEntry {
    default(entry(
        ArticulationName::Vocabulary(articulation),
        ArticulationVia::Patch,
    ))
}

fn muted(entry: ArticulationEntry) -> ArticulationEntry {
    ArticulationEntry {
        mute: Mute::Generic,
        ..entry
    }
}

fn non_vibrato(entry: ArticulationEntry) -> ArticulationEntry {
    ArticulationEntry {
        vibrato: Vibrato::Off,
        ..entry
    }
}

/// The four articulations of VSCO 2 CE's string key-switch patches.
fn string_switches() -> Vec<ArticulationEntry> {
    vec![
        default(switch(Articulation::Sustain, 36)),
        switch(Articulation::Tremolo, 37),
        switch(Articulation::Spiccato, 38),
        switch(Articulation::Pizzicato, 39),
    ]
}

fn violin_section_ks() -> InstrumentPreset {
    section(mapped(
        preset("ViolinEns-KS.sfz", InstrumentType::Violin),
        string_switches(),
    ))
}

fn violin_solo_ks() -> InstrumentPreset {
    solo(mapped(
        preset("SViolin-KS.sfz", InstrumentType::Violin),
        string_switches(),
    ))
}

/// A horn that plays sustain, staccato and a muted sustain.
fn horn() -> InstrumentPreset {
    mapped(
        preset("Horn.sfz", InstrumentType::HornF),
        vec![
            default(switch(Articulation::Sustain, 36)),
            switch(Articulation::Staccato, 37),
            muted(switch(Articulation::Sustain, 38)),
        ],
    )
}

fn tuba() -> InstrumentPreset {
    solo(mapped(
        preset("Tuba-KS.sfz", InstrumentType::Tuba),
        vec![
            default(switch(Articulation::Sustain, 36)),
            switch(Articulation::Staccato, 37),
        ],
    ))
}

fn request(instrument: InstrumentType, articulations: &[Request]) -> SoundRequest {
    SoundRequest {
        articulations: articulations.to_vec(),
        ..SoundRequest::new(instrument)
    }
}

fn wish(request: SoundRequest, kind: EnsembleKind) -> SoundRequest {
    SoundRequest {
        ensemble: Some(kind),
        ..request
    }
}

fn spanning(request: SoundRequest, low: i64, high: i64) -> SoundRequest {
    SoundRequest {
        span: Some(PitchRange::try_new(low, high).expect("a valid range")),
        ..request
    }
}

fn primary(articulation: Articulation) -> Request {
    Request::new(Some(articulation), Mute::Open, Vibrato::Ordinary).expect("a primary id")
}

fn passage(articulation: Option<Articulation>, mute: Mute, vibrato: Vibrato) -> Request {
    Request::new(articulation, mute, vibrato).expect("a primary id")
}

fn plan(presets: &[InstrumentPreset], request: &SoundRequest) -> SoundPlan {
    choose_sound(presets, request, ChainPreferences::default())
}

fn messages(plan: &SoundPlan) -> Vec<(Step, &str)> {
    plan.notices
        .iter()
        .map(|notice| (notice.step, notice.message.as_str()))
        .collect()
}

fn sampler(path: &str) -> SoundSource {
    SoundSource::Sampler {
        preset: InstrumentId::new(&library(), path),
    }
}

/// How `preset` plays `request` for `instrument`, and what the user is told about it.
fn nearest(
    preset: &InstrumentPreset,
    instrument: InstrumentType,
    request: Request,
) -> (ArticulationChoice, Option<String>) {
    let choice = nearest_articulation(preset, instrument, request);
    let message = choice.message(instrument);
    (choice, message)
}

/// The facts of a patch that loads and sounds on keys `lo..=hi`.
fn facts(lo: u8, hi: u8) -> PatchFacts {
    PatchFacts {
        regions: 4,
        playable_regions: 4,
        sampled: vec![KeyZone {
            lo,
            hi,
            layers: vec![VelocityRange { lo: 1, hi: 127 }],
        }],
        switch_keys: Vec::new(),
        default_switch: None,
        ignored_opcodes: Vec::new(),
        missing_samples: Vec::new(),
        unreadable_samples: Vec::new(),
        notes: Vec::new(),
        decoded_bytes: 1024,
        loadable: true,
        sound: None,
    }
}

fn member(path: &str, facts: Option<PatchFacts>) -> SetMember {
    SetMember {
        path: path.to_owned(),
        patch_path: PathBuf::from("/libraries/lib").join(path),
        availability: Availability::Available,
        facts,
    }
}

/// The articulation `articulation` played by the set member `path`.
fn played_by(articulation: Articulation, path: &str) -> ArticulationEntry {
    ArticulationEntry {
        member: Some(path.to_owned()),
        ..whole(articulation)
    }
}

/// The cello set `lib/@cello` without a verdict of its own, of the members its `entries` name.
fn cello_set(entries: Vec<ArticulationEntry>) -> InstrumentPreset {
    let members = entries
        .iter()
        .filter_map(|entry| entry.member.as_deref())
        .map(|path| member(path, None))
        .collect();
    InstrumentPreset {
        id: InstrumentId::of_set(&library(), "cello"),
        verdict: None,
        kind: PresetKind::Set { members },
        ..section(mapped(preset("cello", InstrumentType::Cello), entries))
    }
}

// What counts as playable.

/// A preset is excluded only by what is known about it: the catalog's availability, facts that
/// show no playable region or a missing sample, or a verdict of `not`. Facts not read yet do not
/// count against it (TST-03).
#[test]
fn a_preset_is_playable_unless_something_known_rules_it_out() {
    let base = || preset("Violin.sfz", InstrumentType::Violin);
    assert!(base().is_playable(), "no facts read: playable");
    let read = InstrumentPreset {
        facts: Some(facts(55, 100)),
        ..base()
    };
    assert!(read.is_playable(), "facts that find nothing wrong");

    let excluded = [
        (
            InstrumentPreset {
                availability: Availability::Missing,
                ..base()
            },
            "a missing file",
        ),
        (
            InstrumentPreset {
                availability: Availability::Invalid {
                    reason: "it is not a file".to_owned(),
                },
                ..base()
            },
            "an invalid file",
        ),
        (
            InstrumentPreset {
                facts: Some(PatchFacts {
                    missing_samples: vec![PathBuf::from("/lib/a.wav")],
                    ..facts(55, 100)
                }),
                ..base()
            },
            "a missing sample",
        ),
        (
            InstrumentPreset {
                facts: Some(PatchFacts {
                    playable_regions: 0,
                    ..facts(55, 100)
                }),
                ..base()
            },
            "no region that can play",
        ),
        (
            InstrumentPreset {
                verdict: Some(profile(verdict(VerdictState::Not, &["silent"]))),
                ..base()
            },
            "a verdict of not",
        ),
    ];
    for (preset, why) in excluded {
        assert!(!preset.is_playable(), "{why}");
    }

    // A set is playable when every member is, as far as their facts go.
    let set = |second| InstrumentPreset {
        kind: PresetKind::Set {
            members: vec![member("a.sfz", None), member("b.sfz", second)],
        },
        ..base()
    };
    assert!(set(Some(facts(55, 100))).is_playable());
    let gap = PatchFacts {
        missing_samples: vec![PathBuf::from("/lib/b.wav")],
        ..facts(55, 100)
    };
    assert!(!set(Some(gap)).is_playable());
}

/// The chain never chooses a preset that cannot play: a missing violin section leaves the part
/// to the solo violin.
#[test]
fn a_preset_that_cannot_play_is_never_chosen() {
    let missing = InstrumentPreset {
        availability: Availability::Missing,
        ..violin_section_ks()
    };
    let plan = plan(
        &[missing, violin_solo_ks()],
        &wish(request(InstrumentType::Violin, &[]), EnsembleKind::Section),
    );
    assert_eq!(plan.source, sampler("SViolin-KS"));
    assert_eq!(plan.step, Step::EnsembleSwap);
}

// Step 1: exact.

/// A preset of the size asked for that plays every requested articulation plays the part, with
/// nothing to tell.
#[test]
fn step_1_an_exact_preset_plays_without_a_notice() {
    let sustain_only = section(mapped(
        preset("ViolinEnsSusVib.sfz", InstrumentType::Violin),
        vec![whole(Articulation::Sustain)],
    ));
    let plan = plan(
        &[sustain_only, violin_section_ks(), violin_solo_ks()],
        &wish(
            request(
                InstrumentType::Violin,
                &[
                    primary(Articulation::Sustain),
                    primary(Articulation::Pizzicato),
                ],
            ),
            EnsembleKind::Section,
        ),
    );
    assert_eq!(plan.step, Step::Exact);
    assert_eq!(plan.source, sampler("ViolinEns-KS"));
    assert_eq!(plan.neighbour, None);
    assert_eq!(plan.confirm, None);
    assert!(plan.notices.is_empty(), "{:?}", plan.notices);
    assert_eq!(
        plan.articulations,
        [
            ArticulationChoice {
                requested: primary(Articulation::Sustain),
                reach: Reach::Mapped(default(switch(Articulation::Sustain, 36))),
                substitute: None,
            },
            ArticulationChoice {
                requested: primary(Articulation::Pizzicato),
                reach: Reach::Mapped(switch(Articulation::Pizzicato, 39)),
                substitute: None,
            },
        ]
    );
}

/// A preset that plays partly is told with its losses.
#[test]
fn step_1_a_preset_that_plays_partly_names_its_losses() {
    let cellos = InstrumentPreset {
        display_name: "Cello Section, key switches".to_owned(),
        verdict: Some(profile(verdict(
            VerdictState::Partly,
            &["ampeg_dynamic", "keys 84 to 87 switch articulations"],
        ))),
        ..section(mapped(
            preset("CelloEns-KS.sfz", InstrumentType::Cello),
            string_switches(),
        ))
    };
    let plan = plan(&[cellos], &request(InstrumentType::Cello, &[]));
    assert_eq!(plan.step, Step::Exact);
    assert_eq!(
        messages(&plan),
        [(
            Step::Exact,
            "Cello Section, key switches (lib/CelloEns-KS) plays partly: ampeg_dynamic; keys 84 \
             to 87 switch articulations."
        )]
    );
}

/// Percussion with one sound per key plays an instrument on the keys its key map gives it, even
/// when the patch plays several instruments and names none.
#[test]
fn step_1_a_key_map_plays_an_instrument_on_its_keys() {
    let row = |keys: &[u8], instrument, articulation| KeymapEntry {
        keys: keys.to_vec(),
        instrument,
        articulation,
        name: None,
    };
    let kit = InstrumentPreset {
        instrument: None,
        keymap: Some(profile(vec![
            row(&[36], InstrumentType::BassDrum, None),
            row(&[38], InstrumentType::SnareDrum, None),
            row(
                &[39],
                InstrumentType::SnareDrum,
                Some(ArticulationName::Vocabulary(Articulation::Roll)),
            ),
        ])),
        ..preset("GM-StylePerc.sfz", InstrumentType::SnareDrum)
    };
    let plan = plan(
        &[kit],
        &request(
            InstrumentType::SnareDrum,
            &[Request::default(), primary(Articulation::Roll)],
        ),
    );
    assert_eq!(plan.step, Step::Exact);
    assert_eq!(plan.source, sampler("GM-StylePerc"));
    assert!(plan.notices.is_empty(), "{:?}", plan.notices);
    let reaches: Vec<&Reach> = plan.articulations.iter().map(|c| &c.reach).collect();
    assert_eq!(reaches, [&Reach::Keys(vec![38]), &Reach::Keys(vec![39])]);
}

// Step 2: nearest articulation.

/// A missing articulation is played by its first stand-in that the preset plays, and the chain
/// tells it once per substituted articulation.
#[test]
fn step_2_a_stand_in_plays_a_missing_articulation() {
    let violins = mapped(
        preset("Violin.sfz", InstrumentType::Violin),
        vec![
            default(switch(Articulation::Sustain, 36)),
            switch(Articulation::Staccato, 37),
        ],
    );
    let (choice, message) = nearest(
        &violins,
        InstrumentType::Violin,
        primary(Articulation::Spiccato),
    );
    assert_eq!(
        message.as_deref(),
        Some("Spiccato is not available for Violin; playing staccato.")
    );
    assert_eq!(
        choice.reach,
        Reach::Mapped(switch(Articulation::Staccato, 37))
    );
    assert_eq!(
        choice.substitute,
        Some(Substitute {
            plays: primary(Articulation::Staccato),
            rewrites: Vec::new(),
            only: None,
        })
    );

    let plan = plan(
        &[violins],
        &request(
            InstrumentType::Violin,
            &[
                primary(Articulation::Sustain),
                primary(Articulation::Spiccato),
            ],
        ),
    );
    assert_eq!(plan.step, Step::Exact);
    assert_eq!(
        messages(&plan),
        [(
            Step::NearestArticulation,
            "Spiccato is not available for Violin; playing staccato."
        )]
    );
}

/// A stand-in may come with a rewrite of the notes, and may suit one kind of passage only; both
/// are told.
#[test]
fn step_2_a_stand_in_brings_its_rewrite_and_its_scope() {
    let oboe = mapped(
        preset("Oboe.sfz", InstrumentType::Oboe),
        vec![whole(Articulation::Sustain)],
    );
    let (staccato, message) = nearest(&oboe, InstrumentType::Oboe, primary(Articulation::Staccato));
    assert_eq!(
        message.as_deref(),
        Some(
            "Staccato is not available for Oboe; playing sustain, each note shortened to a \
             quarter of its length."
        )
    );
    assert_eq!(
        staccato.substitute.map(|s| s.rewrites),
        Some(vec![Emulation::Shorten { divisor: 4 }])
    );
    let (legato, message) = nearest(&oboe, InstrumentType::Oboe, primary(Articulation::Legato));
    assert_eq!(
        message.as_deref(),
        Some(
            "Legato is not available for Oboe; playing sustain, each note overlapping the next \
             (for single lines only)."
        )
    );
    assert_eq!(
        legato.substitute.and_then(|s| s.only),
        Some(Scope::SingleLines)
    );
}

/// With no stand-in, the ordinary sound plays with the notes rewritten to imitate the
/// articulation; with no rewrite either, the ordinary sound plays as it is.
#[test]
fn step_2_without_a_stand_in_the_notes_imitate_or_the_ordinary_sound_plays() {
    let violins = mapped(
        preset("Violin.sfz", InstrumentType::Violin),
        vec![default(switch(Articulation::Sustain, 36))],
    );
    let (trill, message) = nearest(
        &violins,
        InstrumentType::Violin,
        primary(Articulation::Trill),
    );
    assert_eq!(
        message.as_deref(),
        Some(
            "Trill is not available for Violin; playing sustain, as a trill written out at 12 \
             to 16 notes per second."
        )
    );
    assert_eq!(
        trill.reach,
        Reach::Mapped(default(switch(Articulation::Sustain, 36)))
    );

    let timpani = mapped(
        preset("Timpani.sfz", InstrumentType::Timpani),
        vec![default(entry(
            ArticulationName::Ordinario,
            ArticulationVia::Patch,
        ))],
    );
    let (sustain, message) = nearest(
        &timpani,
        InstrumentType::Timpani,
        primary(Articulation::Sustain),
    );
    assert_eq!(
        message.as_deref(),
        Some("Sustain is not available for Timpani; playing its ordinary sound.")
    );
    assert_eq!(
        sustain.substitute,
        Some(Substitute {
            plays: Request::default(),
            rewrites: Vec::new(),
            only: None,
        })
    );
}

/// An articulation that refines another falls back to the one it refines.
#[test]
fn step_2_a_refinement_falls_back_to_what_it_refines() {
    let flute = mapped(
        preset("Flute.sfz", InstrumentType::Flute),
        vec![
            default(switch(Articulation::Sustain, 36)),
            switch(Articulation::Trill, 37),
        ],
    );
    let (_, message) = nearest(
        &flute,
        InstrumentType::Flute,
        primary(Articulation::TrillHalf),
    );
    assert_eq!(
        message.as_deref(),
        Some("Trill half is not available for Flute; playing trill.")
    );
}

/// A mute: a muted entry plays a muted passage as asked; a more general mute stands in for a
/// particular one; with no mute at all the passage plays open, a dynamic step softer.
#[test]
fn step_2_a_missing_mute_falls_back_to_a_general_one_then_to_playing_softer() {
    let horn = horn();
    let con_sordino = |articulation| passage(articulation, Mute::Generic, Vibrato::Ordinary);
    let (exact, message) = nearest(
        &horn,
        InstrumentType::HornF,
        con_sordino(Some(Articulation::Sustain)),
    );
    assert_eq!(message, None);
    assert_eq!(
        exact.reach,
        Reach::Mapped(muted(switch(Articulation::Sustain, 38)))
    );
    // The ordinary sound of a muted passage is the muted entry.
    let (ordinary, message) = nearest(&horn, InstrumentType::HornF, con_sordino(None));
    assert_eq!((ordinary.reach, message), (exact.reach, None));

    let cup = passage(None, Mute::Cup, Vibrato::Ordinary);
    let (_, message) = nearest(&horn, InstrumentType::HornF, cup);
    assert_eq!(
        message.as_deref(),
        Some("Mute cup is not available for Horn in F; playing sustain con sordino.")
    );

    let open_horn = mapped(
        preset("Horn.sfz", InstrumentType::HornF),
        vec![default(switch(Articulation::Sustain, 36))],
    );
    let (softer, message) = nearest(&open_horn, InstrumentType::HornF, con_sordino(None));
    assert_eq!(
        message.as_deref(),
        Some("Con sordino is not available for Horn in F; playing sustain, a dynamic step softer.")
    );
    assert_eq!(
        softer.substitute.map(|s| s.rewrites),
        Some(vec![Emulation::Soften])
    );
}

/// The note's own articulation is kept before the passage's mute: a muted staccato on a horn
/// without one plays an open staccato, softer, not the muted sustain.
#[test]
fn step_2_keeps_the_articulation_before_the_mute() {
    let muted_staccato = passage(
        Some(Articulation::Staccato),
        Mute::Generic,
        Vibrato::Ordinary,
    );
    let (choice, message) = nearest(&horn(), InstrumentType::HornF, muted_staccato);
    assert_eq!(
        choice.reach,
        Reach::Mapped(switch(Articulation::Staccato, 37))
    );
    assert_eq!(
        message.as_deref(),
        Some(
            "Staccato con sordino is not available for Horn in F; playing staccato, a dynamic \
             step softer."
        )
    );
}

/// A vibrato the preset does not record gives way to its ordinary vibrato; an ordinary request
/// is served by any vibrato, the ordinary one first.
#[test]
fn step_2_a_missing_vibrato_gives_way_to_the_ordinary_one() {
    let flute = mapped(
        preset("Flute-KS.sfz", InstrumentType::Flute),
        vec![
            default(non_vibrato(switch(Articulation::Sustain, 36))),
            switch(Articulation::Sustain, 37),
        ],
    );
    let (sustain, message) = nearest(
        &flute,
        InstrumentType::Flute,
        primary(Articulation::Sustain),
    );
    assert_eq!(message, None);
    assert_eq!(
        sustain.reach,
        Reach::Mapped(switch(Articulation::Sustain, 37))
    );
    let (ordinary, message) = nearest(&flute, InstrumentType::Flute, Request::default());
    assert_eq!(message, None, "the patch's default is its ordinary sound");
    assert_eq!(
        ordinary.reach,
        Reach::Mapped(default(non_vibrato(switch(Articulation::Sustain, 36))))
    );

    let vibrato_only = mapped(
        preset("FluteSusVib.sfz", InstrumentType::Flute),
        vec![whole(Articulation::Sustain)],
    );
    let wanted = passage(Some(Articulation::Sustain), Mute::Open, Vibrato::Off);
    let (_, message) = nearest(&vibrato_only, InstrumentType::Flute, wanted);
    assert_eq!(
        message.as_deref(),
        Some("Sustain non vibrato is not available for Flute; playing sustain.")
    );
}

/// A preset that maps nothing plays its own sound as the ordinary sound; the names of a
/// version 1 manifest count as played, reached in a way the catalog does not know.
#[test]
fn step_2_an_unmapped_preset_plays_its_own_sound() {
    let unmapped = preset("Violin.sfz", InstrumentType::Violin);
    let (ordinary, message) = nearest(&unmapped, InstrumentType::Violin, Request::default());
    assert_eq!((ordinary.reach, message), (Reach::Own, None));
    let (_, message) = nearest(
        &unmapped,
        InstrumentType::Violin,
        primary(Articulation::Pizzicato),
    );
    assert_eq!(
        message.as_deref(),
        Some("Pizzicato is not available for Violin; playing its ordinary sound.")
    );

    let named = InstrumentPreset {
        articulation_map: Some(Claim {
            value: ArticulationList::Names(vec!["pizzicato".to_owned(), "Bowed".to_owned()]),
            source: ClaimSource::Declared,
        }),
        ..preset("Violin.sfz", InstrumentType::Violin)
    };
    let (pizzicato, message) = nearest(
        &named,
        InstrumentType::Violin,
        primary(Articulation::Pizzicato),
    );
    assert_eq!((pizzicato.reach, message), (Reach::Own, None));
}

// Step 3: solo or section swap.

/// With no preset of the size asked for, the other size plays the part, and the user is told;
/// a preset that does not say its size is taken to be either.
#[test]
fn step_3_the_other_size_plays_when_the_size_asked_for_is_missing() {
    let solo_part = wish(request(InstrumentType::Violin, &[]), EnsembleKind::Solo);
    let plan_solo = plan(&[violin_section_ks()], &solo_part);
    assert_eq!(plan_solo.step, Step::EnsembleSwap);
    assert_eq!(plan_solo.source, sampler("ViolinEns-KS"));
    assert_eq!(
        messages(&plan_solo),
        [(
            Step::EnsembleSwap,
            "No solo Violin is installed; a Violin section plays the part."
        )]
    );

    let section_part = wish(request(InstrumentType::Violin, &[]), EnsembleKind::Section);
    let plan_section = plan(&[violin_solo_ks()], &section_part);
    assert_eq!(
        messages(&plan_section),
        [(
            Step::EnsembleSwap,
            "No Violin section is installed; a solo Violin plays the part."
        )]
    );

    let unstated = mapped(
        preset("Violin.sfz", InstrumentType::Violin),
        string_switches(),
    );
    assert_eq!(plan(&[unstated], &section_part).step, Step::Exact);
}

/// By default the size asked for is kept and articulations are substituted (step 2 before step
/// 3); a user who prefers to keep the articulations gets the other size instead.
#[test]
fn step_3_the_preference_swaps_steps_2_and_3() {
    let solo_sustain = solo(mapped(
        preset("SViolinVib.sfz", InstrumentType::Violin),
        vec![whole(Articulation::Sustain)],
    ));
    let presets = [solo_sustain, violin_section_ks()];
    let part = wish(
        request(InstrumentType::Violin, &[primary(Articulation::Pizzicato)]),
        EnsembleKind::Solo,
    );

    let kept_size = plan(&presets, &part);
    assert_eq!(kept_size.step, Step::Exact);
    assert_eq!(kept_size.source, sampler("SViolinVib"));
    assert_eq!(
        messages(&kept_size),
        [(
            Step::NearestArticulation,
            "Pizzicato is not available for Violin; playing sustain."
        )]
    );

    let kept_articulations = choose_sound(
        &presets,
        &part,
        ChainPreferences {
            keep_articulations: true,
        },
    );
    assert_eq!(kept_articulations.step, Step::EnsembleSwap);
    assert_eq!(kept_articulations.source, sampler("ViolinEns-KS"));
    assert_eq!(
        messages(&kept_articulations),
        [(
            Step::EnsembleSwap,
            "No solo Violin installed plays all the articulations the part asks for; a Violin \
             section plays the part."
        )]
    );
}

// Step 4: family neighbour.

/// With no preset of the instrument, a neighbour whose playable range holds the part plays it,
/// with the part's own articulations.
#[test]
fn step_4_a_neighbour_plays_an_instrument_that_is_not_installed() {
    let part = spanning(
        request(InstrumentType::Cimbasso, &[primary(Articulation::Staccato)]),
        29,
        60,
    );
    let plan = plan(&[tuba()], &part);
    assert_eq!(plan.step, Step::FamilyNeighbour);
    assert_eq!(plan.source, sampler("Tuba-KS"));
    assert_eq!(
        plan.neighbour.map(|n| n.instrument),
        Some(InstrumentType::Tuba)
    );
    assert_eq!(plan.confirm, None);
    assert_eq!(
        messages(&plan),
        [(
            Step::FamilyNeighbour,
            "No Cimbasso is installed; Tuba (lib/Tuba-KS) plays the part."
        )]
    );
    assert_eq!(
        plan.articulations,
        [ArticulationChoice {
            requested: primary(Articulation::Staccato),
            reach: Reach::Mapped(switch(Articulation::Staccato, 37)),
            substitute: None,
        }]
    );
}

/// A part that reaches outside the neighbour's playable range is never moved silently: the user
/// is asked, and the answer "no" leaves the neighbour out. A neighbour whose range holds the
/// part comes before one whose range does not.
#[test]
fn step_4_a_neighbour_out_of_range_asks_first() {
    let part = spanning(request(InstrumentType::Cimbasso, &[]), 34, 70);
    let plan_tuba = plan(&[tuba()], &part);
    assert_eq!(plan_tuba.step, Step::FamilyNeighbour);
    let confirm = plan_tuba.confirm.expect("a question");
    assert_eq!(
        confirm.question,
        "The part reaches A#1 to A#4, beyond Tuba's playable range of A0 to G4. Play it on Tuba \
         with the notes outside that range moved by an octave, or leave Tuba out?"
    );
    // The answer yes moves the notes above G4 into the tuba's range, which the caller does: the
    // question carries the range (oracle: the tuba's record), and the plan keeps the notes.
    assert_eq!(confirm.move_into, playable(InstrumentType::Tuba));
    assert_eq!(
        confirm
            .move_into
            .map(|range| (range.low().get(), range.high().get())),
        Some((21, 67))
    );
    assert_eq!(confirm.otherwise.step, Step::Builtin);
    assert_eq!(
        messages(&confirm.otherwise),
        [(
            Step::Builtin,
            "No sampled sound for Cimbasso is installed; the built-in Poly Synth preset 'Brass' \
             plays the part, a synthetic approximation."
        )]
    );

    // The bass trombone (22 to 77) holds the part, the tuba (21 to 67) does not.
    let trombone = solo(preset("BassTrombone.sfz", InstrumentType::TromboneBass));
    let plan_both = plan(&[tuba(), trombone], &part);
    assert_eq!(plan_both.source, sampler("BassTrombone"));
    assert_eq!(plan_both.confirm, None);
}

/// A neighbour that changes the part's role asks first; the answer "no" goes on down the chain.
#[test]
fn step_4_a_neighbour_that_changes_the_role_asks_first() {
    let sopranos = section(preset("Choir.sfz", InstrumentType::ChoirSoprano));
    let plan = plan(&[sopranos], &request(InstrumentType::VoiceSoprano, &[]));
    assert_eq!(plan.step, Step::FamilyNeighbour);
    assert_eq!(
        messages(&plan),
        [(
            Step::FamilyNeighbour,
            "No Soprano is installed; Sopranos (lib/Choir) plays the part."
        )]
    );
    let confirm = plan.confirm.expect("a question");
    assert_eq!(
        confirm.question,
        "No Soprano is installed; Sopranos (lib/Choir) can play the part, but it changes the \
         part's role. Use it?"
    );
    assert_eq!(
        confirm.move_into, None,
        "the part names no range, so no note moves"
    );
    assert_eq!(
        confirm.otherwise.source,
        SoundSource::Builtin(Builtin::Synth {
            preset: "Choir Pad"
        })
    );
}

/// How a neighbour plays the part is told and carried into the choices: softer, in another
/// articulation, or on one note of a pitched neighbour.
#[test]
fn step_4_a_neighbour_plays_the_part_in_its_manner() {
    let trumpet = solo(mapped(
        preset("Trumpet.sfz", InstrumentType::TrumpetBb),
        vec![whole(Articulation::Sustain)],
    ));
    let softer = plan(&[trumpet], &request(InstrumentType::Flugelhorn, &[]));
    assert_eq!(softer.neighbour.map(|n| n.manner), Some(Manner::Softer));
    assert_eq!(
        messages(&softer),
        [(
            Step::FamilyNeighbour,
            "No Flugelhorn is installed; Trumpet in B♭ (lib/Trumpet) plays the part, a dynamic \
             step softer."
        )]
    );
    assert_eq!(
        softer.articulations,
        [ArticulationChoice {
            requested: Request::default(),
            reach: Reach::Mapped(whole(Articulation::Sustain)),
            substitute: Some(Substitute {
                plays: primary(Articulation::Sustain),
                rewrites: vec![Emulation::Soften],
                only: None,
            }),
        }]
    );

    let cymbal = InstrumentPreset {
        keymap: Some(profile(vec![
            KeymapEntry {
                keys: vec![68],
                instrument: InstrumentType::SuspendedCymbal,
                articulation: None,
                name: None,
            },
            KeymapEntry {
                keys: vec![71],
                instrument: InstrumentType::SuspendedCymbal,
                articulation: Some(ArticulationName::Vocabulary(Articulation::Roll)),
                name: None,
            },
        ])),
        ..preset("Suspended Cymbal.sfz", InstrumentType::SuspendedCymbal)
    };
    let rolled = plan(&[cymbal], &request(InstrumentType::TamTam, &[]));
    assert_eq!(
        messages(&rolled),
        [(
            Step::FamilyNeighbour,
            "No Tam-tam is installed; Suspended cymbal (lib/Suspended Cymbal) plays the part, \
             playing roll throughout."
        )]
    );
    assert_eq!(
        rolled.articulations,
        [ArticulationChoice {
            requested: Request::default(),
            reach: Reach::Keys(vec![71]),
            substitute: Some(Substitute {
                plays: primary(Articulation::Roll),
                rewrites: Vec::new(),
                only: None,
            }),
        }]
    );

    let crotales = preset("Crotales.sfz", InstrumentType::Crotales);
    let on_top = plan(&[crotales], &request(InstrumentType::Triangle, &[]));
    assert_eq!(
        messages(&on_top),
        [(
            Step::FamilyNeighbour,
            "No Triangle is installed; Crotales (lib/Crotales) plays the part, every stroke on \
             its highest playable note, C8."
        )]
    );
}

// Step 5: General MIDI.

/// This build has no General MIDI engine, so an instrument with a General MIDI program goes on
/// from the neighbours to a built-in sound.
#[test]
fn step_5_without_a_general_midi_engine_is_passed_over() {
    assert!(matches!(
        InstrumentType::Violin.get().gm(),
        GmVoice::Program(_)
    ));
    let plan = plan(&[], &request(InstrumentType::Violin, &[]));
    assert_eq!(plan.step, Step::Builtin);
    assert_eq!(
        plan.source,
        SoundSource::Builtin(Builtin::Synth { preset: "Strings" })
    );
}

// Step 6: built-in.

/// Without any sample, a built-in sound plays the part and says that it approximates it.
#[test]
fn step_6_a_built_in_sound_approximates_the_instrument() {
    let violin = plan(
        &[],
        &request(InstrumentType::Violin, &[primary(Articulation::Staccato)]),
    );
    assert_eq!(
        messages(&violin),
        [
            (
                Step::Builtin,
                "No sampled sound for Violin is installed; the built-in Poly Synth preset \
                 'Strings' plays the part, a synthetic approximation."
            ),
            (
                Step::NearestArticulation,
                "Staccato is not available for Violin; playing its ordinary sound, each note \
                 shortened to a quarter of its length."
            ),
        ]
    );
    assert_eq!(violin.articulations[0].reach, Reach::Own);
    assert_eq!(violin.confirm, None);

    let snare = plan(&[], &request(InstrumentType::SnareDrum, &[]));
    assert_eq!(
        snare.source,
        SoundSource::Builtin(Builtin::Drums {
            preset: "Cinematic Toms",
            keys: &[38]
        })
    );
    assert_eq!(
        messages(&snare),
        [(
            Step::Builtin,
            "No sampled sound for Snare drum is installed; the built-in Drum Kit preset \
             'Cinematic Toms' plays the part on key 38, a synthetic approximation."
        )]
    );

    let timpani = plan(&[], &request(InstrumentType::Timpani, &[]));
    assert_eq!(
        messages(&timpani),
        [(
            Step::Builtin,
            "No sampled sound for Timpani is installed; the built-in Drum Kit preset 'Cinematic \
             Toms' plays the part on keys 41, 43, 45, 47, 48 and 50, a synthetic approximation \
             that loses the pitches."
        )]
    );

    let triangle = plan(&[], &request(InstrumentType::Triangle, &[]));
    assert_eq!(
        messages(&triangle),
        [(
            Step::Builtin,
            "No sampled sound for Triangle is installed; the built-in Poly Synth preset 'Glass \
             Bells' plays the part with every stroke on C7, a synthetic approximation."
        )]
    );
}

// Step 7: silence.

/// A built-in sound that changes the part's character asks first; the answer "no" leaves the
/// track silent, and says so.
#[test]
fn step_7_declining_the_last_sound_leaves_the_track_silent() {
    let plan = plan(&[], &request(InstrumentType::Whip, &[]));
    assert_eq!(plan.step, Step::Builtin);
    let confirm = plan.confirm.expect("a question");
    assert_eq!(
        confirm.question,
        "No sampled sound for Whip is installed. The built-in Drum Kit preset 'Cinematic Toms' \
         can play the part on key 39, but as another kind of sound. Use it, or leave the track \
         silent?"
    );
    assert_eq!(confirm.move_into, None);
    assert_eq!(
        *confirm.otherwise,
        SoundPlan {
            step: Step::Silence,
            source: SoundSource::Silence,
            neighbour: None,
            articulations: Vec::new(),
            notices: vec![Notice {
                step: Step::Silence,
                message: "No sound source for Whip; the track is silent.".to_owned(),
            }],
            confirm: None,
        }
    );
}

// Ranking within a step.

/// Among presets that play as many requested articulations, the better verdict wins: as is,
/// then partly, then not judged.
#[test]
fn the_better_verdict_wins_a_tie() {
    let judged = |path: &str, state: Option<VerdictState>| InstrumentPreset {
        verdict: state.map(|state| profile(verdict(state, &[]))),
        ..preset(path, InstrumentType::Harp)
    };
    let presets = [
        judged("a.sfz", None),
        judged("b.sfz", Some(VerdictState::Partly)),
        judged("c.sfz", Some(VerdictState::AsIs)),
    ];
    let part = request(InstrumentType::Harp, &[]);
    assert_eq!(plan(&presets, &part).source, sampler("c"));
    assert_eq!(plan(&presets[..2], &part).source, sampler("b"));
}

/// A set without a verdict of its own is judged by its members' presets: a set of patches that
/// play as is beats a key-switch patch that plays partly, and a member that plays partly makes
/// the set play partly, with the member's losses.
#[test]
fn a_set_is_judged_by_its_members() {
    let ks = InstrumentPreset {
        verdict: Some(profile(verdict(
            VerdictState::Partly,
            &["switch keys in range"],
        ))),
        ..section(mapped(
            preset("CelloEns-KS.sfz", InstrumentType::Cello),
            string_switches(),
        ))
    };
    let set = cello_set(vec![
        played_by(Articulation::Sustain, "CelloSus.sfz"),
        played_by(Articulation::Pizzicato, "CelloPizz.sfz"),
    ]);
    let sustain = section(preset("CelloSus.sfz", InstrumentType::Cello));
    let pizzicato = section(preset("CelloPizz.sfz", InstrumentType::Cello));
    let part = request(
        InstrumentType::Cello,
        &[
            primary(Articulation::Sustain),
            primary(Articulation::Pizzicato),
        ],
    );
    let chosen = plan(&[ks, set.clone(), sustain, pizzicato.clone()], &part);
    assert_eq!(chosen.source, sampler("@cello"));
    assert!(chosen.notices.is_empty(), "{:?}", chosen.notices);
    assert_eq!(
        chosen.articulations[1].reach,
        Reach::Mapped(played_by(Articulation::Pizzicato, "CelloPizz.sfz"))
    );

    // The member's preset judges the set even when it does not itself claim the instrument.
    let partly_member = InstrumentPreset {
        instrument: None,
        verdict: Some(profile(verdict(VerdictState::Partly, &["one layer"]))),
        ..pizzicato
    };
    let told = plan(&[set, partly_member], &part);
    assert_eq!(told.source, sampler("@cello"));
    assert_eq!(
        messages(&told),
        [(
            Step::Exact,
            "cello (lib/@cello) plays partly: CelloPizz.sfz: one layer."
        )]
    );
}

/// A preset whose sampled keys hold the part's notes wins a tie, and a chosen preset that lacks
/// samples for some of them says so. A key offset moves the notes onto the keys first.
#[test]
fn the_sampled_keys_decide_a_tie_and_gaps_are_told() {
    let sampled = |path: &str, lo, hi| InstrumentPreset {
        facts: Some(facts(lo, hi)),
        ..preset(path, InstrumentType::Timpani)
    };
    let part = spanning(request(InstrumentType::Timpani, &[]), 38, 55);
    let presets = [sampled("a.sfz", 41, 60), sampled("b.sfz", 36, 60)];
    assert_eq!(plan(&presets, &part).source, sampler("b"));

    let gappy = plan(&presets[..1], &part);
    assert_eq!(gappy.source, sampler("a"));
    assert_eq!(
        messages(&gappy),
        [(
            Step::Exact,
            "a (lib/a) has no samples for some notes from D2 to G3; those notes do not sound."
        )]
    );

    // A patch keyed an octave above the pitch it sounds holds the part on its keys 50 to 67.
    let shifted = InstrumentPreset {
        key_offset: Some(profile(12)),
        ..sampled("c.sfz", 50, 67)
    };
    let on_its_keys = plan(std::slice::from_ref(&shifted), &part);
    assert!(on_its_keys.notices.is_empty(), "{:?}", on_its_keys.notices);
    let unshifted = InstrumentPreset {
        key_offset: None,
        ..shifted
    };
    assert_eq!(plan(&[unshifted], &part).notices.len(), 1);
}

/// The library's own word comes before Sonara's profile, and both before a guess from the path.
#[test]
fn a_declared_claim_wins_over_a_profile_and_a_guess() {
    let claimed = |path: &str, source| InstrumentPreset {
        instrument: Some(InstrumentClaim {
            instrument: InstrumentType::Harp,
            source,
        }),
        ..preset(path, InstrumentType::Harp)
    };
    let presets = [
        claimed("a.sfz", ClaimSource::Inferred),
        claimed("b.sfz", ClaimSource::Profile),
        claimed("c.sfz", ClaimSource::Declared),
    ];
    let part = request(InstrumentType::Harp, &[]);
    assert_eq!(plan(&presets, &part).source, sampler("c"));
    assert_eq!(plan(&presets[..2], &part).source, sampler("b"));
}

/// Between presets of the profiled libraries that tie on everything else, the order of
/// preference decides, not the ID: VSCO 2 CE's vibrato sustain is the flute's ordinary sound
/// before its staccato, which sorts first by ID.
#[test]
fn the_order_of_preference_breaks_a_tie_between_profiled_presets() {
    let vsco = LibraryId::parse("vsco2ce").expect("valid");
    let flute = |path: &str, articulation| InstrumentPreset {
        library: vsco.clone(),
        id: InstrumentId::new(&vsco, path),
        ..mapped(
            preset(path, InstrumentType::Flute),
            vec![whole(articulation)],
        )
    };
    let presets = [
        flute("FluteStac.sfz", Articulation::Staccato),
        flute("FluteSusVib.sfz", Articulation::Sustain),
    ];
    assert!(presets[0].id < presets[1].id);
    let plan = plan(&presets, &request(InstrumentType::Flute, &[]));
    assert_eq!(
        plan.source,
        SoundSource::Sampler {
            preset: InstrumentId::new(&vsco, "FluteSusVib.sfz")
        }
    );
}

// The tables.

/// Every preset of the order of preference is one that a shipped profile says plays the row's
/// instrument, listed once, and every row is the only one of its instrument; a profile entry
/// that is renamed or remapped fails here instead of leaving a row that ranks nothing.
#[test]
fn the_order_of_preference_names_presets_the_shipped_profiles_map() {
    let mut claimed: HashSet<(InstrumentType, String)> = HashSet::new();
    for profile in profiles::shipped() {
        for entry in &profile.claims.entries {
            let id = match &entry.target {
                EntryTarget::Path(path) => InstrumentId::new(&profile.id, path),
                EntryTarget::Set(slug) => InstrumentId::of_set(&profile.id, slug),
            };
            let keyed = entry
                .keymap
                .value()
                .into_iter()
                .flatten()
                .map(|row| row.instrument);
            for instrument in entry.instrument.value().copied().into_iter().chain(keyed) {
                claimed.insert((instrument, id.to_string()));
            }
        }
    }
    let mut rows = HashSet::new();
    for (instrument, ids) in PREFERRED {
        assert!(rows.insert(*instrument), "{} has two rows", instrument.id());
        assert!(ids.len() >= 2, "{}: a row orders presets", instrument.id());
        let mut seen = HashSet::new();
        for id in *ids {
            assert!(seen.insert(*id), "{id} is listed twice");
            assert!(
                claimed.contains(&(*instrument, (*id).to_owned())),
                "{id} is not a shipped preset of {}",
                instrument.id()
            );
        }
    }
}

/// Every built-in sound is a preset of a built-in instrument (the registry is the oracle), every
/// Drum Kit key is a key of its General MIDI map, only an unpitched instrument plays a single
/// key, and only the two sounds that change the part's character ask first.
#[test]
fn every_built_in_sound_names_a_real_preset_and_key() {
    let registry = builtin_registry();
    for (row, instrument) in BUILTINS.iter().zip(InstrumentType::ALL) {
        assert_eq!(row.instrument, instrument);
        let descriptor = registry
            .get(row.sound.plugin_id())
            .expect("a built-in plugin");
        assert!(
            descriptor.preset(row.sound.preset()).is_ok(),
            "{}: {} has no preset {}",
            instrument.id(),
            row.sound.plugin_id(),
            row.sound.preset()
        );
        match row.sound {
            Builtin::Drums { keys, .. } => {
                assert_eq!(row.sound.plugin_id(), DRUM_KIT);
                assert!(!keys.is_empty(), "{}", instrument.id());
                for key in keys {
                    assert!(
                        GM_MAP.iter().any(|(note, _)| note == key),
                        "{}: key {key} is not in the Drum Kit's map",
                        instrument.id()
                    );
                }
            }
            Builtin::Synth { .. } => {
                assert_eq!(row.sound.plugin_id(), POLY_SYNTH);
                assert!(playable(instrument).is_some(), "{}", instrument.id());
            }
            Builtin::SynthKey { .. } => {
                assert_eq!(row.sound.plugin_id(), POLY_SYNTH);
                assert!(playable(instrument).is_none(), "{}", instrument.id());
            }
        }
    }
    let asking: Vec<InstrumentType> = BUILTINS
        .iter()
        .filter(|row| row.asks_first)
        .map(|row| row.instrument)
        .collect();
    assert_eq!(
        asking,
        [InstrumentType::Whip, InstrumentType::PianoPrepared]
    );
}

/// With no library at all, every instrument type gets a built-in sound: the chain never ends
/// without one before silence.
#[test]
fn every_instrument_type_has_a_built_in_sound() {
    for instrument in InstrumentType::ALL {
        let plan = plan(&[], &request(instrument, &[]));
        assert_eq!(plan.step, Step::Builtin, "{}", instrument.id());
        assert!(
            matches!(plan.source, SoundSource::Builtin(_)),
            "{}",
            instrument.id()
        );
    }
}

// Words.

#[test]
fn rewrites_and_requests_read_as_words() {
    let cases = [
        (Emulation::Overlap, "each note overlapping the next"),
        (
            Emulation::Shorten { divisor: 2 },
            "each note shortened to half its length",
        ),
        (
            Emulation::Shorten { divisor: 4 },
            "each note shortened to a quarter of its length",
        ),
        (
            Emulation::Shorten { divisor: 3 },
            "each note shortened to 1/3 of its length",
        ),
        (Emulation::Accent, "a dynamic step louder"),
        (Emulation::Soften, "a dynamic step softer"),
        (
            Emulation::Repeat { per_whole_note: 32 },
            "as repeated 32nd notes",
        ),
        (
            Emulation::Repeat { per_whole_note: 16 },
            "as repeated 16th notes",
        ),
        (
            Emulation::Alternate {
                slowest: 12,
                fastest: 16,
            },
            "as a trill written out at 12 to 16 notes per second",
        ),
        (
            Emulation::Alternate {
                slowest: 8,
                fastest: 8,
            },
            "as a trill written out at 8 notes per second",
        ),
    ];
    for (emulation, text) in cases {
        assert_eq!(words::emulation(emulation), text);
    }
    assert_eq!(words::request(Request::default()), "ordinario");
    assert_eq!(
        words::request(passage(
            Some(Articulation::SulPonticello),
            Mute::Generic,
            Vibrato::Off
        )),
        "sul ponticello con sordino non vibrato"
    );
    assert_eq!(
        words::articulation(Articulation::ColLegnoBattuto),
        "col legno battuto"
    );
}
