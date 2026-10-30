//! Contract tests of the orchestration metadata through its public API (SON-P06).

use sonara_midi::orchestration::{
    Articulation, ArticulationAvailability, Axis, CheckError, Clef, EnsembleProfile, Era,
    EraPlayers, Family, Forms, INSTRUMENTS, Instrument, InstrumentError, InstrumentSpec,
    InstrumentType, Kind, Mute, PitchModel, PitchRange, Pitched, PlayerRange, Provenance,
    RangeCheck, SoundingPitch, SourcedRange, Transposition, Vibrato, WrittenPitch, instrument,
    instruments,
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

/// A pitched spec with an estimated playable range and the given clefs.
fn written_in_clefs(
    semitones: i8,
    (low, high): (i64, i64),
    clefs: &'static [Clef],
) -> InstrumentSpec {
    InstrumentSpec {
        pitch: PitchModel::Pitched(Pitched {
            transposition: Transposition::new(semitones),
            playable: SourcedRange {
                range: PitchRange::new(low, high),
                provenance: ESTIMATE,
            },
            comfortable: None,
        }),
        clefs,
        ..VALID
    }
}

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

/// Every provenance of a record: the articulation list, the playable and comfortable ranges and
/// each era of the ensemble profile.
fn provenances(instrument: &Instrument) -> Vec<Provenance> {
    let mut all = vec![instrument.articulations_provenance()];
    if let PitchModel::Pitched(pitched) = instrument.pitch() {
        all.push(pitched.playable.provenance);
        all.extend(
            pitched
                .comfortable
                .map(|comfortable| comfortable.provenance),
        );
    }
    if let Some(profile) = instrument.ensemble() {
        all.extend(Era::ALL.map(|era| profile.players(era).provenance));
    }
    all
}

/// Whether a reader can trace `provenance`: a cited source that is not blank and names a URL
/// (`http://` or `https://`) or a page (`p. 12`, `pp. 12-14`), or an estimate with a reason.
/// Some sources are printed books without a URL, so a page is enough.
fn is_traceable(provenance: Provenance) -> bool {
    match provenance {
        Provenance::Cited { source } => {
            !source.trim().is_empty()
                && (source.contains("https://") || source.contains("http://") || names_page(source))
        }
        Provenance::Estimate { reason } => !reason.trim().is_empty(),
    }
}

/// Whether `text` holds `p.` or `pp.` as a word, followed by a space and a digit.
fn names_page(text: &str) -> bool {
    ["p. ", "pp. "].iter().any(|marker| {
        text.match_indices(marker).any(|(at, _)| {
            let starts_word = text[..at]
                .chars()
                .next_back()
                .is_none_or(|before| !before.is_alphanumeric());
            let digit_follows = text[at + marker.len()..]
                .chars()
                .next()
                .is_some_and(|after| after.is_ascii_digit());
            starts_word && digit_follows
        })
    })
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
        assert_eq!(spec.clefs, instrument.clefs());
        // Every record states its clefs, each once.
        let mut clefs: Vec<&str> = instrument.clefs().iter().map(|clef| clef.slug()).collect();
        assert!(!clefs.is_empty(), "{}", instrument.id());
        clefs.sort_unstable();
        clefs.dedup();
        assert_eq!(clefs.len(), instrument.clefs().len(), "{}", instrument.id());
        for articulation in instrument.articulations() {
            assert_eq!(
                Articulation::from_id(articulation.id()),
                Some(*articulation)
            );
        }
        // A cited source names a URL or a page; an estimate gives its reason.
        for provenance in provenances(instrument) {
            assert!(
                is_traceable(provenance),
                "{}: {provenance:?}",
                instrument.id()
            );
        }
        match instrument.pitch() {
            PitchModel::Pitched(pitched) => {
                if let Some(comfortable) = pitched.comfortable {
                    assert!(pitched.playable.range.encloses(comfortable.range));
                }
                // Every playable pitch has a written pitch in every listed clef. In a clef without
                // an octave of its own that is the transposition's, and checking it as written
                // gives the answer for the sounding pitch.
                let range = pitched.playable.range;
                for &clef in instrument.clefs() {
                    for note in range.low().get()..=range.high().get() {
                        let pitch = sounding(note);
                        let in_clef = instrument.written_in(clef, pitch);
                        assert!(in_clef.is_some(), "{} {clef:?} {note}", instrument.id());
                        if clef.octave() == 0 {
                            assert_eq!(in_clef, pitched.transposition.to_written(pitch));
                            let check = in_clef.map(|w| instrument.check_written(w));
                            assert_eq!(check, Some(instrument.check_sounding(pitch)));
                        }
                    }
                }
            }
            PitchModel::Unpitched => {
                assert_eq!(instrument.written_playable(), None);
                for clef in Clef::ALL {
                    assert_eq!(instrument.written_in(clef, sounding(60)), None, "{clef:?}");
                }
            }
        }
    }
}

/// The clefs of the records, in order of use (TST-10: the expected values are slugs written
/// independently of the data's `Clef` values).
#[test]
fn record_clefs_match_the_expected_slugs() {
    let table: [(&str, &[&str]); 16] = [
        ("piccolo", &["treble"]),
        ("flute", &["treble"]),
        ("oboe", &["treble"]),
        ("english_horn", &["treble"]),
        ("clarinet_bb", &["treble"]),
        ("bassoon", &["bass", "tenor", "treble"]),
        ("contrabassoon", &["bass", "tenor"]),
        ("horn_f", &["treble", "bass"]),
        ("trumpet_bb", &["treble"]),
        ("trombone_tenor", &["bass", "tenor", "alto"]),
        ("snare_drum", &["single_line", "treble", "percussion"]),
        ("glockenspiel", &["treble"]),
        ("xylophone", &["treble"]),
        ("violin", &["treble"]),
        ("cello", &["bass", "tenor", "treble"]),
        ("double_bass", &["bass", "tenor", "treble"]),
    ];
    assert_eq!(table.len(), INSTRUMENTS.len());
    for (id, slugs) in table {
        let clefs: Vec<&str> = get(id).clefs().iter().map(|clef| clef.slug()).collect();
        assert_eq!(clefs, slugs, "{id}");
    }
}

/// A tenor voice is written at transposition 0 and takes its octave from the clef: in
/// `treble_8vb` its sounding C3 is written C4, and on a bass-clef staff shared with the basses
/// the same note is written at pitch. The published written ranges in `treble_8vb` (solo C4-D6
/// for a sounding C3-D5, choir C4-Bb5 for C3-Bb4) are the oracle. The octave stated twice, as a
/// -12 transposition with `treble_8vb`, is rejected, because it would write the part two octaves
/// above the sound.
#[test]
fn tenor_parts_take_their_octave_from_the_clef() {
    let solo = Instrument::try_new(
        InstrumentType::VoiceTenor,
        written_in_clefs(0, (48, 74), &[Clef::Treble8vb]),
    )
    .expect("valid");
    let choir = Instrument::try_new(
        InstrumentType::ChoirTenor,
        written_in_clefs(0, (48, 70), &[Clef::Treble8vb, Clef::Bass]),
    )
    .expect("valid");
    let in_treble_8vb = |voice: &Instrument, note| {
        voice
            .written_in(Clef::Treble8vb, sounding(note))
            .map(WrittenPitch::get)
    };
    assert_eq!(in_treble_8vb(&solo, 48), Some(60));
    assert_eq!(in_treble_8vb(&solo, 74), Some(86));
    assert_eq!(in_treble_8vb(&choir, 48), Some(60));
    assert_eq!(in_treble_8vb(&choir, 70), Some(82));
    for voice in [solo, choir] {
        assert_eq!(voice.transposition(), Some(Transposition::NONE));
        for note in [48, 55, 60, 70] {
            assert_eq!(in_treble_8vb(&voice, note), Some(note + 12), "{note}");
            assert_eq!(
                voice.written_in(Clef::Bass, sounding(note)),
                Some(written(note)),
                "{note}"
            );
        }
        // The clef-free views keep the sounding octave: the written range is the sounding one,
        // and a written pitch is checked as the pitch it names.
        assert_eq!(
            voice.written_playable().map(|(low, _)| low),
            Some(written(48))
        );
        assert_eq!(voice.check_written(written(48)), Ok(RangeCheck::Within));
        assert_eq!(
            voice.check_written(written(47)),
            Ok(RangeCheck::BelowPlayable)
        );
    }
    for clefs in [&[Clef::Treble8vb][..], &[Clef::Treble8vb, Clef::Bass][..]] {
        assert_eq!(
            Instrument::try_new(
                InstrumentType::ChoirTenor,
                written_in_clefs(-12, (48, 70), clefs)
            ),
            Err(InstrumentError::OctaveClefWithTransposition),
            "{clefs:?}"
        );
    }
}

/// Each clef check rejects its fixture, in the documented order, and accepts the repaired one
/// (TST-03).
#[test]
fn validator_rejects_broken_clefs() {
    let check = |instrument_type, spec| Instrument::try_new(instrument_type, spec).map(|_| ());
    let viola = |semitones, playable, clefs| {
        check(
            InstrumentType::Viola,
            written_in_clefs(semitones, playable, clefs),
        )
    };
    let triangle = |clefs| check(InstrumentType::Triangle, InstrumentSpec { clefs, ..VALID });
    let cases: [(Result<(), InstrumentError>, Result<(), InstrumentError>); 13] = [
        (
            viola(0, (48, 91), &[Clef::Alto, Clef::Treble, Clef::Alto]),
            Err(InstrumentError::DuplicateClef),
        ),
        (
            triangle(&[Clef::Treble, Clef::Treble]),
            Err(InstrumentError::DuplicateClef),
        ),
        (
            viola(0, (48, 91), &[Clef::Alto, Clef::Percussion]),
            Err(InstrumentError::PitchlessClef),
        ),
        (
            viola(0, (48, 91), &[Clef::SingleLine]),
            Err(InstrumentError::PitchlessClef),
        ),
        (
            viola(-12, (48, 91), &[Clef::Treble8vb]),
            Err(InstrumentError::OctaveClefWithTransposition),
        ),
        (
            viola(-2, (48, 91), &[Clef::Treble, Clef::Treble8vb]),
            Err(InstrumentError::OctaveClefWithTransposition),
        ),
        (
            viola(12, (48, 91), &[Clef::Treble8vb]),
            Err(InstrumentError::OctaveClefWithTransposition),
        ),
        // Written an octave up in treble_8vb, a top of 116 leaves MIDI; 115 does not.
        (
            viola(0, (48, 116), &[Clef::Treble8vb]),
            Err(InstrumentError::WrittenOutOfMidi),
        ),
        (viola(0, (48, 115), &[Clef::Treble8vb]), Ok(())),
        (viola(0, (48, 116), &[Clef::Treble]), Ok(())),
        // Clef by clef in list order: the first clef's failure is reported.
        (
            viola(-12, (48, 91), &[Clef::Percussion, Clef::Treble8vb]),
            Err(InstrumentError::PitchlessClef),
        ),
        (
            viola(-12, (48, 91), &[Clef::Treble8vb, Clef::Percussion]),
            Err(InstrumentError::OctaveClefWithTransposition),
        ),
        // An unpitched instrument may be written on any staff.
        (
            triangle(&[Clef::SingleLine, Clef::Treble, Clef::Percussion]),
            Ok(()),
        ),
    ];
    for (index, (got, expected)) in cases.into_iter().enumerate() {
        assert_eq!(got, expected, "case {index}");
    }
    // A duplicate is reported before a pitchless clef.
    assert_eq!(
        viola(0, (48, 91), &[Clef::Percussion, Clef::Percussion]),
        Err(InstrumentError::DuplicateClef)
    );
}

/// The written pitch in a clef that is not listed: a part may use another staff (TST-03 for the
/// unpitched and out-of-MIDI cases).
#[test]
fn written_in_any_clef_and_its_failures() {
    let clarinet = get("clarinet_bb");
    // Sounding Bb3 is written C4 in treble, and the bass clef shows the same written pitch.
    assert_eq!(
        clarinet.written_in(Clef::Treble, sounding(58)),
        Some(written(60))
    );
    assert_eq!(
        clarinet.written_in(Clef::Bass, sounding(58)),
        Some(written(60))
    );
    assert_eq!(clarinet.written_in(Clef::Percussion, sounding(58)), None);
    // The piccolo is written an octave below the sound in every pitched clef.
    let piccolo = get("piccolo");
    assert_eq!(
        piccolo.written_in(Clef::Treble, sounding(84)),
        Some(written(72))
    );
    assert_eq!(piccolo.written_in(Clef::Treble, sounding(11)), None);
    assert_eq!(
        get("snare_drum").written_in(Clef::Treble, sounding(60)),
        None
    );
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
    assert_eq!(empty.clefs, []);
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

/// The dataset check traces a cited source only by a URL or a page, and an estimate only by its
/// reason (TST-03; the cases are written by hand).
#[test]
fn a_cited_source_must_name_a_url_or_a_page() {
    let cited = |source| Provenance::Cited { source };
    let estimate = |reason| Provenance::Estimate { reason };
    for traceable in [
        cited("Wikipedia, Oboe, https://en.wikipedia.org/wiki/Oboe: Bb3 to about G6"),
        cited("Rhodes Service Manual, http://www.fenderrhodes.com/org/manual/toc.html: 73 keys"),
        cited("A. Author, A Book, 2nd edition (2002), p. 112: top D7"),
        cited("A. Author, A Book (2002), pp. 112-114: top D7"),
        estimate("general orchestration practice"),
    ] {
        assert!(is_traceable(traceable), "{traceable:?}");
    }
    for untraceable in [
        cited("A. Author, A Book, 2nd edition (2002): top D7"),
        cited("A. Author, A Book, p. xii: top D7"),
        cited("A. Author, A Book, app. 3: top D7"),
        cited("A. Author, A Book, www.example.com: top D7"),
        cited(" "),
        estimate(""),
        estimate(" \t"),
    ] {
        assert!(!is_traceable(untraceable), "{untraceable:?}");
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

/// The articulation vocabulary in its frozen order (TST-05). A choice list built from
/// `Articulation::ALL` is automated by index, so this list only ever grows at the end: an id is
/// never removed, renamed or moved.
const ARTICULATION_ORDER: [&str; 77] = [
    // The fifteen original ids, in their original order.
    "sustain",
    "legato",
    "staccato",
    "spiccato",
    "tenuto",
    "marcato",
    "pizzicato",
    "tremolo",
    "trill",
    "col_legno",
    "harmonics",
    "sul_ponticello",
    "con_sordino",
    "flutter_tongue",
    "roll",
    // The established technique terms, alphabetical.
    "accent",
    "air_tone",
    "bartok_pizzicato",
    "bisbigliando",
    "choke",
    "col_legno_battuto",
    "col_legno_tratto",
    "crescendo_swell",
    "cuivre",
    "damped",
    "dead_stroke",
    "doit",
    "drag",
    "fall",
    "falsetto",
    "flam",
    "flautando",
    "fortepiano",
    "glissando",
    "growl",
    "half_valve",
    "harmonics_artificial",
    "harmonics_natural",
    "humming",
    "key_click",
    "let_ring",
    "multiphonic",
    "non_vibrato",
    "palm_mute",
    "portamento",
    "portato",
    "pres_de_la_table",
    "rim",
    "rimshot",
    "rip",
    "sforzando",
    "shake",
    "slap_tongue",
    "staccatissimo",
    "stopped",
    "sul_tasto",
    "tremolo_measured",
    "trill_half",
    "trill_whole",
    "vibrato_on",
    "vowel",
    "xylophonic",
    // The newer terms that at least one sample library records, alphabetical.
    "bells_up",
    "bowed",
    "multi_tongue",
    "mute_bucket",
    "mute_cup",
    "mute_harmon",
    "mute_plunger",
    "scoop",
    "scrape",
    "shake_stroke",
    "snares_off",
    "su_una_corda",
    "text",
    "wah_wah",
    "whisper",
];

#[test]
fn articulation_vocabulary_keeps_its_frozen_order() {
    let ids: Vec<&str> = Articulation::ALL.iter().map(|a| a.id()).collect();
    assert_eq!(ids, ARTICULATION_ORDER);
    // The groups after the original fifteen are each in strict alphabetical (byte) order.
    let (original, added) = ARTICULATION_ORDER.split_at(15);
    let (established, recorded) = added.split_at(47);
    for group in [established, recorded] {
        assert!(group.windows(2).all(|pair| pair[0] < pair[1]), "{group:?}");
    }
    // Exactly the original fifteen are the stable ids, and they never refine another id.
    for (position, articulation) in Articulation::ALL.into_iter().enumerate() {
        assert_eq!(
            articulation.is_stable(),
            position < original.len(),
            "{}",
            articulation.id()
        );
        if articulation.is_stable() {
            assert_eq!(articulation.parent(), None, "{}", articulation.id());
        }
    }
}

#[test]
fn articulation_lookup_is_strict() {
    for articulation in Articulation::ALL {
        assert_eq!(Articulation::from_id(articulation.id()), Some(articulation));
    }
    // No id names the ordinary sound: that is the absence of an articulation.
    assert_eq!(Articulation::from_id("ordinario"), None);
    // Display names, abbreviations and other spellings are not ids.
    for text in [
        "Col legno",
        "pizz.",
        "multitongue",
        "muted_string",
        "pitch_bend",
    ] {
        assert_eq!(Articulation::from_id(text), None, "{text}");
    }
    // Terms that no sample library records are not in the vocabulary yet.
    for text in ["fingered_tremolo", "detache", "rute", "cluster"] {
        assert_eq!(Articulation::from_id(text), None, "{text}");
    }
    // The struck col legno keeps an id of its own beside the general one, which it refines.
    assert_eq!(
        Articulation::from_id("col_legno_battuto"),
        Some(Articulation::ColLegnoBattuto)
    );
    assert_eq!(
        Articulation::ColLegnoBattuto.parent(),
        Some(Articulation::ColLegno)
    );
}

/// The members of each articulation kind, in the order of `Articulation::ALL`.
const ARTICULATION_KINDS: [(Kind, &[&str]); 10] = [
    (
        Kind::LongNote,
        &[
            "sustain",
            "tremolo",
            "flutter_tongue",
            "roll",
            "bisbigliando",
            "tremolo_measured",
        ],
    ),
    (
        Kind::ShortNote,
        &[
            "staccato",
            "spiccato",
            "tenuto",
            "pizzicato",
            "bartok_pizzicato",
            "choke",
            "dead_stroke",
            "portato",
            "staccatissimo",
            "multi_tongue",
            "shake_stroke",
        ],
    ),
    (
        Kind::Ornament,
        &[
            "trill",
            "drag",
            "flam",
            "shake",
            "trill_half",
            "trill_whole",
        ],
    ),
    (
        Kind::Transition,
        &[
            "legato",
            "doit",
            "fall",
            "glissando",
            "half_valve",
            "portamento",
            "rip",
            "scoop",
        ],
    ),
    (
        Kind::Effect,
        &[
            "col_legno",
            "harmonics",
            "air_tone",
            "col_legno_battuto",
            "col_legno_tratto",
            "growl",
            "harmonics_artificial",
            "harmonics_natural",
            "key_click",
            "multiphonic",
            "rim",
            "rimshot",
            "slap_tongue",
            "bowed",
            "scrape",
            "wah_wah",
        ],
    ),
    (
        Kind::TimbreModifier,
        &[
            "sul_ponticello",
            "cuivre",
            "flautando",
            "non_vibrato",
            "pres_de_la_table",
            "sul_tasto",
            "vibrato_on",
            "bells_up",
            "snares_off",
            "su_una_corda",
        ],
    ),
    (
        Kind::Mute,
        &[
            "con_sordino",
            "palm_mute",
            "stopped",
            "xylophonic",
            "mute_bucket",
            "mute_cup",
            "mute_harmon",
            "mute_plunger",
        ],
    ),
    (
        Kind::DynamicShape,
        &[
            "marcato",
            "accent",
            "crescendo_swell",
            "fortepiano",
            "sforzando",
        ],
    ),
    (Kind::KeyboardControl, &["damped", "let_ring"]),
    (
        Kind::Vocal,
        &["falsetto", "humming", "vowel", "text", "whisper"],
    ),
];

#[test]
fn every_articulation_has_one_kind_and_a_description() {
    let mut counted = 0;
    for (kind, expected) in ARTICULATION_KINDS {
        let members: Vec<&str> = Articulation::ALL
            .into_iter()
            .filter(|articulation| articulation.kind() == kind)
            .map(Articulation::id)
            .collect();
        assert_eq!(members, expected, "{kind:?}");
        counted += members.len();
    }
    assert_eq!(counted, Articulation::ALL.len());
    // Every articulation has a one-sentence description.
    for articulation in Articulation::ALL {
        let description = articulation.description();
        assert_eq!(description.trim(), description, "{}", articulation.id());
        assert!(
            description.starts_with(char::is_uppercase) && description.ends_with('.'),
            "{}: {description}",
            articulation.id()
        );
    }
}

#[test]
fn mute_and_vibrato_ids_name_passage_states_one_to_one() {
    // State to id: every state but the default is named by an id on its own axis.
    for mute in Mute::ALL {
        match mute.articulation() {
            None => assert_eq!(mute, Mute::Open),
            Some(articulation) => assert_eq!(articulation.axis(), Axis::Mute(mute), "{mute:?}"),
        }
    }
    for vibrato in Vibrato::ALL {
        match vibrato.articulation() {
            None => assert_eq!(vibrato, Vibrato::Ordinary),
            Some(articulation) => {
                assert_eq!(articulation.axis(), Axis::Vibrato(vibrato), "{vibrato:?}");
            }
        }
    }
    assert_eq!(Mute::default(), Mute::Open);
    assert_eq!(Vibrato::default(), Vibrato::Ordinary);
    // Id to state: the states map back, and the mute kind is exactly the mute axis.
    let mut mute_ids = Vec::new();
    let mut vibrato_ids = Vec::new();
    for articulation in Articulation::ALL {
        let id = articulation.id();
        match articulation.axis() {
            Axis::Primary => assert_ne!(articulation.kind(), Kind::Mute, "{id}"),
            Axis::Mute(mute) => {
                assert_eq!(mute.articulation(), Some(articulation), "{id}");
                assert_eq!(articulation.kind(), Kind::Mute, "{id}");
                mute_ids.push(id);
            }
            Axis::Vibrato(vibrato) => {
                assert_eq!(vibrato.articulation(), Some(articulation), "{id}");
                vibrato_ids.push(id);
            }
        }
    }
    assert_eq!(
        mute_ids,
        [
            "con_sordino",
            "palm_mute",
            "stopped",
            "xylophonic",
            "mute_bucket",
            "mute_cup",
            "mute_harmon",
            "mute_plunger"
        ]
    );
    assert_eq!(vibrato_ids, ["non_vibrato", "vibrato_on"]);
    assert_eq!(Mute::ALL.len(), mute_ids.len() + 1);
    assert_eq!(Vibrato::ALL.len(), vibrato_ids.len() + 1);
}

#[test]
fn refinements_fall_back_to_a_general_parent() {
    let parents: Vec<(&str, &str)> = Articulation::ALL
        .into_iter()
        .filter_map(|child| child.parent().map(|parent| (child.id(), parent.id())))
        .collect();
    assert_eq!(
        parents,
        [
            ("accent", "marcato"),
            ("bartok_pizzicato", "pizzicato"),
            ("choke", "damped"),
            ("col_legno_battuto", "col_legno"),
            ("flautando", "sul_tasto"),
            ("harmonics_artificial", "harmonics"),
            ("harmonics_natural", "harmonics"),
            ("shake", "trill"),
            ("staccatissimo", "staccato"),
            ("stopped", "con_sordino"),
            ("tremolo_measured", "tremolo"),
            ("trill_half", "trill"),
            ("trill_whole", "trill"),
            ("xylophonic", "palm_mute"),
            ("mute_bucket", "con_sordino"),
            ("mute_cup", "con_sordino"),
            ("mute_harmon", "con_sordino"),
            ("mute_plunger", "con_sordino"),
        ]
    );
    for articulation in Articulation::ALL {
        // A parent chain is acyclic and ends at an id without a parent.
        let mut chain = vec![articulation];
        let mut current = articulation;
        while let Some(parent) = current.parent() {
            assert!(!chain.contains(&parent), "{chain:?}");
            chain.push(parent);
            current = parent;
        }
        // A refinement stays on its parent's axis: a mute refines a mute, a primary id a primary
        // id.
        if let Some(parent) = articulation.parent() {
            assert_eq!(
                std::mem::discriminant(&articulation.axis()),
                std::mem::discriminant(&parent.axis()),
                "{}",
                articulation.id()
            );
        }
    }
}

#[test]
fn percussion_rolls_follow_the_vocabulary() {
    // A mallet instrument's roll is `tremolo`; the timpani and unpitched percussion roll with
    // `roll`.
    for instrument in instruments() {
        if instrument.family() != Family::Percussion {
            continue;
        }
        let articulations = instrument.articulations();
        let drum = instrument.instrument_type() == InstrumentType::Timpani
            || matches!(instrument.pitch(), PitchModel::Unpitched);
        if drum {
            assert!(
                !articulations.contains(&Articulation::Tremolo),
                "{}",
                instrument.id()
            );
        } else {
            assert!(
                !articulations.contains(&Articulation::Roll),
                "{}",
                instrument.id()
            );
        }
    }
}
