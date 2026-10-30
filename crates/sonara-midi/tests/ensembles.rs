//! Contract tests of the ensemble templates through the public API of the orchestration metadata.

use sonara_midi::orchestration::{
    ENSEMBLES, Ensemble, EnsembleError, EnsembleId, EnsembleSpec, InstrumentType, Pan, Part,
    PartOrder, PlayerRange, Players, Provenance, Seating, StringSection, StringSize, ensemble,
    ensembles, instrument,
};

const REQ: bool = true;
const OPT: bool = false;

const ESTIMATE: Provenance = Provenance::Estimate {
    reason: "test fixture",
};

/// Whether two pans agree; the data writes two-decimal values.
fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

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

fn get(id: &str) -> &'static Ensemble {
    ensemble(id).unwrap_or_else(|| panic!("{id} is a template"))
}

/// The players of a part in the notation of the roster tables: `1`, `16-20`, the section's name
/// for a string section, or `by <slug>` for a doubling.
fn players(part: &Part) -> String {
    match part.players {
        Players::Own(range) if range.min() == range.max() => range.min().to_string(),
        Players::Own(range) => format!("{}-{}", range.min(), range.max()),
        Players::Section(section) => match section {
            StringSection::ViolinsI => "violins I",
            StringSection::ViolinsII => "violins II",
            StringSection::Violas => "violas",
            StringSection::Cellos => "cellos",
            StringSection::Basses => "basses",
        }
        .to_owned(),
        Players::Doubling { by } => format!("by {by}"),
    }
}

/// Every template's id, written out independently of the enum.
const IDS: [&str; 13] = [
    "classical_orchestra",
    "beethoven_orchestra",
    "romantic_orchestra",
    "late_romantic_orchestra",
    "film_orchestra",
    "chamber_orchestra",
    "string_orchestra",
    "big_band",
    "choir_and_orchestra",
    "string_quartet",
    "wind_quintet",
    "brass_quintet",
    "piano_trio",
];

#[test]
fn ensemble_ids_match_the_expected_ids_one_to_one() {
    let ids: Vec<&str> = EnsembleId::ALL.iter().map(|id| id.id()).collect();
    assert_eq!(ids, IDS);
    for (id, template) in EnsembleId::ALL.into_iter().zip(ensembles()) {
        assert_eq!(EnsembleId::from_id(id.id()), Some(id));
        assert_eq!(template.id(), id);
        assert_eq!(id.get().id(), id);
        assert_eq!(ensemble(id.id()).map(Ensemble::id), Some(id));
    }
    assert_eq!(ENSEMBLES.len(), IDS.len());
}

#[test]
fn ensemble_lookup_is_strict() {
    // TST-03: a display name, another case or a near spelling is not an id.
    for text in [
        "Romantic orchestra",
        "romantic",
        "ROMANTIC_ORCHESTRA",
        "romantic-orchestra",
        " romantic_orchestra",
        "",
    ] {
        assert_eq!(EnsembleId::from_id(text), None, "{text:?}");
        assert!(ensemble(text).is_none(), "{text:?}");
    }
    let romantic = get("romantic_orchestra");
    assert_eq!(romantic.name(), "Romantic orchestra");
    assert_eq!(
        romantic.part("horn_3").map(|part| part.name),
        Some("Horn 3")
    );
    assert!(romantic.part("Horn 3").is_none());
    assert!(romantic.part("horn_5").is_none());
    assert_eq!(
        romantic.string_size("medium").map(|size| size.counts),
        Some([12, 10, 8, 6, 4])
    );
    assert!(romantic.string_size("Medium").is_none());
}

type Row = (&'static str, &'static str, &'static str, bool);

/// The parts of every template as `(slug, instrument id, players, required)`, written from the
/// scorings and handbooks the templates cite, with the decisions their provenance states: the
/// stand-ins, the doublers and the parts left out because no instrument type covers them. The
/// third value says whether the five string sections follow, and whether the basses are required.
const ROSTERS: [(&str, &[Row], Option<bool>); 13] = [
    (
        "classical_orchestra",
        &[
            ("flute_1", "flute", "1", REQ),
            ("flute_2", "flute", "1", OPT),
            ("oboe_1", "oboe", "1", REQ),
            ("oboe_2", "oboe", "1", REQ),
            ("clarinet_1", "clarinet_a", "1", OPT),
            ("clarinet_2", "clarinet_a", "1", OPT),
            ("bassoon_1", "bassoon", "1", REQ),
            ("bassoon_2", "bassoon", "1", REQ),
            ("horn_1", "horn_f", "1", REQ),
            ("horn_2", "horn_f", "1", REQ),
            ("trumpet_1", "trumpet_c", "1", OPT),
            ("trumpet_2", "trumpet_c", "1", OPT),
            ("timpani", "timpani", "1", OPT),
        ],
        Some(REQ),
    ),
    (
        "beethoven_orchestra",
        &[
            ("piccolo", "piccolo", "1", OPT),
            ("flute_1", "flute", "1", REQ),
            ("flute_2", "flute", "1", REQ),
            ("oboe_1", "oboe", "1", REQ),
            ("oboe_2", "oboe", "1", REQ),
            ("clarinet_1", "clarinet_bb", "1", REQ),
            ("clarinet_2", "clarinet_bb", "1", REQ),
            ("bassoon_1", "bassoon", "1", REQ),
            ("bassoon_2", "bassoon", "1", REQ),
            ("contrabassoon", "contrabassoon", "1", OPT),
            ("horn_1", "horn_f", "1", REQ),
            ("horn_2", "horn_f", "1", REQ),
            ("horn_3", "horn_f", "1", OPT),
            ("horn_4", "horn_f", "1", OPT),
            ("trumpet_1", "trumpet_c", "1", REQ),
            ("trumpet_2", "trumpet_c", "1", REQ),
            ("alto_trombone", "trombone_tenor", "1", OPT),
            ("tenor_trombone", "trombone_tenor", "1", OPT),
            ("bass_trombone", "trombone_bass", "1", OPT),
            ("timpani", "timpani", "1", REQ),
            ("bass_drum", "bass_drum", "1", OPT),
            ("cymbals", "clash_cymbals", "1", OPT),
            ("triangle", "triangle", "1", OPT),
        ],
        Some(REQ),
    ),
    (
        "romantic_orchestra",
        &[
            ("flute_1", "flute", "1", REQ),
            ("flute_2", "flute", "1", REQ),
            ("flute_3", "flute", "1", OPT),
            ("piccolo", "piccolo", "by flute_3", OPT),
            ("oboe_1", "oboe", "1", REQ),
            ("oboe_2", "oboe", "1", REQ),
            ("clarinet_1", "clarinet_bb", "1", REQ),
            ("clarinet_2", "clarinet_bb", "1", REQ),
            ("bassoon_1", "bassoon", "1", REQ),
            ("bassoon_2", "bassoon", "1", REQ),
            ("contrabassoon", "contrabassoon", "1", OPT),
            ("horn_1", "horn_f", "1", REQ),
            ("horn_2", "horn_f", "1", REQ),
            ("horn_3", "horn_f", "1", REQ),
            ("horn_4", "horn_f", "1", REQ),
            ("trumpet_1", "trumpet_bb", "1", REQ),
            ("trumpet_2", "trumpet_bb", "1", REQ),
            ("trombone_1", "trombone_tenor", "1", REQ),
            ("trombone_2", "trombone_tenor", "1", REQ),
            ("trombone_3", "trombone_bass", "1", REQ),
            ("tuba", "tuba", "1", OPT),
            ("timpani", "timpani", "1", REQ),
            ("bass_drum", "bass_drum", "1", OPT),
            ("cymbals", "clash_cymbals", "1", OPT),
            ("tam_tam", "tam_tam", "by bass_drum", OPT),
            ("harp", "harp", "1", OPT),
        ],
        Some(REQ),
    ),
    (
        "late_romantic_orchestra",
        &[
            ("piccolo", "piccolo", "1", REQ),
            ("flute_1", "flute", "1", REQ),
            ("flute_2", "flute", "1", REQ),
            ("flute_3", "flute", "1", REQ),
            ("flute_4", "flute", "1", OPT),
            ("oboe_1", "oboe", "1", REQ),
            ("oboe_2", "oboe", "1", REQ),
            ("oboe_3", "oboe", "1", REQ),
            ("oboe_4", "oboe", "1", OPT),
            ("english_horn", "english_horn", "1", REQ),
            ("clarinet_eb", "clarinet_eb", "1", REQ),
            ("clarinet_1", "clarinet_bb", "1", REQ),
            ("clarinet_2", "clarinet_bb", "1", REQ),
            ("clarinet_3", "clarinet_bb", "1", OPT),
            ("bass_clarinet", "bass_clarinet", "1", REQ),
            ("bassoon_1", "bassoon", "1", REQ),
            ("bassoon_2", "bassoon", "1", REQ),
            ("bassoon_3", "bassoon", "1", REQ),
            ("bassoon_4", "bassoon", "1", OPT),
            ("contrabassoon", "contrabassoon", "1", REQ),
            ("horn_1", "horn_f", "1", REQ),
            ("horn_2", "horn_f", "1", REQ),
            ("horn_3", "horn_f", "1", REQ),
            ("horn_4", "horn_f", "1", REQ),
            ("horn_5", "horn_f", "1", REQ),
            ("horn_6", "horn_f", "1", REQ),
            ("horn_7", "horn_f", "1", REQ),
            ("horn_8", "horn_f", "1", REQ),
            ("wagner_tuba_tenor_1", "wagner_tuba_tenor", "by horn_5", OPT),
            ("wagner_tuba_tenor_2", "wagner_tuba_tenor", "by horn_6", OPT),
            ("wagner_tuba_bass_1", "wagner_tuba_bass", "by horn_7", OPT),
            ("wagner_tuba_bass_2", "wagner_tuba_bass", "by horn_8", OPT),
            ("trumpet_1", "trumpet_bb", "1", REQ),
            ("trumpet_2", "trumpet_bb", "1", REQ),
            ("trumpet_3", "trumpet_bb", "1", REQ),
            ("trumpet_4", "trumpet_bb", "1", REQ),
            ("trumpet_5", "trumpet_bb", "1", OPT),
            ("trumpet_6", "trumpet_bb", "1", OPT),
            ("trombone_1", "trombone_tenor", "1", REQ),
            ("trombone_2", "trombone_tenor", "1", REQ),
            ("trombone_3", "trombone_bass", "1", REQ),
            ("trombone_4", "trombone_bass", "1", OPT),
            ("tenor_tuba", "euphonium", "1", OPT),
            ("tuba", "tuba", "1", REQ),
            ("timpani_1", "timpani", "1", REQ),
            ("timpani_2", "timpani", "1", OPT),
            ("bass_drum", "bass_drum", "1", REQ),
            ("snare_drum", "snare_drum", "1", REQ),
            ("cymbals", "clash_cymbals", "1", REQ),
            ("suspended_cymbal", "suspended_cymbal", "by cymbals", REQ),
            ("tam_tam", "tam_tam", "by bass_drum", REQ),
            ("triangle", "triangle", "by snare_drum", REQ),
            ("glockenspiel", "glockenspiel", "1", REQ),
            ("xylophone", "xylophone", "by glockenspiel", REQ),
            ("tubular_bells", "tubular_bells", "by glockenspiel", REQ),
            ("harp_1", "harp", "1", REQ),
            ("harp_2", "harp", "1", REQ),
            ("harp_3", "harp", "1", OPT),
            ("harp_4", "harp", "1", OPT),
            ("celesta", "celesta", "1", OPT),
        ],
        Some(REQ),
    ),
    (
        "film_orchestra",
        &[
            ("flute_1", "flute", "1", REQ),
            ("flute_2", "flute", "1", REQ),
            ("flute_3", "flute", "1", OPT),
            ("piccolo", "piccolo", "by flute_3", OPT),
            ("alto_flute", "alto_flute", "by flute_2", OPT),
            ("oboe_1", "oboe", "1", REQ),
            ("oboe_2", "oboe", "1", REQ),
            ("english_horn", "english_horn", "1", OPT),
            ("clarinet_1", "clarinet_bb", "1", REQ),
            ("clarinet_2", "clarinet_bb", "1", REQ),
            ("bass_clarinet", "bass_clarinet", "1", OPT),
            ("bassoon_1", "bassoon", "1", REQ),
            ("bassoon_2", "bassoon", "1", REQ),
            ("contrabassoon", "contrabassoon", "1", OPT),
            ("horn_1", "horn_f", "1", REQ),
            ("horn_2", "horn_f", "1", REQ),
            ("horn_3", "horn_f", "1", REQ),
            ("horn_4", "horn_f", "1", REQ),
            ("horn_5", "horn_f", "1", REQ),
            ("horn_6", "horn_f", "1", REQ),
            ("horn_7", "horn_f", "1", OPT),
            ("horn_8", "horn_f", "1", OPT),
            ("trumpet_1", "trumpet_bb", "1", REQ),
            ("trumpet_2", "trumpet_bb", "1", REQ),
            ("trumpet_3", "trumpet_bb", "1", REQ),
            ("trombone_1", "trombone_tenor", "1", REQ),
            ("trombone_2", "trombone_tenor", "1", REQ),
            ("bass_trombone_1", "trombone_bass", "1", REQ),
            ("bass_trombone_2", "trombone_bass", "1", OPT),
            ("tuba", "tuba", "1", REQ),
            ("cimbasso", "cimbasso", "by tuba", OPT),
            ("timpani", "timpani", "1", REQ),
            ("bass_drum", "bass_drum", "1", REQ),
            ("snare_drum", "snare_drum", "1", REQ),
            ("cymbals", "clash_cymbals", "1", REQ),
            ("suspended_cymbal", "suspended_cymbal", "by cymbals", REQ),
            ("tam_tam", "tam_tam", "by bass_drum", REQ),
            ("glockenspiel", "glockenspiel", "by snare_drum", OPT),
            ("xylophone", "xylophone", "by cymbals", OPT),
            ("tubular_bells", "tubular_bells", "by bass_drum", OPT),
            ("crotales", "crotales", "by snare_drum", OPT),
            ("harp", "harp", "1", REQ),
            ("celesta", "celesta", "by piano", OPT),
            ("piano", "piano_concert_grand", "1", OPT),
            ("choir_sopranos", "choir_soprano", "16-20", OPT),
            ("choir_altos", "choir_alto", "16-20", OPT),
            ("choir_tenors", "choir_tenor", "16-20", OPT),
            ("choir_basses", "choir_bass", "16-20", OPT),
        ],
        Some(REQ),
    ),
    (
        "chamber_orchestra",
        &[
            ("flute_1", "flute", "1", REQ),
            ("flute_2", "flute", "1", REQ),
            ("oboe_1", "oboe", "1", REQ),
            ("oboe_2", "oboe", "1", REQ),
            ("clarinet_1", "clarinet_bb", "1", OPT),
            ("clarinet_2", "clarinet_bb", "1", OPT),
            ("bassoon_1", "bassoon", "1", REQ),
            ("bassoon_2", "bassoon", "1", REQ),
            ("horn_1", "horn_f", "1", REQ),
            ("horn_2", "horn_f", "1", REQ),
            ("trumpet_1", "trumpet_c", "1", OPT),
            ("trumpet_2", "trumpet_c", "1", OPT),
            ("timpani", "timpani", "1", OPT),
            ("harpsichord", "harpsichord", "1", OPT),
        ],
        Some(REQ),
    ),
    ("string_orchestra", &[], Some(OPT)),
    (
        "big_band",
        &[
            ("alto_sax_1", "saxophone_alto", "1", REQ),
            ("soprano_sax", "saxophone_soprano", "by alto_sax_1", OPT),
            ("alto_sax_2", "saxophone_alto", "1", REQ),
            ("flute", "flute", "by alto_sax_2", OPT),
            ("piccolo", "piccolo", "by alto_sax_2", OPT),
            ("tenor_sax_1", "saxophone_tenor", "1", REQ),
            ("clarinet", "clarinet_bb", "by tenor_sax_1", OPT),
            ("tenor_sax_2", "saxophone_tenor", "1", REQ),
            ("baritone_sax", "saxophone_baritone", "1", REQ),
            ("bass_clarinet", "bass_clarinet", "by baritone_sax", OPT),
            ("trumpet_1", "trumpet_bb", "1", REQ),
            ("trumpet_2", "trumpet_bb", "1", REQ),
            ("flugelhorn", "flugelhorn", "by trumpet_2", OPT),
            ("trumpet_3", "trumpet_bb", "1", REQ),
            ("trumpet_4", "trumpet_bb", "1", REQ),
            ("cornet", "cornet", "by trumpet_4", OPT),
            ("trombone_1", "trombone_tenor", "1", REQ),
            ("trombone_2", "trombone_tenor", "1", REQ),
            ("trombone_3", "trombone_tenor", "1", REQ),
            ("bass_trombone", "trombone_bass", "1", REQ),
            ("piano", "piano_concert_grand", "1", REQ),
            ("bass", "double_bass", "1", REQ),
            ("vibraphone", "vibraphone", "1", OPT),
        ],
        None,
    ),
    (
        "choir_and_orchestra",
        &[
            ("piccolo", "piccolo", "1", REQ),
            ("flute_1", "flute", "1", REQ),
            ("flute_2", "flute", "1", REQ),
            ("oboe_1", "oboe", "1", REQ),
            ("oboe_2", "oboe", "1", REQ),
            ("clarinet_1", "clarinet_bb", "1", REQ),
            ("clarinet_2", "clarinet_bb", "1", REQ),
            ("bassoon_1", "bassoon", "1", REQ),
            ("bassoon_2", "bassoon", "1", REQ),
            ("contrabassoon", "contrabassoon", "1", OPT),
            ("horn_1", "horn_f", "1", REQ),
            ("horn_2", "horn_f", "1", REQ),
            ("horn_3", "horn_f", "1", REQ),
            ("horn_4", "horn_f", "1", REQ),
            ("trumpet_1", "trumpet_bb", "1", REQ),
            ("trumpet_2", "trumpet_bb", "1", REQ),
            ("trombone_1", "trombone_tenor", "1", REQ),
            ("trombone_2", "trombone_tenor", "1", REQ),
            ("trombone_3", "trombone_bass", "1", REQ),
            ("tuba", "tuba", "1", REQ),
            ("timpani", "timpani", "1", REQ),
            ("harp_1", "harp", "1", REQ),
            ("harp_2", "harp", "1", OPT),
            ("organ", "organ_pipe", "1", OPT),
            ("soprano_solo", "voice_soprano", "1", REQ),
            ("baritone_solo", "voice_baritone", "1", REQ),
            ("choir_sopranos", "choir_soprano", "16-20", REQ),
            ("choir_altos", "choir_alto", "16-20", REQ),
            ("choir_tenors", "choir_tenor", "16-20", REQ),
            ("choir_basses", "choir_bass", "16-20", REQ),
        ],
        Some(REQ),
    ),
    (
        "string_quartet",
        &[
            ("violin_1", "violin", "1", REQ),
            ("violin_2", "violin", "1", REQ),
            ("viola", "viola", "1", REQ),
            ("cello", "cello", "1", REQ),
        ],
        None,
    ),
    (
        "wind_quintet",
        &[
            ("flute", "flute", "1", REQ),
            ("oboe", "oboe", "1", REQ),
            ("clarinet", "clarinet_bb", "1", REQ),
            ("horn", "horn_f", "1", REQ),
            ("bassoon", "bassoon", "1", REQ),
        ],
        None,
    ),
    (
        "brass_quintet",
        &[
            ("trumpet_1", "trumpet_bb", "1", REQ),
            ("trumpet_2", "trumpet_bb", "1", REQ),
            ("horn", "horn_f", "1", REQ),
            ("trombone", "trombone_tenor", "1", REQ),
            ("tuba", "tuba", "1", REQ),
        ],
        None,
    ),
    (
        "piano_trio",
        &[
            ("violin", "violin", "1", REQ),
            ("cello", "cello", "1", REQ),
            ("piano", "piano_concert_grand", "1", REQ),
        ],
        None,
    ),
];

#[test]
fn rosters_match_the_researched_tables() {
    // TST-10: the tables above are written from the cited scorings, independently of the data.
    let strings = |basses_required: bool| -> [Row; 5] {
        [
            ("violins_1", "violin", "violins I", REQ),
            ("violins_2", "violin", "violins II", REQ),
            ("violas", "viola", "violas", REQ),
            ("cellos", "cello", "cellos", REQ),
            ("basses", "double_bass", "basses", basses_required),
        ]
    };
    let ids: Vec<&str> = ROSTERS.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(ids, IDS);
    for (id, rows, basses_required) in ROSTERS {
        let mut expected = rows.to_vec();
        expected.extend(basses_required.map(strings).into_iter().flatten());
        let actual: Vec<(&str, &str, String, bool)> = get(id)
            .parts()
            .iter()
            .map(|part| {
                (
                    part.slug,
                    part.instrument.id(),
                    players(part),
                    part.required,
                )
            })
            .collect();
        let expected: Vec<(&str, &str, String, bool)> = expected
            .into_iter()
            .map(|(slug, instrument, players, required)| {
                (slug, instrument, players.to_owned(), required)
            })
            .collect();
        assert_eq!(actual, expected, "{id}");
    }
}

/// The parts and the people of `template`: required ones, then all, with its default strings.
fn counts(template: &Ensemble) -> ((usize, u32), (usize, u32)) {
    let size = template.default_string_size();
    let people = |part: &Part| {
        u32::from(
            part.players
                .count(size)
                .expect("every section has a size")
                .max(),
        )
    };
    let required: Vec<&Part> = template
        .parts()
        .iter()
        .filter(|part| part.required)
        .collect();
    (
        (
            required.len(),
            required.iter().map(|part| people(part)).sum(),
        ),
        (
            template.parts().len(),
            template.parts().iter().map(people).sum(),
        ),
    )
}

#[test]
fn track_and_player_counts_match_the_sources() {
    // TST-10: one track per part. The Classical orchestra has 12 required tracks and 18 with every
    // option, 30 and 36 players with its default strings (23 of them strings); the chamber
    // orchestra 13 and 19 tracks, 29 and 35 players.
    assert_eq!(counts(get("classical_orchestra")), ((12, 30), (18, 36)));
    assert_eq!(counts(get("chamber_orchestra")), ((13, 29), (19, 35)));
    // The late-Romantic orchestra has 49 required tracks and 70 with every option, of which the
    // bass trumpet, the contrabass trombone, the cowbells, the hammer and the Rute have no part.
    let late = counts(get("late_romantic_orchestra"));
    assert_eq!((late.0.0, late.1.0), (49, 70 - 5));
    // A big band has 17 players; the guitar and the drum kit have no part.
    assert_eq!(counts(get("big_band")).0.1, 17 - 2);
    // One player to a part in the chamber groups.
    for (id, parts, people) in [
        ("string_quartet", 4, 4),
        ("wind_quintet", 5, 5),
        ("brass_quintet", 5, 5),
        ("piano_trio", 3, 3),
    ] {
        assert_eq!(counts(get(id)), ((parts, people), (parts, people)), "{id}");
    }
}

#[test]
fn string_sizes_match_the_sources() {
    // TST-10: the counts as the sources state them, with the totals they give (60 for the full
    // orchestra as recorded in place, 84 for Gurre-Lieder, 12 and 21 for the chamber sizes).
    let expected: [(&str, [u8; 5], u16); 7] = [
        ("full", [16, 14, 12, 10, 8], 60),
        ("medium", [12, 10, 8, 6, 4], 40),
        ("small", [8, 6, 4, 3, 2], 23),
        ("die_walkure", [16, 16, 12, 12, 8], 64),
        ("gurre_lieder", [20, 20, 16, 16, 12], 84),
        ("chamber", [6, 5, 4, 4, 2], 21),
        ("small_chamber", [4, 3, 2, 2, 1], 12),
    ];
    let mut seen = Vec::new();
    for template in ensembles() {
        for size in template.string_sizes() {
            let (_, counts, players) = expected
                .iter()
                .find(|(slug, _, _)| *slug == size.slug)
                .unwrap_or_else(|| panic!("{}: unexpected size {}", template.name(), size.slug));
            assert_eq!(size.counts, *counts, "{}", size.slug);
            assert_eq!(size.players(), *players, "{}", size.slug);
            for (section, count) in StringSection::ALL.into_iter().zip(counts) {
                assert_eq!(size.count(section), *count);
            }
            assert!(is_traceable(size.provenance), "{}", size.slug);
            seen.push(size.slug);
        }
    }
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), expected.len(), "every size is used");

    // The default size and the sizes offered, per template.
    let offered = |id: &str| -> Vec<&str> {
        get(id)
            .string_sizes()
            .iter()
            .map(|size| size.slug)
            .collect()
    };
    assert_eq!(offered("classical_orchestra"), ["small", "medium", "full"]);
    assert_eq!(offered("beethoven_orchestra"), ["medium", "small", "full"]);
    assert_eq!(offered("romantic_orchestra"), ["full", "medium", "small"]);
    assert_eq!(
        offered("late_romantic_orchestra"),
        ["die_walkure", "full", "gurre_lieder"]
    );
    assert_eq!(offered("film_orchestra"), ["full", "medium"]);
    assert_eq!(offered("chamber_orchestra"), ["chamber", "small_chamber"]);
    assert_eq!(
        offered("string_orchestra"),
        ["chamber", "small_chamber", "full"]
    );
    assert_eq!(offered("choir_and_orchestra"), ["full", "medium", "small"]);
    for id in [
        "big_band",
        "string_quartet",
        "wind_quintet",
        "brass_quintet",
        "piano_trio",
    ] {
        assert_eq!(offered(id), Vec::<&str>::new(), "{id}");
        assert!(get(id).default_string_size().is_none(), "{id}");
    }
    assert_eq!(
        get("romantic_orchestra")
            .default_string_size()
            .map(|size| size.slug),
        Some("full")
    );
}

/// The stage place of an orchestra's part as `(American, antiphonal)`, written from the seating
/// table: strings by section, the other parts by instrument.
#[expect(
    clippy::match_same_arms,
    reason = "one arm per row of the seating table"
)]
fn stage(part: &Part) -> (f32, f32) {
    if let Players::Section(section) = part.players {
        return match section {
            StringSection::ViolinsI => (-0.55, -0.55),
            StringSection::ViolinsII => (-0.30, 0.55),
            StringSection::Violas => (0.10, 0.20),
            StringSection::Cellos => (0.35, -0.20),
            StringSection::Basses => (0.45, -0.35),
        };
    }
    let same = |pan| (pan, pan);
    match part.instrument.id() {
        "piccolo" | "bass_clarinet" => same(-0.20),
        "flute" | "alto_flute" | "clarinet_eb" => same(-0.15),
        "oboe" | "bassoon" | "trumpet_bb" | "trumpet_c" | "choir_tenor" => same(0.10),
        "english_horn" | "contrabassoon" => same(0.20),
        "clarinet_bb" | "clarinet_a" | "timpani" | "choir_alto" => same(-0.10),
        "horn_f" | "wagner_tuba_tenor" | "wagner_tuba_bass" => (-0.35, 0.35),
        "trombone_tenor" => same(0.25),
        "trombone_bass" | "choir_bass" => same(0.35),
        "euphonium" | "tuba" | "cimbasso" => same(0.40),
        "bass_drum" | "snare_drum" | "clash_cymbals" | "suspended_cymbal" | "tam_tam"
        | "triangle" | "glockenspiel" | "xylophone" | "tubular_bells" | "crotales" => same(0.20),
        "harp" => same(-0.60),
        "celesta" | "piano_concert_grand" | "harpsichord" | "organ_pipe" => same(-0.45),
        "voice_soprano" | "voice_baritone" => same(0.0),
        "choir_soprano" => same(-0.35),
        other => panic!("the seating table places no {other}"),
    }
}

#[test]
fn pans_follow_the_seating_table() {
    let orchestras = [
        "classical_orchestra",
        "beethoven_orchestra",
        "romantic_orchestra",
        "late_romantic_orchestra",
        "film_orchestra",
        "chamber_orchestra",
        "string_orchestra",
        "choir_and_orchestra",
    ];
    for id in orchestras {
        for part in get(id).parts() {
            let (american, antiphonal) = stage(part);
            assert!(
                close(part.pan.american, american) && close(part.pan.antiphonal, antiphonal),
                "{id} {}: {:?}",
                part.slug,
                part.pan
            );
            // A dry mix sounds no wider than a stage.
            assert!(part.pan.american.abs() <= 0.6 && part.pan.antiphonal.abs() <= 0.6);
        }
    }
    // The big band: leads in the middle, solo chairs on the rhythm section's side, a doubled
    // instrument where its player sits.
    let big_band: [(&str, f32); 23] = [
        ("alto_sax_1", 0.0),
        ("soprano_sax", 0.0),
        ("alto_sax_2", -0.25),
        ("flute", -0.25),
        ("piccolo", -0.25),
        ("tenor_sax_1", -0.50),
        ("clarinet", -0.50),
        ("tenor_sax_2", 0.25),
        ("baritone_sax", 0.50),
        ("bass_clarinet", 0.50),
        ("trumpet_1", -0.10),
        ("trumpet_2", -0.30),
        ("flugelhorn", -0.30),
        ("trumpet_3", 0.10),
        ("trumpet_4", 0.30),
        ("cornet", 0.30),
        ("trombone_1", -0.10),
        ("trombone_2", -0.30),
        ("trombone_3", 0.10),
        ("bass_trombone", 0.30),
        ("piano", -0.45),
        ("bass", 0.0),
        ("vibraphone", 0.20),
    ];
    let band = get("big_band");
    assert_eq!(band.parts().len(), big_band.len());
    for (slug, pan) in big_band {
        let part = band.part(slug).expect("listed part");
        assert!(close(part.pan.at(None), pan), "{slug}");
        for seating in Seating::ALL {
            assert!(close(part.pan.at(Some(seating)), pan), "{slug}");
        }
    }
    // The chamber groups stay centred.
    for id in [
        "string_quartet",
        "wind_quintet",
        "brass_quintet",
        "piano_trio",
    ] {
        for part in get(id).parts() {
            assert_eq!(part.pan, Pan::CENTRE, "{id} {}", part.slug);
        }
    }
}

#[test]
fn seatings_offer_a_default_and_place_each_part() {
    let seatings = |id: &str| get(id).seatings().to_vec();
    let antiphonal_first = vec![Seating::Antiphonal, Seating::American];
    let american_first = vec![Seating::American, Seating::Antiphonal];
    for id in [
        "classical_orchestra",
        "beethoven_orchestra",
        "late_romantic_orchestra",
    ] {
        assert_eq!(seatings(id), antiphonal_first, "{id}");
        assert_eq!(get(id).default_seating(), Some(Seating::Antiphonal));
    }
    for id in [
        "romantic_orchestra",
        "film_orchestra",
        "chamber_orchestra",
        "string_orchestra",
        "choir_and_orchestra",
    ] {
        assert_eq!(seatings(id), american_first, "{id}");
        assert_eq!(get(id).default_seating(), Some(Seating::American));
    }
    for id in [
        "big_band",
        "string_quartet",
        "wind_quintet",
        "brass_quintet",
        "piano_trio",
    ] {
        assert_eq!(seatings(id), Vec::<Seating>::new(), "{id}");
        assert_eq!(get(id).default_seating(), None);
    }
    // The second violins change sides with the seating; the flutes do not.
    let romantic = get("romantic_orchestra");
    let violins_2 = romantic.part("violins_2").expect("listed part").pan;
    assert!(close(violins_2.at(Some(Seating::American)), -0.30));
    assert!(close(violins_2.at(Some(Seating::Antiphonal)), 0.55));
    assert!(close(violins_2.at(None), -0.30));
    let flute = romantic.part("flute_1").expect("listed part").pan;
    assert!(close(flute.at(Some(Seating::Antiphonal)), -0.15));
    assert_eq!(
        Pan::same(0.4),
        Pan {
            american: 0.4,
            antiphonal: 0.4
        }
    );
}

#[test]
fn doublings_are_played_by_one_player_of_another_instrument() {
    let mut doublings = 0;
    for template in ensembles() {
        for part in template.parts() {
            let Players::Doubling { by } = part.players else {
                assert!(template.player_of(part).is_none());
                continue;
            };
            doublings += 1;
            let player = template
                .player_of(part)
                .unwrap_or_else(|| panic!("{} {}: no player", template.name(), part.slug));
            assert_eq!(player.slug, by);
            assert_eq!(player.players, Players::Own(PlayerRange::new(1, 1)));
            assert_ne!(player.instrument, part.instrument, "{}", part.slug);
            assert!(player.required || !part.required, "{}", part.slug);
        }
    }
    // Romantic 2, late-Romantic 9, film 10, big band 7.
    assert_eq!(doublings, 2 + 9 + 10 + 7, "every doubling is checked");
}

#[test]
fn parts_are_unique_and_named_within_each_template() {
    for template in ensembles() {
        let parts = template.parts();
        for (i, part) in parts.iter().enumerate() {
            assert!(!part.name.trim().is_empty() && !part.short.trim().is_empty());
            for other in &parts[..i] {
                assert_ne!(other.slug, part.slug, "{}", template.name());
                assert_ne!(other.name, part.name, "{}", template.name());
                assert_ne!(
                    other.short,
                    part.short,
                    "{}: {}",
                    template.name(),
                    part.short
                );
            }
            // Every part's instrument has a record, and an alternative is another instrument of
            // its family.
            assert!(instrument(part.instrument.id()).is_some());
            for alternative in part.alternatives {
                assert_ne!(*alternative, part.instrument);
                assert_eq!(alternative.family(), part.instrument.family());
            }
        }
        assert_ne!(template.name().trim(), "");
        assert!(template.description().ends_with('.'), "{}", template.name());
    }
    // The alternatives the scorings name.
    let alternatives = |id: &str, slug: &str| {
        get(id)
            .part(slug)
            .expect("listed part")
            .alternatives
            .to_vec()
    };
    assert_eq!(
        alternatives("romantic_orchestra", "clarinet_1"),
        [InstrumentType::ClarinetA]
    );
    assert_eq!(
        alternatives("beethoven_orchestra", "clarinet_2"),
        [InstrumentType::ClarinetA]
    );
    assert_eq!(
        alternatives("brass_quintet", "trumpet_1"),
        [InstrumentType::TrumpetC]
    );
    assert_eq!(alternatives("classical_orchestra", "horn_1"), []);
}

/// Whether `parts` follow full-score order: the instrument types never decrease, except a
/// doubling directly below its player's part (or below another doubling placed so).
fn in_full_score_order(parts: &[Part]) -> bool {
    let mut previous: Option<InstrumentType> = None;
    let mut player: Option<&str> = None;
    for part in parts {
        if let (Players::Doubling { by }, Some(player)) = (part.players, player)
            && by == player
        {
            continue;
        }
        if previous.is_some_and(|previous| part.instrument < previous) {
            return false;
        }
        previous = Some(part.instrument);
        player = Some(part.slug);
    }
    true
}

#[test]
fn full_score_templates_follow_score_order_and_editions_do_not() {
    for template in ensembles() {
        let expected = match template.id() {
            EnsembleId::BigBand
            | EnsembleId::WindQuintet
            | EnsembleId::BrassQuintet
            | EnsembleId::PianoTrio => PartOrder::Edition,
            _ => PartOrder::FullScore,
        };
        assert_eq!(template.order(), expected, "{}", template.name());
        // An edition's order is not the full score's, or it would be marked as one.
        assert_eq!(
            in_full_score_order(template.parts()),
            expected == PartOrder::FullScore,
            "{}",
            template.name()
        );
    }
    // The orders the editions print.
    let slugs = |id: &str| -> Vec<&str> { get(id).parts().iter().map(|part| part.slug).collect() };
    assert_eq!(
        slugs("wind_quintet"),
        ["flute", "oboe", "clarinet", "horn", "bassoon"]
    );
    assert_eq!(
        slugs("brass_quintet"),
        ["trumpet_1", "trumpet_2", "horn", "trombone", "tuba"]
    );
    assert_eq!(slugs("piano_trio"), ["violin", "cello", "piano"]);
}

#[test]
fn every_template_names_its_sources_and_what_it_leaves_out() {
    for template in ensembles() {
        assert!(!template.provenance().is_empty(), "{}", template.name());
        assert!(
            template
                .provenance()
                .iter()
                .any(|provenance| matches!(provenance, Provenance::Cited { .. })),
            "{} cites a source",
            template.name()
        );
        for provenance in template.provenance() {
            assert!(
                is_traceable(*provenance),
                "{}: {provenance:?}",
                template.name()
            );
        }
    }
    // The parts no instrument type covers are named where they are left out.
    let estimates = |id: &str| -> String {
        get(id)
            .provenance()
            .iter()
            .filter_map(|provenance| match provenance {
                Provenance::Estimate { reason } => Some(*reason),
                Provenance::Cited { .. } => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let late = estimates("late_romantic_orchestra");
    for left_out in [
        "bass trumpet",
        "contrabass trombone",
        "cowbells",
        "hammer",
        "Rute",
    ] {
        assert!(late.contains(left_out), "{left_out}");
    }
    assert!(estimates("film_orchestra").contains("synthesizer"));
    let band = estimates("big_band");
    assert!(band.contains("guitar") && band.contains("drum kit"));
    // A stand-in says what it stands in for.
    let classical = get("classical_orchestra");
    assert!(
        classical
            .part("horn_1")
            .expect("listed part")
            .note
            .contains("natural horn")
    );
    assert!(
        get("late_romantic_orchestra")
            .part("tubular_bells")
            .expect("listed part")
            .note
            .contains("deep bells")
    );
}

#[test]
fn players_count_by_string_size() {
    let full = get("romantic_orchestra").default_string_size();
    assert_eq!(
        Players::Section(StringSection::Cellos).count(full),
        Some(PlayerRange::new(10, 10))
    );
    assert_eq!(Players::Section(StringSection::Cellos).count(None), None);
    assert_eq!(
        Players::Own(PlayerRange::new(16, 20)).count(None),
        Some(PlayerRange::new(16, 20))
    );
    assert_eq!(
        Players::Doubling { by: "flute_3" }.count(full),
        Some(PlayerRange::NOT_USED)
    );
}

// Fixtures for the validator.

fn leak<T>(items: Vec<T>) -> &'static [T] {
    Box::leak(items.into_boxed_slice())
}

fn part(slug: &'static str, instrument: InstrumentType) -> Part {
    Part {
        slug,
        name: slug,
        short: slug,
        instrument,
        alternatives: &[],
        players: Players::Own(PlayerRange::new(1, 1)),
        required: REQ,
        pan: Pan::CENTRE,
        note: "",
    }
}

fn doubling(slug: &'static str, instrument: InstrumentType, by: &'static str) -> Part {
    Part {
        players: Players::Doubling { by },
        required: OPT,
        ..part(slug, instrument)
    }
}

fn section(slug: &'static str, section: StringSection) -> Part {
    Part {
        players: Players::Section(section),
        ..part(slug, section.instrument_type())
    }
}

const SIZE: StringSize = StringSize {
    slug: "full",
    name: "Full",
    counts: [16, 14, 12, 10, 8],
    provenance: ESTIMATE,
};

/// A valid template: a flute with a piccolo doubling directly below it, a horn, and the first
/// violins.
fn valid() -> EnsembleSpec {
    EnsembleSpec {
        name: "Fixture",
        description: "A fixture.",
        order: PartOrder::FullScore,
        seatings: &[Seating::American, Seating::Antiphonal],
        string_sizes: &[SIZE],
        parts: leak(vec![
            part("flute", InstrumentType::Flute),
            doubling("piccolo", InstrumentType::Piccolo, "flute"),
            part("horn", InstrumentType::HornF),
            section("violins_1", StringSection::ViolinsI),
        ]),
        provenance: &[ESTIMATE],
    }
}

fn with_parts(parts: Vec<Part>) -> EnsembleSpec {
    EnsembleSpec {
        parts: leak(parts),
        ..valid()
    }
}

fn check(spec: EnsembleSpec) -> Result<Ensemble, EnsembleError> {
    Ensemble::try_new(EnsembleId::RomanticOrchestra, spec)
}

#[test]
fn validator_accepts_a_valid_template() {
    let spec = valid();
    let template = check(spec).expect("a valid template");
    assert_eq!(template.id(), EnsembleId::RomanticOrchestra);
    assert_eq!(template.spec(), spec);
    assert_eq!(template.name(), "Fixture");
    assert_eq!(template.default_seating(), Some(Seating::American));
    assert_eq!(template.default_string_size(), Some(&SIZE));
    let piccolo = template.part("piccolo").expect("listed part");
    assert_eq!(
        template.player_of(piccolo).map(|part| part.slug),
        Some("flute")
    );
    // The same parts in an edition's order, or a seating-free template whose parts sit the same
    // in both seatings.
    let violins_first = vec![
        section("violins_1", StringSection::ViolinsI),
        part("flute", InstrumentType::Flute),
    ];
    assert!(
        check(EnsembleSpec {
            order: PartOrder::Edition,
            ..with_parts(violins_first)
        })
        .is_ok()
    );
    assert!(
        check(EnsembleSpec {
            seatings: &[],
            string_sizes: &[],
            parts: leak(vec![Part {
                pan: Pan::same(-0.5),
                ..part("flute", InstrumentType::Flute)
            }]),
            ..valid()
        })
        .is_ok()
    );
}

#[test]
fn validator_rejects_broken_names_and_provenance() {
    // TST-03: each failure with the error the documentation gives.
    assert_eq!(
        check(EnsembleSpec {
            name: " ",
            ..valid()
        }),
        Err(EnsembleError::EmptyName)
    );
    assert_eq!(
        check(EnsembleSpec {
            description: "",
            ..valid()
        }),
        Err(EnsembleError::EmptyName)
    );
    assert_eq!(
        check(EnsembleSpec {
            provenance: &[],
            ..valid()
        }),
        Err(EnsembleError::EmptyProvenance)
    );
    assert_eq!(
        check(EnsembleSpec {
            provenance: &[Provenance::Cited { source: "  " }],
            ..valid()
        }),
        Err(EnsembleError::EmptyProvenance)
    );
    let unsourced = StringSize {
        provenance: Provenance::Estimate { reason: "" },
        ..SIZE
    };
    assert_eq!(
        check(EnsembleSpec {
            string_sizes: leak(vec![unsourced]),
            ..valid()
        }),
        Err(EnsembleError::EmptyProvenance)
    );
}

#[test]
fn validator_rejects_broken_string_sizes_and_seatings() {
    let sizes = |sizes: Vec<StringSize>| EnsembleSpec {
        string_sizes: leak(sizes),
        ..valid()
    };
    for broken in [
        vec![StringSize {
            slug: "Full",
            ..SIZE
        }],
        vec![StringSize { name: " ", ..SIZE }],
        vec![StringSize {
            counts: [16, 14, 12, 10, 0],
            ..SIZE
        }],
        vec![
            SIZE,
            StringSize {
                name: "Other",
                ..SIZE
            },
        ],
    ] {
        let slug = broken.last().expect("a size").slug;
        assert_eq!(
            check(sizes(broken)),
            Err(EnsembleError::InvalidStringSize(slug))
        );
    }
    assert_eq!(
        check(EnsembleSpec {
            parts: leak(vec![part("flute", InstrumentType::Flute)]),
            ..valid()
        }),
        Err(EnsembleError::UnusedStringSizes)
    );
    assert_eq!(
        check(EnsembleSpec {
            seatings: &[Seating::American, Seating::American],
            ..valid()
        }),
        Err(EnsembleError::DuplicateSeating)
    );
}

/// Asserts that each case, with the first violins added so that the sizes are used, is rejected
/// with its error.
fn assert_rejected(cases: Vec<(Vec<Part>, EnsembleError)>) {
    for (parts, error) in cases {
        let mut parts = parts;
        parts.push(section("violins_1", StringSection::ViolinsI));
        assert_eq!(check(with_parts(parts)).map(|_| ()), Err(error));
    }
}

#[test]
fn validator_rejects_broken_part_names_and_alternatives() {
    let flute = part("flute", InstrumentType::Flute);
    let cases: Vec<(Vec<Part>, EnsembleError)> = vec![
        (
            vec![part("Flute 1", InstrumentType::Flute)],
            EnsembleError::InvalidPartSlug("Flute 1"),
        ),
        (
            vec![Part { name: " ", ..flute }],
            EnsembleError::EmptyPartName("flute"),
        ),
        (
            vec![Part { short: "", ..flute }],
            EnsembleError::EmptyPartName("flute"),
        ),
        (
            vec![
                flute,
                Part {
                    name: "Other",
                    ..flute
                },
            ],
            EnsembleError::DuplicatePart("flute"),
        ),
        (
            vec![
                flute,
                Part {
                    slug: "flute_2",
                    ..flute
                },
            ],
            EnsembleError::DuplicatePart("flute_2"),
        ),
        (
            vec![Part {
                alternatives: &[InstrumentType::Flute],
                ..flute
            }],
            EnsembleError::InvalidAlternative("flute"),
        ),
        (
            vec![Part {
                alternatives: &[InstrumentType::Violin],
                ..flute
            }],
            EnsembleError::InvalidAlternative("flute"),
        ),
        (
            vec![Part {
                alternatives: &[InstrumentType::AltoFlute, InstrumentType::AltoFlute],
                ..flute
            }],
            EnsembleError::InvalidAlternative("flute"),
        ),
    ];
    assert_rejected(cases);
}

#[test]
fn validator_rejects_broken_players_and_pans() {
    let flute = part("flute", InstrumentType::Flute);
    let cases: Vec<(Vec<Part>, EnsembleError)> = vec![
        (
            vec![Part {
                players: Players::Own(PlayerRange::NOT_USED),
                ..flute
            }],
            EnsembleError::NoPlayers("flute"),
        ),
        (
            vec![Part {
                players: Players::Own(PlayerRange::new(0, 2)),
                ..flute
            }],
            EnsembleError::NoPlayers("flute"),
        ),
        (
            vec![Part {
                pan: Pan::same(1.5),
                ..flute
            }],
            EnsembleError::PanOutOfRange("flute"),
        ),
        (
            vec![Part {
                pan: Pan {
                    american: -1.01,
                    antiphonal: 0.0,
                },
                ..flute
            }],
            EnsembleError::PanOutOfRange("flute"),
        ),
        (
            vec![Part {
                pan: Pan::same(f32::NAN),
                ..flute
            }],
            EnsembleError::PanOutOfRange("flute"),
        ),
    ];
    assert_rejected(cases);
    // The pans may lie anywhere in -1..=1.
    let edges = vec![
        Part {
            pan: Pan {
                american: -1.0,
                antiphonal: 1.0,
            },
            ..flute
        },
        section("violins_1", StringSection::ViolinsI),
    ];
    assert!(check(with_parts(edges)).is_ok());
}

#[test]
fn validator_rejects_broken_sections() {
    let violas_as_violins = Part {
        instrument: InstrumentType::Viola,
        ..section("violins_1", StringSection::ViolinsI)
    };
    assert_eq!(
        check(with_parts(vec![violas_as_violins])).map(|_| ()),
        Err(EnsembleError::InvalidSection("violins_1"))
    );
    let twice = vec![
        section("violins_1", StringSection::ViolinsI),
        section("violins_1b", StringSection::ViolinsI),
    ];
    assert_eq!(
        check(with_parts(twice)).map(|_| ()),
        Err(EnsembleError::InvalidSection("violins_1"))
    );
    assert_eq!(
        check(EnsembleSpec {
            string_sizes: &[],
            ..valid()
        })
        .map(|_| ()),
        Err(EnsembleError::InvalidSection("violins_1"))
    );
}

#[test]
fn validator_rejects_broken_doublings() {
    let violins = section("violins_1", StringSection::ViolinsI);
    let flute = part("flute", InstrumentType::Flute);
    let piccolo = |by| doubling("piccolo", InstrumentType::Piccolo, by);
    let cases: Vec<Vec<Part>> = vec![
        // No such part.
        vec![flute, piccolo("flute_3"), violins],
        // A doubling of a doubling.
        vec![
            flute,
            doubling("alto_flute", InstrumentType::AltoFlute, "flute"),
            piccolo("alto_flute"),
            violins,
        ],
        // A string section changes no instrument.
        vec![
            flute,
            doubling("piccolo", InstrumentType::Piccolo, "violins_1"),
            violins,
        ],
        // Two players.
        vec![
            Part {
                players: Players::Own(PlayerRange::new(2, 2)),
                ..flute
            },
            piccolo("flute"),
            violins,
        ],
        // The same instrument.
        vec![
            flute,
            doubling("flute_2", InstrumentType::Flute, "flute"),
            violins,
        ],
        // Required while its player's part is optional.
        vec![
            Part {
                required: OPT,
                ..flute
            },
            Part {
                required: REQ,
                ..piccolo("flute")
            },
            violins,
        ],
    ];
    for parts in cases {
        let doubled = parts
            .iter()
            .rev()
            .find(|part| matches!(part.players, Players::Doubling { .. }))
            .expect("a doubling")
            .slug;
        assert_eq!(
            check(with_parts(parts.clone())).map(|_| ()),
            Err(EnsembleError::InvalidDoubling(doubled)),
            "{parts:?}"
        );
    }
}

#[test]
fn validator_rejects_seating_free_pans_that_depend_on_a_seating() {
    let parts = leak(vec![Part {
        pan: Pan {
            american: -0.3,
            antiphonal: 0.3,
        },
        ..part("horn", InstrumentType::HornF)
    }]);
    let spec = EnsembleSpec {
        seatings: &[],
        string_sizes: &[],
        parts,
        ..valid()
    };
    assert_eq!(
        check(spec).map(|_| ()),
        Err(EnsembleError::PanDependsOnSeating("horn"))
    );
    // With a choice of seating the same part is valid.
    assert!(
        check(EnsembleSpec {
            seatings: &[Seating::Antiphonal],
            ..spec
        })
        .is_ok()
    );
}

#[test]
fn validator_rejects_templates_without_a_required_part() {
    let all_optional = vec![
        Part {
            required: OPT,
            ..part("flute", InstrumentType::Flute)
        },
        Part {
            required: OPT,
            ..section("violins_1", StringSection::ViolinsI)
        },
    ];
    assert_eq!(
        check(with_parts(all_optional)).map(|_| ()),
        Err(EnsembleError::NoRequiredPart)
    );
    assert_eq!(
        check(EnsembleSpec {
            string_sizes: &[],
            parts: &[],
            ..valid()
        })
        .map(|_| ()),
        Err(EnsembleError::NoRequiredPart)
    );
}

#[test]
fn validator_rejects_parts_out_of_score_order() {
    let violins = section("violins_1", StringSection::ViolinsI);
    // The violins above the flute.
    assert_eq!(
        check(with_parts(vec![
            violins,
            part("flute", InstrumentType::Flute)
        ]))
        .map(|_| ()),
        Err(EnsembleError::OutOfScoreOrder("flute"))
    );
    // A doubling away from its player's part keeps score order: here the piccolo below the horn.
    let away = vec![
        part("flute", InstrumentType::Flute),
        part("horn", InstrumentType::HornF),
        doubling("piccolo", InstrumentType::Piccolo, "flute"),
        violins,
    ];
    assert_eq!(
        check(with_parts(away)).map(|_| ()),
        Err(EnsembleError::OutOfScoreOrder("piccolo"))
    );
    // Below its player's part, after another doubling of the same player, it may break it, and
    // the next part is compared with the player's instrument.
    let below = vec![
        part("flute", InstrumentType::Flute),
        doubling("alto_flute", InstrumentType::AltoFlute, "flute"),
        doubling("piccolo", InstrumentType::Piccolo, "flute"),
        part("oboe", InstrumentType::Oboe),
        violins,
    ];
    assert!(check(with_parts(below)).is_ok());
    // Equal types may follow each other.
    let pair = vec![
        part("flute_1", InstrumentType::Flute),
        part("flute_2", InstrumentType::Flute),
        violins,
    ];
    assert!(check(with_parts(pair)).is_ok());
}

#[test]
#[should_panic(expected = "invalid ensemble template")]
fn new_panics_on_an_invalid_template() {
    let _ = Ensemble::new(
        EnsembleId::RomanticOrchestra,
        EnsembleSpec {
            name: "",
            ..valid()
        },
    );
}
