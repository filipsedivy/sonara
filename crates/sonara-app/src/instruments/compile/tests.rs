//! Compiling sampler mappings over hand-built presets (TST-08), and through the installed
//! instruments of a library on disk. The oracle for a choice the preset does not map is the
//! substitution data of the degradation chain: a staccato on a violin that plays spiccato is
//! played as spiccato, a legato without a legato claim as a sustain.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use sonara_core::plugins::{ParamKind, SAMPLER_AUTO, SAMPLER_ID, SAMPLER_ORDINARIO};
use sonara_dsp::bank::{
    ArticulationTarget, DynamicsDrive, LegatoSpec, Passage, SamplerMapping, Selector, TargetTable,
};
use sonara_midi::orchestration::{Articulation, InstrumentType, Mute, Vibrato};

use super::{
    ARTICULATION_CHOICES, Compiler, LEADING_CHOICES, MUTE_CHOICES, VIBRATO_CHOICES, count,
    sampler_mapping, targets,
};
use crate::instruments::manifest::{
    ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, DynamicsMode,
    KeymapEntry, LegatoMode,
};
use crate::instruments::model::{
    Availability, Claim, ClaimSource, InstrumentClaim, InstrumentId, InstrumentPreset, LibraryId,
    PresetKind, SetMember,
};
use crate::instruments::{FamilyClaim, InstalledInstruments, InstrumentCatalog, InstrumentFamily};
use crate::samples::registry_for;

// Fixtures.

fn profile<T>(value: T) -> Claim<T> {
    Claim {
        value,
        source: ClaimSource::Profile,
    }
}

/// An available violin patch about which nothing is claimed but what it plays.
fn violin() -> InstrumentPreset {
    let library = LibraryId::parse("lib").expect("a valid library id");
    InstrumentPreset {
        id: InstrumentId::new(&library, "Violin.sfz"),
        display_name: "Violin".to_owned(),
        family: FamilyClaim::Profile(InstrumentFamily::Strings),
        library,
        patch_path: PathBuf::from("/libraries/lib/Violin.sfz"),
        kind: PresetKind::Patch,
        articulations: Vec::new(),
        license: None,
        availability: Availability::Available,
        instrument: Some(InstrumentClaim {
            instrument: InstrumentType::Violin,
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

fn on_key(articulation: Articulation, key: u8) -> ArticulationEntry {
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

/// The articulations of VSCO 2 CE's `SViolin-KS.sfz`: sustain (36, the default), tremolo (37),
/// spiccato (38) and pizzicato (39).
fn keyswitched_violin() -> InstrumentPreset {
    mapped(
        violin(),
        vec![
            default(on_key(Articulation::Sustain, 36)),
            on_key(Articulation::Tremolo, 37),
            on_key(Articulation::Spiccato, 38),
            on_key(Articulation::Pizzicato, 39),
        ],
    )
}

/// The index of `articulation` among the choices of the `articulation` parameter.
fn choice(articulation: Articulation) -> u16 {
    let position = Articulation::ALL
        .iter()
        .position(|candidate| *candidate == articulation)
        .expect("in the vocabulary");
    u16::try_from(LEADING_CHOICES + position).expect("a small index")
}

/// The target that makes keyswitch `key` active.
fn switch(key: u8) -> ArticulationTarget {
    reached(Selector::Keyswitch(key))
}

/// The target `selector` reaches, without legato.
fn reached(selector: Selector) -> ArticulationTarget {
    ArticulationTarget {
        member: 0,
        selector: Some(selector),
        legato: false,
    }
}

// Presets without claims.

#[test]
fn a_preset_without_claims_compiles_to_the_default_mapping() {
    // The sampler then plays the patch as it is (`SamplerMapping::default`).
    assert_eq!(sampler_mapping(&violin()), SamplerMapping::default());
    let unnamed = InstrumentPreset {
        instrument: None,
        ..violin()
    };
    assert_eq!(sampler_mapping(&unnamed), SamplerMapping::default());
}

#[test]
fn articulation_names_that_do_not_say_how_compile_to_no_target() {
    // A version 1 manifest names its articulations without saying how each is reached.
    let mut preset = violin();
    preset.articulation_map = Some(Claim {
        value: ArticulationList::Names(vec!["staccato".to_owned(), "pizzicato".to_owned()]),
        source: ClaimSource::Declared,
    });
    assert_eq!(sampler_mapping(&preset), SamplerMapping::default());
}

#[test]
fn a_preset_that_names_no_instrument_type_gets_no_targets() {
    let preset = InstrumentPreset {
        instrument: None,
        ..keyswitched_violin()
    };
    assert_eq!(sampler_mapping(&preset), SamplerMapping::default());
}

#[test]
fn the_keys_of_a_key_map_give_no_target() {
    // A percussion patch: the notes choose the keys, so no articulation choice selects one.
    let mut preset = violin();
    preset.instrument = Some(InstrumentClaim {
        instrument: InstrumentType::SnareDrum,
        source: ClaimSource::Profile,
    });
    preset.keymap = Some(profile(vec![
        KeymapEntry {
            keys: vec![38],
            instrument: InstrumentType::SnareDrum,
            articulation: None,
            name: None,
        },
        KeymapEntry {
            keys: vec![40],
            instrument: InstrumentType::SnareDrum,
            articulation: Some(ArticulationName::Vocabulary(Articulation::Roll)),
            name: None,
        },
    ]));
    let preset = mapped(
        preset,
        vec![default(entry(
            ArticulationName::Ordinario,
            ArticulationVia::Patch,
        ))],
    );
    let mapping = sampler_mapping(&preset);
    assert_eq!(mapping.target(SamplerMapping::ORDINARIO), None);
    assert_eq!(mapping.target(choice(Articulation::Roll)), None);
}

// Mapped articulations and step 2.

#[test]
fn keyswitched_articulations_compile_to_their_switches() {
    let mapping = sampler_mapping(&keyswitched_violin());
    assert_eq!(mapping.target(SamplerMapping::AUTO), None, "auto has none");
    assert_eq!(
        mapping.default_target(),
        Some(switch(36)),
        "ordinario: the default"
    );
    for (articulation, key) in [
        (Articulation::Sustain, 36),
        (Articulation::Tremolo, 37),
        (Articulation::Spiccato, 38),
        (Articulation::Pizzicato, 39),
    ] {
        assert_eq!(
            mapping.target(choice(articulation)),
            Some(switch(key)),
            "{articulation:?}"
        );
    }
    assert_eq!(mapping.dynamics, DynamicsDrive::Velocity);
    assert_eq!(mapping.legato, None);
    assert_eq!(mapping.key_offset, 0);
}

#[test]
fn a_choice_the_preset_does_not_map_plays_the_nearest_it_does() {
    // The substitution rows: staccato's first stand-in is spiccato; legato's is a sustain (with
    // overlapping notes, which the performance layer writes); tenuto's is a sustain.
    let mapping = sampler_mapping(&keyswitched_violin());
    assert_eq!(
        mapping.target(choice(Articulation::Staccato)),
        Some(switch(38))
    );
    assert_eq!(
        mapping.target(choice(Articulation::Legato)),
        Some(switch(36))
    );
    assert_eq!(
        mapping.target(choice(Articulation::Tenuto)),
        Some(switch(36))
    );
    // Nothing nearer than the ordinary sound: a harmonic plays the sustain.
    assert_eq!(
        mapping.target(choice(Articulation::Harmonics)),
        Some(switch(36))
    );
    // A preset that maps how its articulations are reached gives every choice but `auto` one of
    // its own targets, so the render thread never falls back to the clip's keyswitch notes.
    let keys = [36, 37, 38, 39];
    for (index, articulation) in Articulation::ALL.iter().enumerate() {
        let target = mapping.target(u16::try_from(LEADING_CHOICES + index).expect("small"));
        assert!(
            keys.iter().any(|key| target == Some(switch(*key))),
            "{articulation:?} has {target:?}"
        );
    }
}

#[test]
fn a_mute_or_vibrato_choice_asks_for_the_ordinary_sound_under_it() {
    let muted = ArticulationEntry {
        mute: Mute::Generic,
        ..on_key(Articulation::Sustain, 40)
    };
    let still = ArticulationEntry {
        vibrato: Vibrato::Off,
        ..on_key(Articulation::Sustain, 41)
    };
    let preset = mapped(
        violin(),
        vec![default(on_key(Articulation::Sustain, 36)), muted, still],
    );
    let mapping = sampler_mapping(&preset);
    assert_eq!(
        mapping.target(choice(Articulation::ConSordino)),
        Some(switch(40))
    );
    assert_eq!(
        mapping.target(choice(Articulation::NonVibrato)),
        Some(switch(41))
    );
    // A mute the preset does not record gives way to the open sound.
    assert_eq!(
        mapping.target(choice(Articulation::MuteCup)),
        Some(switch(40))
    );
    assert_eq!(
        mapping.target(choice(Articulation::VibratoOn)),
        Some(switch(36))
    );
    assert_eq!(mapping.default_target(), Some(switch(36)));
}

#[test]
fn velocity_controller_and_patch_articulations_compile_to_their_selectors() {
    let preset = mapped(
        violin(),
        vec![
            default(entry(
                ArticulationName::Ordinario,
                ArticulationVia::Velocity { lo: 1, hi: 90 },
            )),
            entry(
                ArticulationName::Vocabulary(Articulation::Pizzicato),
                ArticulationVia::Cc { cc: 32, value: 5 },
            ),
            entry(
                ArticulationName::Vocabulary(Articulation::Tremolo),
                ArticulationVia::Patch,
            ),
        ],
    );
    let mapping = sampler_mapping(&preset);
    assert_eq!(
        mapping.default_target(),
        Some(reached(Selector::Velocity { lo: 1, hi: 90 }))
    );
    assert_eq!(
        mapping.target(choice(Articulation::Pizzicato)),
        Some(reached(Selector::Controller {
            number: 32,
            value: 5
        }))
    );
    // A patch that is the articulation has nothing to select.
    assert_eq!(
        mapping.target(choice(Articulation::Tremolo)),
        Some(ArticulationTarget {
            member: 0,
            selector: None,
            legato: false,
        })
    );
}

// Mute and vibrato.

/// The index of `state` among the choices of the `mute` parameter: `auto`, then `Mute::ALL`.
fn mute(state: Mute) -> u16 {
    let position = Mute::ALL
        .iter()
        .position(|candidate| *candidate == state)
        .expect("a mute");
    u16::try_from(1 + position).expect("a small index")
}

/// The index of `state` among the choices of the `vibrato` parameter: `auto`, then
/// `Vibrato::ALL`.
fn vibrato(state: Vibrato) -> u16 {
    let position = Vibrato::ALL
        .iter()
        .position(|candidate| *candidate == state)
        .expect("a vibrato");
    u16::try_from(1 + position).expect("a small index")
}

/// The passage of the `mute` and `vibrato` choices `mute` and `vibrato`, `auto` for `None`.
fn passage(mute_state: Option<Mute>, vibrato_state: Option<Vibrato>) -> Passage {
    Passage {
        mute: mute_state.map_or(0, mute),
        vibrato: vibrato_state.map_or(0, vibrato),
    }
}

/// The articulations of VSCO 2 CE's `Flute-KS.sfz` as Sonara's profile maps them: sustain
/// without vibrato on 36 (the default), sustain on 37, sustain with vibrato on 38 and staccato
/// on 39.
fn keyswitched_flute() -> InstrumentPreset {
    let mut preset = mapped(
        violin(),
        vec![
            default(ArticulationEntry {
                vibrato: Vibrato::Off,
                ..on_key(Articulation::Sustain, 36)
            }),
            on_key(Articulation::Sustain, 37),
            ArticulationEntry {
                vibrato: Vibrato::On,
                ..on_key(Articulation::Sustain, 38)
            },
            on_key(Articulation::Staccato, 39),
        ],
    );
    preset.instrument = Some(InstrumentClaim {
        instrument: InstrumentType::Flute,
        source: ClaimSource::Profile,
    });
    preset
}

/// A keyswitched trumpet: sustain on 36 (the default), sustain with a mute of no named type on
/// 40 and with a harmon mute on 41, staccato on 39.
fn muted_trumpet() -> InstrumentPreset {
    let mut preset = mapped(
        violin(),
        vec![
            default(on_key(Articulation::Sustain, 36)),
            ArticulationEntry {
                mute: Mute::Generic,
                ..on_key(Articulation::Sustain, 40)
            },
            ArticulationEntry {
                mute: Mute::Harmon,
                ..on_key(Articulation::Sustain, 41)
            },
            on_key(Articulation::Staccato, 39),
        ],
    );
    preset.instrument = Some(InstrumentClaim {
        instrument: InstrumentType::TrumpetBb,
        source: ClaimSource::Profile,
    });
    preset
}

#[test]
fn a_recorded_vibrato_is_chosen_when_the_state_matches() {
    let mapping = sampler_mapping(&keyswitched_flute());
    let sustain = choice(Articulation::Sustain);
    let under = |state| mapping.target_under(sustain, passage(None, Some(state)));
    // `auto` keeps the articulation's own choice: the sustain with the ordinary vibrato.
    assert_eq!(mapping.target(sustain), Some(switch(37)));
    assert_eq!(under(Vibrato::On), Some(switch(38)));
    assert_eq!(under(Vibrato::Off), Some(switch(36)));
    assert_eq!(under(Vibrato::Ordinary), Some(switch(37)));
    // A staccato is recorded only with the ordinary vibrato: the state is dropped.
    for state in Vibrato::ALL {
        assert_eq!(
            mapping.target_under(choice(Articulation::Staccato), passage(None, Some(state))),
            Some(switch(39)),
            "{state:?}"
        );
    }
    // The ordinary sound under a vibrato.
    assert_eq!(mapping.default_target(), Some(switch(36)));
    assert_eq!(
        mapping.target_under(SamplerMapping::ORDINARIO, passage(None, Some(Vibrato::On))),
        Some(switch(38))
    );
    // A stated vibrato replaces the one a vibrato choice names.
    let non_vibrato = choice(Articulation::NonVibrato);
    assert_eq!(mapping.target(non_vibrato), Some(switch(36)));
    assert_eq!(
        mapping.target_under(non_vibrato, passage(None, Some(Vibrato::On))),
        Some(switch(38))
    );
}

#[test]
fn a_recorded_mute_is_chosen_when_the_state_matches() {
    let mapping = sampler_mapping(&muted_trumpet());
    let sustain = choice(Articulation::Sustain);
    let under = |state| mapping.target_under(sustain, passage(Some(state), None));
    assert_eq!(mapping.target(sustain), Some(switch(36)));
    assert_eq!(under(Mute::Open), Some(switch(36)));
    assert_eq!(under(Mute::Generic), Some(switch(40)));
    assert_eq!(under(Mute::Harmon), Some(switch(41)));
    // A mute the preset does not record gives way to the more general mute, then to none.
    assert_eq!(under(Mute::Cup), Some(switch(40)), "a cup mute is a mute");
    assert_eq!(under(Mute::Palm), Some(switch(36)), "no palm mute");
    // A staccato is recorded only open.
    assert_eq!(
        mapping.target_under(
            choice(Articulation::Staccato),
            passage(Some(Mute::Harmon), None)
        ),
        Some(switch(39))
    );
    // A mute choice of `articulation` keeps its mute under `auto`; a stated mute replaces it.
    let con_sordino = choice(Articulation::ConSordino);
    assert_eq!(mapping.target(con_sordino), Some(switch(40)));
    assert_eq!(
        mapping.target_under(con_sordino, passage(Some(Mute::Open), None)),
        Some(switch(36))
    );
    assert_eq!(
        mapping.target_under(con_sordino, passage(Some(Mute::Harmon), None)),
        Some(switch(41))
    );
    // A stated vibrato the trumpet does not record changes nothing.
    assert_eq!(
        mapping.target_under(sustain, passage(Some(Mute::Harmon), Some(Vibrato::Off))),
        Some(switch(41))
    );
}

#[test]
fn auto_plays_a_stated_mute_or_vibrato_only_when_the_preset_records_it() {
    let trumpet = sampler_mapping(&muted_trumpet());
    let auto = |mapping: &SamplerMapping, mute_state, vibrato_state| {
        mapping.target_under(SamplerMapping::AUTO, passage(mute_state, vibrato_state))
    };
    // The ordinary sound under the mute.
    assert_eq!(auto(&trumpet, Some(Mute::Generic), None), Some(switch(40)));
    assert_eq!(auto(&trumpet, Some(Mute::Harmon), None), Some(switch(41)));
    assert_eq!(auto(&trumpet, Some(Mute::Bucket), None), Some(switch(40)));
    // Open and the ordinary vibrato name nothing to select: the clips' keyswitches decide.
    assert_eq!(auto(&trumpet, Some(Mute::Open), None), None);
    assert_eq!(auto(&trumpet, None, Some(Vibrato::Ordinary)), None);
    assert_eq!(
        auto(&trumpet, Some(Mute::Open), Some(Vibrato::Ordinary)),
        None
    );
    // A state the preset does not record leaves `auto` alone, not on the ordinary sound.
    assert_eq!(auto(&trumpet, Some(Mute::Palm), None), None);
    assert_eq!(auto(&trumpet, None, Some(Vibrato::On)), None);
    let flute = sampler_mapping(&keyswitched_flute());
    assert_eq!(auto(&flute, None, Some(Vibrato::On)), Some(switch(38)));
    assert_eq!(auto(&flute, None, Some(Vibrato::Off)), Some(switch(36)));
    // The flute's ordinary sound has no vibrato, but no mute was kept.
    assert_eq!(auto(&flute, Some(Mute::Generic), None), None);
    for mapping in [&trumpet, &flute] {
        assert_eq!(auto(mapping, None, None), None, "nothing is forced");
    }
}

#[test]
fn a_legato_under_a_state_keeps_the_state() {
    // Overlap legato: the ordinary sound under the state, joined.
    let mut overlap = muted_trumpet();
    overlap.legato = Some(profile(LegatoMode::Overlap {
        crossfade_ms: 90,
        start_offset_ms: 60,
        gap_ms: 5,
    }));
    let mapping = sampler_mapping(&overlap);
    let legato = choice(Articulation::Legato);
    let joined = |key| ArticulationTarget {
        member: 0,
        selector: Some(Selector::Keyswitch(key)),
        legato: true,
    };
    assert_eq!(mapping.target(legato), Some(joined(36)));
    assert_eq!(
        mapping.target_under(legato, passage(Some(Mute::Generic), None)),
        Some(joined(40))
    );
    // Legato regions: the mapped legato, under the state when it is recorded so, else open;
    // without a mapped legato, the ordinary sound under the state.
    let mut regions = mapped(
        muted_trumpet(),
        [
            mapped_entries(&muted_trumpet()),
            vec![on_key(Articulation::Legato, 42)],
        ]
        .concat(),
    );
    regions.legato = Some(profile(LegatoMode::Trigger));
    let mapping = sampler_mapping(&regions);
    assert_eq!(mapping.target(legato), Some(switch(42)));
    assert_eq!(
        mapping.target_under(legato, passage(Some(Mute::Generic), None)),
        Some(switch(42))
    );
    let mut ordinary = muted_trumpet();
    ordinary.legato = Some(profile(LegatoMode::Trigger));
    let mapping = sampler_mapping(&ordinary);
    assert_eq!(mapping.target(legato), Some(switch(36)));
    assert_eq!(
        mapping.target_under(legato, passage(Some(Mute::Harmon), None)),
        Some(switch(41))
    );
}

/// The mapped articulations of `preset`.
fn mapped_entries(preset: &InstrumentPreset) -> Vec<ArticulationEntry> {
    match preset.articulation_map.as_ref().map(|claim| &claim.value) {
        Some(ArticulationList::Mapped(entries)) => entries.clone(),
        _ => Vec::new(),
    }
}

#[test]
fn a_preset_without_mutes_or_vibratos_keeps_one_target_per_choice() {
    // Every mute and vibrato choice plays as `auto`, so the mapping equals the one built from
    // the targets of the articulation choices alone (TYP-10), however the passage is stated.
    for preset in [keyswitched_violin(), violin()] {
        let mapping = sampler_mapping(&preset);
        let alone: Vec<Option<ArticulationTarget>> = (0..count(ARTICULATION_CHOICES))
            .map(|index| mapping.target(index))
            .collect();
        assert_eq!(
            mapping,
            SamplerMapping::new(alone, DynamicsDrive::Velocity, None, 0)
        );
        for index in 0..count(ARTICULATION_CHOICES) {
            for mute_state in Mute::ALL {
                for vibrato_state in Vibrato::ALL {
                    assert_eq!(
                        mapping.target_under(index, passage(Some(mute_state), Some(vibrato_state))),
                        mapping.target(index)
                    );
                }
            }
        }
    }
    // Only the vibrato gets columns of the flute's.
    let flute = sampler_mapping(&keyswitched_flute());
    for state in Mute::ALL {
        assert_eq!(
            flute.target_under(choice(Articulation::Sustain), passage(Some(state), None)),
            Some(switch(37)),
            "{state:?}"
        );
    }
}

#[test]
fn a_mute_or_vibrato_choice_beyond_the_lists_plays_as_auto() {
    let mapping = sampler_mapping(&muted_trumpet());
    let sustain = choice(Articulation::Sustain);
    for beyond in [count(MUTE_CHOICES), u16::MAX] {
        assert_eq!(
            mapping.target_under(
                sustain,
                Passage {
                    mute: beyond,
                    vibrato: 0
                }
            ),
            Some(switch(36))
        );
    }
    assert_eq!(
        mapping.target_under(
            sustain,
            Passage {
                mute: mute(Mute::Generic),
                vibrato: u16::MAX
            }
        ),
        Some(switch(40))
    );
}

/// Every cell of `preset`'s targets computed one by one, every mute and vibrato choice with a
/// column of its own: the oracle of the columns [`targets`] leaves out.
fn every_cell(preset: &InstrumentPreset) -> TargetTable {
    let legato = preset.legato.as_ref().map(|claim| claim.value);
    let compiler = Compiler::of(preset);
    TargetTable::tabulate(
        count(ARTICULATION_CHOICES),
        count(MUTE_CHOICES),
        count(VIBRATO_CHOICES),
        |index, passage| compiler.target(index, passage, legato),
    )
}

#[test]
fn leaving_out_the_states_a_preset_does_not_record_changes_no_target() {
    let mut overlap = keyswitched_flute();
    overlap.legato = Some(profile(LegatoMode::Overlap {
        crossfade_ms: 90,
        start_offset_ms: 60,
        gap_ms: 5,
    }));
    let mut regions = muted_trumpet();
    regions.legato = Some(profile(LegatoMode::Trigger));
    for preset in [
        violin(),
        keyswitched_violin(),
        keyswitched_flute(),
        muted_trumpet(),
        overlap,
        regions,
    ] {
        let legato = preset.legato.as_ref().map(|claim| claim.value);
        assert_eq!(
            targets(&preset, legato),
            every_cell(&preset),
            "{}",
            preset.id
        );
    }
}

#[test]
fn the_states_follow_the_order_of_the_mute_and_vibrato_choices() {
    // The sampler indexes the columns by the choices its parameters hold, so the order is the
    // descriptor's, which the application builds (`crate::samples`).
    let registry = registry_for(&["lib/Violin".to_owned()]);
    let descriptor = registry
        .find(SAMPLER_ID)
        .expect("the sampler is registered");
    let choices_of = |id: &str| -> Vec<String> {
        descriptor
            .params
            .iter()
            .find(|spec| spec.id == id)
            .expect("the parameter")
            .choices
            .clone()
    };
    let mutes = choices_of("mute");
    assert_eq!(mutes.len(), MUTE_CHOICES);
    assert_eq!(mutes.first().map(String::as_str), Some(SAMPLER_AUTO));
    for state in Mute::ALL {
        let named = state.articulation().map_or("open", Articulation::id);
        assert_eq!(
            mutes.get(usize::from(mute(state))).map(String::as_str),
            Some(named)
        );
    }
    let vibratos = choices_of("vibrato");
    assert_eq!(vibratos.len(), VIBRATO_CHOICES);
    assert_eq!(vibratos.first().map(String::as_str), Some(SAMPLER_AUTO));
    for state in Vibrato::ALL {
        let named = state.articulation().map_or("ordinary", Articulation::id);
        assert_eq!(
            vibratos
                .get(usize::from(vibrato(state)))
                .map(String::as_str),
            Some(named)
        );
    }
    let vibrato_on = vibratos
        .iter()
        .position(|id| id == "vibrato_on")
        .and_then(|index| u16::try_from(index).ok())
        .expect("offered");
    assert_eq!(
        sampler_mapping(&keyswitched_flute()).target_under(
            choice(Articulation::Sustain),
            Passage {
                mute: 0,
                vibrato: vibrato_on
            }
        ),
        Some(switch(38))
    );
}

// Dynamics, legato and the key offset.

#[test]
fn dynamics_and_the_key_offset_are_the_presets_claims() {
    let mut preset = violin();
    preset.dynamics = Some(profile(DynamicsMode::Cc(1)));
    preset.key_offset = Some(profile(-12));
    let mapping = sampler_mapping(&preset);
    assert_eq!(mapping.dynamics, DynamicsDrive::Controller(1));
    assert_eq!(mapping.key_offset, -12);
    assert_eq!(mapping.target(SamplerMapping::ORDINARIO), None);
    preset.dynamics = Some(profile(DynamicsMode::Fixed(100)));
    assert_eq!(sampler_mapping(&preset).dynamics, DynamicsDrive::Fixed(100));
    preset.dynamics = Some(profile(DynamicsMode::Velocity));
    preset.key_offset = Some(profile(0));
    assert_eq!(sampler_mapping(&preset), SamplerMapping::default());
}

#[test]
fn an_overlap_legato_plays_the_ordinary_sound_joined() {
    let mut preset = keyswitched_violin();
    preset.legato = Some(profile(LegatoMode::Overlap {
        crossfade_ms: 90,
        start_offset_ms: 60,
        gap_ms: 5,
    }));
    let mapping = sampler_mapping(&preset);
    assert_eq!(
        mapping.legato,
        Some(LegatoSpec {
            crossfade_ms: 90,
            start_offset_ms: 60,
            gap_ms: 5,
        })
    );
    assert_eq!(
        mapping.target(choice(Articulation::Legato)),
        Some(ArticulationTarget {
            member: 0,
            selector: Some(Selector::Keyswitch(36)),
            legato: true,
        })
    );
    // Only the legato choice joins notes: a sustained chord stays a chord.
    assert_eq!(
        mapping.target(choice(Articulation::Sustain)),
        Some(switch(36))
    );
    assert_eq!(mapping.default_target(), Some(switch(36)));
    // A patch whose articulations are not mapped plays itself joined.
    let mut plain = violin();
    plain.legato = preset.legato.clone();
    assert_eq!(
        sampler_mapping(&plain).target(choice(Articulation::Legato)),
        Some(ArticulationTarget {
            member: 0,
            selector: None,
            legato: true,
        })
    );
}

#[test]
fn a_patch_whose_regions_play_legato_plays_its_own_legato_or_its_ordinary_sound() {
    // Without a legato claim, step 2 plays a legato as a tenuto here: a sustain, the nearer
    // stand-in, is not mapped.
    let unclaimed = mapped(violin(), ordinario_and_tenuto());
    assert_eq!(
        sampler_mapping(&unclaimed).target(choice(Articulation::Legato)),
        Some(switch(41))
    );
    // A claim of no legato leaves the choice to step 2 too.
    let mut none = mapped(violin(), ordinario_and_tenuto());
    none.legato = Some(profile(LegatoMode::None));
    assert_eq!(
        sampler_mapping(&none).target(choice(Articulation::Legato)),
        Some(switch(41))
    );
    // Its own legato regions do the work in the ordinary sound.
    let mut regions = mapped(violin(), ordinario_and_tenuto());
    regions.legato = Some(profile(LegatoMode::Trigger));
    let mapping = sampler_mapping(&regions);
    assert_eq!(
        mapping.target(choice(Articulation::Legato)),
        Some(switch(36))
    );
    assert_eq!(mapping.legato, None, "no overlap legato");
    // A mapped legato articulation is played as it is.
    let mut own = mapped(
        violin(),
        [
            ordinario_and_tenuto(),
            vec![on_key(Articulation::Legato, 42)],
        ]
        .concat(),
    );
    own.legato = Some(profile(LegatoMode::Trigger));
    assert_eq!(
        sampler_mapping(&own).target(choice(Articulation::Legato)),
        Some(switch(42))
    );
}

/// The ordinary sound on key 36, the default, and a tenuto on 41.
fn ordinario_and_tenuto() -> Vec<ArticulationEntry> {
    vec![
        default(entry(
            ArticulationName::Ordinario,
            ArticulationVia::Keyswitch { key: 36 },
        )),
        on_key(Articulation::Tenuto, 41),
    ]
}

// The contract with the descriptor.

#[test]
fn the_targets_follow_the_order_of_the_articulation_choices() {
    // The sampler indexes the targets by the choice the parameter holds, so the order is the
    // descriptor's, which the application builds (`crate::samples`).
    let registry = registry_for(&["lib/Violin".to_owned()]);
    let descriptor = registry
        .find(SAMPLER_ID)
        .expect("the sampler is registered");
    let spec = descriptor
        .params
        .iter()
        .find(|spec| spec.id == "articulation")
        .expect("an articulation parameter");
    assert_eq!(spec.kind, ParamKind::Enum);
    let choices: Vec<&str> = spec.choices.iter().map(String::as_str).collect();
    assert_eq!(
        choices.get(usize::from(SamplerMapping::AUTO)),
        Some(&SAMPLER_AUTO)
    );
    assert_eq!(
        choices.get(usize::from(SamplerMapping::ORDINARIO)),
        Some(&SAMPLER_ORDINARIO)
    );
    assert_eq!(choices.len(), LEADING_CHOICES + Articulation::ALL.len());
    for articulation in Articulation::ALL {
        assert_eq!(
            choices.get(usize::from(choice(articulation))),
            Some(&articulation.id()),
            "{articulation:?}"
        );
    }
    let spiccato = choices
        .iter()
        .position(|id| *id == "spiccato")
        .and_then(|index| u16::try_from(index).ok())
        .expect("offered");
    assert_eq!(
        sampler_mapping(&keyswitched_violin()).target(spiccato),
        Some(switch(38))
    );
}

// Through the installed instruments.

fn write_patch(path: &Path) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    fs::write(path, "<region> sample=*silence key=60\n").expect("write the patch");
}

#[test]
fn an_installed_preset_resolves_with_its_compiled_mapping() {
    let dir = tempfile::tempdir().expect("tempdir");
    // A library Sonara ships a profile for: VSCO 2 CE's keyswitched tuba, sustain on 84 (the
    // default) and staccato on 85.
    let vsco = dir.path().join("VSCO");
    write_patch(&vsco.join("Tuba-KS.sfz"));
    fs::write(
        vsco.join(crate::instruments::MANIFEST_FILE),
        json!({"version": 2, "id": "vsco2ce"}).to_string(),
    )
    .expect("write the manifest");
    // A library nothing is claimed about.
    let plain = dir.path().join("Plain");
    write_patch(&plain.join("Strings/Violin.sfz"));
    let installed =
        InstalledInstruments::from_catalog(&InstrumentCatalog::discover(&[vsco, plain]));

    let tuba = installed.resolve("vsco2ce/Tuba-KS").expect("installed");
    assert_eq!(tuba.mapping.default_target(), Some(switch(84)));
    assert_eq!(
        tuba.mapping.target(choice(Articulation::Staccato)),
        Some(switch(85))
    );
    assert_eq!(
        tuba.mapping.target(choice(Articulation::Sustain)),
        Some(switch(84))
    );
    assert_eq!(tuba.mapping.target(SamplerMapping::AUTO), None);

    let violin = installed
        .resolve("plain/Strings/Violin")
        .expect("installed");
    assert_eq!(*violin.mapping, SamplerMapping::default());
}

/// A library at `root` that holds every patch the shipped profile `text` names, each an empty
/// patch, with a manifest that names the profile's library `id`, so the profile applies to all
/// of them.
fn profiled_library(root: &Path, id: &str, text: &str) {
    fn paths(value: &serde_json::Value, found: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, field) in map {
                    match field.as_str() {
                        Some(path) if key == "path" => found.push(path.to_owned()),
                        _ => paths(field, found),
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter().for_each(|item| paths(item, found)),
            _ => {}
        }
    }
    let profile: serde_json::Value = serde_json::from_str(text).expect("a JSON profile");
    let mut found = Vec::new();
    paths(&profile, &mut found);
    assert!(found.len() > 50, "{id}: the profile names its patches");
    for path in found {
        write_patch(&root.join(path));
    }
    fs::write(
        root.join(crate::instruments::MANIFEST_FILE),
        json!({"version": 2, "id": id}).to_string(),
    )
    .expect("write the manifest");
}

#[test]
fn the_shipped_profiles_compile_their_states_as_every_cell_does() {
    // The compiled mapping of every preset of both shipped profiles equals the one computed cell
    // by cell under every mute and vibrato choice, so leaving out the states a preset does not
    // record never changes what plays (TST-10).
    let dir = tempfile::tempdir().expect("tempdir");
    let vsco = dir.path().join("VSCO");
    let vcsl = dir.path().join("VCSL");
    profiled_library(
        &vsco,
        "vsco2ce",
        include_str!("../../../profiles/vsco2ce.json"),
    );
    profiled_library(&vcsl, "vcsl", include_str!("../../../profiles/vcsl.json"));
    let catalog = InstrumentCatalog::discover(&[vsco, vcsl]);
    let mut stated = Vec::new();
    for preset in catalog.presets() {
        let legato = preset.legato.as_ref().map(|claim| claim.value);
        let compiled = targets(preset, legato);
        assert_eq!(compiled, every_cell(preset), "{}", preset.id);
        let alone: Vec<Option<ArticulationTarget>> = (0..count(ARTICULATION_CHOICES))
            .map(|index| compiled.target(index, Passage::AUTO))
            .collect();
        if compiled != TargetTable::from(alone) {
            stated.push(preset.id.to_string());
        }
    }
    assert!(catalog.presets().len() > 250, "both libraries are listed");
    // The keyswitched patches whose profile records vibrato variants switch on the state.
    for id in [
        "vsco2ce/Flute-KS",
        "vcsl/Aerophones/Reed Aerophones/Saxello - Keyswitch",
    ] {
        assert!(
            stated.iter().any(|stated| stated == id),
            "{id} in {stated:?}"
        );
    }
}

#[test]
fn an_installed_flute_switches_its_vibrato_with_the_state() {
    // VSCO 2 CE's `Flute-KS` through its shipped profile: sustain without vibrato on 36 (the
    // default), sustain on 37, sustain with vibrato on 38 and staccato on 39.
    let dir = tempfile::tempdir().expect("tempdir");
    let vsco = dir.path().join("VSCO");
    write_patch(&vsco.join("Flute-KS.sfz"));
    fs::write(
        vsco.join(crate::instruments::MANIFEST_FILE),
        json!({"version": 2, "id": "vsco2ce"}).to_string(),
    )
    .expect("write the manifest");
    let installed = InstalledInstruments::from_catalog(&InstrumentCatalog::discover(&[vsco]));
    let flute = installed.resolve("vsco2ce/Flute-KS").expect("installed");
    let sustain = choice(Articulation::Sustain);
    assert_eq!(flute.mapping.target(sustain), Some(switch(37)));
    for (state, key) in [
        (Vibrato::On, 38),
        (Vibrato::Off, 36),
        (Vibrato::Ordinary, 37),
    ] {
        assert_eq!(
            flute
                .mapping
                .target_under(sustain, passage(None, Some(state))),
            Some(switch(key)),
            "{state:?}"
        );
    }
    assert_eq!(
        flute.mapping.target_under(
            choice(Articulation::Staccato),
            passage(None, Some(Vibrato::On))
        ),
        Some(switch(39))
    );
    assert_eq!(
        flute
            .mapping
            .target_under(SamplerMapping::AUTO, passage(Some(Mute::Generic), None)),
        None,
        "the flute records no mute"
    );
}

// Multi-patch sets.

/// The target played by patch `member` of a set, reached there with `selector`.
fn in_member(member: u8, selector: Option<Selector>) -> ArticulationTarget {
    ArticulationTarget {
        member,
        ..ArticulationTarget::select(selector)
    }
}

/// An articulation of a set, played by the patch `path`.
fn played_by(articulation: Articulation, via: ArticulationVia, path: &str) -> ArticulationEntry {
    ArticulationEntry {
        member: Some(path.to_owned()),
        ..entry(ArticulationName::Vocabulary(articulation), via)
    }
}

/// A violin set of three patches, in this order: sustain (`Sus.sfz`), tremolo (`Trem.sfz`) and
/// a keyswitched patch of short notes (`Short.sfz`) with spiccato on key 24 and pizzicato on
/// key 25, whose entry spells the patch in other letters. The default is the articulation
/// `default` names.
fn violin_set(default_articulation: Articulation) -> InstrumentPreset {
    let library = LibraryId::parse("lib").expect("a valid library id");
    let member = |path: &str| SetMember {
        path: path.to_owned(),
        patch_path: PathBuf::from(format!("/libraries/lib/{path}")),
        availability: Availability::Available,
        facts: None,
    };
    let entries = [
        played_by(Articulation::Sustain, ArticulationVia::Patch, "Sus.sfz"),
        played_by(Articulation::Tremolo, ArticulationVia::Patch, "Trem.sfz"),
        played_by(
            Articulation::Spiccato,
            ArticulationVia::Keyswitch { key: 24 },
            "Short.sfz",
        ),
        played_by(
            Articulation::Pizzicato,
            ArticulationVia::Keyswitch { key: 25 },
            "short.SFZ",
        ),
    ]
    .into_iter()
    .map(|entry| {
        if entry.name == ArticulationName::Vocabulary(default_articulation) {
            default(entry)
        } else {
            entry
        }
    })
    .collect();
    InstrumentPreset {
        id: InstrumentId::of_set(&library, "violins"),
        kind: PresetKind::Set {
            members: vec![member("Sus.sfz"), member("Trem.sfz"), member("Short.sfz")],
        },
        ..mapped(violin(), entries)
    }
}

#[test]
fn a_set_compiles_each_articulation_to_the_patch_that_plays_it() {
    let mapping = sampler_mapping(&violin_set(Articulation::Sustain));
    assert_eq!(mapping.default_target(), Some(in_member(0, None)));
    assert_eq!(mapping.default_member(), 0);
    for (articulation, target) in [
        (Articulation::Sustain, in_member(0, None)),
        (Articulation::Tremolo, in_member(1, None)),
        (
            Articulation::Spiccato,
            in_member(2, Some(Selector::Keyswitch(24))),
        ),
        // The patch is matched ignoring case, as discovery matches paths.
        (
            Articulation::Pizzicato,
            in_member(2, Some(Selector::Keyswitch(25))),
        ),
        // Not mapped: the nearest it plays, spiccato, in its patch.
        (
            Articulation::Staccato,
            in_member(2, Some(Selector::Keyswitch(24))),
        ),
    ] {
        assert_eq!(
            mapping.target(choice(articulation)),
            Some(target),
            "{articulation:?}"
        );
    }
    assert_eq!(mapping.target(SamplerMapping::AUTO), None);
}

#[test]
fn a_set_plays_its_ordinary_sound_in_the_patch_of_its_default() {
    // The default in the second patch: `ordinario`, and so `auto`, play that patch.
    let mapping = sampler_mapping(&violin_set(Articulation::Tremolo));
    assert_eq!(mapping.default_target(), Some(in_member(1, None)));
    assert_eq!(mapping.default_member(), 1);
    // An overlap legato joins the ordinary sound in its patch.
    let mut legato = violin_set(Articulation::Tremolo);
    legato.legato = Some(profile(LegatoMode::Overlap {
        crossfade_ms: 90,
        start_offset_ms: 60,
        gap_ms: 0,
    }));
    assert_eq!(
        sampler_mapping(&legato).target(choice(Articulation::Legato)),
        Some(ArticulationTarget {
            legato: true,
            ..in_member(1, None)
        })
    );
}

#[test]
fn an_articulation_of_a_patch_the_set_does_not_hold_gets_no_target() {
    // TST-03: an entry that names a patch outside the set's patches, or none, selects nothing,
    // so its choice plays as `auto` rather than in a wrong patch.
    let mut preset = violin_set(Articulation::Sustain);
    let Some(Claim {
        value: ArticulationList::Mapped(entries),
        ..
    }) = preset.articulation_map.as_mut()
    else {
        panic!("mapped");
    };
    entries.push(played_by(
        Articulation::ColLegno,
        ArticulationVia::Patch,
        "Gone.sfz",
    ));
    entries.push(entry(
        ArticulationName::Vocabulary(Articulation::Harmonics),
        ArticulationVia::Patch,
    ));
    let mapping = sampler_mapping(&preset);
    assert_eq!(mapping.target(choice(Articulation::ColLegno)), None);
    assert_eq!(mapping.target(choice(Articulation::Harmonics)), None);
    assert_eq!(
        mapping.target(choice(Articulation::Tremolo)),
        Some(in_member(1, None))
    );
}

#[test]
fn an_installed_set_resolves_to_its_patches_in_the_order_of_its_targets() {
    // VSCO 2 CE's trumpet set through its shipped profile: sustain (the default), sustain with
    // vibrato, staccato, and sustain with a straight and with a harmon mute, each a patch.
    let dir = tempfile::tempdir().expect("tempdir");
    let vsco = dir.path().join("VSCO");
    profiled_library(
        &vsco,
        "vsco2ce",
        include_str!("../../../profiles/vsco2ce.json"),
    );
    let installed = InstalledInstruments::from_catalog(&InstrumentCatalog::discover(
        std::slice::from_ref(&vsco),
    ));
    assert!(
        installed.ids().iter().any(|id| id == "vsco2ce/@trumpet"),
        "the set is offered"
    );
    let trumpet = installed.resolve("vsco2ce/@trumpet").expect("installed");
    assert_eq!(trumpet.id, "vsco2ce/@trumpet");
    let patches: Vec<PathBuf> = [
        "TrumpetSus.sfz",
        "TrumpetSusVib.sfz",
        "TrumpetStac.sfz",
        "TrumpetStraightMuteSus.sfz",
        "TrumpetHarmonMuteSus.sfz",
    ]
    .iter()
    .map(|path| vsco.join(path))
    .collect();
    assert_eq!(trumpet.patches, patches);
    let sustain = choice(Articulation::Sustain);
    let mapping = &trumpet.mapping;
    assert_eq!(mapping.default_target(), Some(in_member(0, None)));
    assert_eq!(mapping.target(sustain), Some(in_member(0, None)));
    assert_eq!(
        mapping.target(choice(Articulation::Staccato)),
        Some(in_member(2, None))
    );
    for (state, member) in [
        (passage(None, Some(Vibrato::On)), 1),
        (passage(Some(Mute::Generic), None), 3),
        (passage(Some(Mute::Harmon), None), 4),
    ] {
        assert_eq!(
            mapping.target_under(sustain, state),
            Some(in_member(member, None)),
            "{state:?}"
        );
    }
    // Its patches stay presets of their own, each patch 0 of itself.
    let alone = installed.resolve("vsco2ce/TrumpetStac").expect("installed");
    assert_eq!(alone.patches, [vsco.join("TrumpetStac.sfz")]);
}
