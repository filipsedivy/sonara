//! Contract tests of the orchestration metadata through its public API (SON-P06).

use sonara_midi::orchestration::{
    Articulation, ArticulationAvailability, CheckError, Family, INSTRUMENTS, Instrument,
    InstrumentError, PitchModel, PitchRange, Provenance, RangeCheck, SoundingPitch, SourcedRange,
    Transposition, WrittenPitch, instrument,
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
        "fixture",
        "Fixture",
        Family::Other,
        PitchModel::Pitched(sonara_midi::orchestration::Pitched {
            transposition: Transposition::new(semitones),
            playable: sourced(playable),
            comfortable: comfortable.map(sourced),
        }),
        &[],
        ESTIMATE,
    )
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
        let rebuilt = Instrument::try_new(
            instrument.id(),
            instrument.name(),
            instrument.family(),
            *instrument.pitch(),
            instrument.articulations(),
            instrument.articulations_provenance(),
        );
        assert_eq!(rebuilt, Ok(*instrument), "{}", instrument.id());
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
            "Bad Id",
            "x",
            Family::Other,
            PitchModel::Unpitched,
            &[],
            ESTIMATE
        ),
        Err(InstrumentError::BadId)
    );
    assert_eq!(
        Instrument::try_new(
            "dup",
            "x",
            Family::Other,
            PitchModel::Unpitched,
            &[Articulation::Roll, Articulation::Roll],
            ESTIMATE
        ),
        Err(InstrumentError::DuplicateArticulation)
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
