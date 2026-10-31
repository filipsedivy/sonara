//! Tests of the coverage counts: the shipped profiles' numbers pinned, every class checked
//! against what the degradation chain answers for a copy of the profiled libraries (TST-10), and
//! the rules for verdicts, sizes, key maps, sets and neighbours on profiles written for the test.

use std::path::{Path, PathBuf};

use sonara_midi::orchestration::fallback::Manner;
use sonara_midi::orchestration::{Forms, InstrumentType};

use super::{Coverage, CoverageCounts, ProfileCoverage};
use crate::instruments::manifest::{ArticulationList, EntryTarget};
use crate::instruments::profiles::{self, Profile, shipped};
use crate::instruments::{
    ChainPreferences, EnsembleKind, InstrumentCatalog, SoundRequest, Step, choose_sound,
};

/// The ids of the types whose coverage `keep` accepts, in score order.
fn ids(coverage: &ProfileCoverage, keep: fn(Coverage) -> bool) -> Vec<&'static str> {
    coverage
        .rows()
        .iter()
        .filter(|(_, class)| keep(*class))
        .map(|(instrument, _)| instrument.id())
        .collect()
}

/// Every patch a profile names, its own entries, its sets' members and its detect paths,
/// written below `root` as a patch of one region, so that discovery finds a copy of the library
/// with nothing missing.
fn install(profile: &Profile, root: &Path) {
    let mut paths = profile.detect.clone();
    for entry in &profile.claims.entries {
        match &entry.target {
            EntryTarget::Path(path) => paths.push(path.clone()),
            EntryTarget::Set(_) => {
                if let Some(ArticulationList::Mapped(list)) = entry.articulations.value() {
                    paths.extend(list.iter().filter_map(|a| a.member.clone()));
                }
            }
        }
    }
    for path in paths {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&file, "<region> sample=a.wav\n").expect("write");
    }
}

/// The size a part of `instrument` asks for: one player or a section where scores give it
/// only one of them, either where they give it both.
fn wish(instrument: InstrumentType) -> Option<EnsembleKind> {
    match instrument.get().ensemble().map(|profile| profile.forms) {
        Some(Forms::Solo) => Some(EnsembleKind::Solo),
        Some(Forms::Section) => Some(EnsembleKind::Section),
        Some(Forms::Both) | None => None,
    }
}

/// Checks every row of the coverage by `profiles` against the chain's answer, over a copy of
/// every profiled library, for a part of the type in the size it asks for.
fn assert_the_chain_agrees(profiles: &[Profile]) {
    let dir = tempfile::tempdir().expect("tempdir");
    let roots: Vec<PathBuf> = profiles
        .iter()
        .map(|profile| {
            let root = dir.path().join(profile.id.as_str());
            install(profile, &root);
            root
        })
        .collect();
    let catalog = InstrumentCatalog::discover_with(&roots, profiles);
    assert_eq!(catalog.issues(), []);

    for (instrument, coverage) in ProfileCoverage::of(profiles).rows() {
        let request = SoundRequest {
            ensemble: wish(*instrument),
            ..SoundRequest::new(*instrument)
        };
        let plan = choose_sound(catalog.presets(), &request, ChainPreferences::default());
        let what = format!("{}: {coverage:?} {:?}", instrument.id(), plan.source);
        match coverage {
            Coverage::Exact => assert_eq!(plan.step, Step::Exact, "{what}"),
            Coverage::OtherEnsemble => assert_eq!(plan.step, Step::EnsembleSwap, "{what}"),
            Coverage::Substitute(neighbour) => {
                assert_eq!(plan.step, Step::FamilyNeighbour, "{what}");
                assert_eq!(plan.neighbour.as_ref(), Some(neighbour), "{what}");
                assert_eq!(plan.confirm.is_some(), neighbour.asks_first, "{what}");
            }
            Coverage::Unsampled => {
                assert!(matches!(plan.step, Step::Builtin | Step::Silence), "{what}");
            }
        }
    }
}

/// A profile written for a test, with library ID `id`, from its entries.
fn profile(id: &str, entries: &serde_json::Value) -> Profile {
    let json = serde_json::json!({
        "version": 2, "id": id, "name": id, "detect": [format!("{id}.sfz")],
        "instruments": entries,
    });
    profiles::parse(&json.to_string()).expect("a valid profile")
}

/// The types the shipped profiles play themselves: every one in a size scores give it.
#[test]
fn the_shipped_profiles_play_these_types() {
    let coverage = ProfileCoverage::shipped();
    assert_eq!(
        ids(&coverage, |class| class == Coverage::Exact),
        [
            "piccolo",
            "flute",
            "oboe",
            "clarinet_bb",
            "saxophone_soprano",
            "saxophone_tenor",
            "bassoon",
            "horn_f",
            "trumpet_bb",
            "trombone_tenor",
            "tuba",
            "timpani",
            "bass_drum",
            "snare_drum",
            "clash_cymbals",
            "suspended_cymbal",
            "tam_tam",
            "triangle",
            "tambourine",
            "wood_block",
            "sleigh_bells",
            "shaker",
            "whip",
            "glockenspiel",
            "xylophone",
            "vibraphone",
            "marimba",
            "tubular_bells",
            "harp",
            "piano_studio_grand",
            "piano_upright",
            "harpsichord",
            "organ_pipe",
            "violin",
            "viola",
            "cello",
            "double_bass",
        ]
    );
    // Every profiled patch of a type is of a size scores give it.
    assert_eq!(
        ids(&coverage, |class| class == Coverage::OtherEnsemble),
        [] as [&str; 0]
    );
}

/// The types only a neighbour of the shipped profiles plays, with the neighbour and its manner.
#[test]
fn a_neighbour_plays_these_types() {
    let substitutes: Vec<(&str, &str, Manner)> = ProfileCoverage::shipped()
        .rows()
        .iter()
        .filter_map(|(instrument, class)| match class {
            Coverage::Substitute(neighbour) => {
                // None of them needs the user's word first, and each suits any passage.
                assert!(!neighbour.asks_first, "{}", instrument.id());
                assert_eq!(neighbour.only, None, "{}", instrument.id());
                Some((instrument.id(), neighbour.instrument.id(), neighbour.manner))
            }
            _ => None,
        })
        .collect();
    let written = Manner::AsWritten;
    assert_eq!(
        substitutes,
        [
            ("alto_flute", "flute", written),
            ("bass_flute", "flute", written),
            ("oboe_damore", "oboe", written),
            ("english_horn", "oboe", written),
            ("clarinet_eb", "clarinet_bb", written),
            ("clarinet_a", "clarinet_bb", written),
            ("bass_clarinet", "bassoon", written),
            ("saxophone_alto", "saxophone_tenor", written),
            ("saxophone_baritone", "saxophone_tenor", written),
            ("contrabassoon", "bassoon", written),
            ("wagner_tuba_tenor", "horn_f", written),
            ("wagner_tuba_bass", "horn_f", written),
            ("trumpet_piccolo", "trumpet_bb", written),
            ("trumpet_c", "trumpet_bb", written),
            ("cornet", "trumpet_bb", written),
            ("flugelhorn", "trumpet_bb", Manner::Softer),
            ("trombone_bass", "trombone_tenor", written),
            ("euphonium", "trombone_tenor", written),
            ("cimbasso", "tuba", written),
            ("castanets", "wood_block", written),
            ("temple_blocks", "wood_block", written),
            ("maracas", "shaker", written),
            ("crotales", "glockenspiel", written),
            ("carillon", "tubular_bells", written),
            ("guitar_classical", "harp", written),
            ("lute", "harp", written),
            ("celesta", "piano_upright", written),
            ("piano_concert_grand", "piano_studio_grand", written),
            ("piano_felt", "piano_upright", Manner::Softer),
            ("piano_honky_tonk", "piano_upright", written),
            ("clavichord", "harpsichord", Manner::Softer),
            ("clavinet", "harpsichord", written),
            ("organ_hammond", "organ_pipe", written),
            ("accordion", "organ_pipe", written),
        ]
    );
}

/// The types left without any sampled sound, which a built-in sound or silence plays, and the
/// counts: the loose one and its parts.
#[test]
fn these_types_have_no_sampled_sound() {
    let coverage = ProfileCoverage::shipped();
    let unsampled: Vec<&str> = coverage.unsampled().map(InstrumentType::id).collect();
    assert_eq!(
        unsampled,
        [
            "mandolin",
            "piano_prepared",
            "electric_piano_rhodes",
            "electric_piano_wurlitzer",
            "voice_soprano",
            "voice_mezzo_soprano",
            "voice_alto",
            "voice_tenor",
            "voice_baritone",
            "voice_bass",
            "choir_soprano",
            "choir_alto",
            "choir_tenor",
            "choir_bass",
        ]
    );

    let counts = coverage.counts();
    assert_eq!(
        counts,
        CoverageCounts {
            exact: 37,
            other_ensemble: 0,
            substitutes: 34,
            unsampled: 14,
        }
    );
    assert_eq!(counts.loose(), 71);
    assert_eq!(counts.total(), InstrumentType::ALL.len());
}

#[test]
fn the_shipped_coverage_is_what_the_chain_answers() {
    assert_the_chain_agrees(shipped());
}

/// Each rule on a profile written for the test, with the same answer from the chain: a verdict
/// of `not` plays nothing, alone or through a set's member; the other size counts apart; a key
/// map and a set play what they claim; a neighbour stands in only for a type with nothing.
#[test]
fn verdicts_sizes_key_maps_sets_and_neighbours_decide_the_class() {
    let lib = profile(
        "lib",
        &serde_json::json!([
            // The right size: scores give the oboe one player or a section.
            {"path": "Oboe.sfz", "instrument": "oboe", "ensemble": {"kind": "solo"},
             "verdict": {"state": "partly", "lost": ["sw_default"]}},
            // A choir part sung by one singer is the other size...
            {"path": "Soprano.sfz", "instrument": "choir_soprano", "ensemble": {"kind": "solo"},
             "verdict": {"state": "as_is", "lost": []}},
            // ...and so is a section of piccolos, which scores give one player each...
            {"path": "Piccolos.sfz", "instrument": "piccolo", "ensemble": {"kind": "section"},
             "verdict": {"state": "as_is", "lost": []}},
            // ...but a patch that states no size counts as the right one.
            {"path": "Tuba.sfz", "instrument": "tuba", "verdict": {"state": "as_is", "lost": []}},
            // A key map plays each instrument it has keys for.
            {"path": "Kit.sfz", "family": "percussion",
             "keymap": [{"keys": [38], "instrument": "snare_drum"},
                        {"keys": [54], "instrument": "tambourine"}],
             "verdict": {"state": "as_is", "lost": []}},
            // A patch that does not play usefully plays nothing.
            {"path": "Harp.sfz", "instrument": "harp",
             "verdict": {"state": "not", "lost": ["no regions"]}},
            // A set plays through members that claim no instrument themselves...
            {"path": "Trumpet/Sus.sfz", "instrument": null, "family": "brass",
             "verdict": {"state": "as_is", "lost": []}},
            {"set": "trumpet", "name": "Trumpet", "instrument": "trumpet_bb",
             "ensemble": {"kind": "solo"},
             "articulations": [{"id": "sustain", "via": "patch", "path": "Trumpet/Sus.sfz",
                                "default": true}]},
            // ...but not when a member does not play and the set has no verdict of its own.
            {"path": "Glock/Hit.sfz", "instrument": null, "family": "percussion",
             "verdict": {"state": "not", "lost": ["missing samples"]}},
            {"set": "glock", "name": "Glockenspiel", "instrument": "glockenspiel",
             "ensemble": {"kind": "solo"},
             "articulations": [{"id": "sustain", "via": "patch", "path": "Glock/Hit.sfz",
                                "default": true}]},
        ]),
    );
    let profiles = [lib];
    let coverage = ProfileCoverage::of(&profiles);
    let class = |id: &str| {
        coverage
            .rows()
            .iter()
            .find(|(instrument, _)| instrument.id() == id)
            .map_or_else(|| panic!("no row {id}"), |(_, class)| *class)
    };

    for id in ["oboe", "tuba", "snare_drum", "tambourine", "trumpet_bb"] {
        assert_eq!(class(id), Coverage::Exact, "{id}");
    }
    for id in ["choir_soprano", "piccolo"] {
        assert_eq!(class(id), Coverage::OtherEnsemble, "{id}");
    }
    // A type with nothing of its own takes its first neighbour that has a patch, in the
    // neighbours' order; a neighbour with only patches judged `not` does not count.
    let neighbour = |id: &str| match class(id) {
        Coverage::Substitute(neighbour) => neighbour.instrument.id(),
        other => panic!("{id}: {other:?}"),
    };
    assert_eq!(neighbour("english_horn"), "oboe");
    assert_eq!(neighbour("trumpet_c"), "trumpet_bb");
    assert_eq!(neighbour("cimbasso"), "tuba");
    for id in ["harp", "glockenspiel", "mandolin"] {
        assert_eq!(class(id), Coverage::Unsampled, "{id}");
    }

    let counts = coverage.counts();
    assert_eq!(counts.other_ensemble, 2);
    assert_eq!(counts.total(), InstrumentType::ALL.len());
    assert_eq!(
        counts.loose(),
        counts.exact + counts.other_ensemble + counts.substitutes
    );
    assert_the_chain_agrees(&profiles);
}

/// Without a profile nothing is sampled.
#[test]
fn no_profile_samples_nothing() {
    let coverage = ProfileCoverage::of(&[]);
    assert_eq!(
        coverage.counts(),
        CoverageCounts {
            unsampled: InstrumentType::ALL.len(),
            ..CoverageCounts::default()
        }
    );
    assert_eq!(coverage.counts().loose(), 0);
    assert!(coverage.unsampled().eq(InstrumentType::ALL));
}
