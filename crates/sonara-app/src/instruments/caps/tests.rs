//! The catalog's view of what a preset can do, field by field, over hand-built presets and facts
//! (TST-08), and over every preset of the shipped profiles. Each expected value is written from
//! the claims and facts of its fixture (TST-10); every result passes `SoundCaps::check` (TST-07).

use std::fs;
use std::path::PathBuf;

use sonara_midi::orchestration::{Articulation, InstrumentType, Mute, PitchRange, Vibrato};
use sonara_midi::performance::{DynamicsKind, LegatoKind, SoundCaps};

use super::sound_caps;
use crate::instruments::chain::{ChainPreferences, SoundRequest, SoundSource, choose_sound};
use crate::instruments::manifest::{
    ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, DynamicsMode,
    KeymapEntry, LegatoMode,
};
use crate::instruments::model::{
    Availability, Claim, ClaimSource, InstrumentClaim, InstrumentId, InstrumentPreset, LibraryId,
    PresetKind, SetMember,
};
use crate::instruments::{
    FamilyClaim, InstrumentCatalog, KeyZone, PatchFacts, RegionAttack, SoundFacts, VelocityRange,
    profiles,
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

/// An available patch preset `lib/<path without its extension>` that the profile says plays
/// `instrument`, with nothing else claimed and no facts read.
fn preset(path: &str, instrument: InstrumentType) -> InstrumentPreset {
    let library = library();
    InstrumentPreset {
        id: InstrumentId::new(&library, path),
        display_name: path.to_owned(),
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
        verdict: None,
        facts: None,
    }
}

fn violin() -> InstrumentPreset {
    preset("Violin.sfz", InstrumentType::Violin)
}

fn mapped(mut preset: InstrumentPreset, entries: Vec<ArticulationEntry>) -> InstrumentPreset {
    preset.articulation_map = Some(profile(ArticulationList::Mapped(entries)));
    preset
}

fn legato(mut preset: InstrumentPreset, mode: LegatoMode) -> InstrumentPreset {
    preset.legato = Some(profile(mode));
    preset
}

fn checked(mut preset: InstrumentPreset, facts: PatchFacts) -> InstrumentPreset {
    preset.facts = Some(facts);
    preset
}

/// `articulation` on key switch `key`, open, with the ordinary vibrato.
fn switch(articulation: Articulation, key: u8) -> ArticulationEntry {
    ArticulationEntry {
        name: ArticulationName::Vocabulary(articulation),
        via: ArticulationVia::Keyswitch { key },
        member: None,
        label: None,
        is_default: false,
        mute: Mute::Open,
        vibrato: Vibrato::Ordinary,
        attack_ms: None,
    }
}

fn default(entry: ArticulationEntry) -> ArticulationEntry {
    ArticulationEntry {
        is_default: true,
        ..entry
    }
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

fn stated(entry: ArticulationEntry, attack_ms: u16) -> ArticulationEntry {
    ArticulationEntry {
        attack_ms: Some(attack_ms),
        ..entry
    }
}

/// Keys `lo..=hi` with the velocity layers `layers`.
fn zone(lo: u8, hi: u8, layers: &[(u8, u8)]) -> KeyZone {
    KeyZone {
        lo,
        hi,
        layers: layers
            .iter()
            .map(|&(lo, hi)| VelocityRange { lo, hi })
            .collect(),
    }
}

/// The facts of a patch that loads, sounds on `sampled` and has no switch keys; nothing decoded.
fn facts(sampled: Vec<KeyZone>) -> PatchFacts {
    PatchFacts {
        regions: 1,
        playable_regions: 1,
        sampled,
        switch_keys: Vec::new(),
        default_switch: None,
        ignored_opcodes: Vec::new(),
        missing_samples: Vec::new(),
        unreadable_samples: Vec::new(),
        notes: Vec::new(),
        decoded_bytes: 0,
        loadable: true,
        sound: None,
    }
}

fn full_keys() -> Vec<KeyZone> {
    vec![zone(0, 127, &[(1, 127)])]
}

/// `facts` with the switch keys `keys` and the default switch `default`.
fn switched(facts: PatchFacts, keys: &[u8], default: Option<u8>) -> PatchFacts {
    PatchFacts {
        switch_keys: keys.to_vec(),
        default_switch: default,
        ..facts
    }
}

/// `facts` with decoded samples: one region at every velocity per `(switch, attack_ms)`, and the
/// longest hold `max_hold_ms`.
fn decoded(
    facts: PatchFacts,
    attacks: &[(Option<u8>, u32)],
    max_hold_ms: Option<u32>,
) -> PatchFacts {
    PatchFacts {
        sound: Some(SoundFacts {
            attacks: attacks
                .iter()
                .map(|&(switch, attack_ms)| RegionAttack {
                    switch,
                    velocities: VelocityRange { lo: 1, hi: 127 },
                    attack_ms,
                })
                .collect(),
            max_hold_ms,
        }),
        ..facts
    }
}

/// A set of `members`, each `(path, facts)`, whose default articulation the first plays.
fn set(members: Vec<(&str, Option<PatchFacts>)>) -> InstrumentPreset {
    let root = PathBuf::from("/libraries/lib");
    let members: Vec<SetMember> = members
        .into_iter()
        .map(|(path, facts)| SetMember {
            path: path.to_owned(),
            patch_path: root.join(path),
            availability: Availability::Available,
            facts,
        })
        .collect();
    let mut preset = violin();
    preset.id = InstrumentId::of_set(&library(), "violin");
    preset.patch_path = members
        .first()
        .map_or_else(|| root.clone(), |member| member.patch_path.clone());
    preset.kind = PresetKind::Set { members };
    preset
}

/// `articulations` in the vocabulary's order, each once.
fn in_order(articulations: &[Articulation]) -> Vec<Articulation> {
    Articulation::ALL
        .into_iter()
        .filter(|articulation| articulations.contains(articulation))
        .collect()
}

/// The caps of `preset`, which must pass the coherence check (TST-07).
fn caps(preset: &InstrumentPreset) -> SoundCaps {
    let caps = sound_caps(preset);
    assert_eq!(caps.check(), Ok(()), "{}: {caps:?}", preset.id);
    caps
}

// Tests.

/// A preset whose catalog entry says nothing beyond its instrument, and that no check has read, is
/// the sound nothing is known about; so is one whose version 1 manifest only names articulations,
/// which says nothing about how to reach them (TST-03).
#[test]
fn a_preset_nothing_is_known_about_has_the_default_caps() {
    assert_eq!(caps(&violin()), SoundCaps::default());
    let mut named = violin();
    named.articulation_map = Some(profile(ArticulationList::Names(vec![
        "sustain".to_owned(),
        "pizzicato".to_owned(),
    ])));
    assert_eq!(caps(&named), SoundCaps::default());
}

/// The listed articulations are the choices of the `articulation` parameter that the preset plays
/// as asked, in the vocabulary's order: each mapped id, the mute and vibrato states that a mapped
/// recording carries, and nothing a substitute plays (staccato here, which spiccato would play).
#[test]
fn articulations_are_the_choices_the_preset_plays_as_asked() {
    let strings = mapped(
        violin(),
        vec![
            default(switch(Articulation::Sustain, 36)),
            switch(Articulation::Pizzicato, 37),
            switch(Articulation::Spiccato, 38),
            muted(switch(Articulation::Sustain, 39)),
            non_vibrato(switch(Articulation::Sustain, 40)),
        ],
    );
    assert_eq!(
        caps(&strings).articulations,
        in_order(&[
            Articulation::Sustain,
            Articulation::Spiccato,
            Articulation::Pizzicato,
            Articulation::ConSordino,
            Articulation::NonVibrato,
        ])
    );

    // A sustain recorded only under a mute does not play an open sustain as asked.
    let muted_only = mapped(
        violin(),
        vec![
            default(switch(Articulation::Pizzicato, 36)),
            muted(switch(Articulation::Sustain, 37)),
        ],
    );
    assert_eq!(
        caps(&muted_only).articulations,
        in_order(&[Articulation::Pizzicato, Articulation::ConSordino])
    );

    // The parameter selects nothing on a preset that names no instrument type, or whose
    // articulations are reached by the keys of a percussion key map (TST-03).
    let mut untyped = strings.clone();
    untyped.instrument = None;
    assert_eq!(caps(&untyped).articulations, []);
    let mut kit = preset("Kit.sfz", InstrumentType::SnareDrum);
    kit.instrument = None;
    kit.keymap = Some(profile(vec![KeymapEntry {
        keys: vec![38],
        instrument: InstrumentType::SnareDrum,
        articulation: Some(ArticulationName::Vocabulary(Articulation::Roll)),
        name: None,
    }]));
    assert_eq!(caps(&kit), SoundCaps::default());
}

/// The legato kind is the preset's claim. Legato by overlap or by the patch's own regions lists
/// the `legato` choice, which then plays legato although no legato articulation is mapped; without
/// such a claim it is listed only when a legato recording is mapped.
#[test]
fn legato_is_the_claim_and_lists_the_legato_choice() {
    let overlap = LegatoMode::Overlap {
        crossfade_ms: 90,
        start_offset_ms: 60,
        gap_ms: 0,
    };
    let sustain = || mapped(violin(), vec![default(switch(Articulation::Sustain, 36))]);

    let by_overlap = caps(&legato(sustain(), overlap));
    assert_eq!(
        by_overlap.legato,
        LegatoKind::Overlap { crossfade_ms: 90.0 }
    );
    assert_eq!(
        by_overlap.articulations,
        in_order(&[Articulation::Sustain, Articulation::Legato])
    );
    let by_regions = caps(&legato(sustain(), LegatoMode::Trigger));
    assert_eq!(by_regions.legato, LegatoKind::Trigger);
    assert_eq!(by_regions.articulations, by_overlap.articulations);

    for none in [None, Some(LegatoMode::None)] {
        let mut preset = sustain();
        preset.legato = none.map(profile);
        let caps = caps(&preset);
        assert_eq!(caps.legato, LegatoKind::None);
        assert_eq!(caps.articulations, [Articulation::Sustain]);
    }
    let recorded = mapped(
        violin(),
        vec![
            default(switch(Articulation::Sustain, 36)),
            switch(Articulation::Legato, 37),
        ],
    );
    assert_eq!(caps(&recorded).legato, LegatoKind::None);
    assert_eq!(
        caps(&recorded).articulations,
        in_order(&[Articulation::Sustain, Articulation::Legato])
    );

    // The sampler joins overlapping notes of an overlap-legato preset whatever it maps.
    let mut unmapped = legato(violin(), overlap);
    unmapped.instrument = None;
    assert_eq!(caps(&unmapped).articulations, [Articulation::Legato]);
}

/// The layers keep every velocity at which some key changes the regions it plays: the top of each
/// layer, and the top of the velocities below a layer that no region plays at. One layer while
/// the patch is not checked.
#[test]
fn layers_keep_every_boundary_of_every_key() {
    let layers = |zones: Vec<KeyZone>| caps(&checked(violin(), facts(zones))).layers;
    assert_eq!(
        layers(vec![zone(21, 108, &[(1, 62), (63, 127)])]),
        [62, 127]
    );
    assert_eq!(
        layers(vec![
            zone(21, 60, &[(1, 62), (63, 127)]),
            zone(61, 100, &[(1, 40), (41, 100)]),
        ]),
        [40, 62, 100, 127],
        "the second zone's top layer ends at 100, so the velocities above play nothing there"
    );
    assert_eq!(
        layers(vec![zone(21, 108, &[(64, 127)])]),
        [63, 127],
        "velocities 1 to 63 play nothing"
    );
    assert_eq!(
        layers(vec![zone(21, 108, &[(0, 0), (1, 127)])]),
        [127],
        "velocity 0 is no note"
    );
    assert_eq!(layers(Vec::new()), [127]);
    assert_eq!(caps(&violin()).layers, [127], "not checked");

    let two = || facts(vec![zone(21, 108, &[(1, 62), (63, 127)])]);
    let three = || facts(vec![zone(21, 108, &[(1, 40), (41, 90), (91, 127)])]);
    assert_eq!(
        caps(&set(vec![("A.sfz", Some(two())), ("B.sfz", Some(three()))])).layers,
        [40, 62, 90, 127]
    );
    assert_eq!(
        caps(&set(vec![("A.sfz", Some(two())), ("B.sfz", None)])).layers,
        [127],
        "a member is not checked"
    );
}

/// The dynamics kind is what the claim drives; velocity when nothing is claimed.
#[test]
fn dynamics_follow_the_claim() {
    let dynamics = |mode: Option<DynamicsMode>| {
        let mut preset = violin();
        preset.dynamics = mode.map(profile);
        caps(&preset).dynamics
    };
    assert_eq!(dynamics(None), DynamicsKind::Velocity);
    assert_eq!(
        dynamics(Some(DynamicsMode::Velocity)),
        DynamicsKind::Velocity
    );
    assert_eq!(dynamics(Some(DynamicsMode::Cc(1))), DynamicsKind::Cc);
    assert_eq!(
        dynamics(Some(DynamicsMode::Fixed(100))),
        DynamicsKind::Fixed(100)
    );
}

/// Each listed articulation has the attack of the recording its choice plays, the stated one
/// first, and the ordinary sound the attack of what plays without a choice. Sustain is mapped
/// open and muted but has one attack, the open recording's.
#[test]
fn attacks_are_known_per_articulation_the_statement_first() {
    let strings = mapped(
        violin(),
        vec![
            default(switch(Articulation::Sustain, 36)),
            switch(Articulation::Spiccato, 37),
            stated(switch(Articulation::Pizzicato, 38), 25),
            muted(switch(Articulation::Sustain, 39)),
            switch(Articulation::Tremolo, 41),
        ],
    );
    let measured = decoded(
        switched(facts(full_keys()), &[36, 37, 38, 39, 41], Some(36)),
        &[
            (Some(36), 70),
            (Some(36), 90),
            (Some(37), 10),
            (Some(38), 40),
            (Some(39), 120),
        ],
        None,
    );
    let caps = caps(&checked(strings.clone(), measured));
    assert_eq!(
        caps.attack_ms,
        [
            (None, 80.0),
            (Some(Articulation::Sustain), 80.0),
            (Some(Articulation::Spiccato), 10.0),
            (Some(Articulation::Pizzicato), 25.0),
            (Some(Articulation::ConSordino), 120.0),
        ],
        "tremolo has no region of its own, so its attack is not known"
    );
    assert!(caps.articulations.contains(&Articulation::Tremolo));
    assert_eq!(
        in_order(&caps.articulations),
        caps.articulations,
        "the attacks follow the listed order"
    );

    // Before the patch is decoded only the stated attack is known.
    assert_eq!(
        sound_caps(&strings).attack_ms,
        [(Some(Articulation::Pizzicato), 25.0)]
    );
}

/// A preset the parameter does not select on plays its patch as it is: the ordinary sound is the
/// patch's default switch, or every region of a patch without switches. A patch with switches but
/// no default has no known ordinary attack, nor has one that is not decoded (TST-03).
#[test]
fn the_ordinary_sound_of_an_unmapped_patch_is_the_patch_as_it_plays() {
    let ordinary = |facts: PatchFacts| caps(&checked(violin(), facts)).attack_ms;
    assert_eq!(
        ordinary(decoded(
            facts(full_keys()),
            &[(None, 10), (None, 90), (None, 30)],
            None
        )),
        [(None, 30.0)]
    );
    let switches = |default| {
        decoded(
            switched(facts(full_keys()), &[24, 25], default),
            &[(Some(24), 50), (Some(25), 5), (None, 60)],
            None,
        )
    };
    assert_eq!(
        ordinary(switches(Some(24))),
        [(None, 55.0)],
        "the median of the default switch's region and the one under every switch"
    );
    assert_eq!(ordinary(switches(None)), []);
    assert_eq!(ordinary(facts(full_keys())), [], "not decoded");
}

/// The legato choice of an overlap-legato preset plays the ordinary sound joined by the sampler,
/// so it has the ordinary sound's attack; a patch whose own regions play legato has its mapped
/// legato recording's, else the ordinary sound's.
#[test]
fn legato_starts_from_the_ordinary_sound() {
    let measured = |keys: &[u8]| {
        decoded(
            switched(facts(full_keys()), keys, Some(36)),
            &[(Some(36), 60), (Some(37), 15)],
            None,
        )
    };
    let sustain = || mapped(violin(), vec![default(switch(Articulation::Sustain, 36))]);
    let overlap = LegatoMode::Overlap {
        crossfade_ms: 90,
        start_offset_ms: 60,
        gap_ms: 0,
    };
    let attack = |preset: &InstrumentPreset| {
        caps(preset)
            .attack_ms
            .iter()
            .find(|(articulation, _)| *articulation == Some(Articulation::Legato))
            .map(|&(_, ms)| ms)
    };
    assert_eq!(
        attack(&checked(legato(sustain(), overlap), measured(&[36]))),
        Some(60.0)
    );
    assert_eq!(
        attack(&checked(
            legato(sustain(), LegatoMode::Trigger),
            measured(&[36])
        )),
        Some(60.0)
    );
    let recorded = mapped(
        violin(),
        vec![
            default(switch(Articulation::Sustain, 36)),
            switch(Articulation::Legato, 37),
        ],
    );
    assert_eq!(
        attack(&checked(
            legato(recorded.clone(), LegatoMode::Trigger),
            measured(&[36, 37])
        )),
        Some(15.0)
    );
    assert_eq!(
        attack(&checked(legato(recorded, overlap), measured(&[36, 37]))),
        Some(60.0),
        "the overlap legato plays the ordinary sound, not the legato recording"
    );
}

/// The longest hold is the measured one in seconds; unknown before decoding and for a patch that
/// loops, and known for a set only when every member was measured.
#[test]
fn the_longest_hold_is_in_seconds() {
    let hold = |max_hold_ms| decoded(facts(full_keys()), &[], max_hold_ms);
    assert_eq!(
        caps(&checked(violin(), hold(Some(2_670)))).max_hold_s,
        Some(2.67)
    );
    assert_eq!(caps(&checked(violin(), hold(None))).max_hold_s, None);
    assert_eq!(
        caps(&checked(violin(), facts(full_keys()))).max_hold_s,
        None
    );
    assert_eq!(
        caps(&set(vec![
            ("A.sfz", Some(hold(Some(1_500)))),
            ("B.sfz", Some(hold(Some(11_200)))),
        ]))
        .max_hold_s,
        Some(11.2)
    );
    assert_eq!(
        caps(&set(vec![
            ("A.sfz", Some(hold(Some(1_500)))),
            ("B.sfz", None)
        ]))
        .max_hold_s,
        None
    );
}

/// The sampled range is in the notes a clip writes: the keys from the lowest to the highest that
/// sound, less the key offset, within MIDI. A set sounds where every member does.
#[test]
fn sampled_keys_are_the_notes_a_clip_writes() {
    let range = |low, high| PitchRange::try_new(low, high).expect("a valid range");
    let sampled = |zones: Vec<KeyZone>, offset: Option<i8>| {
        let mut preset = checked(violin(), facts(zones));
        preset.key_offset = offset.map(profile);
        caps(&preset).sampled
    };
    let layer = [(1, 127)];
    assert_eq!(
        sampled(vec![zone(36, 50, &layer), zone(52, 96, &layer)], None),
        Some(range(36, 96)),
        "the gap at 51 is not told apart"
    );
    assert_eq!(
        sampled(vec![zone(36, 96, &layer)], Some(12)),
        Some(range(24, 84)),
        "a note plays the key 12 above it"
    );
    assert_eq!(
        sampled(vec![zone(0, 20, &layer)], Some(12)),
        Some(range(0, 8))
    );
    assert_eq!(
        sampled(vec![zone(120, 127, &layer)], Some(-12)),
        None,
        "no note reaches the keys"
    );
    assert_eq!(sampled(Vec::new(), None), None, "no key sounds");
    assert_eq!(caps(&violin()).sampled, None, "not checked");

    let member = |lo, hi| Some(facts(vec![zone(lo, hi, &layer)]));
    assert_eq!(
        caps(&set(vec![
            ("A.sfz", member(36, 96)),
            ("B.sfz", member(40, 100))
        ]))
        .sampled,
        Some(range(40, 96))
    );
    assert_eq!(
        caps(&set(vec![("A.sfz", member(36, 96)), ("B.sfz", None)])).sampled,
        None
    );
}

/// The caps of a chosen sound: a preset's own, or nothing known for a built-in sound, silence and
/// a preset that is not in the snapshot (TST-03).
#[test]
fn a_chosen_source_has_the_caps_of_its_sound() {
    let strings = mapped(
        violin(),
        vec![
            default(switch(Articulation::Sustain, 36)),
            switch(Articulation::Pizzicato, 37),
        ],
    );
    let presets = [strings.clone()];
    let source = SoundSource::Sampler {
        preset: strings.id.clone(),
    };
    assert_eq!(source.caps(&presets), sound_caps(&strings));
    assert_ne!(source.caps(&presets), SoundCaps::default());
    assert_eq!(source.caps(&[]), SoundCaps::default());
    let builtin = choose_sound(
        &[],
        &SoundRequest::new(InstrumentType::Violin),
        ChainPreferences::default(),
    )
    .source;
    assert!(matches!(builtin, SoundSource::Builtin(_)), "{builtin:?}");
    assert_eq!(builtin.caps(&presets), SoundCaps::default());
    assert_eq!(SoundSource::Silence.caps(&presets), SoundCaps::default());
}

/// Every preset of the shipped profiles has coherent caps, and the VSCO 2 CE key-switch violins
/// list the four switches the profile maps (TST-07).
#[test]
fn every_shipped_preset_has_coherent_caps() {
    let dir = tempfile::tempdir().expect("tempdir");
    let roots: Vec<PathBuf> = profiles::shipped()
        .iter()
        .map(|profile| {
            let root = dir.path().join(profile.id.as_str());
            for file in &profile.detect {
                let path = root.join(file);
                fs::create_dir_all(path.parent().expect("a file below the root")).expect("folder");
                fs::write(&path, "<region> sample=a.wav\n").expect("patch");
            }
            root
        })
        .collect();
    let catalog = InstrumentCatalog::discover(&roots);
    assert!(catalog.presets().len() > 200, "both profiles apply");
    for preset in catalog.presets() {
        caps(preset);
    }
    let violins = catalog
        .presets()
        .iter()
        .find(|preset| preset.id.as_str() == "vsco2ce/ViolinEns-KS")
        .expect("the profile lists the patch");
    assert_eq!(
        caps(violins).articulations,
        in_order(&[
            Articulation::Sustain,
            Articulation::Tremolo,
            Articulation::Spiccato,
            Articulation::Pizzicato,
        ])
    );
}
