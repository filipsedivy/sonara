//! Contract tests of the fallback tables through the public API of the orchestration metadata:
//! the neighbours of every instrument type and the substitutes of the original articulations.

use sonara_midi::orchestration::fallback::{
    ArticulationSubstitute, Emulation, Manner, NEIGHBOURS, Neighbour, SUBSTITUTIONS, Scope,
    StandIn, neighbours, substitutes, substitution,
};
use sonara_midi::orchestration::{
    Articulation, Axis, InstrumentType, PitchModel, PitchRange, Provenance, Vibrato,
};

/// Whether a reader can trace `provenance`: a cited source that names a URL, or an estimate with
/// a reason.
fn is_traceable(provenance: Provenance) -> bool {
    match provenance {
        Provenance::Cited { source } => {
            !source.trim().is_empty() && (source.contains("https://") || source.contains("http://"))
        }
        Provenance::Estimate { reason } => !reason.trim().is_empty(),
    }
}

/// The playable range of `instrument`, or `None` when it is unpitched.
fn playable(instrument: InstrumentType) -> Option<PitchRange> {
    let record = instrument.get().expect("every type has a record");
    match record.pitch() {
        PitchModel::Pitched(pitched) => Some(pitched.playable.range),
        PitchModel::Unpitched => None,
    }
}

fn scope(scope: Scope) -> &'static str {
    match scope {
        Scope::ShortNotes => "short notes",
        Scope::SingleLines => "single lines",
        Scope::TremoloPassages => "tremolo passages",
        Scope::Accents => "accents",
        Scope::Chords => "chords",
        Scope::Arpeggios => "arpeggios",
        Scope::SingleStrokes => "single strokes",
    }
}

fn emulation(emulation: Emulation) -> String {
    match emulation {
        Emulation::Overlap => "overlapped".to_owned(),
        Emulation::Shorten { divisor } => format!("shortened to 1/{divisor}"),
        Emulation::Accent => "accented".to_owned(),
        Emulation::Soften => "softer".to_owned(),
        Emulation::Repeat { per_whole_note } => format!("repeated 1/{per_whole_note}"),
        Emulation::Alternate { slowest, fastest } => format!("alternating {slowest}-{fastest}/s"),
    }
}

/// A neighbour in the notation of the expected table: its id, then how it plays, the passages
/// it suits and whether the user confirms it.
fn describe_neighbour(neighbour: &Neighbour) -> String {
    let mut text = neighbour.instrument.id().to_owned();
    match neighbour.manner {
        Manner::AsWritten => {}
        Manner::Softer => text.push_str(" softer"),
        Manner::With(articulation) => {
            text.push_str(" with ");
            text.push_str(articulation.id());
        }
        Manner::OnHighestNote => text.push_str(" on highest note"),
        Manner::OnLowestNote => text.push_str(" on lowest note"),
    }
    if let Some(only) = neighbour.only {
        text.push_str(" only ");
        text.push_str(scope(only));
    }
    if neighbour.asks_first {
        text.push_str(" asks");
    }
    text
}

/// A substitute in the notation of the expected table: what stands in, then the rewrite, the
/// vibrato and the passages it suits.
fn describe_substitute(substitute: ArticulationSubstitute) -> String {
    let mut text = match substitute.stand_in {
        StandIn::Articulation(articulation) => articulation.id().to_owned(),
        StandIn::OtherEnsemble => "other ensemble".to_owned(),
        StandIn::Instrument(instrument) => format!("instrument {}", instrument.id()),
    };
    if let Some(rewrite) = substitute.rewrite {
        text.push(' ');
        text.push_str(&emulation(rewrite));
    }
    match substitute.vibrato {
        None => {}
        Some(Vibrato::Ordinary) => text.push_str(" ordinary vibrato"),
        Some(Vibrato::On) => text.push_str(" vibrato on"),
        Some(Vibrato::Off) => text.push_str(" vibrato off"),
    }
    if let Some(only) = substitute.only {
        text.push_str(" only ");
        text.push_str(scope(only));
    }
    text
}

/// Every type's neighbours, best first, in score order, written out by hand independently of
/// the data.
const EXPECTED_NEIGHBOURS: [(&str, &[&str]); 85] = [
    ("piccolo", &["flute"]),
    ("flute", &["alto_flute", "piccolo"]),
    ("alto_flute", &["flute"]),
    ("bass_flute", &["alto_flute", "flute"]),
    ("oboe", &["english_horn"]),
    ("oboe_damore", &["oboe", "english_horn"]),
    ("english_horn", &["oboe", "bassoon"]),
    ("clarinet_eb", &["clarinet_bb"]),
    ("clarinet_bb", &["clarinet_a", "oboe"]),
    ("clarinet_a", &["clarinet_bb"]),
    ("bass_clarinet", &["bassoon", "clarinet_bb"]),
    ("saxophone_soprano", &["saxophone_alto", "oboe"]),
    ("saxophone_alto", &["saxophone_tenor"]),
    ("saxophone_tenor", &["saxophone_alto", "saxophone_baritone"]),
    ("saxophone_baritone", &["saxophone_tenor", "bass_clarinet"]),
    ("bassoon", &["contrabassoon", "bass_clarinet"]),
    ("contrabassoon", &["bassoon"]),
    ("horn_f", &["trombone_tenor", "euphonium"]),
    (
        "wagner_tuba_tenor",
        &["horn_f", "euphonium", "trombone_tenor"],
    ),
    ("wagner_tuba_bass", &["horn_f", "tuba"]),
    ("trumpet_piccolo", &["trumpet_bb"]),
    ("trumpet_c", &["trumpet_bb"]),
    ("trumpet_bb", &["cornet", "flugelhorn"]),
    ("cornet", &["trumpet_bb"]),
    ("flugelhorn", &["trumpet_bb softer", "horn_f"]),
    ("trombone_tenor", &["trombone_bass", "euphonium", "horn_f"]),
    ("trombone_bass", &["trombone_tenor", "tuba"]),
    ("euphonium", &["trombone_tenor", "tuba", "horn_f"]),
    ("tuba", &["trombone_bass", "cimbasso"]),
    ("cimbasso", &["tuba", "trombone_bass"]),
    ("timpani", &["bass_drum only accents asks"]),
    ("bass_drum", &["timpani on lowest note only accents"]),
    ("snare_drum", &[]),
    ("clash_cymbals", &["suspended_cymbal"]),
    ("suspended_cymbal", &["clash_cymbals only single strokes"]),
    ("tam_tam", &["suspended_cymbal with roll"]),
    (
        "triangle",
        &["crotales on highest note", "glockenspiel on highest note"],
    ),
    ("tambourine", &["sleigh_bells"]),
    ("castanets", &["wood_block"]),
    ("wood_block", &["temple_blocks"]),
    ("temple_blocks", &["wood_block"]),
    ("sleigh_bells", &["tambourine"]),
    ("maracas", &["shaker"]),
    ("shaker", &["maracas"]),
    ("whip", &[]),
    ("glockenspiel", &["celesta asks", "crotales"]),
    ("xylophone", &["marimba", "glockenspiel"]),
    ("vibraphone", &["glockenspiel", "celesta asks"]),
    ("marimba", &["xylophone", "vibraphone"]),
    ("tubular_bells", &["carillon", "glockenspiel"]),
    ("crotales", &["glockenspiel"]),
    ("carillon", &["tubular_bells"]),
    ("harp", &["piano_concert_grand softer only arpeggios asks"]),
    ("guitar_classical", &["harp", "lute"]),
    ("lute", &["guitar_classical", "harp"]),
    ("mandolin", &["guitar_classical"]),
    ("celesta", &["piano_upright", "glockenspiel asks"]),
    (
        "piano_concert_grand",
        &["piano_studio_grand", "piano_upright"],
    ),
    (
        "piano_studio_grand",
        &["piano_concert_grand", "piano_upright"],
    ),
    ("piano_upright", &["piano_studio_grand"]),
    (
        "piano_felt",
        &["piano_upright softer", "piano_studio_grand softer"],
    ),
    ("piano_prepared", &[]),
    ("piano_honky_tonk", &["piano_upright"]),
    ("harpsichord", &["clavichord", "piano_upright softer"]),
    ("clavichord", &["harpsichord softer"]),
    ("electric_piano_rhodes", &["electric_piano_wurlitzer"]),
    ("electric_piano_wurlitzer", &["electric_piano_rhodes"]),
    ("clavinet", &["harpsichord"]),
    ("organ_pipe", &["organ_hammond only chords"]),
    ("organ_hammond", &["organ_pipe"]),
    ("accordion", &["organ_pipe"]),
    ("voice_soprano", &["choir_soprano asks"]),
    (
        "voice_mezzo_soprano",
        &["choir_soprano asks", "choir_alto asks"],
    ),
    ("voice_alto", &["choir_alto asks"]),
    ("voice_tenor", &["choir_tenor asks"]),
    ("voice_baritone", &["choir_tenor asks", "choir_bass asks"]),
    ("voice_bass", &["choir_bass asks"]),
    ("choir_soprano", &[]),
    ("choir_alto", &[]),
    ("choir_tenor", &[]),
    ("choir_bass", &[]),
    ("violin", &["viola"]),
    ("viola", &["violin", "cello"]),
    ("cello", &["viola", "double_bass"]),
    ("double_bass", &["cello"]),
];

#[test]
fn every_type_has_the_neighbours_of_the_table() {
    // TST-10: the expected lists are written out by hand, in score order.
    for (place, (id, expected)) in EXPECTED_NEIGHBOURS.iter().enumerate() {
        let instrument_type = InstrumentType::from_id(id).expect("the table names known ids");
        assert_eq!(instrument_type, InstrumentType::ALL[place], "{id}");
        let actual: Vec<String> = neighbours(instrument_type)
            .iter()
            .map(describe_neighbour)
            .collect();
        assert_eq!(actual, *expected, "{id}");
    }
}

#[test]
fn neighbour_rows_are_in_score_order() {
    for (place, row) in NEIGHBOURS.iter().enumerate() {
        assert_eq!(row.instrument, InstrumentType::ALL[place]);
        assert_eq!(neighbours(row.instrument), row.neighbours);
        assert_eq!(row.check(), Ok(()), "{}", row.instrument.id());
    }
}

#[test]
fn neighbours_hold_their_invariants() {
    // TST-07: every neighbour is another type, listed once, with a traceable reason; a change of
    // family or a lost pitch is confirmed first; a pitched neighbour of an unpitched part names
    // the note it plays; two pitched instruments share notes; an articulation it plays with is
    // one its record lists.
    for instrument_type in InstrumentType::ALL {
        let list = neighbours(instrument_type);
        for (i, neighbour) in list.iter().enumerate() {
            let id = format!("{} -> {}", instrument_type.id(), neighbour.instrument.id());
            assert_ne!(neighbour.instrument, instrument_type, "{id}");
            assert!(
                list.iter()
                    .skip(i + 1)
                    .all(|other| other.instrument != neighbour.instrument),
                "{id} is listed twice"
            );
            assert!(is_traceable(neighbour.provenance), "{id}");
            if neighbour.instrument.family() != instrument_type.family() {
                assert!(neighbour.asks_first, "{id} changes family");
            }
            let part = playable(instrument_type);
            let other = playable(neighbour.instrument);
            match (part, other) {
                (Some(part), Some(other)) => assert!(
                    part.low() <= other.high() && other.low() <= part.high(),
                    "{id} shares no note"
                ),
                (Some(_), None) => assert!(neighbour.asks_first, "{id} loses the pitch"),
                (None, Some(_)) => assert!(
                    matches!(
                        neighbour.manner,
                        Manner::OnHighestNote | Manner::OnLowestNote
                    ),
                    "{id} names no note"
                ),
                (None, None) => {}
            }
            if let Manner::With(articulation) = neighbour.manner {
                assert_eq!(articulation.axis(), Axis::Primary, "{id}");
                let record = neighbour.instrument.get().expect("a record");
                assert!(record.articulations().contains(&articulation), "{id}");
            }
            assert_eq!(neighbour.check(instrument_type), Ok(()), "{id}");
        }
    }
}

#[test]
fn only_four_neighbours_cross_a_family() {
    // The neighbours stay within the family except where the instruments' sounds meet across
    // it: a piano for the harp, and the celesta's and the metallic mallets' bells. Each of them
    // is confirmed by the user first (TST-07, checked above).
    let mut crossing: Vec<(&str, &str)> = Vec::new();
    for instrument_type in InstrumentType::ALL {
        for neighbour in neighbours(instrument_type) {
            if neighbour.instrument.family() != instrument_type.family() {
                crossing.push((instrument_type.id(), neighbour.instrument.id()));
            }
        }
    }
    assert_eq!(
        crossing,
        [
            ("glockenspiel", "celesta"),
            ("vibraphone", "celesta"),
            ("harp", "piano_concert_grand"),
            ("celesta", "glockenspiel"),
        ]
    );
}

#[test]
fn some_instruments_have_no_neighbour() {
    // TST-03: no instrument comes near these, so the caller goes on to the next step.
    for id in [
        "snare_drum",
        "whip",
        "piano_prepared",
        "choir_soprano",
        "choir_alto",
        "choir_tenor",
        "choir_bass",
    ] {
        let instrument_type = InstrumentType::from_id(id).expect("a known id");
        assert!(neighbours(instrument_type).is_empty(), "{id}");
    }
}

#[test]
fn handbook_substitutes_cite_their_source() {
    let provenance = |of: &str, by: &str| {
        let of = InstrumentType::from_id(of).expect("a known id");
        neighbours(of)
            .iter()
            .find(|neighbour| neighbour.instrument.id() == by)
            .map(|neighbour| neighbour.provenance)
            .expect("a listed neighbour")
    };
    let cited = |provenance: Provenance, author: &str| match provenance {
        Provenance::Cited { source } => source.contains(author),
        Provenance::Estimate { .. } => false,
    };
    assert!(cited(
        provenance("celesta", "piano_upright"),
        "Rimsky-Korsakov"
    ));
    assert!(cited(
        provenance("harp", "piano_concert_grand"),
        "Cecil Forsyth"
    ));
    assert!(cited(
        provenance("carillon", "tubular_bells"),
        "Tubular Bells"
    ));
    assert!(cited(
        provenance("voice_baritone", "choir_bass"),
        "Rimsky-Korsakov"
    ));
    // The handbook advises against the glockenspiel for a celesta, so that one stays a judgement.
    assert!(matches!(
        provenance("celesta", "glockenspiel"),
        Provenance::Estimate { .. }
    ));
}

/// The substitutes and the emulation of every original articulation, in vocabulary order,
/// written out by hand independently of the data.
const EXPECTED_SUBSTITUTIONS: [(&str, &[&str], Option<&str>); 15] = [
    ("sustain", &["tenuto", "legato", "marcato"], None),
    (
        "legato",
        &["sustain overlapped only single lines", "tenuto"],
        Some("overlapped"),
    ),
    (
        "staccato",
        &[
            "spiccato",
            "marcato",
            "staccatissimo",
            "sustain shortened to 1/4",
        ],
        Some("shortened to 1/4"),
    ),
    (
        "spiccato",
        &["staccato", "col_legno softer", "sustain shortened to 1/4"],
        Some("shortened to 1/4"),
    ),
    ("tenuto", &["sustain", "legato"], None),
    (
        "marcato",
        &["sustain accented", "staccato only short notes"],
        Some("accented"),
    ),
    (
        "pizzicato",
        &[
            "other ensemble",
            "instrument harp only single lines",
            "staccato softer",
        ],
        None,
    ),
    ("tremolo", &["sustain", "staccato"], Some("repeated 1/32")),
    ("trill", &[], Some("alternating 12-16/s")),
    ("col_legno", &["spiccato softer", "staccato softer"], None),
    ("harmonics", &["sustain softer vibrato off"], None),
    (
        "sul_ponticello",
        &["tremolo only tremolo passages", "sustain"],
        None,
    ),
    ("con_sordino", &[], Some("softer")),
    ("flutter_tongue", &["tremolo", "sustain"], None),
    ("roll", &["tremolo"], Some("repeated 1/32")),
];

#[test]
fn original_articulations_have_the_substitutes_of_the_table() {
    // TST-10: the expected rows are written out by hand, in vocabulary order.
    for (place, (id, expected, expected_emulation)) in EXPECTED_SUBSTITUTIONS.iter().enumerate() {
        let articulation = Articulation::from_id(id).expect("the table names known ids");
        assert_eq!(articulation, Articulation::ALL[place], "{id}");
        let row = substitution(articulation).expect("an original articulation has a row");
        assert_eq!(row, &SUBSTITUTIONS[place], "{id}");
        let actual: Vec<String> = substitutes(articulation)
            .iter()
            .copied()
            .map(describe_substitute)
            .collect();
        assert_eq!(actual, *expected, "{id}");
        assert_eq!(
            row.emulation.map(emulation).as_deref(),
            *expected_emulation,
            "{id}"
        );
    }
}

#[test]
fn substitution_rows_hold_their_invariants() {
    // TST-07: a row is a traceable musical judgement; a stand-in articulation is a note's own
    // articulation, never the one asked for; every rewrite is valid.
    for row in &SUBSTITUTIONS {
        let id = row.articulation.id();
        assert!(
            matches!(row.provenance, Provenance::Estimate { .. }) && is_traceable(row.provenance),
            "{id}"
        );
        assert_eq!(row.check(), Ok(()), "{id}");
        for substitute in row.substitutes {
            if let StandIn::Articulation(stand_in) = substitute.stand_in {
                assert_ne!(stand_in, row.articulation, "{id}");
                assert_eq!(stand_in.axis(), Axis::Primary, "{id}");
            }
            if let Some(rewrite) = substitute.rewrite {
                assert_eq!(rewrite.check(), Ok(()), "{id}");
            }
        }
        if let Some(emulation) = row.emulation {
            assert_eq!(emulation.check(), Ok(()), "{id}");
        }
    }
}

#[test]
fn only_the_original_articulations_have_rows() {
    // TST-03: any other id has no row and no substitutes; one that refines an original id
    // reaches that id's row through its parent.
    for articulation in Articulation::ALL {
        assert_eq!(
            substitution(articulation).is_some(),
            articulation.is_stable(),
            "{}",
            articulation.id()
        );
        if !articulation.is_stable() {
            assert!(
                substitutes(articulation).is_empty(),
                "{}",
                articulation.id()
            );
        }
        if let Some(parent) = articulation.parent()
            && parent.is_stable()
        {
            assert!(substitution(parent).is_some(), "{}", articulation.id());
        }
    }
    // A cup mute falls back to a mute of no named type, whose row plays the passage open and
    // softer.
    assert_eq!(substitution(Articulation::MuteCup), None);
    assert_eq!(
        Articulation::MuteCup.parent(),
        Some(Articulation::ConSordino)
    );
    let mute = substitution(Articulation::ConSordino).expect("an original articulation");
    assert_eq!(mute.substitutes, []);
    assert_eq!(mute.emulation, Some(Emulation::Soften));
    // A vibrato state refines nothing and has no row.
    assert_eq!(substitutes(Articulation::VibratoOn), []);
    assert_eq!(Articulation::VibratoOn.parent(), None);
}
