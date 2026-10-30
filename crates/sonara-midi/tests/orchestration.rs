//! Contract tests of the orchestration metadata through its public API (SON-P06).

use sonara_midi::orchestration::{
    Articulation, ArticulationAvailability, CheckError, EnsembleProfile, Era, EraPlayers, Family,
    Forms, INSTRUMENTS, Instrument, InstrumentError, InstrumentSpec, InstrumentType, PitchModel,
    PitchRange, Pitched, PlayerRange, Provenance, RangeCheck, SoundingPitch, SourcedRange,
    Transposition, WrittenPitch, instrument, instruments,
};

const ESTIMATE: Provenance = Provenance::Estimate {
    reason: "test fixture",
};

fn get(id: &str) -> &'static Instrument {
    instrument(id).unwrap_or_else(|| panic!("{id} is in the dataset"))
}

fn sounding(note: i64) -> SoundingPitch {
    SoundingPitch::new(note).expect("valid note")
}

fn written(note: i64) -> WrittenPitch {
    WrittenPitch::new(note).expect("valid note")
}

fn fixture(
    playable: (i64, i64),
    comfortable: Option<(i64, i64)>,
    semitones: i8,
) -> Result<Instrument, InstrumentError> {
    let sourced = |(low, high)| SourcedRange {
        range: PitchRange::new(low, high),
        provenance: ESTIMATE,
    };
    Instrument::try_new(
        InstrumentType::Viola,
        InstrumentSpec {
            pitch: PitchModel::Pitched(Pitched {
                transposition: Transposition::new(semitones),
                playable: sourced(playable),
                comfortable: comfortable.map(sourced),
            }),
            ..VALID
        },
    )
}

/// A valid unpitched spec: the base of the fixtures that break one field each.
const VALID: InstrumentSpec = InstrumentSpec {
    name: "Fixture",
    articulations_provenance: ESTIMATE,
    ..InstrumentSpec::EMPTY
};

fn era_players(players: Option<PlayerRange>, note: &'static str) -> EraPlayers {
    EraPlayers {
        players,
        note,
        provenance: ESTIMATE,
    }
}

/// A profile whose four eras hold four different values, so a mix-up of eras shows.
fn profile() -> EnsembleProfile {
    EnsembleProfile {
        forms: Forms::Both,
        classical: era_players(Some(PlayerRange::new(6, 10)), "classical"),
        romantic: era_players(Some(PlayerRange::new(16, 16)), "romantic"),
        film: era_players(None, "film"),
        chamber: era_players(Some(PlayerRange::NOT_USED), "chamber"),
    }
}

#[test]
fn dataset_invariants_hold() {
    let mut ids: Vec<&str> = INSTRUMENTS.iter().map(Instrument::id).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), count, "ids are unique");

    for instrument in &INSTRUMENTS {
        // Re-validate every record through the constructor the data went through.
        let spec = instrument.spec();
        let rebuilt = Instrument::try_new(instrument.instrument_type(), spec);
        assert_eq!(rebuilt, Ok(*instrument), "{}", instrument.id());
        // The accessors read the spec the record was built from.
        assert_eq!(spec.name, instrument.name());
        assert_eq!(&spec.pitch, instrument.pitch());
        assert_eq!(spec.articulations, instrument.articulations());
        assert_eq!(
            spec.articulations_provenance,
            instrument.articulations_provenance()
        );
        assert_eq!(spec.ensemble.as_ref(), instrument.ensemble());
        for articulation in instrument.articulations() {
            assert_eq!(
                Articulation::from_id(articulation.id()),
                Some(*articulation)
            );
        }
        match instrument.pitch() {
            PitchModel::Pitched(pitched) => {
                assert!(
                    matches!(
                        pitched.playable.provenance,
                        Provenance::Cited { source } if source.starts_with("https://")
                    ) || matches!(pitched.playable.provenance, Provenance::Estimate { reason } if !reason.is_empty())
                );
                if let Some(comfortable) = pitched.comfortable {
                    assert!(pitched.playable.range.encloses(comfortable.range));
                }
            }
            PitchModel::Unpitched => assert_eq!(instrument.written_playable(), None),
        }
    }
}

#[test]
fn validator_rejects_broken_metadata() {
    assert!(fixture((40, 80), Some((50, 70)), 0).is_ok());
    assert_eq!(
        fixture((40, 80), Some((39, 70)), 0),
        Err(InstrumentError::ComfortableOutsidePlayable)
    );
    assert_eq!(
        fixture((60, 120), None, -12),
        Err(InstrumentError::WrittenOutOfMidi)
    );
    assert_eq!(
        Instrument::try_new(
            InstrumentType::Triangle,
            InstrumentSpec {
                articulations: &[Articulation::Roll, Articulation::Roll],
                ..VALID
            }
        ),
        Err(InstrumentError::DuplicateArticulation)
    );
    assert!(
        Instrument::try_new(
            InstrumentType::Triangle,
            InstrumentSpec {
                articulations: &[Articulation::Roll, Articulation::Marcato],
                ..VALID
            }
        )
        .is_ok()
    );
}

/// `EMPTY` is the base of a record, not a record: the fields a record must state are rejected
/// until it states them (TST-03).
#[test]
fn empty_spec_needs_a_name_and_an_articulation_provenance() {
    let empty = InstrumentSpec::EMPTY;
    assert_eq!(empty.name, "");
    assert_eq!(empty.pitch, PitchModel::Unpitched);
    assert_eq!(empty.articulations, []);
    assert_eq!(empty.ensemble, None);
    assert_eq!(
        Instrument::try_new(InstrumentType::Triangle, empty),
        Err(InstrumentError::EmptyName)
    );
    let named = InstrumentSpec {
        name: "Triangle",
        ..InstrumentSpec::EMPTY
    };
    assert_eq!(
        Instrument::try_new(InstrumentType::Triangle, named),
        Err(InstrumentError::EmptyProvenance)
    );
    let stated = InstrumentSpec {
        articulations_provenance: ESTIMATE,
        ..named
    };
    let triangle = Instrument::try_new(InstrumentType::Triangle, stated).expect("valid");
    assert_eq!(triangle.name(), "Triangle");
    assert_eq!(triangle.spec(), stated);
    assert_eq!(triangle.ensemble(), None);
    assert_eq!(triangle.written_playable(), None);
}

#[test]
fn blank_names_are_rejected() {
    for name in ["", " ", "\t\n"] {
        assert_eq!(
            Instrument::try_new(InstrumentType::Triangle, InstrumentSpec { name, ..VALID }),
            Err(InstrumentError::EmptyName),
            "{name:?}"
        );
    }
}

/// Every provenance of a record names a source or a reason: the playable and comfortable
/// ranges, the articulation list and each era of the ensemble profile (TST-03).
#[test]
fn every_provenance_must_be_stated() {
    let blank_cited = Provenance::Cited { source: " " };
    let blank_estimate = Provenance::Estimate { reason: "" };
    let range = |low, high, provenance| SourcedRange {
        range: PitchRange::new(low, high),
        provenance,
    };
    let pitched = |playable, comfortable| {
        PitchModel::Pitched(Pitched {
            transposition: Transposition::NONE,
            playable,
            comfortable,
        })
    };
    let mut cases = vec![
        InstrumentSpec {
            pitch: pitched(range(40, 80, blank_cited), None),
            ..VALID
        },
        InstrumentSpec {
            pitch: pitched(range(40, 80, blank_estimate), None),
            ..VALID
        },
        InstrumentSpec {
            pitch: pitched(range(40, 80, ESTIMATE), Some(range(50, 70, blank_estimate))),
            ..VALID
        },
        InstrumentSpec {
            articulations_provenance: blank_cited,
            ..VALID
        },
    ];
    for era in Era::ALL {
        let mut broken = profile();
        let players = match era {
            Era::Classical => &mut broken.classical,
            Era::Romantic => &mut broken.romantic,
            Era::Film => &mut broken.film,
            Era::Chamber => &mut broken.chamber,
        };
        players.provenance = blank_estimate;
        cases.push(InstrumentSpec {
            ensemble: Some(broken),
            ..VALID
        });
    }
    for (index, spec) in cases.into_iter().enumerate() {
        assert_eq!(
            Instrument::try_new(InstrumentType::Timpani, spec),
            Err(InstrumentError::EmptyProvenance),
            "case {index}"
        );
    }
    // The same fixtures with stated provenances are valid.
    let stated = InstrumentSpec {
        pitch: pitched(range(40, 80, ESTIMATE), Some(range(50, 70, ESTIMATE))),
        ensemble: Some(profile()),
        ..VALID
    };
    assert!(Instrument::try_new(InstrumentType::Timpani, stated).is_ok());
}

/// Only percussion may be unpitched: every other family plays definite pitches (TST-07).
#[test]
fn only_percussion_may_be_unpitched() {
    let mut accepted = 0;
    for instrument_type in InstrumentType::ALL {
        let result = Instrument::try_new(instrument_type, VALID);
        if instrument_type.family() == Family::Percussion {
            assert!(result.is_ok(), "{instrument_type:?}");
            accepted += 1;
        } else {
            assert_eq!(
                result,
                Err(InstrumentError::UnpitchedOutsidePercussion),
                "{instrument_type:?}"
            );
        }
    }
    assert_eq!(accepted, 22);
    for instrument_type in [
        InstrumentType::Violin,
        InstrumentType::Flute,
        InstrumentType::HornF,
        InstrumentType::Harp,
        InstrumentType::PianoConcertGrand,
        InstrumentType::VoiceSoprano,
    ] {
        assert_eq!(
            Instrument::try_new(instrument_type, VALID),
            Err(InstrumentError::UnpitchedOutsidePercussion),
            "{instrument_type:?}"
        );
    }
    // A pitched record outside the percussion is valid.
    assert!(fixture((40, 80), None, 0).is_ok());
}

#[test]
#[should_panic(expected = "invalid instrument metadata")]
fn new_panics_on_invalid_metadata() {
    let _ = Instrument::new(InstrumentType::Violin, VALID);
}

/// A player range holds `min <= max`; `0..=0` means not used (TST-03).
#[test]
fn player_ranges_reject_inverted_counts() {
    assert_eq!(
        PlayerRange::try_new(3, 2),
        Err(InstrumentError::InvertedPlayerRange { min: 3, max: 2 })
    );
    let range = PlayerRange::try_new(16, 16).expect("valid");
    assert_eq!((range.min(), range.max()), (16, 16));
    let range = PlayerRange::new(6, 10);
    assert_eq!((range.min(), range.max()), (6, 10));
    assert_eq!(PlayerRange::new(0, 0), PlayerRange::NOT_USED);
    assert_eq!(
        (PlayerRange::NOT_USED.min(), PlayerRange::NOT_USED.max()),
        (0, 0)
    );
}

#[test]
#[should_panic(expected = "invalid player range")]
fn inverted_player_range_panics_in_new() {
    let _ = PlayerRange::new(2, 1);
}

/// The ensemble profile travels through the record unchanged, and each era reads its own entry.
#[test]
fn ensemble_profile_is_read_per_era() {
    let violin = Instrument::try_new(
        InstrumentType::Violin,
        InstrumentSpec {
            pitch: PitchModel::Pitched(Pitched {
                transposition: Transposition::NONE,
                playable: SourcedRange {
                    range: PitchRange::new(55, 105),
                    provenance: ESTIMATE,
                },
                comfortable: None,
            }),
            ensemble: Some(profile()),
            ..VALID
        },
    )
    .expect("valid");
    let profile = violin.ensemble().expect("recorded");
    assert_eq!(profile.forms, Forms::Both);
    let notes: Vec<&str> = Era::ALL
        .into_iter()
        .map(|era| profile.players(era).note)
        .collect();
    assert_eq!(notes, ["classical", "romantic", "film", "chamber"]);
    assert_eq!(
        profile.players(Era::Classical).players,
        Some(PlayerRange::new(6, 10))
    );
    assert_eq!(profile.players(Era::Film).players, None);
    assert_eq!(
        profile.players(Era::Chamber).players,
        Some(PlayerRange::NOT_USED)
    );
}

#[test]
fn lookup_is_by_stable_id_not_display_name() {
    assert!(instrument("violin").is_some());
    assert!(instrument("Violin").is_none());
    assert!(instrument("Clarinet in Bb").is_none());
    assert_eq!(get("clarinet_bb").name(), "Clarinet in Bb");
    assert_eq!(get("clarinet_bb").family().slug(), "woodwinds");
}

/// The 85 instrument ids, written independently of `InstrumentType` and sorted alphabetically:
/// the oracle for its transcription (not a copy of its score order).
const EXPECTED_IDS: [&str; 85] = [
    "accordion",
    "alto_flute",
    "bass_clarinet",
    "bass_drum",
    "bass_flute",
    "bassoon",
    "carillon",
    "castanets",
    "celesta",
    "cello",
    "choir_alto",
    "choir_bass",
    "choir_soprano",
    "choir_tenor",
    "cimbasso",
    "clarinet_a",
    "clarinet_bb",
    "clarinet_eb",
    "clash_cymbals",
    "clavichord",
    "clavinet",
    "contrabassoon",
    "cornet",
    "crotales",
    "double_bass",
    "electric_piano_rhodes",
    "electric_piano_wurlitzer",
    "english_horn",
    "euphonium",
    "flugelhorn",
    "flute",
    "glockenspiel",
    "guitar_classical",
    "harp",
    "harpsichord",
    "horn_f",
    "lute",
    "mandolin",
    "maracas",
    "marimba",
    "oboe",
    "oboe_damore",
    "organ_hammond",
    "organ_pipe",
    "piano_concert_grand",
    "piano_felt",
    "piano_honky_tonk",
    "piano_prepared",
    "piano_studio_grand",
    "piano_upright",
    "piccolo",
    "saxophone_alto",
    "saxophone_baritone",
    "saxophone_soprano",
    "saxophone_tenor",
    "shaker",
    "sleigh_bells",
    "snare_drum",
    "suspended_cymbal",
    "tam_tam",
    "tambourine",
    "temple_blocks",
    "timpani",
    "triangle",
    "trombone_bass",
    "trombone_tenor",
    "trumpet_bb",
    "trumpet_c",
    "trumpet_piccolo",
    "tuba",
    "tubular_bells",
    "vibraphone",
    "viola",
    "violin",
    "voice_alto",
    "voice_baritone",
    "voice_bass",
    "voice_mezzo_soprano",
    "voice_soprano",
    "voice_tenor",
    "wagner_tuba_bass",
    "wagner_tuba_tenor",
    "whip",
    "wood_block",
    "xylophone",
];

fn position(instrument_type: InstrumentType) -> usize {
    InstrumentType::ALL
        .iter()
        .position(|candidate| *candidate == instrument_type)
        .unwrap_or_else(|| panic!("{instrument_type:?} is in ALL"))
}

/// Every instrument type once, with its expected id, round-tripping through `from_id` (TST-07).
#[test]
fn instrument_types_match_the_expected_ids_one_to_one() {
    let mut ids: Vec<&str> = InstrumentType::ALL
        .into_iter()
        .map(InstrumentType::id)
        .collect();
    ids.sort_unstable();
    let mut expected = EXPECTED_IDS;
    expected.sort_unstable();
    assert_eq!(ids, expected);

    for (index, instrument_type) in InstrumentType::ALL.into_iter().enumerate() {
        // `ALL` is in declaration order, so its order is the derived `Ord`.
        assert_eq!(instrument_type as usize, index, "{instrument_type:?}");
        let id = instrument_type.id();
        assert!(
            id.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
            "{id}"
        );
        assert_eq!(InstrumentType::from_id(id), Some(instrument_type));
    }
    assert!(InstrumentType::ALL.windows(2).all(|pair| pair[0] < pair[1]));
}

/// `from_id` accepts the exact id only: no display name, case change, padding or prefix (TST-03).
#[test]
fn instrument_type_lookup_is_strict() {
    for text in [
        "",
        "Violin",
        "VIOLIN",
        " violin",
        "violin ",
        "violins",
        "viol",
        "Horn in F",
        "horn",
        "clarinet",
        "clarinet-bb",
    ] {
        assert_eq!(InstrumentType::from_id(text), None, "{text:?}");
        assert_eq!(instrument(text), None, "{text:?}");
    }
    assert_eq!(
        InstrumentType::from_id("viola"),
        Some(InstrumentType::Viola)
    );
}

/// Each family is one block of the score order, the blocks in full-score order, with the expected
/// count of instruments per family.
#[test]
fn families_form_one_block_each_in_score_order() {
    let mut blocks: Vec<(Family, usize)> = Vec::new();
    for instrument_type in InstrumentType::ALL {
        match blocks.last_mut() {
            Some((family, count)) if *family == instrument_type.family() => *count += 1,
            _ => blocks.push((instrument_type.family(), 1)),
        }
    }
    assert_eq!(
        blocks,
        [
            (Family::Woodwinds, 17),
            (Family::Brass, 13),
            (Family::Percussion, 22),
            (Family::Plucked, 4),
            (Family::Keyboards, 15),
            (Family::Choir, 10),
            (Family::Strings, 4),
        ]
    );
}

/// The score-order conventions the order rests on, each as an explicit relation.
#[test]
fn score_order_follows_the_conventions() {
    use InstrumentType as T;
    let ascending = |types: &[InstrumentType]| {
        assert!(
            types.windows(2).all(|pair| pair[0] < pair[1]),
            "{types:?} are in score order"
        );
    };
    // Woodwinds high to low, auxiliaries beside their own family, saxophones after the clarinets.
    ascending(&[
        T::Piccolo,
        T::Flute,
        T::Oboe,
        T::EnglishHorn,
        T::ClarinetEb,
        T::ClarinetBb,
        T::BassClarinet,
        T::SaxophoneSoprano,
        T::SaxophoneBaritone,
        T::Bassoon,
        T::Contrabassoon,
    ]);
    assert_eq!(InstrumentType::ALL.first(), Some(&T::Piccolo));
    assert_eq!(position(T::HornF), position(T::Contrabassoon) + 1);
    // Horns head the brass, the Wagner tubas directly below them; trumpets, trombones, tuba.
    assert_eq!(position(T::WagnerTubaTenor), position(T::HornF) + 1);
    assert_eq!(position(T::WagnerTubaBass), position(T::HornF) + 2);
    ascending(&[
        T::HornF,
        T::TrumpetBb,
        T::TromboneTenor,
        T::TromboneBass,
        T::Tuba,
    ]);
    // Timpani first in the percussion, then the untuned, then the tuned instruments.
    assert_eq!(position(T::Timpani), position(T::Cimbasso) + 1);
    let untuned = [
        T::BassDrum,
        T::SnareDrum,
        T::ClashCymbals,
        T::SuspendedCymbal,
        T::TamTam,
        T::Triangle,
        T::Tambourine,
        T::Castanets,
        T::WoodBlock,
        T::TempleBlocks,
        T::SleighBells,
        T::Maracas,
        T::Shaker,
        T::Whip,
    ];
    let tuned = [
        T::Glockenspiel,
        T::Xylophone,
        T::Vibraphone,
        T::Marimba,
        T::TubularBells,
        T::Crotales,
        T::Carillon,
    ];
    for low in untuned {
        assert!(T::Timpani < low, "{low:?}");
        for high in tuned {
            assert!(low < high, "{low:?} before {high:?}");
        }
    }
    // The harp heads its block; the celesta comes before the pianos and the organs.
    assert_eq!(position(T::Harp), position(T::Carillon) + 1);
    ascending(&[T::Harp, T::Celesta, T::PianoConcertGrand, T::OrganPipe]);
    // Solo voices before the choir; the strings last, violin to double bass.
    for voice in [T::VoiceSoprano, T::VoiceBass] {
        for section in [T::ChoirSoprano, T::ChoirBass] {
            assert!(voice < section, "{voice:?} before {section:?}");
        }
    }
    ascending(&[T::ChoirBass, T::Violin, T::Viola, T::Cello, T::DoubleBass]);
    assert_eq!(InstrumentType::ALL.last(), Some(&T::DoubleBass));
}

/// The records are in score order, at most one per type, and each type finds its own record
/// (TST-07).
#[test]
fn records_are_in_score_order_and_found_by_type() {
    assert_eq!(instruments(), &INSTRUMENTS[..]);
    assert!(
        instruments()
            .windows(2)
            .all(|pair| pair[0].instrument_type() < pair[1].instrument_type())
    );
    let mut covered = Vec::new();
    for instrument_type in InstrumentType::ALL {
        let by_id = instrument(instrument_type.id());
        if let Some(record) = instrument_type.get() {
            assert_eq!(record.instrument_type(), instrument_type);
            assert_eq!(record.id(), instrument_type.id());
            assert_eq!(record.family(), instrument_type.family());
            assert!(by_id.is_some_and(|found| std::ptr::eq(found, record)));
            covered.push(instrument_type.id());
        } else {
            assert_eq!(by_id, None, "{instrument_type:?}");
            assert!(
                !instruments()
                    .iter()
                    .any(|record| record.instrument_type() == instrument_type)
            );
        }
    }
    covered.sort_unstable();
    assert_eq!(
        covered,
        [
            "bassoon",
            "cello",
            "clarinet_bb",
            "contrabassoon",
            "double_bass",
            "english_horn",
            "flute",
            "glockenspiel",
            "horn_f",
            "oboe",
            "piccolo",
            "snare_drum",
            "trombone_tenor",
            "trumpet_bb",
            "violin",
            "xylophone",
        ]
    );
}

/// The families of the records did not change when the family moved from the record to its
/// type (TST-06).
#[test]
fn record_families_are_unchanged() {
    let table = [
        ("violin", Family::Strings),
        ("cello", Family::Strings),
        ("double_bass", Family::Strings),
        ("flute", Family::Woodwinds),
        ("piccolo", Family::Woodwinds),
        ("oboe", Family::Woodwinds),
        ("english_horn", Family::Woodwinds),
        ("clarinet_bb", Family::Woodwinds),
        ("bassoon", Family::Woodwinds),
        ("contrabassoon", Family::Woodwinds),
        ("horn_f", Family::Brass),
        ("trumpet_bb", Family::Brass),
        ("trombone_tenor", Family::Brass),
        ("glockenspiel", Family::Percussion),
        ("xylophone", Family::Percussion),
        ("snare_drum", Family::Percussion),
    ];
    for (id, family) in table {
        assert_eq!(get(id).family(), family, "{id}");
    }
}

/// Written ranges that the sources state, against the ranges the data derives from the sounding
/// ones: the oracle for transposition and the stored endpoints.
#[test]
fn derived_written_ranges_match_the_sources() {
    let table = [
        ("clarinet_bb", 52, 96),   // written E3-C7
        ("trumpet_bb", 54, 84),    // written F#3-C6 (as sources state)
        ("english_horn", 59, 91),  // written B3-C6 sounding E3-C6: written is a fifth up
        ("piccolo", 62, 96),       // written D4-C7, sounding D5-C8
        ("contrabassoon", 34, 74), // written Bb1 (A1) up to D5 notation per octave rule
    ];
    for (id, low, high) in table {
        let (got_low, got_high) = get(id).written_playable().expect("pitched");
        assert_eq!((got_low.get(), got_high.get()), (low, high), "{id}");
    }
}

#[test]
fn transposing_conversions() {
    assert_eq!(get("violin").transposition(), Some(Transposition::NONE));
    let clarinet = get("clarinet_bb").transposition().expect("pitched");
    assert_eq!(clarinet.to_sounding(written(60)), Some(sounding(58)));
    assert_eq!(clarinet.to_written(sounding(58)), Some(written(60)));
    let horn = get("horn_f").transposition().expect("pitched");
    assert_eq!(horn.to_sounding(written(72)), Some(sounding(65)));
    let bass = get("double_bass").transposition().expect("pitched");
    assert_eq!(bass.to_sounding(written(40)), Some(sounding(28)));
    let glock = get("glockenspiel").transposition().expect("pitched");
    assert_eq!(glock.to_sounding(written(60)), Some(sounding(84)));
    assert_eq!(get("snare_drum").transposition(), None);
}

#[test]
fn range_checks_do_not_clamp_and_distinguish_comfortable_from_playable() {
    let violin = get("violin");
    let cases = [
        (54, RangeCheck::BelowPlayable),
        (55, RangeCheck::Within),
        (100, RangeCheck::Within),
        (101, RangeCheck::OutsideComfortable),
        (108, RangeCheck::OutsideComfortable),
        (109, RangeCheck::AbovePlayable),
    ];
    for (note, expected) in cases {
        assert_eq!(
            violin.check_sounding(sounding(note)),
            Ok(expected),
            "{note}"
        );
    }
    // No comfortable range known: the playable range alone decides.
    let flute = get("flute");
    assert_eq!(flute.check_sounding(sounding(59)), Ok(RangeCheck::Within));
    assert_eq!(
        flute.check_sounding(sounding(97)),
        Ok(RangeCheck::AbovePlayable)
    );
}

#[test]
fn written_pitch_is_converted_once() {
    let clarinet = get("clarinet_bb");
    // Written E3 (52) sounds D3 (50), the lowest note.
    assert_eq!(clarinet.check_written(written(52)), Ok(RangeCheck::Within));
    assert_eq!(
        clarinet.check_written(written(51)),
        Ok(RangeCheck::BelowPlayable)
    );
    // The same number as a sounding pitch is a different question: 52 is inside, 49 is not.
    assert_eq!(
        clarinet.check_sounding(sounding(49)),
        Ok(RangeCheck::BelowPlayable)
    );
    assert_eq!(clarinet.check_written(written(96)), Ok(RangeCheck::Within));
    assert_eq!(
        clarinet.check_written(written(97)),
        Ok(RangeCheck::AbovePlayable)
    );
}

#[test]
fn unpitched_and_out_of_midi_checks_fail_explicitly() {
    assert_eq!(
        get("snare_drum").check_sounding(sounding(60)),
        Err(CheckError::Unpitched)
    );
    assert_eq!(
        get("piccolo").check_written(written(127)),
        Err(CheckError::ConversionOutOfRange(127))
    );
}

#[test]
fn articulation_availability_needs_a_library_declaration_for_support() {
    let violin = get("violin");
    let declared = ["sustain", "pizzicato"];
    let check = |id, declared: &[&str]| violin.articulation_availability(id, declared);
    assert_eq!(
        check("pizzicato", &declared),
        ArticulationAvailability::Supported
    );
    assert_eq!(
        check("spiccato", &declared),
        ArticulationAvailability::IdiomaticUnverified
    );
    assert_eq!(
        check("spiccato", &[]),
        ArticulationAvailability::IdiomaticUnverified
    );
    assert_eq!(
        check("flutter_tongue", &declared),
        ArticulationAvailability::NotIdiomatic
    );
    assert_eq!(
        check("whistling", &declared),
        ArticulationAvailability::UnknownArticulation
    );
    assert_eq!(
        check("", &[]),
        ArticulationAvailability::UnknownArticulation
    );
    // A declaration of an id outside the vocabulary never counts as support.
    assert_eq!(
        check("whistling", &["whistling"]),
        ArticulationAvailability::UnknownArticulation
    );
    // Instruments differ: a flute has no col legno.
    assert_eq!(
        get("flute").articulation_availability("col_legno", &[]),
        ArticulationAvailability::NotIdiomatic
    );
}
