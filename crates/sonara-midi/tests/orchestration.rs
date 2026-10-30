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
        // A cited source names a URL or a page; an estimate gives its reason. Every text, notes
        // included, is trimmed and has single spaces, so its parts joined cleanly.
        let mut texts: Vec<&str> = provenances(instrument)
            .into_iter()
            .map(|provenance| match provenance {
                Provenance::Cited { source } => source,
                Provenance::Estimate { reason } => reason,
            })
            .collect();
        if let Some(profile) = instrument.ensemble() {
            texts.extend(Era::ALL.map(|era| profile.players(era).note));
        }
        for text in texts {
            assert_eq!(text.trim(), text, "{}", instrument.id());
            assert!(!text.contains("  "), "{}: {text}", instrument.id());
        }
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
/// independently of the data's `Clef` values). A tenor voice's usual clef is `treble_8vb`.
#[test]
fn record_clefs_match_the_expected_slugs() {
    let table: [(&str, &[&str]); 85] = [
        ("piccolo", &["treble"]),
        ("flute", &["treble"]),
        ("alto_flute", &["treble"]),
        ("bass_flute", &["treble"]),
        ("oboe", &["treble"]),
        ("oboe_damore", &["treble"]),
        ("english_horn", &["treble"]),
        ("clarinet_eb", &["treble"]),
        ("clarinet_bb", &["treble"]),
        ("clarinet_a", &["treble"]),
        ("bass_clarinet", &["treble", "bass"]),
        ("saxophone_soprano", &["treble"]),
        ("saxophone_alto", &["treble"]),
        ("saxophone_tenor", &["treble"]),
        ("saxophone_baritone", &["treble"]),
        ("bassoon", &["bass", "tenor", "treble"]),
        ("contrabassoon", &["bass", "tenor"]),
        ("horn_f", &["treble", "bass"]),
        ("wagner_tuba_tenor", &["treble", "bass"]),
        ("wagner_tuba_bass", &["treble", "bass"]),
        ("trumpet_piccolo", &["treble"]),
        ("trumpet_c", &["treble"]),
        ("trumpet_bb", &["treble"]),
        ("cornet", &["treble"]),
        ("flugelhorn", &["treble"]),
        ("trombone_tenor", &["bass", "tenor", "alto"]),
        ("trombone_bass", &["bass", "tenor"]),
        ("euphonium", &["bass", "tenor", "treble"]),
        ("tuba", &["bass"]),
        ("cimbasso", &["bass"]),
        ("timpani", &["bass"]),
        ("bass_drum", &["single_line", "bass", "percussion"]),
        ("snare_drum", &["single_line", "treble", "percussion"]),
        ("clash_cymbals", &["single_line", "bass", "percussion"]),
        ("suspended_cymbal", &["single_line", "bass", "percussion"]),
        ("tam_tam", &["single_line", "bass", "percussion"]),
        ("triangle", &["single_line", "treble", "percussion"]),
        ("tambourine", &["single_line", "treble", "percussion"]),
        ("castanets", &["single_line", "treble", "percussion"]),
        ("wood_block", &["single_line", "percussion"]),
        ("temple_blocks", &["percussion"]),
        ("sleigh_bells", &["single_line", "percussion"]),
        ("maracas", &["single_line", "percussion"]),
        ("shaker", &["single_line", "percussion"]),
        ("whip", &["single_line", "percussion"]),
        ("glockenspiel", &["treble"]),
        ("xylophone", &["treble"]),
        ("vibraphone", &["treble"]),
        ("marimba", &["treble", "bass"]),
        ("tubular_bells", &["treble"]),
        ("crotales", &["treble"]),
        ("carillon", &["treble", "bass"]),
        ("harp", &["treble", "bass"]),
        ("guitar_classical", &["treble"]),
        ("lute", &["treble"]),
        ("mandolin", &["treble"]),
        ("celesta", &["treble", "bass"]),
        ("piano_concert_grand", &["treble", "bass"]),
        ("piano_studio_grand", &["treble", "bass"]),
        ("piano_upright", &["treble", "bass"]),
        ("piano_felt", &["treble", "bass"]),
        ("piano_prepared", &["treble", "bass"]),
        ("piano_honky_tonk", &["treble", "bass"]),
        ("harpsichord", &["treble", "bass"]),
        ("clavichord", &["treble", "bass"]),
        ("electric_piano_rhodes", &["treble", "bass"]),
        ("electric_piano_wurlitzer", &["treble", "bass"]),
        ("clavinet", &["treble", "bass"]),
        ("organ_pipe", &["treble", "bass"]),
        ("organ_hammond", &["treble", "bass"]),
        ("accordion", &["treble", "bass"]),
        ("voice_soprano", &["treble"]),
        ("voice_mezzo_soprano", &["treble"]),
        ("voice_alto", &["treble"]),
        ("voice_tenor", &["treble_8vb"]),
        ("voice_baritone", &["bass"]),
        ("voice_bass", &["bass"]),
        ("choir_soprano", &["treble"]),
        ("choir_alto", &["treble"]),
        ("choir_tenor", &["treble_8vb", "bass"]),
        ("choir_bass", &["bass"]),
        ("violin", &["treble"]),
        ("viola", &["alto", "treble"]),
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
    assert!(instrument("Clarinet in B♭").is_none());
    assert_eq!(get("clarinet_bb").name(), "Clarinet in B♭");
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

/// The records are in score order, one per type, and each type finds its own record (TST-07).
#[test]
fn records_are_in_score_order_and_found_by_type() {
    assert_eq!(instruments(), &INSTRUMENTS[..]);
    assert_eq!(instruments().len(), InstrumentType::ALL.len());
    for (record, instrument_type) in instruments().iter().zip(InstrumentType::ALL) {
        assert_eq!(record.instrument_type(), instrument_type);
        assert_eq!(record.id(), instrument_type.id());
        assert_eq!(record.family(), instrument_type.family());
        let by_type = instrument_type.get().expect("every type has a record");
        assert!(std::ptr::eq(by_type, record), "{instrument_type:?}");
        let by_id = instrument(instrument_type.id());
        assert!(by_id.is_some_and(|found| std::ptr::eq(found, record)));
    }
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

/// The pitched records' researched values, written out independently of the data constants
/// (TST-10). A row holds the id, the transposition, the sounding playable range, the sounding
/// comfortable range (`-` when none is known), the written playable range in the record's first
/// clef, and the provenance kinds of the playable and comfortable ranges. The written range is
/// the one the sources publish, so it checks the transposition and the clef's octave against the
/// stored sounding range: the tenor voices are at transposition 0 and written an octave up in
/// `treble_8vb`.
const PITCHED: [&str; 71] = [
    "piccolo 12 74-108 80-106 62-96 cited cited",
    "flute 0 59-98 60-94 59-98 cited cited",
    "alto_flute -5 55-91 55-81 60-96 cited estimate",
    "bass_flute -12 47-84 48-81 59-96 cited cited",
    "oboe 0 58-91 60-87 58-91 cited cited",
    "oboe_damore -3 56-88 56-85 59-91 cited cited",
    "english_horn -7 52-84 52-81 59-91 cited cited",
    "clarinet_eb 3 55-96 63-94 52-93 cited cited",
    "clarinet_bb -2 50-94 50-91 52-96 cited cited",
    "clarinet_a -3 49-93 49-90 52-96 cited cited",
    "bass_clarinet -14 34-86 37-74 48-100 cited cited",
    "saxophone_soprano -2 56-94 56-87 58-96 cited cited",
    "saxophone_alto -9 49-87 49-80 58-96 cited cited",
    "saxophone_tenor -14 44-82 44-75 58-96 cited cited",
    "saxophone_baritone -21 36-75 37-68 57-96 cited cited",
    "bassoon 0 34-80 34-72 34-80 cited cited",
    "contrabassoon -12 22-62 22-57 34-74 cited cited",
    "horn_f -7 35-77 41-72 42-84 cited estimate",
    "wagner_tuba_tenor -2 27-82 40-74 29-84 cited estimate",
    "wagner_tuba_bass -7 27-77 34-67 34-84 cited estimate",
    "trumpet_piccolo 10 59-91 62-86 49-81 cited cited",
    "trumpet_c 0 54-89 54-84 54-89 cited cited",
    "trumpet_bb -2 52-91 52-82 54-93 cited cited",
    "cornet -2 52-91 52-82 54-93 cited cited",
    "flugelhorn -2 52-84 52-82 54-86 cited cited",
    "trombone_tenor 0 40-77 40-74 40-77 cited cited",
    "trombone_bass 0 22-77 31-67 22-77 cited estimate",
    "euphonium 0 23-74 40-67 23-74 cited cited",
    "tuba 0 21-67 29-60 21-67 cited cited",
    "cimbasso 0 29-65 30-60 29-65 cited estimate",
    "timpani 0 36-61 38-57 36-61 cited cited",
    "glockenspiel 24 72-108 79-108 48-84 cited cited",
    "xylophone 12 60-108 65-108 48-96 cited cited",
    "vibraphone 0 48-89 53-89 48-89 cited cited",
    "marimba 0 36-96 45-96 36-96 cited cited",
    "tubular_bells 0 51-79 60-77 51-79 cited cited",
    "crotales 24 84-108 84-108 60-84 cited cited",
    "carillon 0 48-96 - 48-96 estimate",
    "harp 0 23-103 36-96 23-103 cited estimate",
    "guitar_classical -12 40-83 40-76 52-95 estimate estimate",
    "lute -12 36-79 43-74 48-91 estimate estimate",
    "mandolin 0 55-91 55-88 55-91 estimate estimate",
    "celesta 12 48-113 60-108 36-101 cited cited",
    "piano_concert_grand 0 21-108 21-108 21-108 cited estimate",
    "piano_studio_grand 0 21-108 21-108 21-108 cited estimate",
    "piano_upright 0 21-108 21-108 21-108 cited estimate",
    "piano_felt 0 21-108 21-108 21-108 cited estimate",
    "piano_prepared 0 21-108 21-108 21-108 cited estimate",
    "piano_honky_tonk 0 21-108 21-108 21-108 estimate estimate",
    "harpsichord 0 29-89 36-84 29-89 cited estimate",
    "clavichord 0 29-96 36-86 29-96 cited cited",
    "electric_piano_rhodes 0 21-108 28-100 21-108 cited estimate",
    "electric_piano_wurlitzer 0 33-96 33-96 33-96 cited estimate",
    "clavinet 0 29-88 29-88 29-88 cited estimate",
    "organ_pipe 0 36-96 36-91 36-96 cited cited",
    "organ_hammond 0 36-96 36-96 36-96 cited estimate",
    "accordion 0 36-98 53-93 36-98 cited cited",
    "voice_soprano 0 60-88 62-83 60-88 cited cited",
    "voice_mezzo_soprano 0 57-84 59-79 57-84 cited cited",
    "voice_alto 0 53-79 55-77 53-79 cited cited",
    "voice_tenor 0 48-74 50-72 60-86 cited cited",
    "voice_baritone 0 45-69 47-67 45-69 cited cited",
    "voice_bass 0 40-65 43-64 40-65 cited cited",
    "choir_soprano 0 60-84 62-81 60-84 cited cited",
    "choir_alto 0 53-77 55-74 53-77 cited cited",
    "choir_tenor 0 48-70 50-69 60-82 cited cited",
    "choir_bass 0 36-66 43-62 36-66 cited cited",
    "violin 0 55-105 55-93 55-105 cited cited",
    "viola 0 48-93 48-81 48-93 cited cited",
    "cello 0 36-88 36-76 36-88 cited cited",
    "double_bass -12 23-67 28-55 35-79 cited cited",
];

/// A `low-high` range of a table row.
fn parse_range(text: &str) -> (i64, i64) {
    let (low, high) = text.split_once('-').expect("low-high");
    (low.parse().expect("low"), high.parse().expect("high"))
}

/// The records without a definite pitch.
const UNPITCHED: [&str; 14] = [
    "bass_drum",
    "snare_drum",
    "clash_cymbals",
    "suspended_cymbal",
    "tam_tam",
    "triangle",
    "tambourine",
    "castanets",
    "wood_block",
    "temple_blocks",
    "sleigh_bells",
    "maracas",
    "shaker",
    "whip",
];

/// The kind of a provenance, as written in the tables above.
fn kind(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::Cited { .. } => "cited",
        Provenance::Estimate { .. } => "estimate",
    }
}

#[test]
fn pitched_records_match_the_researched_values() {
    for row in PITCHED {
        let fields: Vec<&str> = row.split_whitespace().collect();
        let [id, semitones, playable, comfortable, written, kinds @ ..] = fields.as_slice() else {
            panic!("malformed row {row}");
        };
        let (id, kinds) = (*id, kinds.join(" "));
        let semitones: i8 = semitones.parse().expect("transposition");
        let (low, high) = parse_range(playable);
        let comfortable = (*comfortable != "-").then(|| parse_range(comfortable));
        let (written_low, written_high) = parse_range(written);
        let record = get(id);
        let PitchModel::Pitched(pitched) = record.pitch() else {
            panic!("{id} is pitched");
        };
        assert_eq!(pitched.transposition, Transposition::new(semitones), "{id}");
        assert_eq!(pitched.playable.range, PitchRange::new(low, high), "{id}");
        assert_eq!(
            pitched.comfortable.map(|sourced| sourced.range),
            comfortable.map(|(low, high)| PitchRange::new(low, high)),
            "{id}"
        );
        let mut recorded = vec![kind(pitched.playable.provenance)];
        recorded.extend(pitched.comfortable.map(|sourced| kind(sourced.provenance)));
        assert_eq!(recorded.join(" "), kinds, "{id}");
        let clef = record.clefs()[0];
        let written = |note| {
            record
                .written_in(clef, sounding(note))
                .map(WrittenPitch::get)
        };
        assert_eq!(
            (written(low), written(high)),
            (Some(written_low), Some(written_high)),
            "{id}"
        );
    }
    for id in UNPITCHED {
        assert_eq!(get(id).pitch(), &PitchModel::Unpitched, "{id}");
    }
    assert_eq!(PITCHED.len() + UNPITCHED.len(), INSTRUMENTS.len());
}

/// The sixteen records of the first dataset against the researched values that replaced them
/// (TST-06). A row holds the id, then the earlier playable and comfortable ranges and, after `|`,
/// the current ones, each a `low-high` range and its provenance kind, or `-` for none. Fourteen
/// records changed; the tenor trombone and the snare drum (unpitched, not listed) did not.
const EARLIER_AND_RESEARCHED: [&str; 15] = [
    "violin 55-108 cited 55-100 cited | 55-105 cited 55-93 cited",
    "cello 36-93 estimate - | 36-88 cited 36-76 cited",
    "double_bass 24-67 estimate 28-55 cited | 23-67 cited 28-55 cited",
    "flute 59-96 cited - | 59-98 cited 60-94 cited",
    "piccolo 74-108 cited - | 74-108 cited 80-106 cited",
    "oboe 58-91 cited - | 58-91 cited 60-87 cited",
    "english_horn 52-84 cited - | 52-84 cited 52-81 cited",
    "clarinet_bb 50-94 cited - | 50-94 cited 50-91 cited",
    "bassoon 34-80 cited 34-76 cited | 34-80 cited 34-72 cited",
    "contrabassoon 22-62 cited - | 22-62 cited 22-57 cited",
    "horn_f 35-77 estimate - | 35-77 cited 41-72 estimate",
    "trumpet_bb 52-82 cited - | 52-91 cited 52-82 cited",
    "trombone_tenor 40-77 cited 40-74 cited | 40-77 cited 40-74 cited",
    "glockenspiel 79-108 estimate - | 72-108 cited 79-108 cited",
    "xylophone 65-108 estimate - | 60-108 cited 65-108 cited",
];

/// The playable and comfortable ranges of a table row as `low-high kind` pairs, `-` for none.
fn parse_sourced(text: &str) -> Vec<Option<((i64, i64), &str)>> {
    let mut tokens = text.split_whitespace();
    let mut ranges = Vec::new();
    while let Some(token) = tokens.next() {
        ranges.push((token != "-").then(|| {
            let kind = tokens.next().expect("a provenance kind follows a range");
            (parse_range(token), kind)
        }));
    }
    ranges
}

#[test]
fn researched_ranges_replace_the_earlier_values() {
    let mut changed = 0;
    for row in EARLIER_AND_RESEARCHED {
        let (id, rest) = row.split_once(' ').expect("id");
        let (earlier, current) = rest.split_once('|').expect("earlier | current");
        let PitchModel::Pitched(pitched) = get(id).pitch() else {
            panic!("{id} is pitched");
        };
        let recorded: Vec<_> = [Some(&pitched.playable), pitched.comfortable.as_ref()]
            .into_iter()
            .map(|range| {
                range.map(|sourced| {
                    let (low, high) = (sourced.range.low(), sourced.range.high());
                    ((low.get(), high.get()), kind(sourced.provenance))
                })
            })
            .collect();
        assert_eq!(recorded, parse_sourced(current), "{id}");
        if parse_sourced(earlier) != parse_sourced(current) {
            changed += 1;
        }
    }
    assert_eq!(get("snare_drum").pitch(), &PitchModel::Unpitched);
    assert_eq!(changed, 14);
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
    // The violin plays G3-A7 (55-105) and is comfortable up to A6 (93); the flute plays B3-D7
    // (59-98) and is comfortable from C4 to Bb6 (60-94).
    let cases = [
        ("violin", 54, RangeCheck::BelowPlayable),
        ("violin", 55, RangeCheck::Within),
        ("violin", 93, RangeCheck::Within),
        ("violin", 94, RangeCheck::OutsideComfortable),
        ("violin", 105, RangeCheck::OutsideComfortable),
        ("violin", 106, RangeCheck::AbovePlayable),
        ("flute", 58, RangeCheck::BelowPlayable),
        ("flute", 59, RangeCheck::OutsideComfortable),
        ("flute", 60, RangeCheck::Within),
        ("flute", 94, RangeCheck::Within),
        ("flute", 95, RangeCheck::OutsideComfortable),
        ("flute", 98, RangeCheck::OutsideComfortable),
        ("flute", 99, RangeCheck::AbovePlayable),
        // No comfortable range known (a carillon's belongs to each tower): the playable range
        // alone decides.
        ("carillon", 47, RangeCheck::BelowPlayable),
        ("carillon", 48, RangeCheck::Within),
        ("carillon", 96, RangeCheck::Within),
        ("carillon", 97, RangeCheck::AbovePlayable),
    ];
    for (id, note, expected) in cases {
        assert_eq!(
            get(id).check_sounding(sounding(note)),
            Ok(expected),
            "{id} {note}"
        );
    }
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
    // Written A6 (93) sounds G6 (91), the top of the comfortable range; written C7 (96) sounds
    // Bb6 (94), the top of the playable one.
    assert_eq!(clarinet.check_written(written(93)), Ok(RangeCheck::Within));
    assert_eq!(
        clarinet.check_written(written(94)),
        Ok(RangeCheck::OutsideComfortable)
    );
    assert_eq!(
        clarinet.check_written(written(96)),
        Ok(RangeCheck::OutsideComfortable)
    );
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

/// Display names keep the first form of an instrument's name: an alternate in parentheses
/// ("Timpani (kettledrums)", "Cello (violoncello)") is not part of the name, and a flat is
/// spelled `♭`.
#[test]
fn display_names_drop_their_parenthetical_alternates() {
    let table = [
        ("clarinet_eb", "E♭ clarinet"),
        ("clarinet_bb", "Clarinet in B♭"),
        ("wagner_tuba_tenor", "Tenor Wagner tuba"),
        ("wagner_tuba_bass", "Bass Wagner tuba"),
        ("trumpet_bb", "Trumpet in B♭"),
        ("cornet", "Cornet in B♭"),
        ("timpani", "Timpani"),
        ("glockenspiel", "Glockenspiel"),
        ("tubular_bells", "Tubular bells"),
        ("crotales", "Crotales"),
        ("electric_piano_rhodes", "Rhodes piano"),
        ("voice_alto", "Contralto"),
        ("choir_soprano", "Sopranos"),
        ("choir_alto", "Altos"),
        ("choir_tenor", "Tenors"),
        ("choir_bass", "Basses"),
        ("cello", "Cello"),
    ];
    for (id, name) in table {
        assert_eq!(get(id).name(), name, "{id}");
    }
    for record in instruments() {
        assert!(!record.name().contains(['(', ')']), "{}", record.id());
    }
}

/// Ids that instruments idiomatically play but that the vocabulary does not have yet. They join
/// the vocabulary, and the records, when they are appended to it.
const NOT_YET_IN_VOCABULARY: [&str; 19] = [
    "behind_the_bridge",
    "cluster",
    "detache",
    "fingered_tremolo",
    "golpe",
    "jet_whistle",
    "left_hand_pizzicato",
    "loure",
    "martele",
    "rasgueado",
    "ricochet",
    "rute",
    "sautille",
    "slap_pizzicato",
    "strum",
    "subtone",
    "thumb_roll",
    "tongue_ram",
    "whistle_tone",
];

/// The ordinary sound carries no articulation id (the vocabulary's module doc): a mallet
/// instrument's ordinary stroke is not `staccato`, and the freely decaying note of a piano, a
/// harpsichord, a celesta or a plucked instrument is not `sustain`, which is the long, held note.
const ORDINARY_SOUND: [(&str, &str); 14] = [
    ("xylophone", "staccato"),
    ("marimba", "staccato"),
    ("piano_concert_grand", "sustain"),
    ("piano_studio_grand", "sustain"),
    ("piano_upright", "sustain"),
    ("piano_felt", "sustain"),
    ("piano_prepared", "sustain"),
    ("piano_honky_tonk", "sustain"),
    ("harpsichord", "sustain"),
    ("celesta", "sustain"),
    ("harp", "sustain"),
    ("guitar_classical", "sustain"),
    ("lute", "sustain"),
    ("mandolin", "sustain"),
];

/// Each record's idiomatic articulations, written out independently of the data constants
/// (TST-10): a row is the id, then the ids of the original vocabulary and, after `|`, the newer
/// ones, in the record's order. An id of [`NOT_YET_IN_VOCABULARY`] is left out of the record, and
/// so is the ordinary sound of [`ORDINARY_SOUND`].
const ARTICULATION_LISTS: [&str; 85] = [
    "piccolo: sustain legato staccato tenuto marcato trill flutter_tongue | staccatissimo \
        multi_tongue fingered_tremolo sforzando fortepiano crescendo_swell rip fall key_click \
        air_tone vibrato_on",
    "flute: sustain legato staccato tenuto marcato trill flutter_tongue harmonics | staccatissimo \
        portato multi_tongue fingered_tremolo sforzando crescendo_swell rip fall glissando \
        key_click slap_tongue tongue_ram air_tone jet_whistle whistle_tone multiphonic vibrato_on \
        non_vibrato",
    "alto_flute: sustain legato staccato tenuto marcato trill flutter_tongue | staccatissimo \
        portato multi_tongue sforzando crescendo_swell rip fall key_click air_tone vibrato_on \
        non_vibrato",
    "bass_flute: sustain legato staccato tenuto marcato trill flutter_tongue | staccatissimo \
        portato multi_tongue sforzando crescendo_swell rip fall vibrato_on",
    "oboe: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | portato \
        sforzando fortepiano crescendo_swell trill_half trill_whole fingered_tremolo glissando \
        multiphonic key_click slap_tongue vibrato_on non_vibrato",
    "oboe_damore: sustain legato staccato tenuto marcato trill flutter_tongue | portato \
        trill_half trill_whole",
    "english_horn: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | \
        portato sforzando fortepiano crescendo_swell trill_half trill_whole fingered_tremolo \
        glissando bisbigliando vibrato_on non_vibrato",
    "clarinet_eb: sustain legato staccato trill flutter_tongue | staccatissimo portato sforzando \
        trill_half trill_whole crescendo_swell",
    "clarinet_bb: sustain legato staccato tenuto marcato trill flutter_tongue | staccatissimo \
        portato sforzando trill_half trill_whole fingered_tremolo multi_tongue glissando growl \
        crescendo_swell",
    "clarinet_a: sustain legato staccato tenuto marcato trill flutter_tongue | staccatissimo \
        portato sforzando trill_half trill_whole fingered_tremolo multi_tongue glissando growl \
        crescendo_swell",
    "bass_clarinet: sustain legato staccato marcato trill flutter_tongue | staccatissimo portato \
        sforzando trill_half trill_whole multi_tongue",
    "saxophone_soprano: sustain legato staccato tenuto marcato trill flutter_tongue harmonics | \
        staccatissimo fortepiano crescendo_swell vibrato_on non_vibrato trill_half trill_whole \
        glissando scoop fall doit growl subtone slap_tongue multiphonic key_click",
    "saxophone_alto: sustain legato staccato tenuto marcato trill flutter_tongue harmonics | \
        staccatissimo fortepiano crescendo_swell vibrato_on non_vibrato trill_half trill_whole \
        glissando scoop fall doit rip growl subtone slap_tongue multiphonic key_click",
    "saxophone_tenor: sustain legato staccato tenuto marcato trill flutter_tongue harmonics | \
        staccatissimo fortepiano crescendo_swell vibrato_on non_vibrato trill_half trill_whole \
        glissando scoop fall doit rip growl subtone slap_tongue multiphonic key_click",
    "saxophone_baritone: sustain legato staccato tenuto marcato trill flutter_tongue harmonics | \
        staccatissimo fortepiano crescendo_swell vibrato_on non_vibrato trill_half trill_whole \
        glissando scoop fall doit rip growl subtone slap_tongue multiphonic key_click",
    "bassoon: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | portato \
        sforzando crescendo_swell trill_half trill_whole fingered_tremolo glissando multiphonic \
        key_click slap_tongue vibrato_on non_vibrato",
    "contrabassoon: sustain legato staccato tenuto marcato flutter_tongue | sforzando fortepiano \
        vibrato_on",
    "horn_f: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | stopped \
        cuivre bells_up glissando rip fall staccatissimo sforzando fortepiano crescendo_swell \
        trill_whole trill_half multi_tongue multiphonic",
    "wagner_tuba_tenor: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | \
        portato sforzando fortepiano glissando multi_tongue vibrato_on",
    "wagner_tuba_bass: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | \
        portato sforzando fortepiano glissando multi_tongue vibrato_on",
    "trumpet_piccolo: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | \
        portato sforzando fortepiano multi_tongue vibrato_on non_vibrato",
    "trumpet_c: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | accent \
        sforzando fortepiano crescendo_swell portato glissando half_valve cuivre multi_tongue \
        vibrato_on non_vibrato mute_cup mute_harmon mute_bucket",
    "trumpet_bb: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | accent \
        sforzando fortepiano crescendo_swell portato glissando portamento half_valve fall doit \
        shake scoop rip growl cuivre multi_tongue vibrato_on non_vibrato mute_cup mute_harmon \
        mute_plunger mute_bucket wah_wah",
    "cornet: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | accent \
        portato multi_tongue glissando half_valve fall shake growl vibrato_on mute_cup \
        mute_harmon mute_plunger wah_wah",
    "flugelhorn: sustain legato staccato tenuto marcato trill flutter_tongue | portato \
        multi_tongue vibrato_on fall shake glissando half_valve",
    "trombone_tenor: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | \
        glissando staccatissimo sforzando fortepiano crescendo_swell rip fall doit cuivre \
        vibrato_on growl",
    "trombone_bass: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | \
        glissando staccatissimo sforzando fortepiano crescendo_swell rip fall cuivre vibrato_on",
    "euphonium: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | \
        staccatissimo sforzando crescendo_swell vibrato_on half_valve",
    "tuba: sustain legato staccato tenuto marcato trill flutter_tongue con_sordino | \
        staccatissimo sforzando fortepiano crescendo_swell rip fall half_valve vibrato_on",
    "cimbasso: sustain legato staccato tenuto marcato flutter_tongue con_sordino | staccatissimo \
        sforzando crescendo_swell cuivre rip fall half_valve",
    "timpani: roll marcato con_sordino | let_ring damped glissando crescendo_swell \
        tremolo_measured fortepiano",
    "bass_drum: roll marcato con_sordino | damped let_ring rute",
    "snare_drum: roll marcato con_sordino | flam drag rimshot rim snares_off",
    "clash_cymbals: roll marcato | let_ring damped choke",
    "suspended_cymbal: roll marcato | crescendo_swell let_ring damped choke bowed scrape",
    "tam_tam: roll marcato | let_ring damped crescendo_swell scrape bowed",
    "triangle: roll marcato | let_ring damped",
    "tambourine: roll marcato | thumb_roll shake_stroke",
    "castanets: roll marcato | ",
    "wood_block: marcato roll | ",
    "temple_blocks: marcato roll | glissando",
    "sleigh_bells: roll marcato | shake_stroke",
    "maracas: roll marcato | shake_stroke",
    "shaker: roll marcato | shake_stroke",
    "whip: marcato | ",
    "glockenspiel: staccato marcato tremolo trill | let_ring damped glissando bowed",
    "xylophone: staccato marcato tremolo | glissando dead_stroke bowed",
    "vibraphone: staccato marcato tremolo | let_ring damped dead_stroke glissando bowed \
        vibrato_on non_vibrato portamento",
    "marimba: staccato marcato tremolo trill | glissando dead_stroke bowed",
    "tubular_bells: marcato tremolo | let_ring damped glissando",
    "crotales: marcato tremolo | let_ring damped bowed",
    "carillon: tremolo marcato | let_ring",
    "harp: sustain harmonics tremolo trill | glissando bisbigliando pres_de_la_table let_ring \
        damped portamento xylophonic",
    "guitar_classical: sustain legato tremolo trill harmonics sul_ponticello | harmonics_natural \
        harmonics_artificial sul_tasto rasgueado strum palm_mute bartok_pizzicato golpe \
        vibrato_on let_ring damped glissando",
    "lute: sustain trill | let_ring",
    "mandolin: sustain staccato tremolo | strum let_ring",
    "celesta: sustain staccato legato trill tremolo | ",
    "piano_concert_grand: sustain legato staccato tenuto marcato trill tremolo | staccatissimo \
        portato accent sforzando glissando let_ring cluster",
    "piano_studio_grand: sustain legato staccato tenuto marcato trill tremolo | staccatissimo \
        portato accent sforzando glissando let_ring cluster",
    "piano_upright: sustain legato staccato tenuto marcato trill tremolo | staccatissimo portato \
        accent sforzando glissando let_ring cluster",
    "piano_felt: sustain legato staccato tenuto | portato let_ring",
    "piano_prepared: sustain staccato marcato tremolo harmonics pizzicato | let_ring glissando \
        damped cluster palm_mute",
    "piano_honky_tonk: sustain staccato marcato trill tremolo | accent glissando let_ring",
    "harpsichord: sustain legato staccato trill | ",
    "clavichord: sustain legato staccato trill | vibrato_on portato",
    "electric_piano_rhodes: sustain legato staccato tenuto marcato trill | glissando",
    "electric_piano_wurlitzer: sustain legato staccato tenuto marcato trill | glissando",
    "clavinet: sustain legato staccato marcato trill | portato damped",
    "organ_pipe: sustain legato staccato tenuto trill | glissando",
    "organ_hammond: sustain legato staccato | glissando",
    "accordion: sustain legato staccato tremolo | crescendo_swell air_tone",
    "voice_soprano: sustain legato staccato tenuto marcato trill | vowel text humming whisper \
        crescendo_swell sforzando vibrato_on non_vibrato portamento",
    "voice_mezzo_soprano: sustain legato staccato tenuto marcato trill | vowel text humming \
        whisper crescendo_swell sforzando vibrato_on non_vibrato portamento",
    "voice_alto: sustain legato staccato tenuto marcato trill | vowel text humming whisper \
        crescendo_swell sforzando vibrato_on non_vibrato portamento",
    "voice_tenor: sustain legato staccato tenuto marcato trill | vowel text humming whisper \
        crescendo_swell sforzando vibrato_on non_vibrato portamento falsetto",
    "voice_baritone: sustain legato staccato tenuto marcato trill | vowel text humming whisper \
        crescendo_swell sforzando vibrato_on non_vibrato portamento falsetto",
    "voice_bass: sustain legato staccato tenuto marcato trill | vowel text humming whisper \
        crescendo_swell sforzando vibrato_on non_vibrato portamento falsetto",
    "choir_soprano: sustain legato staccato tenuto marcato | vowel text humming whisper \
        crescendo_swell sforzando vibrato_on non_vibrato portamento",
    "choir_alto: sustain legato staccato tenuto marcato | vowel text humming whisper \
        crescendo_swell sforzando vibrato_on non_vibrato portamento",
    "choir_tenor: sustain legato staccato tenuto marcato | vowel text humming whisper \
        crescendo_swell sforzando vibrato_on non_vibrato portamento falsetto",
    "choir_bass: sustain legato staccato tenuto marcato | vowel text humming whisper \
        crescendo_swell sforzando vibrato_on non_vibrato portamento falsetto",
    "violin: sustain legato staccato spiccato tenuto marcato pizzicato tremolo trill col_legno \
        harmonics sul_ponticello con_sordino | detache martele sautille ricochet loure portato \
        sul_tasto flautando bartok_pizzicato left_hand_pizzicato col_legno_battuto \
        col_legno_tratto tremolo_measured fingered_tremolo harmonics_natural harmonics_artificial \
        trill_half trill_whole portamento glissando fortepiano crescendo_swell non_vibrato \
        vibrato_on su_una_corda behind_the_bridge",
    "viola: sustain legato staccato spiccato tenuto marcato pizzicato tremolo trill col_legno \
        harmonics sul_ponticello con_sordino | detache martele sautille ricochet loure portato \
        sul_tasto flautando bartok_pizzicato left_hand_pizzicato col_legno_battuto \
        col_legno_tratto tremolo_measured fingered_tremolo harmonics_natural harmonics_artificial \
        trill_half trill_whole portamento glissando fortepiano crescendo_swell non_vibrato \
        vibrato_on su_una_corda behind_the_bridge",
    "cello: sustain legato staccato spiccato tenuto marcato pizzicato tremolo trill col_legno \
        harmonics sul_ponticello con_sordino | detache martele sautille ricochet loure portato \
        sul_tasto flautando bartok_pizzicato left_hand_pizzicato col_legno_battuto \
        col_legno_tratto tremolo_measured fingered_tremolo harmonics_natural harmonics_artificial \
        trill_half trill_whole portamento glissando fortepiano crescendo_swell non_vibrato \
        vibrato_on su_una_corda behind_the_bridge",
    "double_bass: sustain legato staccato spiccato tenuto marcato pizzicato tremolo trill \
        col_legno harmonics sul_ponticello con_sordino | detache martele loure portato sul_tasto \
        flautando bartok_pizzicato left_hand_pizzicato slap_pizzicato col_legno_battuto \
        col_legno_tratto tremolo_measured fingered_tremolo harmonics_natural harmonics_artificial \
        trill_half trill_whole portamento glissando fortepiano crescendo_swell non_vibrato \
        vibrato_on",
];

#[test]
fn articulation_lists_match_the_researched_ones() {
    let mut not_yet = std::collections::BTreeSet::new();
    for row in ARTICULATION_LISTS {
        let (id, lists) = row.split_once(':').expect("id: lists");
        let (original, newer) = lists.split_once('|').expect("original | newer");
        let mut expected = Vec::new();
        for slug in original.split_whitespace().chain(newer.split_whitespace()) {
            let is_ordinary = ORDINARY_SOUND.contains(&(id, slug));
            match Articulation::from_id(slug) {
                Some(articulation) if !is_ordinary => expected.push(articulation),
                Some(_) => {}
                None => {
                    not_yet.insert(slug);
                }
            }
        }
        // The original ids are all in the vocabulary.
        assert!(
            original
                .split_whitespace()
                .all(|slug| Articulation::from_id(slug).is_some_and(Articulation::is_stable)),
            "{id}"
        );
        assert_eq!(get(id).articulations(), expected.as_slice(), "{id}");
        assert!(!expected.is_empty(), "{id}");
    }
    assert_eq!(
        not_yet.into_iter().collect::<Vec<_>>(),
        NOT_YET_IN_VOCABULARY
    );
    assert_eq!(ARTICULATION_LISTS.len(), INSTRUMENTS.len());
}

#[test]
fn the_ordinary_sound_carries_no_articulation() {
    for (id, slug) in ORDINARY_SOUND {
        let articulation = Articulation::from_id(slug).expect("in the vocabulary");
        assert!(!get(id).articulations().contains(&articulation), "{id}");
        assert_eq!(
            get(id).articulation_availability(slug, &[]),
            ArticulationAvailability::NotIdiomatic,
            "{id}"
        );
    }
    // A held note on an instrument that sustains, and a short note on one whose notes ring, keep
    // their ids.
    assert!(
        get("organ_pipe")
            .articulations()
            .contains(&Articulation::Sustain)
    );
    assert!(
        get("vibraphone")
            .articulations()
            .contains(&Articulation::Staccato)
    );
}

/// The instruments recorded as sections or as both soloist and section; every other one is a
/// soloist.
const SECTION: [&str; 6] = [
    "wagner_tuba_tenor",
    "wagner_tuba_bass",
    "choir_soprano",
    "choir_alto",
    "choir_tenor",
    "choir_bass",
];
const BOTH: [&str; 18] = [
    "flute",
    "oboe",
    "clarinet_bb",
    "clarinet_a",
    "bassoon",
    "horn_f",
    "trumpet_c",
    "trumpet_bb",
    "cornet",
    "flugelhorn",
    "trombone_tenor",
    "trombone_bass",
    "euphonium",
    "harp",
    "violin",
    "viola",
    "cello",
    "double_bass",
];

/// Every record carries an ensemble profile with the researched forms, and a use stated without
/// a number of players says what it is (TST-07).
#[test]
fn ensemble_profiles_record_forms_and_players() {
    for record in instruments() {
        let profile = record.ensemble().expect("every record has a profile");
        let expected = if SECTION.contains(&record.id()) {
            Forms::Section
        } else if BOTH.contains(&record.id()) {
            Forms::Both
        } else {
            Forms::Solo
        };
        assert_eq!(profile.forms, expected, "{}", record.id());
        for era in Era::ALL {
            let players = profile.players(era);
            if players.players.is_none() {
                assert!(!players.note.trim().is_empty(), "{} {era:?}", record.id());
            }
        }
    }
    // A few counts read from their sources (TST-10).
    let cases = [
        (
            "horn_f",
            Era::Romantic,
            Some(PlayerRange::new(4, 4)),
            "estimate",
        ),
        (
            "violin",
            Era::Romantic,
            Some(PlayerRange::new(30, 32)),
            "cited",
        ),
        ("viola", Era::Film, Some(PlayerRange::new(12, 12)), "cited"),
        (
            "oboe",
            Era::Classical,
            Some(PlayerRange::new(2, 2)),
            "cited",
        ),
        ("oboe", Era::Film, Some(PlayerRange::new(2, 3)), "estimate"),
        (
            "bass_flute",
            Era::Classical,
            Some(PlayerRange::NOT_USED),
            "estimate",
        ),
        ("clarinet_eb", Era::Chamber, None, "estimate"),
        (
            "choir_soprano",
            Era::Romantic,
            Some(PlayerRange::new(8, 32)),
            "cited",
        ),
        ("choir_bass", Era::Chamber, None, "cited"),
        (
            "tam_tam",
            Era::Classical,
            Some(PlayerRange::NOT_USED),
            "cited",
        ),
    ];
    for (id, era, expected, provenance) in cases {
        let players = get(id).ensemble().expect("recorded").players(era);
        assert_eq!(players.players, expected, "{id} {era:?}");
        assert_eq!(kind(players.provenance), provenance, "{id} {era:?}");
    }
    assert_eq!(
        get("horn_f")
            .ensemble()
            .map(|profile| profile.romantic.note),
        Some("6-8 in large scores; Wagner's Ring 8")
    );
}
