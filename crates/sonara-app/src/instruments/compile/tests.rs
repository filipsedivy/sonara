//! Compiling sampler mappings over hand-built presets (TST-08), and through the installed
//! instruments of a library on disk. The oracle for a choice the preset does not map is the
//! substitution data of the degradation chain: a staccato on a violin that plays spiccato is
//! played as spiccato, a legato without a legato claim as a sustain.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use sonara_core::plugins::{ParamKind, SAMPLER_AUTO, SAMPLER_ID, SAMPLER_ORDINARIO};
use sonara_dsp::bank::{ArticulationTarget, DynamicsDrive, LegatoSpec, SamplerMapping, Selector};
use sonara_midi::orchestration::{Articulation, InstrumentType, Mute, Vibrato};

use super::{LEADING_CHOICES, sampler_mapping};
use crate::instruments::manifest::{
    ArticulationEntry, ArticulationList, ArticulationName, ArticulationVia, DynamicsMode,
    KeymapEntry, LegatoMode,
};
use crate::instruments::model::{
    Availability, Claim, ClaimSource, InstrumentClaim, InstrumentId, InstrumentPreset, LibraryId,
    PresetKind,
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
            selector: None,
            legato: false,
        })
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
