//! Contract tests of the orchestration metadata through its public API (SON-P06).

use std::ops::Range;

use proptest::collection::vec;
use proptest::prelude::*;
use sonara_midi::gm::GM_MAP;
use sonara_midi::orchestration::{
    Action, Alias, Articulation, ArticulationAvailability, Axis, CategoryId, CheckError, Clef,
    Controller, Decay, EnsembleProfile, Era, EraPlayers, Fact, FactKind, Family, Forms, GmNote,
    GmProgram, GmTarget, GmVoice, INSTRUMENTS, Instrument, InstrumentError, InstrumentSpec,
    InstrumentType, KeyboardMechanics, Kind, Lang, LetterName, Mute, Names, Noise, Number,
    ORCHESTRAL_PERCUSSION_MAP, Pedal, PedalKind, PitchModel, PitchRange, Pitched, PlayerRange,
    Provenance, RangeCheck, Request, RequestAvailability, RequestError, Resonance, Selection,
    SoundingPitch, SourcedRange, Span, SubstituteFit, Target, TimbreState, Transposition, Vibrato,
    WrittenPitch, all_aliases, category, fold, instrument, instruments, percussion_instrument,
    resolve,
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

/// Display names that pass the checks.
const NAMES: Names = Names {
    en: "Fixture",
    cs: "Fixture",
    de: "Fixture",
    fr: "Fixture",
    it: "Fixture",
    abbreviation: "Fx.",
};

/// A valid unpitched spec: the base of the fixtures that break one field each. Its GM voice,
/// `NoEquivalent`, fits a pitched and an unpitched instrument alike.
const VALID: InstrumentSpec = InstrumentSpec {
    names: NAMES,
    articulations_provenance: ESTIMATE,
    gm: GmVoice::NoEquivalent,
    ..InstrumentSpec::EMPTY
};

/// The GM1 channel-10 key `number`; a fixture outside 35-81 does not compile.
const fn key(number: u8) -> GmNote {
    match GmNote::new(number) {
        Some(note) => note,
        None => panic!("GM1 channel-10 keys are 35-81"),
    }
}

/// 81 Open Triangle, then 80 Mute Triangle.
const TRIANGLE_KEYS: &[GmNote] = &[key(81), key(80)];

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

/// The controller `number`; a fixture above 119 does not compile.
const fn cc(number: u8) -> Controller {
    match Controller::new(number) {
        Some(controller) => controller,
        None => panic!("a pedal sends a controller of 0-119"),
    }
}

/// A pedal of `kind` on the controller `number`.
const fn pedal(kind: PedalKind, number: u8) -> Pedal {
    Pedal {
        kind,
        controller: Some(cc(number)),
    }
}

/// A damper pedal on controller 64 and a practice muffler without a controller.
const DAMPER_AND_MUFFLER: &[Pedal] = &[
    pedal(PedalKind::Sustain, 64),
    Pedal {
        kind: PedalKind::PracticeMuffler,
        controller: None,
    },
];

/// Valid keyboard mechanics with keys from `low` to `high`: a damper pedal and a practice muffler
/// without a controller, and two noises.
fn mechanics(low: i64, high: i64) -> KeyboardMechanics {
    KeyboardMechanics {
        action: Action::Upright,
        keys: PitchRange::new(low, high),
        pedals: DAMPER_AND_MUFFLER,
        resonance: Resonance::DampersRaised,
        noises: &[Noise::KeyRelease, Noise::DamperReturn],
        decay: Decay::TwoStage,
        timbre: None,
        note: "",
        provenance: ESTIMATE,
    }
}

/// Every provenance of a record: the articulation list, the playable and comfortable ranges, each
/// era of the ensemble profile, a GM substitute, the keyboard mechanics and each playability fact.
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
    if let GmVoice::Substitute { provenance, .. } = instrument.gm() {
        all.push(provenance);
    }
    if let Some(keyboard) = instrument.keyboard() {
        all.push(keyboard.provenance);
    }
    all.extend(instrument.facts().iter().map(|fact| fact.provenance));
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
        assert_eq!(spec.names.en, instrument.name());
        assert_eq!(&spec.names, instrument.names());
        assert_eq!(spec.aliases, instrument.aliases());
        assert_eq!(spec.category, instrument.category());
        assert_eq!(&spec.pitch, instrument.pitch());
        assert_eq!(spec.articulations, instrument.articulations());
        assert_eq!(
            spec.articulations_provenance,
            instrument.articulations_provenance()
        );
        assert_eq!(spec.ensemble.as_ref(), instrument.ensemble());
        assert_eq!(spec.gm, instrument.gm());
        assert_eq!(spec.keyboard.as_ref(), instrument.keyboard());
        assert_eq!(spec.facts, instrument.facts());
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
        if let Some(keyboard) = instrument.keyboard() {
            texts.push(keyboard.note);
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
fn empty_spec_needs_a_name_an_articulation_provenance_and_a_gm_voice() {
    let empty = InstrumentSpec::EMPTY;
    for lang in Lang::ALL {
        assert_eq!(empty.names.get(lang), "");
    }
    assert_eq!(empty.names.abbreviation, "");
    assert_eq!(empty.aliases, []);
    assert_eq!(empty.category, None);
    assert_eq!(empty.pitch, PitchModel::Unpitched);
    assert_eq!(empty.articulations, []);
    assert_eq!(empty.clefs, []);
    assert_eq!(empty.ensemble, None);
    assert_eq!(empty.keyboard, None);
    assert_eq!(empty.facts, []);
    assert_eq!(empty.gm, GmVoice::Percussion(&[]));
    assert_eq!(
        Instrument::try_new(InstrumentType::Triangle, empty),
        Err(InstrumentError::EmptyName)
    );
    let named = InstrumentSpec {
        names: Names {
            en: "Triangle",
            cs: "triangl",
            de: "Triangel",
            fr: "triangle",
            it: "triangolo",
            abbreviation: "Trgl.",
        },
        ..InstrumentSpec::EMPTY
    };
    assert_eq!(
        Instrument::try_new(InstrumentType::Triangle, named),
        Err(InstrumentError::EmptyProvenance)
    );
    let provenance_stated = InstrumentSpec {
        articulations_provenance: ESTIMATE,
        ..named
    };
    assert_eq!(
        Instrument::try_new(InstrumentType::Triangle, provenance_stated),
        Err(InstrumentError::NoGmKeys)
    );
    let stated = InstrumentSpec {
        gm: GmVoice::Percussion(TRIANGLE_KEYS),
        ..provenance_stated
    };
    let triangle = Instrument::try_new(InstrumentType::Triangle, stated).expect("valid");
    assert_eq!(triangle.name(), "Triangle");
    assert_eq!(triangle.spec(), stated);
    assert_eq!(triangle.ensemble(), None);
    assert_eq!(triangle.written_playable(), None);
    assert_eq!(triangle.gm(), GmVoice::Percussion(TRIANGLE_KEYS));
    assert_eq!(triangle.keyboard(), None);
    assert_eq!(triangle.facts(), []);
}

/// Each of the five names and the abbreviation must be stated (TST-03).
#[test]
fn blank_names_are_rejected() {
    let blank_one: [fn(&mut Names, &'static str); 6] = [
        |names, text| names.en = text,
        |names, text| names.cs = text,
        |names, text| names.de = text,
        |names, text| names.fr = text,
        |names, text| names.it = text,
        |names, text| names.abbreviation = text,
    ];
    for (field, blank) in blank_one.iter().enumerate() {
        for text in ["", " ", "\t\n"] {
            let mut names = NAMES;
            blank(&mut names, text);
            assert_eq!(
                Instrument::try_new(InstrumentType::Triangle, InstrumentSpec { names, ..VALID }),
                Err(InstrumentError::EmptyName),
                "field {field}, {text:?}"
            );
        }
    }
    assert!(Instrument::try_new(InstrumentType::Triangle, VALID).is_ok());
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

/// A fact that fits the timpani, with a blank provenance.
const BLANK_FACT: &[Fact] = &[Fact {
    provenance: Provenance::Estimate { reason: " " },
    ..RETUNE
}];

/// Every provenance of a record names a source or a reason: the playable and comfortable
/// ranges, the articulation list, each era of the ensemble profile, the keyboard mechanics and
/// each playability fact (TST-03).
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
    cases.push(InstrumentSpec {
        pitch: pitched(range(40, 80, ESTIMATE), None),
        keyboard: Some(KeyboardMechanics {
            provenance: blank_cited,
            ..mechanics(40, 80)
        }),
        ..VALID
    });
    cases.push(InstrumentSpec {
        facts: BLANK_FACT,
        ..VALID
    });
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
        keyboard: Some(mechanics(40, 80)),
        facts: &[RETUNE],
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

/// A request holds one primary articulation and the two passage states; a mute or vibrato id is
/// never its primary (PRI-04, TST-03).
#[test]
fn requests_reject_a_passage_state_as_the_primary() {
    let request = Request::new(Some(Articulation::Pizzicato), Mute::Generic, Vibrato::Off)
        .expect("pizzicato is a primary articulation");
    assert_eq!(request.primary(), Some(Articulation::Pizzicato));
    assert_eq!(request.mute(), Mute::Generic);
    assert_eq!(request.vibrato(), Vibrato::Off);
    // No primary is the ordinary sound; the default request is ordinario, open, ordinary vibrato.
    let ordinario = Request::new(None, Mute::Open, Vibrato::Ordinary).expect("no primary");
    assert_eq!(ordinario, Request::default());
    assert_eq!(
        (ordinario.primary(), ordinario.mute(), ordinario.vibrato()),
        (None, Mute::Open, Vibrato::Ordinary)
    );
    // A state id is rejected even when the request also states that state.
    assert_eq!(
        Request::new(
            Some(Articulation::ConSordino),
            Mute::Open,
            Vibrato::Ordinary
        ),
        Err(RequestError::NotPrimary(Articulation::ConSordino))
    );
    assert_eq!(
        Request::new(Some(Articulation::VibratoOn), Mute::Open, Vibrato::On),
        Err(RequestError::NotPrimary(Articulation::VibratoOn))
    );
    assert_eq!(
        RequestError::NotPrimary(Articulation::MuteCup).to_string(),
        "articulation `mute_cup` names a passage state, not a note's primary articulation"
    );
    // Exactly the eight mute ids and the two vibrato ids are rejected, in vocabulary order.
    let rejected: Vec<&str> = Articulation::ALL
        .into_iter()
        .filter(|&articulation| {
            Request::new(Some(articulation), Mute::Open, Vibrato::Ordinary).is_err()
        })
        .map(Articulation::id)
        .collect();
    assert_eq!(
        rejected,
        [
            "con_sordino",
            "non_vibrato",
            "palm_mute",
            "stopped",
            "vibrato_on",
            "xylophonic",
            "mute_bucket",
            "mute_cup",
            "mute_harmon",
            "mute_plunger",
        ]
    );
    // Every other id combines with every mute and every vibrato, and the request keeps them.
    for primary in Articulation::ALL {
        for mute in Mute::ALL {
            for vibrato in Vibrato::ALL {
                let built = Request::new(Some(primary), mute, vibrato);
                if rejected.contains(&primary.id()) {
                    assert_eq!(built, Err(RequestError::NotPrimary(primary)));
                } else {
                    let request = built.expect("a primary articulation");
                    assert_eq!(
                        (request.primary(), request.mute(), request.vibrato()),
                        (Some(primary), mute, vibrato)
                    );
                }
            }
        }
    }
}

/// Each part of a request is answered on its own, as a single id would be: only a library
/// declaration makes a stated part supported, and a part at its default needs none.
#[test]
fn request_availability_answers_each_part() {
    use ArticulationAvailability::{IdiomaticUnverified, NotIdiomatic, Supported};
    let request = |primary, mute, vibrato| {
        Request::new(primary, mute, vibrato).expect("the primary is a primary articulation")
    };
    let each = |primary, mute, vibrato| RequestAvailability {
        primary,
        mute,
        vibrato,
    };
    let trumpet = get("trumpet_c");
    assert_eq!(
        trumpet.request_availability(
            request(Some(Articulation::Sustain), Mute::Cup, Vibrato::Off),
            &["sustain", "mute_cup", "non_vibrato"],
        ),
        each(Supported, Supported, Supported)
    );
    // The C trumpet plays staccato, the harmon mute and vibrato, but the library declared none.
    assert_eq!(
        trumpet.request_availability(
            request(Some(Articulation::Staccato), Mute::Harmon, Vibrato::On),
            &["sustain", "mute_cup"],
        ),
        each(
            IdiomaticUnverified,
            IdiomaticUnverified,
            IdiomaticUnverified
        )
    );
    // The horn is stopped, but its record has no vibrato id.
    assert_eq!(
        get("horn_f").request_availability(
            request(Some(Articulation::Sustain), Mute::Stopped, Vibrato::On),
            &["sustain"],
        ),
        each(Supported, IdiomaticUnverified, NotIdiomatic)
    );
    // A declared general mute does not answer for a specific one: falling back from the cup mute
    // to `con_sordino` is a substitution, not availability.
    assert_eq!(
        get("violin").request_availability(
            request(Some(Articulation::Pizzicato), Mute::Cup, Vibrato::Ordinary),
            &["pizzicato", "con_sordino"],
        ),
        each(Supported, NotIdiomatic, Supported)
    );
    // A declaration counts even where the instrument does not normally play the id.
    assert_eq!(
        get("flute")
            .request_availability(request(None, Mute::Cup, Vibrato::Ordinary), &["mute_cup"]),
        each(Supported, Supported, Supported)
    );
    // The ordinary sound, open, with the ordinary vibrato, needs no declaration on any record.
    for instrument in instruments() {
        assert_eq!(
            instrument.request_availability(Request::default(), &[]),
            each(Supported, Supported, Supported),
            "{}",
            instrument.id()
        );
    }
}

/// Over every record and every part value, each part of a request answers as
/// `articulation_availability` answers for the part's id, and never `UnknownArticulation`.
#[test]
fn request_availability_agrees_with_the_availability_of_each_id() {
    let declared = [
        "pizzicato",
        "roll",
        "con_sordino",
        "mute_harmon",
        "vibrato_on",
    ];
    let single = |instrument: &Instrument, articulation: Option<Articulation>| {
        articulation.map_or(ArticulationAvailability::Supported, |articulation| {
            instrument.articulation_availability(articulation.id(), &declared)
        })
    };
    for instrument in instruments() {
        let id = instrument.id();
        for primary in Articulation::ALL {
            let Ok(request) = Request::new(Some(primary), Mute::Open, Vibrato::Ordinary) else {
                continue;
            };
            let answer = instrument.request_availability(request, &declared);
            assert_eq!(answer.primary, single(instrument, Some(primary)), "{id}");
            assert_ne!(
                answer.primary,
                ArticulationAvailability::UnknownArticulation
            );
        }
        for mute in Mute::ALL {
            let request = Request::new(None, mute, Vibrato::Ordinary).expect("no primary");
            let answer = instrument.request_availability(request, &declared);
            assert_eq!(answer.mute, single(instrument, mute.articulation()), "{id}");
            assert_ne!(answer.mute, ArticulationAvailability::UnknownArticulation);
        }
        for vibrato in Vibrato::ALL {
            let request = Request::new(None, Mute::Open, vibrato).expect("no primary");
            let answer = instrument.request_availability(request, &declared);
            assert_eq!(
                answer.vibrato,
                single(instrument, vibrato.articulation()),
                "{id}"
            );
            assert_ne!(
                answer.vibrato,
                ArticulationAvailability::UnknownArticulation
            );
        }
    }
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
        for lang in Lang::ALL {
            let name = record.names().get(lang);
            assert!(
                !name.contains(['(', ')']),
                "{} {lang:?}: {name}",
                record.id()
            );
        }
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

/// An alias with `text` in English, singular.
const fn en(text: &'static str) -> Alias {
    Alias {
        text,
        lang: Lang::En,
        number: Number::Singular,
    }
}

/// The same text twice in one language, with different numbers.
const TWICE: &[Alias] = &[
    en("viola"),
    en("violas"),
    Alias {
        number: Number::Plural,
        ..en("viola")
    },
];

/// The same text in two languages.
const TWO_LANGUAGES: &[Alias] = &[
    en("viola"),
    Alias {
        lang: Lang::Cs,
        ..en("viola")
    },
];

/// A duplicate followed by an unfolded text.
const TWICE_THEN_UNFOLDED: &[Alias] = &[en("viola"), en("viola"), en("Viola")];

/// A valid record with `aliases`.
fn with_aliases(aliases: &'static [Alias]) -> Result<Instrument, InstrumentError> {
    Instrument::try_new(
        InstrumentType::Triangle,
        InstrumentSpec { aliases, ..VALID },
    )
}

/// An alias is stored folded, has three letters or more and appears once per language in a record;
/// each rejected text is reported (TST-03).
#[test]
fn aliases_must_be_folded_long_enough_and_listed_once() {
    for text in [
        "Housle",
        "lesní rohy",
        " housle",
        "housle ",
        "lesni  rohy",
        "clarinet-in-bb",
        "oboe d'amore",
        "",
        " ",
    ] {
        let aliases: &'static [Alias] = Box::leak(Box::new([en(text)]));
        assert_eq!(
            with_aliases(aliases),
            Err(InstrumentError::AliasNotFolded(text)),
            "{text:?}"
        );
    }
    for text in ["hn", "vc", "b3", "a 1", "s d"] {
        let aliases: &'static [Alias] = Box::leak(Box::new([en(text)]));
        assert_eq!(
            with_aliases(aliases),
            Err(InstrumentError::AliasTooShort(text)),
            "{text:?}"
        );
    }
    // Three letters are enough, with digits and spaces between them or not.
    for text in ["cor", "t bl", "b fl", "b3 bb"] {
        let aliases: &'static [Alias] = Box::leak(Box::new([en(text)]));
        assert!(with_aliases(aliases).is_ok(), "{text:?}");
    }
    // The same text twice in one language is a duplicate, whatever its number; in two languages
    // it is not.
    assert_eq!(
        with_aliases(TWICE),
        Err(InstrumentError::DuplicateAlias("viola"))
    );
    assert!(with_aliases(TWO_LANGUAGES).is_ok());
    // Every alias is checked for its form before any duplicate is reported.
    assert_eq!(
        with_aliases(TWICE_THEN_UNFOLDED),
        Err(InstrumentError::AliasNotFolded("Viola"))
    );
}

/// Every alias of the dataset (records, categories and families) holds only `a-z`, `0-9` and
/// single inner spaces, has three letters or more, and appears once per language and target
/// (TST-07; checked here character by character, apart from the compile-time check).
#[test]
fn every_stored_alias_is_folded() {
    let mut seen = std::collections::HashSet::new();
    let mut count = 0;
    for (target, alias) in all_aliases() {
        let text = alias.text;
        assert!(
            text.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == ' '),
            "{target:?}: {text:?}"
        );
        assert_eq!(text.trim(), text, "{target:?}: {text:?}");
        assert!(!text.contains("  "), "{target:?}: {text:?}");
        assert!(
            text.chars().filter(char::is_ascii_lowercase).count() >= 3,
            "{target:?}: {text:?}"
        );
        assert!(
            seen.insert((target, alias.lang, text)),
            "{target:?}: {text:?} twice"
        );
        count += 1;
    }
    let records: usize = instruments().iter().map(|r| r.aliases().len()).sum();
    let categories: usize = CategoryId::ALL
        .iter()
        .map(|id| id.get().aliases().len())
        .sum();
    let families: usize = [
        Family::Strings,
        Family::Woodwinds,
        Family::Brass,
        Family::Percussion,
        Family::Keyboards,
        Family::Plucked,
        Family::Choir,
        Family::Other,
    ]
    .iter()
    .map(|family| family.aliases().len())
    .sum();
    assert_eq!(count, records + categories + families);
}

/// The table lists the aliases first by instrument in score order, then by category and last by
/// family, so a lookup can return its targets in a stable order (TST-04).
#[test]
fn all_aliases_come_in_data_order() {
    let targets: Vec<Target> = all_aliases().map(|(target, _)| target).collect();
    let mut blocks: Vec<Target> = targets.clone();
    blocks.dedup();
    let mut expected: Vec<Target> = InstrumentType::ALL
        .into_iter()
        .map(Target::Instrument)
        .collect();
    expected.extend(CategoryId::ALL.map(Target::Category));
    expected.extend(
        [
            Family::Strings,
            Family::Woodwinds,
            Family::Brass,
            Family::Percussion,
            Family::Keyboards,
            Family::Plucked,
            Family::Choir,
        ]
        .map(Target::Family),
    );
    assert_eq!(blocks, expected);
    assert_eq!(
        all_aliases().next(),
        Some((
            Target::Instrument(InstrumentType::Piccolo),
            &Alias {
                text: "piccolo",
                lang: Lang::En,
                number: Number::Singular,
            }
        ))
    );
    assert_eq!(Family::Other.aliases(), []);
}

/// Every record has aliases in each of the five languages, and every family but `Other` too.
#[test]
fn every_record_and_family_has_aliases_in_every_language() {
    for record in instruments() {
        for lang in Lang::ALL {
            assert!(
                record.aliases().iter().any(|alias| alias.lang == lang),
                "{} has no {lang:?} alias",
                record.id()
            );
        }
    }
    for family in [
        Family::Strings,
        Family::Woodwinds,
        Family::Brass,
        Family::Percussion,
        Family::Keyboards,
        Family::Plucked,
        Family::Choir,
    ] {
        for lang in Lang::ALL {
            assert!(
                family.aliases().iter().any(|alias| alias.lang == lang),
                "{family:?} has no {lang:?} alias"
            );
        }
    }
}

/// The targets of `text`, in data order, each once.
fn targets_of(text: &str) -> Vec<Target> {
    let mut targets: Vec<Target> = Vec::new();
    for (target, alias) in all_aliases() {
        if alias.text == text && !targets.contains(&target) {
            targets.push(target);
        }
    }
    targets
}

/// The alias `text` in `lang` of `target`, if stored.
fn alias_of(target: Target, text: &str, lang: Lang) -> Option<Alias> {
    all_aliases()
        .find(|(t, alias)| *t == target && alias.text == text && alias.lang == lang)
        .map(|(_, alias)| *alias)
}

/// Folded texts that name several targets, with the targets in data order. One word may name
/// several instruments (`bass`), a word may mean different things in two languages (`alto`), and
/// folding may join two Czech forms (`tenoru` is "tenoru", of the tenor voice, and "tenorů", of
/// the tenors). Each is listed on purpose, so an accidental collision fails the test below.
const KNOWN_AMBIGUOUS: [(&str, &[Target]); 16] = [
    (
        "alto",
        &[
            Target::Instrument(InstrumentType::VoiceAlto),
            Target::Instrument(InstrumentType::Viola),
        ],
    ),
    (
        "altos",
        &[
            Target::Instrument(InstrumentType::ChoirAlto),
            Target::Instrument(InstrumentType::Viola),
        ],
    ),
    (
        "altu",
        &[
            Target::Instrument(InstrumentType::VoiceAlto),
            Target::Instrument(InstrumentType::ChoirAlto),
        ],
    ),
    (
        "bass",
        &[
            Target::Instrument(InstrumentType::VoiceBass),
            Target::Instrument(InstrumentType::DoubleBass),
        ],
    ),
    (
        "basse",
        &[
            Target::Instrument(InstrumentType::VoiceBass),
            Target::Instrument(InstrumentType::ChoirBass),
        ],
    ),
    (
        "basses",
        &[
            Target::Instrument(InstrumentType::ChoirBass),
            Target::Instrument(InstrumentType::DoubleBass),
        ],
    ),
    (
        "basstuba",
        &[
            Target::Instrument(InstrumentType::WagnerTubaBass),
            Target::Instrument(InstrumentType::Tuba),
        ],
    ),
    (
        "basstuben",
        &[
            Target::Instrument(InstrumentType::WagnerTubaBass),
            Target::Instrument(InstrumentType::Tuba),
        ],
    ),
    (
        "basu",
        &[
            Target::Instrument(InstrumentType::VoiceBass),
            Target::Instrument(InstrumentType::ChoirBass),
        ],
    ),
    (
        "cinelu",
        &[
            Target::Instrument(InstrumentType::ClashCymbals),
            Target::Category(CategoryId::Cymbals),
        ],
    ),
    (
        "sopranu",
        &[
            Target::Instrument(InstrumentType::VoiceSoprano),
            Target::Instrument(InstrumentType::ChoirSoprano),
        ],
    ),
    (
        "tenore",
        &[
            Target::Instrument(InstrumentType::VoiceTenor),
            Target::Instrument(InstrumentType::ChoirTenor),
        ],
    ),
    (
        "tenortuba",
        &[
            Target::Instrument(InstrumentType::WagnerTubaTenor),
            Target::Instrument(InstrumentType::Euphonium),
        ],
    ),
    (
        "tenortuben",
        &[
            Target::Instrument(InstrumentType::WagnerTubaTenor),
            Target::Instrument(InstrumentType::Euphonium),
        ],
    ),
    (
        "tenoru",
        &[
            Target::Instrument(InstrumentType::VoiceTenor),
            Target::Instrument(InstrumentType::ChoirTenor),
        ],
    ),
    (
        "tuba basse",
        &[
            Target::Instrument(InstrumentType::Euphonium),
            Target::Instrument(InstrumentType::Tuba),
        ],
    ),
];

/// The texts that name several targets are exactly the listed ones, with the listed targets
/// (TST-07).
#[test]
fn known_ambiguous_aliases_are_exactly_the_listed_ones() {
    let mut texts: Vec<&str> = all_aliases().map(|(_, alias)| alias.text).collect();
    texts.sort_unstable();
    texts.dedup();
    let computed: Vec<(&str, Vec<Target>)> = texts
        .into_iter()
        .map(|text| (text, targets_of(text)))
        .filter(|(_, targets)| targets.len() > 1)
        .collect();
    let listed: Vec<(&str, Vec<Target>)> = KNOWN_AMBIGUOUS
        .iter()
        .map(|(text, targets)| (*text, targets.to_vec()))
        .collect();
    assert_eq!(computed, listed);
}

/// The forms an orchestration request in Czech or English names the strings, the horns and the
/// timpani with ("smyčce hrají legato melodii, lesní rohy a tympány pod tím"; "the strings play
/// a legato melody, horns and timpani underneath"), and the Czech cases of the horn that the
/// planner reads, each with its number (TST-05).
#[test]
fn the_planner_finds_the_forms_it_needs() {
    let horn = Target::Instrument(InstrumentType::HornF);
    let timpani = Target::Instrument(InstrumentType::Timpani);
    let violin = Target::Instrument(InstrumentType::Violin);
    let strings = Target::Family(Family::Strings);
    let cases = [
        (strings, "smycce", Lang::Cs, Number::Plural),
        (strings, "smyccu", Lang::Cs, Number::Plural),
        (strings, "strings", Lang::En, Number::Plural),
        (horn, "lesni roh", Lang::Cs, Number::Singular),
        (horn, "lesniho rohu", Lang::Cs, Number::Singular),
        (horn, "lesni rohy", Lang::Cs, Number::Plural),
        (horn, "lesnich rohu", Lang::Cs, Number::Plural),
        (horn, "lesnim rohum", Lang::Cs, Number::Plural),
        (horn, "lesnimi rohy", Lang::Cs, Number::Plural),
        (horn, "lesnich rozich", Lang::Cs, Number::Plural),
        (horn, "horns", Lang::En, Number::Plural),
        (horn, "horner", Lang::De, Number::Plural),
        (horn, "cors", Lang::Fr, Number::Plural),
        (horn, "corni", Lang::It, Number::Plural),
        (timpani, "tympany", Lang::Cs, Number::Plural),
        (timpani, "tympanu", Lang::Cs, Number::Unmarked),
        (timpani, "kotle", Lang::Cs, Number::Plural),
        (timpani, "timpani", Lang::En, Number::Plural),
        (timpani, "kettledrums", Lang::En, Number::Plural),
        (violin, "housle", Lang::Cs, Number::Unmarked),
        (violin, "housli", Lang::Cs, Number::Unmarked),
        (violin, "houslemi", Lang::Cs, Number::Unmarked),
    ];
    for (target, text, lang, number) in cases {
        assert_eq!(
            alias_of(target, text, lang).map(|alias| alias.number),
            Some(number),
            "{target:?} {text:?} {lang:?}"
        );
        assert_eq!(targets_of(text), [target], "{text:?} names one target");
    }
    // The bare Czech "roh" (also a corner) and two-letter abbreviations name nothing.
    for text in ["roh", "hn", "vc", "cb"] {
        assert!(targets_of(text).is_empty(), "{text:?}");
    }
}

/// A solo voice has the singular forms and a choir section the plural: "tenor" names the solo
/// tenor, "tenors" and the Czech "tenory" the section (TST-07).
#[test]
fn voices_are_singular_and_choir_sections_plural() {
    let voices = [
        InstrumentType::VoiceSoprano,
        InstrumentType::VoiceMezzoSoprano,
        InstrumentType::VoiceAlto,
        InstrumentType::VoiceTenor,
        InstrumentType::VoiceBaritone,
        InstrumentType::VoiceBass,
    ];
    let sections = [
        InstrumentType::ChoirSoprano,
        InstrumentType::ChoirAlto,
        InstrumentType::ChoirTenor,
        InstrumentType::ChoirBass,
    ];
    for voice in voices {
        let record = voice.get().expect("in the dataset");
        for alias in record.aliases() {
            assert_ne!(alias.number, Number::Plural, "{}: {alias:?}", record.id());
        }
    }
    for section in sections {
        let record = section.get().expect("in the dataset");
        for alias in record.aliases() {
            assert_ne!(alias.number, Number::Singular, "{}: {alias:?}", record.id());
        }
    }
    assert_eq!(
        targets_of("tenor"),
        [Target::Instrument(InstrumentType::VoiceTenor)]
    );
    for text in ["tenors", "tenory", "tenori", "chortenor"] {
        assert_eq!(
            targets_of(text),
            [Target::Instrument(InstrumentType::ChoirTenor)],
            "{text:?}"
        );
    }
}

/// The display names of a sample of records in English, Czech, German, French and Italian, and
/// their abbreviation, written by hand from the sources. The German choir sections are compounds
/// with "Chor", because the bare "Tenor" is the solo voice's name.
const NAMES_TABLE: [(&str, [&str; 6]); 10] = [
    (
        "violin",
        ["Violin", "housle", "Violine", "violon", "violino", "Vn."],
    ),
    (
        "cello",
        [
            "Cello",
            "violoncello",
            "Violoncello",
            "violoncelle",
            "violoncello",
            "Vc.",
        ],
    ),
    (
        "double_bass",
        [
            "Double bass",
            "kontrabas",
            "Kontrabass",
            "contrebasse",
            "contrabbasso",
            "Cb.",
        ],
    ),
    (
        "horn_f",
        ["Horn in F", "lesní roh", "Horn", "cor", "corno", "Hn."],
    ),
    (
        "timpani",
        [
            "Timpani", "tympány", "Pauken", "timbales", "timpani", "Timp.",
        ],
    ),
    (
        "clarinet_bb",
        [
            "Clarinet in B♭",
            "klarinet in B",
            "Klarinette in B",
            "clarinette en si♭",
            "clarinetto in si♭",
            "Cl. (B♭)",
        ],
    ),
    (
        "piano_studio_grand",
        [
            "Studio grand piano",
            "křídlo",
            "Studioflügel",
            "piano à queue",
            "pianoforte a coda",
            "Pno.",
        ],
    ),
    (
        "organ_pipe",
        [
            "Pipe organ",
            "varhany",
            "Orgel",
            "orgue",
            "organo a canne",
            "Org.",
        ],
    ),
    (
        "glockenspiel",
        [
            "Glockenspiel",
            "zvonkohra",
            "Glockenspiel",
            "jeu de timbres",
            "campanelli",
            "Glock.",
        ],
    ),
    (
        "choir_tenor",
        ["Tenors", "Tenory", "Chortenor", "Ténors", "Tenori", "T."],
    ),
];

/// The display names in five languages and the abbreviation, for a sample of records, as the
/// sources give them with their parenthesized alternates moved to the aliases (TST-06; the
/// values are written by hand).
#[test]
fn names_in_five_languages_match_the_sources() {
    for (id, [en, cs, de, fr, it, abbreviation]) in NAMES_TABLE {
        assert_eq!(
            *get(id).names(),
            Names {
                en,
                cs,
                de,
                fr,
                it,
                abbreviation,
            },
            "{id}"
        );
    }
    // Two records never share a display name in one language.
    for lang in Lang::ALL {
        let mut names: Vec<&str> = instruments()
            .iter()
            .map(|record| record.names().get(lang))
            .collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "{lang:?}");
    }
    assert_eq!(Lang::ALL.map(Lang::code), ["en", "cs", "de", "fr", "it"]);
}

/// Each category's id, slug, selection rule and members in score order.
const CATEGORY_TABLE: [(CategoryId, &str, Selection, &[InstrumentType]); 12] = [
    (
        CategoryId::Clarinet,
        "clarinet",
        Selection::Default(InstrumentType::ClarinetBb),
        &[
            InstrumentType::ClarinetEb,
            InstrumentType::ClarinetBb,
            InstrumentType::ClarinetA,
            InstrumentType::BassClarinet,
        ],
    ),
    (
        CategoryId::Saxophone,
        "saxophone",
        Selection::Default(InstrumentType::SaxophoneAlto),
        &[
            InstrumentType::SaxophoneSoprano,
            InstrumentType::SaxophoneAlto,
            InstrumentType::SaxophoneTenor,
            InstrumentType::SaxophoneBaritone,
        ],
    ),
    (
        CategoryId::WagnerTuba,
        "wagner_tuba",
        Selection::All,
        &[
            InstrumentType::WagnerTubaTenor,
            InstrumentType::WagnerTubaBass,
        ],
    ),
    (
        CategoryId::Trumpet,
        "trumpet",
        Selection::Default(InstrumentType::TrumpetBb),
        &[
            InstrumentType::TrumpetPiccolo,
            InstrumentType::TrumpetC,
            InstrumentType::TrumpetBb,
        ],
    ),
    (
        CategoryId::Trombone,
        "trombone",
        Selection::Default(InstrumentType::TromboneTenor),
        &[InstrumentType::TromboneTenor, InstrumentType::TromboneBass],
    ),
    (
        CategoryId::Cymbals,
        "cymbals",
        Selection::Default(InstrumentType::ClashCymbals),
        &[
            InstrumentType::ClashCymbals,
            InstrumentType::SuspendedCymbal,
        ],
    ),
    (
        CategoryId::Bells,
        "bells",
        Selection::Ask,
        &[
            InstrumentType::Glockenspiel,
            InstrumentType::TubularBells,
            InstrumentType::Crotales,
            InstrumentType::Carillon,
        ],
    ),
    (
        CategoryId::Piano,
        "piano",
        Selection::Default(InstrumentType::PianoConcertGrand),
        &[
            InstrumentType::PianoConcertGrand,
            InstrumentType::PianoStudioGrand,
            InstrumentType::PianoUpright,
            InstrumentType::PianoFelt,
            InstrumentType::PianoPrepared,
            InstrumentType::PianoHonkyTonk,
        ],
    ),
    (
        CategoryId::ElectricPiano,
        "electric_piano",
        Selection::Default(InstrumentType::ElectricPianoRhodes),
        &[
            InstrumentType::ElectricPianoRhodes,
            InstrumentType::ElectricPianoWurlitzer,
        ],
    ),
    (
        CategoryId::Organ,
        "organ",
        Selection::Default(InstrumentType::OrganPipe),
        &[InstrumentType::OrganPipe, InstrumentType::OrganHammond],
    ),
    (
        CategoryId::SoloVoice,
        "solo_voice",
        Selection::Ask,
        &[
            InstrumentType::VoiceSoprano,
            InstrumentType::VoiceMezzoSoprano,
            InstrumentType::VoiceAlto,
            InstrumentType::VoiceTenor,
            InstrumentType::VoiceBaritone,
            InstrumentType::VoiceBass,
        ],
    ),
    (
        CategoryId::Choir,
        "choir",
        Selection::All,
        &[
            InstrumentType::ChoirSoprano,
            InstrumentType::ChoirAlto,
            InstrumentType::ChoirTenor,
            InstrumentType::ChoirBass,
        ],
    ),
];

/// Each category's selection rule and members, written by hand from the orchestration
/// conventions the data follows (TST-06), and the invariants that tie records and categories
/// together (TST-07).
#[test]
fn categories_choose_among_their_members() {
    assert_eq!(CATEGORY_TABLE.map(|(id, ..)| id), CategoryId::ALL);
    for (id, slug, selection, members) in CATEGORY_TABLE {
        let record = category(id);
        assert_eq!(record.id(), id);
        assert_eq!(id.get(), record);
        assert_eq!(id.slug(), slug);
        assert_eq!(record.selection(), selection, "{slug}");
        assert_eq!(record.members().collect::<Vec<_>>(), members, "{slug}");
        if let Selection::Default(member) = selection {
            assert!(members.contains(&member), "{slug}");
        }
        let Provenance::Estimate { reason } = record.provenance() else {
            panic!("{slug}: a selection rule is an estimate");
        };
        assert!(!reason.trim().is_empty(), "{slug}");
        assert!(!record.aliases().is_empty(), "{slug}");
    }
    // Members point back: a record's category lists it, and no other category does.
    for record in instruments() {
        for id in CategoryId::ALL {
            assert_eq!(
                category(id)
                    .members()
                    .any(|t| t == record.instrument_type()),
                record.category() == Some(id),
                "{} {id:?}",
                record.id()
            );
        }
    }
    // The words for a whole category name the category, not a member.
    for (text, id) in [
        ("clarinet", CategoryId::Clarinet),
        ("klarinety", CategoryId::Clarinet),
        ("klavir", CategoryId::Piano),
        ("organ", CategoryId::Organ),
        ("choir", CategoryId::Choir),
        ("sbor", CategoryId::Choir),
        ("bells", CategoryId::Bells),
    ] {
        assert_eq!(targets_of(text), [Target::Category(id)], "{text:?}");
    }
}

/// Texts and their folded forms, written by hand from the folding rule: marks dropped after
/// decomposition, lower case, the seven characters decomposition leaves whole spelled out, and
/// every run of other characters one space (TST-10).
const FOLD_TABLE: [(&str, &str); 24] = [
    ("Lesní rohy", "lesni rohy"),
    ("lesních rohů", "lesnich rohu"),
    ("smyčců", "smyccu"),
    ("Hörner", "horner"),
    ("Blechbläser", "blechblaser"),
    ("Clarinet in B♭", "clarinet in bb"),
    ("clarinette en si♭", "clarinette en sib"),
    ("Cl. (B♭)", "cl bb"),
    ("F♯", "fis"),
    ("Straße", "strasse"),
    ("STRAẞE", "strasse"),
    ("chœur", "choeur"),
    ("Œuvre", "oeuvre"),
    ("Ærø", "aero"),
    ("Łódź", "lodz"),
    ("oboe d'amore", "oboe d amore"),
    ("  lesni  rohy  ", "lesni rohy"),
    ("2. Violine (Geige)", "2 violine geige"),
    ("ﬁddle", "fiddle"),
    ("ＶＩＯＬＡ", "viola"),
    ("İ", "i"),
    ("скрипка viola", "viola"),
    ("", ""),
    (" — ", ""),
];

/// `fold` spells a text in the form aliases are stored in, and turns each text the record check
/// rejects as not folded into one it accepts (TST-10, TST-03).
#[test]
fn fold_spells_text_in_the_stored_form() {
    for (text, folded) in FOLD_TABLE {
        assert_eq!(fold(text), folded, "{text:?}");
    }
    for text in [
        "Housle",
        "lesní rohy",
        " housle",
        "housle ",
        "lesni  rohy",
        "clarinet-in-bb",
        "oboe d'amore",
    ] {
        let folded: &'static str = Box::leak(fold(text).into_boxed_str());
        let aliases: &'static [Alias] = Box::leak(Box::new([en(folded)]));
        assert!(
            with_aliases(aliases).is_ok(),
            "{text:?} folds to {folded:?}"
        );
    }
    // A text with no letter or digit folds to nothing, which is still no alias.
    let aliases: &'static [Alias] =
        Box::leak(Box::new([en(Box::leak(fold(" ").into_boxed_str()))]));
    assert_eq!(
        with_aliases(aliases),
        Err(InstrumentError::AliasNotFolded(""))
    );
}

/// Every stored alias is its own folded form, and every display name folds to an alias of its
/// own record in its own language, so a name read off a score finds its instrument (TST-07).
#[test]
fn display_names_fold_to_their_own_aliases() {
    for (target, alias) in all_aliases() {
        assert_eq!(fold(alias.text), alias.text, "{target:?}");
    }
    for record in instruments() {
        for lang in Lang::ALL {
            let name = record.names().get(lang);
            let folded = fold(name);
            assert!(
                record
                    .aliases()
                    .iter()
                    .any(|alias| alias.text == folded && alias.lang == lang),
                "{}: the {lang:?} name {name:?} folds to {folded:?}, no alias of the record",
                record.id()
            );
        }
    }
}

/// Each mention of `text` in `languages`: its word positions, its folded text and its targets.
fn mentions(text: &str, languages: &[Lang]) -> Vec<(Range<usize>, &'static str, Vec<Target>)> {
    resolve(text, languages)
        .iter()
        .map(|mention| (mention.words(), mention.text(), mention.targets()))
        .collect()
}

/// Every stored alias, resolved alone in its own language, is one mention of all its words that
/// names its target with that alias; in every language the same holds (TST-07).
#[test]
fn every_alias_resolves_to_its_own_target() {
    for (target, alias) in all_aliases() {
        let words = alias.text.split(' ').count();
        for languages in [&[alias.lang][..], &Lang::ALL[..]] {
            let found = resolve(alias.text, languages);
            let [mention] = found.as_slice() else {
                panic!("{target:?} {alias:?}: {found:?}");
            };
            assert_eq!(mention.words(), 0..words, "{target:?} {alias:?}");
            assert_eq!(mention.text(), alias.text);
            assert!(
                mention.matches().contains(&(target, alias)),
                "{target:?} {alias:?}: {mention:?}"
            );
            assert!(mention.targets().contains(&target));
            for (_, other) in mention.matches() {
                assert_eq!(other.text, alias.text);
                assert!(languages.contains(&other.lang), "{other:?}");
            }
        }
    }
}

/// A text that names several targets is one mention with all of them, in data order; nothing
/// picks one (TST-07, through the index rather than the table).
#[test]
fn ambiguous_texts_resolve_to_every_target() {
    for (text, targets) in KNOWN_AMBIGUOUS {
        assert_eq!(
            mentions(text, &Lang::ALL),
            [(0..text.split(' ').count(), text, targets.to_vec())],
            "{text:?}"
        );
    }
    // The matches keep the order of `all_aliases`, with the language and number of each form.
    let bass = resolve("bass", &Lang::ALL);
    let expected: Vec<(Target, &Alias)> = all_aliases()
        .filter(|(_, alias)| alias.text == "bass")
        .collect();
    assert_eq!(bass.len(), 1);
    assert_eq!(bass[0].matches(), expected.as_slice());
}

/// The sentence an orchestration request is written in, in Czech and in English, names the
/// strings as a family, the horns and the timpani, each in the plural (TST-05).
#[test]
fn product_sentence_resolves() {
    let strings = Target::Family(Family::Strings);
    let horns = Target::Instrument(InstrumentType::HornF);
    let timpani = Target::Instrument(InstrumentType::Timpani);
    for (text, expected) in [
        (
            "smyčce hrají legato melodii, lesní rohy a tympány pod tím",
            [
                (0..1, "smycce", strings),
                (4..6, "lesni rohy", horns),
                (7..8, "tympany", timpani),
            ],
        ),
        (
            "the strings play a legato melody, horns and timpani underneath",
            [
                (1..2, "strings", strings),
                (6..7, "horns", horns),
                (8..9, "timpani", timpani),
            ],
        ),
    ] {
        let found = resolve(text, &Lang::ALL);
        let words: Vec<String> = fold(text).split(' ').map(str::to_owned).collect();
        assert_eq!(found.len(), expected.len(), "{text:?}: {found:?}");
        for (mention, (span, folded, target)) in found.iter().zip(expected) {
            assert_eq!(mention.words(), span, "{text:?}");
            assert_eq!(mention.text(), folded);
            assert_eq!(words[mention.words()].join(" "), folded);
            assert_eq!(mention.targets(), [target]);
            for (_, alias) in mention.matches() {
                assert_eq!(alias.number, Number::Plural, "{alias:?}");
            }
        }
    }
    for text in ["corni", "Hörner", "cors", "lesních rohů"] {
        let found = resolve(text, &Lang::ALL);
        assert_eq!(found.len(), 1, "{text:?}");
        assert_eq!(found[0].targets(), [horns], "{text:?}");
        assert_eq!(found[0].matches()[0].1.number, Number::Plural, "{text:?}");
    }
    // The Czech "klavír" is the piano category, which chooses the concert grand.
    assert_eq!(
        mentions("klavír", &[Lang::Cs]),
        [(0..1, "klavir", vec![Target::Category(CategoryId::Piano)])]
    );
    assert_eq!(
        category(CategoryId::Piano).selection(),
        Selection::Default(InstrumentType::PianoConcertGrand)
    );
}

/// At each word the longest alias that starts there wins, and reading continues after it
/// (TST-07).
#[test]
fn the_longest_alias_wins_from_the_left() {
    let target = |t| vec![Target::Instrument(t)];
    assert_eq!(
        mentions("the bass drum and the bass clarinet", &[Lang::En]),
        [
            (1..3, "bass drum", target(InstrumentType::BassDrum)),
            (5..7, "bass clarinet", target(InstrumentType::BassClarinet)),
        ]
    );
    assert_eq!(
        mentions("Anglický roh", &[Lang::Cs]),
        [(0..2, "anglicky roh", target(InstrumentType::EnglishHorn))]
    );
    // A member's own name wins over the word for its category.
    assert_eq!(
        mentions("klarinet in B", &[Lang::Cs]),
        [(0..3, "klarinet in b", target(InstrumentType::ClarinetBb))]
    );
    assert_eq!(
        mentions("klarinet", &[Lang::Cs]),
        [(
            0..1,
            "klarinet",
            vec![Target::Category(CategoryId::Clarinet)]
        )]
    );
    // A longer alias counts only in the languages asked for: the French "tuba basse" is one
    // ambiguous mention, but in English only "tuba" is an alias.
    assert_eq!(
        mentions("tuba basse", &Lang::ALL),
        [(
            0..2,
            "tuba basse",
            vec![
                Target::Instrument(InstrumentType::Euphonium),
                Target::Instrument(InstrumentType::Tuba),
            ]
        )]
    );
    assert_eq!(
        mentions("tuba basse", &[Lang::En]),
        [(0..1, "tuba", target(InstrumentType::Tuba))]
    );
    // Two names in a row stay two mentions.
    assert_eq!(
        mentions("Violine (Geige)", &[Lang::De]),
        [
            (0..1, "violine", target(InstrumentType::Violin)),
            (1..2, "geige", target(InstrumentType::Violin)),
        ]
    );
}

/// Only aliases in the languages asked for count, and the number tells a solo voice from a
/// section (TST-03, TST-07).
#[test]
fn languages_filter_the_aliases_that_count() {
    let voice_alto = Target::Instrument(InstrumentType::VoiceAlto);
    let viola = Target::Instrument(InstrumentType::Viola);
    assert_eq!(
        mentions("alto", &Lang::ALL),
        [(0..1, "alto", vec![voice_alto, viola])]
    );
    assert_eq!(
        mentions("alto", &[Lang::En]),
        [(0..1, "alto", vec![voice_alto])]
    );
    assert_eq!(mentions("alto", &[Lang::Fr]), [(0..1, "alto", vec![viola])]);
    for lang in [Lang::Cs, Lang::De, Lang::It] {
        assert!(resolve("alto", &[lang]).is_empty(), "{lang:?}");
    }
    assert!(
        resolve("strings", &[Lang::Cs]).is_empty(),
        "the English word is no Czech alias"
    );
    // A language listed twice changes nothing.
    assert_eq!(
        resolve("timpani", &[Lang::En, Lang::En]),
        resolve("timpani", &[Lang::En])
    );
    // "timpani" is English and Italian: one target, matched by both forms.
    let timpani = resolve("Timpani", &Lang::ALL);
    assert_eq!(
        timpani[0].targets(),
        [Target::Instrument(InstrumentType::Timpani)]
    );
    assert_eq!(
        timpani[0]
            .matches()
            .iter()
            .map(|(_, alias)| alias.lang)
            .collect::<Vec<_>>(),
        [Lang::En, Lang::It]
    );
    let tenor = resolve("Tenor", &[Lang::En]);
    assert_eq!(
        tenor[0].targets(),
        [Target::Instrument(InstrumentType::VoiceTenor)]
    );
    assert_eq!(tenor[0].matches()[0].1.number, Number::Singular);
    let tenors = resolve("tenory", &[Lang::Cs]);
    assert_eq!(
        tenors[0].targets(),
        [Target::Instrument(InstrumentType::ChoirTenor)]
    );
    assert_eq!(tenors[0].matches()[0].1.number, Number::Plural);
}

/// Text that spells no alias names nothing: no stemming, no edit distance, no two-letter
/// abbreviations, and no language means no alias (TST-03).
#[test]
fn unknown_words_name_nothing() {
    for text in [
        "",
        " ,.; ",
        "saxhorn",
        "violinn",
        "housl",
        "roh",
        "hn",
        "Vc.",
        "and the a pod tím",
        "bassclarinet",
    ] {
        assert!(resolve(text, &Lang::ALL).is_empty(), "{text:?}");
    }
    for text in ["horns", "lesní rohy", "smyčce", "klavír", "bass"] {
        assert!(resolve(text, &[]).is_empty(), "{text:?}");
    }
}

/// Letters with the diacritics Czech, German and French put on them; each decomposes (NFKD) into
/// the bare letter and combining marks.
const DIACRITICS: [(char, &str); 13] = [
    ('a', "áäàâ"),
    ('c', "čç"),
    ('d', "ď"),
    ('e', "éěèêë"),
    ('i', "íîï"),
    ('n', "ň"),
    ('o', "óöô"),
    ('r', "ř"),
    ('s', "š"),
    ('t', "ť"),
    ('u', "úůüùû"),
    ('y', "ý"),
    ('z', "ž"),
];

/// What may stand between two words, before the first or after the last.
const SEPARATORS: [&str; 7] = [" ", "  ", ", ", " - ", "; ", "/", "\u{a0}"];
const LEADING: [&str; 5] = ["", "(", "\"", "„", "... "];
const TRAILING: [&str; 5] = ["", ")", ".", "!", "“"];

/// `text`, a folded alias, written as a person might: letters upper-cased or given a diacritic,
/// `ss` written `ß`, and punctuation between and around the words, as `choices` pick.
fn perturb(text: &str, choices: &[u8]) -> String {
    let mut choice = choices.iter().copied().cycle().map(usize::from);
    let mut next = move || choice.next().expect("choices are not empty");
    let mut out = String::from(LEADING[next() % LEADING.len()]);
    for (i, word) in text.split(' ').enumerate() {
        if i > 0 {
            out.push_str(SEPARATORS[next() % SEPARATORS.len()]);
        }
        let word = if next() % 2 == 1 {
            word.replacen("ss", "ß", 1)
        } else {
            word.to_owned()
        };
        for c in word.chars() {
            let variants: Vec<char> = DIACRITICS
                .iter()
                .find(|(letter, _)| *letter == c)
                .map_or(Vec::new(), |(_, marked)| marked.chars().collect());
            let pick = next();
            let c = if pick % 2 == 1 && !variants.is_empty() {
                variants[pick / 2 % variants.len()]
            } else {
                c
            };
            if next() % 2 == 1 {
                out.extend(c.to_uppercase());
            } else {
                out.push(c);
            }
        }
    }
    out.push_str(TRAILING[next() % TRAILING.len()]);
    out
}

/// Characters folding treats specially or that are easy to get wrong.
const TRICKY: [char; 24] = [
    'a',
    'Z',
    '7',
    ' ',
    '-',
    '.',
    'á',
    'Ů',
    'ß',
    'ẞ',
    'œ',
    'Æ',
    'ø',
    'Ł',
    '♭',
    '♯',
    'ﬁ',
    'İ',
    '½',
    '\u{301}',
    '\u{a0}',
    'Ж',
    '日',
    '\u{1F3BB}',
];

proptest! {
    /// The folded form holds only `a-z`, `0-9` and single inner spaces, and folding it again
    /// changes nothing, whatever the input (TST-09).
    #[test]
    fn fold_is_idempotent_and_ascii(text in any::<String>(), tricky in vec(proptest::sample::select(TRICKY.to_vec()), 0..40)) {
        for text in [text, tricky.into_iter().collect::<String>()] {
            let folded = fold(&text);
            prop_assert!(
                folded.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == ' '),
                "{:?} -> {:?}", text, folded
            );
            prop_assert_eq!(folded.trim(), folded.as_str());
            prop_assert!(!folded.contains("  "), "{:?}", folded);
            prop_assert_eq!(fold(&folded), folded);
        }
    }

    /// Every stored alias still names its target after case changes, diacritics, `ß` and extra
    /// punctuation around its words (TST-09).
    #[test]
    fn aliases_survive_case_diacritics_and_punctuation(
        (target, alias) in proptest::sample::select(all_aliases().collect::<Vec<_>>()),
        choices in vec(any::<u8>(), 1..64),
    ) {
        let text = perturb(alias.text, &choices);
        let found = resolve(&text, &[alias.lang]);
        prop_assert_eq!(found.len(), 1, "{:?}: {:?}", text, found);
        prop_assert_eq!(found[0].words(), 0..alias.text.split(' ').count());
        prop_assert_eq!(found[0].text(), alias.text);
        prop_assert!(found[0].matches().contains(&(target, alias)), "{:?}: {:?}", text, found);
    }

    /// In any run of aliases and other words the mentions come left to right without overlap,
    /// each spells the folded words it covers, and each match is an alias with that text in a
    /// language asked for (TST-09).
    #[test]
    fn mentions_cover_the_words_they_spell(
        pieces in vec(proptest::sample::select(
            all_aliases()
                .map(|(_, alias)| alias.text)
                .chain(["a", "and", "the", "pod", "tim", "und", "et", "e", "hraji"])
                .collect::<Vec<_>>(),
        ), 0..8),
        languages in proptest::sample::subsequence(Lang::ALL.to_vec(), 0..=5),
    ) {
        let text = pieces.join(", ");
        let words: Vec<String> = fold(&text).split(' ').map(str::to_owned).collect();
        let mut end = 0;
        for mention in resolve(&text, &languages) {
            let span = mention.words();
            prop_assert!(span.start >= end && span.start < span.end, "{:?}", span);
            end = span.end;
            prop_assert_eq!(words[span].join(" "), mention.text());
            prop_assert!(!mention.matches().is_empty());
            for (_, alias) in mention.matches() {
                prop_assert_eq!(alias.text, mention.text());
                prop_assert!(languages.contains(&alias.lang));
            }
        }
        prop_assert!(end <= words.len());
    }
}

/// The GM voice of every record (TST-10: written from the researched GM numbers, not from the
/// data). A row holds the id, then `program n` for an exact GM1 program, `keys k ...` for exact
/// channel-10 keys, the fit (`approximate` or `weak`) followed by the program or by `keys k ...`
/// for a substitute, or `none`. A program counts 1-128; keys come preferred first.
const GM_VOICES: [&str; 85] = [
    "piccolo program 73",
    "flute program 74",
    "alto_flute approximate 74",
    "bass_flute approximate 74",
    "oboe program 69",
    "oboe_damore approximate 69",
    "english_horn program 70",
    "clarinet_eb approximate 72",
    "clarinet_bb program 72",
    "clarinet_a program 72",
    "bass_clarinet approximate 72",
    "saxophone_soprano program 65",
    "saxophone_alto program 66",
    "saxophone_tenor program 67",
    "saxophone_baritone program 68",
    "bassoon program 71",
    "contrabassoon approximate 71",
    "horn_f program 61",
    "wagner_tuba_tenor weak 61",
    "wagner_tuba_bass weak 61",
    "trumpet_piccolo approximate 57",
    "trumpet_c program 57",
    "trumpet_bb program 57",
    "cornet approximate 57",
    "flugelhorn weak 57",
    "trombone_tenor program 58",
    "trombone_bass program 58",
    "euphonium approximate 59",
    "tuba program 59",
    "cimbasso approximate 58",
    "timpani program 48",
    "bass_drum keys 35 36",
    "snare_drum keys 38",
    "clash_cymbals approximate keys 49 57",
    "suspended_cymbal approximate keys 49",
    "tam_tam none",
    "triangle keys 81 80",
    "tambourine keys 54",
    "castanets weak keys 75",
    "wood_block keys 76 77",
    "temple_blocks approximate keys 76 77",
    "sleigh_bells weak keys 54",
    "maracas keys 70",
    "shaker approximate keys 70 69",
    "whip none",
    "glockenspiel program 10",
    "xylophone program 14",
    "vibraphone program 12",
    "marimba program 13",
    "tubular_bells program 15",
    "crotales approximate 10",
    "carillon approximate 15",
    "harp program 47",
    "guitar_classical program 25",
    "lute weak 25",
    "mandolin weak 26",
    "celesta program 9",
    "piano_concert_grand program 1",
    "piano_studio_grand approximate 2",
    "piano_upright approximate 1",
    "piano_felt weak 1",
    "piano_prepared none",
    "piano_honky_tonk program 4",
    "harpsichord program 7",
    "clavichord weak 8",
    "electric_piano_rhodes program 5",
    "electric_piano_wurlitzer program 5",
    "clavinet program 8",
    "organ_pipe program 20",
    "organ_hammond program 17",
    "accordion program 22",
    "voice_soprano approximate 54",
    "voice_mezzo_soprano approximate 54",
    "voice_alto approximate 54",
    "voice_tenor approximate 54",
    "voice_baritone approximate 54",
    "voice_bass approximate 54",
    "choir_soprano program 53",
    "choir_alto program 53",
    "choir_tenor program 53",
    "choir_bass program 53",
    "violin program 41",
    "viola program 42",
    "cello program 43",
    "double_bass program 44",
];

/// A GM voice written as in [`GM_VOICES`].
fn describe(voice: GmVoice) -> String {
    let keys = |keys: &[GmNote]| {
        keys.iter()
            .map(|key| key.get().to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };
    match voice {
        GmVoice::Program(program) => format!("program {}", program.number()),
        GmVoice::Percussion(notes) => format!("keys {}", keys(notes)),
        GmVoice::Substitute { target, fit, .. } => {
            let fit = match fit {
                SubstituteFit::Approximate => "approximate",
                SubstituteFit::Weak => "weak",
            };
            match target {
                GmTarget::Program(program) => format!("{fit} {}", program.number()),
                GmTarget::Notes(notes) => format!("{fit} keys {}", keys(notes)),
            }
        }
        GmVoice::NoEquivalent => "none".to_owned(),
    }
}

/// Every record states the GM voice of the researched table: exact programs and keys only where
/// the GM1 name names the instrument, every other GM number a substitute (TST-05, TST-10).
#[test]
fn gm_voices_match_the_researched_values() {
    let mut ids = Vec::new();
    for row in GM_VOICES {
        let (id, voice) = row.split_once(' ').expect("an id and a voice");
        assert_eq!(describe(get(id).gm()), voice, "{id}");
        ids.push(id);
    }
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), INSTRUMENTS.len(), "every record once");

    let count = |matches: fn(&GmVoice) -> bool| {
        INSTRUMENTS
            .iter()
            .filter(|instrument| matches(&instrument.gm()))
            .count()
    };
    assert_eq!(count(|voice| matches!(voice, GmVoice::Program(_))), 43);
    assert_eq!(count(|voice| matches!(voice, GmVoice::Percussion(_))), 6);
    assert_eq!(
        count(|voice| matches!(voice, GmVoice::Substitute { .. })),
        33
    );
    assert_eq!(count(|voice| matches!(voice, GmVoice::NoEquivalent)), 3);
    // A pitched record takes a program and an unpitched one channel-10 keys, exact or not.
    for instrument in &INSTRUMENTS {
        let pitched = matches!(instrument.pitch(), PitchModel::Pitched(_));
        let plays_keys = !instrument.gm().keys().is_empty();
        if instrument.gm() != GmVoice::NoEquivalent {
            assert_ne!(pitched, plays_keys, "{}", instrument.id());
        }
    }
    // The program change message carries the program number minus one ("Acoustic Grand Piano",
    // program 1, is sent as 0; "Violin", 41, as 40).
    let sent = |id| match get(id).gm() {
        GmVoice::Program(program) => program.program_change(),
        other => panic!("{id} has an exact program, not {other:?}"),
    };
    assert_eq!((sent("piano_concert_grand"), sent("violin")), (0, 40));
}

/// A substitute is an estimate: its reason cites the GM list that has no voice for the
/// instrument and names the program or keys it chose (TST-10).
#[test]
fn gm_substitutes_are_estimates_naming_their_gm_list_and_target() {
    let mut substitutes = 0;
    for instrument in &INSTRUMENTS {
        let GmVoice::Substitute {
            target, provenance, ..
        } = instrument.gm()
        else {
            continue;
        };
        substitutes += 1;
        let id = instrument.id();
        let Provenance::Estimate { reason } = provenance else {
            panic!("{id}: a substitute is a choice, so its provenance is an estimate");
        };
        assert!(reason.contains("https://"), "{id}: {reason}");
        assert!(
            [
                "General_MIDI",
                "gm-level-1-sound-set",
                "gm1sound",
                "1TTM2HI2JAp_XiTJYEJ8gPNvuJpIjJiTP"
            ]
            .iter()
            .any(|list| reason.contains(list)),
            "{id} cites a GM1 list: {reason}"
        );
        let numbers: Vec<u8> = match target {
            GmTarget::Program(program) => vec![program.number()],
            GmTarget::Notes(keys) => keys.iter().map(|key| key.get()).collect(),
        };
        for number in numbers {
            assert!(
                reason.contains(&format!(" {number} ")),
                "{id} names {number}: {reason}"
            );
        }
    }
    assert_eq!(substitutes, 33);
}

/// The GM voice checks of `try_new`, each failure and its valid counterpart (TST-03, TST-07).
#[test]
fn validator_rejects_broken_gm_voices() {
    const EMPTY_KEYS: &[GmNote] = &[];
    const KEY_TWICE: &[GmNote] = &[key(81), key(81)];
    const KEY_TWICE_APART: &[GmNote] = &[key(80), key(81), key(80)];
    let program = GmProgram::new(48).expect("a GM1 program");
    let substitute = |target, provenance| GmVoice::Substitute {
        target,
        fit: SubstituteFit::Approximate,
        provenance,
    };
    let blank = Provenance::Estimate { reason: " " };
    let unpitched =
        |gm| Instrument::try_new(InstrumentType::Triangle, InstrumentSpec { gm, ..VALID });
    let pitched = |gm| {
        Instrument::try_new(
            InstrumentType::Timpani,
            InstrumentSpec {
                gm,
                ..written_in_clefs(0, (45, 57), &[Clef::Bass])
            },
        )
    };

    for (gm, error) in [
        (GmVoice::Percussion(EMPTY_KEYS), InstrumentError::NoGmKeys),
        (
            substitute(GmTarget::Notes(EMPTY_KEYS), ESTIMATE),
            InstrumentError::NoGmKeys,
        ),
        (
            GmVoice::Percussion(KEY_TWICE),
            InstrumentError::DuplicateGmKey,
        ),
        (
            substitute(GmTarget::Notes(KEY_TWICE_APART), ESTIMATE),
            InstrumentError::DuplicateGmKey,
        ),
        (GmVoice::Program(program), InstrumentError::GmVoiceMismatch),
        (
            substitute(GmTarget::Program(program), ESTIMATE),
            InstrumentError::GmVoiceMismatch,
        ),
        (
            substitute(GmTarget::Notes(TRIANGLE_KEYS), blank),
            InstrumentError::EmptyProvenance,
        ),
    ] {
        assert_eq!(unpitched(gm), Err(error), "unpitched {gm:?}");
    }
    for (gm, error) in [
        // A pitched record that does not state its voice keeps `EMPTY`'s keyless one.
        (GmVoice::Percussion(EMPTY_KEYS), InstrumentError::NoGmKeys),
        (
            GmVoice::Percussion(TRIANGLE_KEYS),
            InstrumentError::GmVoiceMismatch,
        ),
        (
            substitute(GmTarget::Notes(TRIANGLE_KEYS), ESTIMATE),
            InstrumentError::GmVoiceMismatch,
        ),
        (
            substitute(GmTarget::Program(program), blank),
            InstrumentError::EmptyProvenance,
        ),
    ] {
        assert_eq!(pitched(gm), Err(error), "pitched {gm:?}");
    }

    for gm in [
        GmVoice::Percussion(TRIANGLE_KEYS),
        substitute(GmTarget::Notes(TRIANGLE_KEYS), ESTIMATE),
        GmVoice::NoEquivalent,
    ] {
        assert_eq!(unpitched(gm).map(|record| record.gm()), Ok(gm));
    }
    for gm in [
        GmVoice::Program(program),
        substitute(GmTarget::Program(program), ESTIMATE),
        GmVoice::NoEquivalent,
    ] {
        assert_eq!(pitched(gm).map(|record| record.gm()), Ok(gm));
    }
    // The GM checks come after the alias checks.
    assert_eq!(
        Instrument::try_new(
            InstrumentType::Triangle,
            InstrumentSpec {
                aliases: TWICE,
                gm: GmVoice::Percussion(EMPTY_KEYS),
                ..VALID
            },
        ),
        Err(InstrumentError::DuplicateAlias("viola"))
    );
}

/// The orchestral percussion map: GM1 key and the id it names (TST-10: written from the GM1
/// percussion key map, not from the data).
const ORCHESTRAL_KEYS: [(u8, &str); 9] = [
    (35, "bass_drum"),
    (36, "bass_drum"),
    (38, "snare_drum"),
    (54, "tambourine"),
    (70, "maracas"),
    (76, "wood_block"),
    (77, "wood_block"),
    (80, "triangle"),
    (81, "triangle"),
];

/// The orchestral percussion map holds the exact keys of the records and no substitute, and it
/// sits beside the drum kit's map without changing it (TST-05, TST-07).
#[test]
fn orchestral_percussion_map_holds_the_exact_keys_only() {
    let map: Vec<(u8, &str)> = ORCHESTRAL_PERCUSSION_MAP
        .iter()
        .map(|(key, instrument_type)| (key.get(), instrument_type.id()))
        .collect();
    assert_eq!(map, ORCHESTRAL_KEYS);
    for number in GmNote::LOWEST..=GmNote::HIGHEST {
        let expected = ORCHESTRAL_KEYS
            .iter()
            .find(|(key, _)| *key == number)
            .map(|(_, id)| *id);
        assert_eq!(
            percussion_instrument(key(number)).map(InstrumentType::id),
            expected,
            "{number}"
        );
    }
    for instrument in &INSTRUMENTS {
        let own = Some(instrument.instrument_type());
        match instrument.gm() {
            GmVoice::Percussion(keys) => {
                for &key in keys {
                    assert_eq!(percussion_instrument(key), own, "{}", instrument.id());
                }
            }
            GmVoice::Substitute {
                target: GmTarget::Notes(keys),
                ..
            } => {
                for &key in keys {
                    assert_ne!(percussion_instrument(key), own, "{}", instrument.id());
                }
            }
            GmVoice::Program(_) | GmVoice::Substitute { .. } | GmVoice::NoEquivalent => {}
        }
    }
    // The kit crashes and the claves stand in for orchestral instruments but name none.
    for number in [49, 57, 75] {
        assert_eq!(percussion_instrument(key(number)), None, "{number}");
    }
    // The drum kit's map has no triangle; a GM1 triangle key still names the orchestral one.
    for number in [80, 81] {
        assert!(GM_MAP.iter().all(|(note, _)| *note != number), "{number}");
        assert_eq!(
            percussion_instrument(key(number)),
            Some(InstrumentType::Triangle)
        );
    }
}

/// The keyboard mechanics of a record, as the research gives them.
struct KeyboardRow {
    id: &'static str,
    action: Action,
    /// The sounding pitches of the lowest and the highest key.
    keys: (i64, i64),
    key_count: u8,
    /// Each pedal with the controller it sends.
    pedals: &'static [(PedalKind, Option<u8>)],
    resonance: Resonance,
    noises: &'static [Noise],
    decay: Decay,
    timbre: Option<TimbreState>,
    /// Whether the force of the stroke sets the loudness.
    touch: bool,
}

/// The pedals of a grand piano with their MIDI 1.0 controllers.
const GRAND: &[(PedalKind, Option<u8>)] = &[
    (PedalKind::Sustain, Some(64)),
    (PedalKind::UnaCorda, Some(67)),
    (PedalKind::Sostenuto, Some(66)),
];

/// The pedals of an upright piano; the practice muffler has no controller.
const UPRIGHT: &[(PedalKind, Option<u8>)] = &[
    (PedalKind::Sustain, Some(64)),
    (PedalKind::Soft, Some(67)),
    (PedalKind::PracticeMuffler, None),
];

/// The keyboard mechanics of the sixteen instruments played from a keyboard: the fifteen
/// keyboards and the carillon (TST-10: written from the researched values, not from the data).
/// The key counts are those the research states, not derived from the key range.
const KEYBOARDS: [KeyboardRow; 16] = [
    KeyboardRow {
        id: "carillon",
        action: Action::Baton,
        keys: (48, 96),
        key_count: 49,
        pedals: &[],
        resonance: Resonance::Undamped,
        noises: &[Noise::Clapper],
        decay: Decay::Free,
        timbre: None,
        touch: true,
    },
    KeyboardRow {
        id: "piano_concert_grand",
        action: Action::Grand,
        keys: (21, 108),
        key_count: 88,
        pedals: GRAND,
        resonance: Resonance::DampersRaised,
        noises: &[
            Noise::DamperReturn,
            Noise::KeyRelease,
            Noise::Pedal,
            Noise::Hammer,
        ],
        decay: Decay::TwoStage,
        timbre: None,
        touch: true,
    },
    KeyboardRow {
        id: "piano_studio_grand",
        action: Action::Grand,
        keys: (21, 108),
        key_count: 88,
        pedals: GRAND,
        resonance: Resonance::DampersRaised,
        noises: &[
            Noise::DamperReturn,
            Noise::KeyRelease,
            Noise::Pedal,
            Noise::Hammer,
        ],
        decay: Decay::TwoStage,
        timbre: None,
        touch: true,
    },
    KeyboardRow {
        id: "piano_upright",
        action: Action::Upright,
        keys: (21, 108),
        key_count: 88,
        pedals: UPRIGHT,
        resonance: Resonance::DampersRaised,
        noises: &[Noise::DamperReturn, Noise::KeyRelease, Noise::Pedal],
        decay: Decay::TwoStage,
        timbre: None,
        touch: true,
    },
    KeyboardRow {
        id: "piano_felt",
        action: Action::Upright,
        keys: (21, 108),
        key_count: 88,
        pedals: UPRIGHT,
        resonance: Resonance::DampersRaised,
        noises: &[Noise::Hammer, Noise::KeyRelease, Noise::Pedal],
        decay: Decay::TwoStage,
        timbre: Some(TimbreState::Felt),
        touch: true,
    },
    KeyboardRow {
        id: "piano_prepared",
        action: Action::Grand,
        keys: (21, 108),
        key_count: 88,
        pedals: GRAND,
        resonance: Resonance::DampersRaised,
        noises: &[Noise::Rattle],
        decay: Decay::PerKey,
        timbre: Some(TimbreState::Prepared),
        touch: true,
    },
    KeyboardRow {
        id: "piano_honky_tonk",
        action: Action::Upright,
        keys: (21, 108),
        key_count: 88,
        pedals: &[(PedalKind::Sustain, Some(64)), (PedalKind::Soft, Some(67))],
        resonance: Resonance::DampersRaised,
        noises: &[Noise::KeyRelease, Noise::DamperReturn],
        decay: Decay::TwoStage,
        timbre: Some(TimbreState::DetunedUnisons),
        touch: true,
    },
    KeyboardRow {
        id: "electric_piano_rhodes",
        action: Action::TineHammer,
        keys: (21, 108),
        key_count: 88,
        pedals: &[(PedalKind::Sustain, Some(64))],
        resonance: Resonance::Weak,
        noises: &[Noise::DamperReturn, Noise::Hammer],
        decay: Decay::Decaying,
        timbre: None,
        touch: true,
    },
    KeyboardRow {
        id: "electric_piano_wurlitzer",
        action: Action::ReedHammer,
        keys: (33, 96),
        key_count: 64,
        pedals: &[(PedalKind::Sustain, Some(64))],
        resonance: Resonance::Weak,
        noises: &[Noise::KeyRelease, Noise::Pedal, Noise::Hammer],
        decay: Decay::Decaying,
        timbre: None,
        touch: true,
    },
    KeyboardRow {
        id: "clavinet",
        action: Action::Plunger,
        keys: (29, 88),
        key_count: 60,
        pedals: &[(PedalKind::FootSwell, Some(11))],
        resonance: Resonance::Absent,
        noises: &[Noise::KeyRelease, Noise::Hammer],
        decay: Decay::Decaying,
        timbre: None,
        touch: true,
    },
    KeyboardRow {
        id: "harpsichord",
        action: Action::Jack,
        keys: (29, 89),
        key_count: 61,
        pedals: &[],
        resonance: Resonance::Weak,
        noises: &[Noise::KeyRelease, Noise::DamperReturn],
        decay: Decay::Decaying,
        timbre: None,
        touch: false,
    },
    KeyboardRow {
        id: "clavichord",
        action: Action::Tangent,
        keys: (29, 89),
        key_count: 61,
        pedals: &[],
        resonance: Resonance::Absent,
        noises: &[Noise::KeyRelease],
        decay: Decay::Decaying,
        timbre: None,
        touch: true,
    },
    KeyboardRow {
        id: "celesta",
        action: Action::PlateHammer,
        keys: (48, 108),
        key_count: 61,
        pedals: &[(PedalKind::Sustain, Some(64))],
        resonance: Resonance::Weak,
        noises: &[],
        decay: Decay::Decaying,
        timbre: None,
        touch: true,
    },
    KeyboardRow {
        id: "organ_pipe",
        action: Action::PipeValve,
        keys: (36, 96),
        key_count: 61,
        pedals: &[
            (PedalKind::Expression, Some(11)),
            (PedalKind::Crescendo, None),
        ],
        resonance: Resonance::Absent,
        noises: &[Noise::Wind],
        decay: Decay::WhileHeld,
        timbre: None,
        touch: false,
    },
    KeyboardRow {
        id: "organ_hammond",
        action: Action::Contacts,
        keys: (36, 96),
        key_count: 61,
        pedals: &[(PedalKind::Expression, Some(11))],
        resonance: Resonance::Absent,
        noises: &[Noise::KeyClick],
        decay: Decay::WhileHeld,
        timbre: None,
        touch: false,
    },
    KeyboardRow {
        id: "accordion",
        action: Action::ReedValve,
        keys: (53, 93),
        key_count: 41,
        pedals: &[],
        resonance: Resonance::Absent,
        noises: &[Noise::Bellows],
        decay: Decay::WhileHeld,
        timbre: None,
        touch: false,
    },
];

/// Every record played from a keyboard has the researched mechanics, and no other record has
/// any: the fifteen keyboards and the carillon, a percussion instrument with a baton clavier
/// (TST-10, TST-07).
#[test]
fn keyboard_mechanics_match_the_researched_values() {
    let mut with_mechanics: Vec<&str> = INSTRUMENTS
        .iter()
        .filter(|record| record.keyboard().is_some())
        .map(Instrument::id)
        .collect();
    with_mechanics.sort_unstable();
    let mut expected: Vec<&str> = KEYBOARDS.iter().map(|row| row.id).collect();
    expected.sort_unstable();
    assert_eq!(with_mechanics, expected);
    for record in &INSTRUMENTS {
        if record.family() == Family::Keyboards {
            assert!(record.keyboard().is_some(), "{}", record.id());
        }
    }

    for row in &KEYBOARDS {
        let record = get(row.id);
        let mechanics = record.keyboard().expect("listed rows have mechanics");
        assert_eq!(mechanics.action, row.action, "{}", row.id);
        assert_eq!(
            (mechanics.keys.low().get(), mechanics.keys.high().get()),
            row.keys,
            "{}",
            row.id
        );
        assert_eq!(mechanics.key_count(), row.key_count, "{}", row.id);
        let pedals: Vec<(PedalKind, Option<u8>)> = mechanics
            .pedals
            .iter()
            .map(|pedal| (pedal.kind, pedal.controller.map(Controller::number)))
            .collect();
        assert_eq!(pedals, row.pedals, "{}", row.id);
        for &(kind, number) in row.pedals {
            assert_eq!(
                mechanics.controller(kind).map(Controller::number),
                number,
                "{} {kind:?}",
                row.id
            );
        }
        assert_eq!(mechanics.resonance, row.resonance, "{}", row.id);
        assert_eq!(mechanics.noises, row.noises, "{}", row.id);
        assert_eq!(mechanics.decay, row.decay, "{}", row.id);
        assert_eq!(mechanics.timbre, row.timbre, "{}", row.id);
        assert_eq!(
            mechanics.action.is_touch_sensitive(),
            row.touch,
            "{}",
            row.id
        );
        // The keys sound inside the playable range (PRI-04: the record check enforces it).
        let PitchModel::Pitched(pitched) = record.pitch() else {
            panic!("{} plays pitches", row.id);
        };
        assert!(
            pitched.playable.range.encloses(mechanics.keys),
            "{}",
            row.id
        );
    }
}

/// A pedal kind sends the same controller wherever it appears, the one MIDI 1.0 names for it:
/// the damper pedal 64, the sostenuto 66, the una corda and the half-blow soft pedal 67, a volume
/// or swell pedal 11; the practice muffler and the organ's crescendo pedal have none (TST-07;
/// the expected numbers are written from the MIDI 1.0 controller list).
#[test]
fn each_pedal_kind_sends_one_controller() {
    let expected = |kind: PedalKind| match kind {
        PedalKind::Sustain => Some(64),
        PedalKind::Sostenuto => Some(66),
        PedalKind::UnaCorda | PedalKind::Soft => Some(67),
        PedalKind::FootSwell | PedalKind::Expression => Some(11),
        PedalKind::PracticeMuffler | PedalKind::Crescendo => None,
    };
    let mut seen = Vec::new();
    for record in &INSTRUMENTS {
        let Some(mechanics) = record.keyboard() else {
            continue;
        };
        for pedal in mechanics.pedals {
            assert_eq!(
                pedal.controller.map(Controller::number),
                expected(pedal.kind),
                "{} {:?}",
                record.id(),
                pedal.kind
            );
            if !seen.contains(&pedal.kind) {
                seen.push(pedal.kind);
            }
        }
    }
    // Every kind of pedal is used by some record.
    assert_eq!(seen.len(), 8, "{seen:?}");
}

/// A damper pedal listed twice, the second time without a controller.
const SUSTAIN_TWICE: &[Pedal] = &[
    pedal(PedalKind::Sustain, 64),
    pedal(PedalKind::Soft, 67),
    Pedal {
        kind: PedalKind::Sustain,
        controller: None,
    },
];

/// Two pedals on controller 11, apart.
const SHARED_CONTROLLER: &[Pedal] = &[
    pedal(PedalKind::Expression, 11),
    Pedal {
        kind: PedalKind::Crescendo,
        controller: None,
    },
    pedal(PedalKind::FootSwell, 11),
];

/// Two pedals without a controller: no controller is shared.
const NO_CONTROLLERS: &[Pedal] = &[
    Pedal {
        kind: PedalKind::PracticeMuffler,
        controller: None,
    },
    Pedal {
        kind: PedalKind::Crescendo,
        controller: None,
    },
];

/// An upright piano playing 21-108 with `keyboard`.
fn upright(keyboard: KeyboardMechanics) -> Result<Instrument, InstrumentError> {
    Instrument::try_new(
        InstrumentType::PianoUpright,
        InstrumentSpec {
            keyboard: Some(keyboard),
            ..written_in_clefs(0, (21, 108), &[Clef::Treble, Clef::Bass])
        },
    )
}

/// The keyboard checks of `try_new` reject each broken fixture, the first failing check in the
/// documented order (TST-03).
#[test]
fn validator_rejects_broken_keyboards() {
    for (keyboard, error) in [
        (mechanics(20, 108), InstrumentError::KeysOutsidePlayable),
        (mechanics(21, 109), InstrumentError::KeysOutsidePlayable),
        (
            KeyboardMechanics {
                pedals: SUSTAIN_TWICE,
                ..mechanics(21, 108)
            },
            InstrumentError::DuplicatePedal,
        ),
        (
            KeyboardMechanics {
                pedals: SHARED_CONTROLLER,
                ..mechanics(21, 108)
            },
            InstrumentError::DuplicatePedalController,
        ),
        (
            KeyboardMechanics {
                noises: &[Noise::KeyRelease, Noise::Hammer, Noise::KeyRelease],
                ..mechanics(21, 108)
            },
            InstrumentError::DuplicateNoise,
        ),
        // The first failing check is reported: keys before pedals, pedals before noises.
        (
            KeyboardMechanics {
                pedals: SUSTAIN_TWICE,
                ..mechanics(12, 108)
            },
            InstrumentError::KeysOutsidePlayable,
        ),
        (
            KeyboardMechanics {
                pedals: SUSTAIN_TWICE,
                noises: &[Noise::Wind, Noise::Wind],
                ..mechanics(21, 108)
            },
            InstrumentError::DuplicatePedal,
        ),
    ] {
        assert_eq!(upright(keyboard), Err(error), "{keyboard:?}");
    }

    // An unpitched instrument has no playable range, so it has no keys.
    assert_eq!(
        Instrument::try_new(
            InstrumentType::Triangle,
            InstrumentSpec {
                keyboard: Some(mechanics(60, 72)),
                ..VALID
            },
        ),
        Err(InstrumentError::KeysOutsidePlayable)
    );
    // The keyboard checks come after the GM checks.
    assert_eq!(
        Instrument::try_new(
            InstrumentType::PianoUpright,
            InstrumentSpec {
                keyboard: Some(mechanics(20, 108)),
                gm: GmVoice::Percussion(TRIANGLE_KEYS),
                ..written_in_clefs(0, (21, 108), &[Clef::Treble])
            },
        ),
        Err(InstrumentError::GmVoiceMismatch)
    );
}

/// The valid counterparts of the broken fixtures: keys at the ends of the playable range or
/// inside it, and pedals that share no controller because they send none. The record returns its
/// mechanics unchanged (TST-07).
#[test]
fn validator_accepts_valid_keyboards() {
    for keyboard in [
        mechanics(21, 108),
        mechanics(36, 96),
        KeyboardMechanics {
            pedals: NO_CONTROLLERS,
            noises: &[],
            timbre: Some(TimbreState::Felt),
            ..mechanics(21, 108)
        },
    ] {
        assert_eq!(
            upright(keyboard).map(|record| record.keyboard().copied()),
            Ok(Some(keyboard))
        );
    }
}

/// A fixture fact with an estimated provenance.
const fn fact(kind: FactKind) -> Fact {
    Fact {
        kind,
        provenance: ESTIMATE,
    }
}

/// A valid timpani retuning: a whole tone in 400 ms, four drums.
const RETUNE: Fact = fact(FactKind::Retune {
    whole_tone: Span::Millis(400),
    drums: 4,
});

/// A valid string mute change.
const MUTE: Fact = fact(FactKind::MuteChange {
    on: Span::ModerateBeats(8),
    off: Span::ModerateBeats(7),
});

/// A valid change from pizzicato back to arco.
const TO_ARCO: Fact = fact(FactKind::ArcoPizzSwitch {
    to_arco: Span::Millis(500),
});

/// A valid count of held notes of a chord.
const STOPS: Fact = fact(FactKind::SustainedStops { notes: 2 });

/// A valid held note.
const BREATH: Fact = fact(FactKind::Breath {
    held_note: Span::Millis(40_000),
});

/// The harp's pedals as Forsyth lays them out: D C B under the left foot, E F G A under the right.
const HARP_PEDALS: FactKind = FactKind::HarpPedals {
    left: &[LetterName::D, LetterName::C, LetterName::B],
    right: &[LetterName::E, LetterName::F, LetterName::G, LetterName::A],
    change: Span::ModerateBeats(1),
};

/// A violin playing 55-105 with pizzicato and a mute, and `facts`.
fn violin(facts: &'static [Fact]) -> Result<Instrument, InstrumentError> {
    Instrument::try_new(
        InstrumentType::Violin,
        InstrumentSpec {
            articulations: &[Articulation::Pizzicato, Articulation::ConSordino],
            facts,
            ..written_in_clefs(0, (55, 105), &[Clef::Treble])
        },
    )
}

/// A harp playing 23-103 with `facts`.
fn harp(facts: &'static [Fact]) -> Result<Instrument, InstrumentError> {
    Instrument::try_new(
        InstrumentType::Harp,
        InstrumentSpec {
            facts,
            ..written_in_clefs(0, (23, 103), &[Clef::Treble, Clef::Bass])
        },
    )
}

/// The facts the sources give a record: each fact's values and whether a source states it
/// (`true`, `Cited`) or it is estimated.
type FactRow = (&'static str, &'static [(FactKind, bool)]);

/// The string section's mute: a couple of bars of moderate time on, a little less off.
const STRING_MUTE: FactKind = FactKind::MuteChange {
    on: Span::ModerateBeats(8),
    off: Span::ModerateBeats(7),
};

/// A brass mute, estimated at two bars each way.
const BRASS_MUTE: (FactKind, bool) = (
    FactKind::MuteChange {
        on: Span::ModerateBeats(8),
        off: Span::ModerateBeats(8),
    },
    false,
);

/// About half a second from pizzicato back to arco.
const HALF_A_SECOND: FactKind = FactKind::ArcoPizzSwitch {
    to_arco: Span::Millis(500),
};

/// The two upper notes of a string chord can be held.
const TWO_HELD: (FactKind, bool) = (FactKind::SustainedStops { notes: 2 }, true);

/// A clarinet holds a medium note piano for about 40 seconds.
const FORTY_SECONDS: (FactKind, bool) = (
    FactKind::Breath {
        held_note: Span::Millis(40_000),
    },
    true,
);

/// A saxophone holds a medium note for at most 10 bars of moderate 4/4.
const TEN_BARS: (FactKind, bool) = (
    FactKind::Breath {
        held_note: Span::ModerateBeats(40),
    },
    true,
);

/// A trumpet holds a medium note piano for 12 bars at crotchet = 120.
const TWELVE_BARS: (FactKind, bool) = (
    FactKind::Breath {
        held_note: Span::ModerateBeats(48),
    },
    true,
);

/// Every record with playability facts (TST-10: the values are written from the sources, not
/// from the data): the held notes of Widor's breath experiments, the string section's mute,
/// pizzicato and chords, the brass mutes, the timpani's retuning and the harp's pedals and hands.
const FACTS: [FactRow; 24] = [
    ("clarinet_bb", &[FORTY_SECONDS]),
    ("clarinet_a", &[FORTY_SECONDS]),
    ("saxophone_soprano", &[TEN_BARS]),
    ("saxophone_alto", &[TEN_BARS]),
    ("saxophone_tenor", &[TEN_BARS]),
    ("saxophone_baritone", &[TEN_BARS]),
    ("horn_f", &[BRASS_MUTE]),
    ("wagner_tuba_tenor", &[BRASS_MUTE]),
    ("wagner_tuba_bass", &[BRASS_MUTE]),
    ("trumpet_piccolo", &[BRASS_MUTE]),
    ("trumpet_c", &[TWELVE_BARS, BRASS_MUTE]),
    ("trumpet_bb", &[TWELVE_BARS, BRASS_MUTE]),
    ("cornet", &[BRASS_MUTE]),
    ("trombone_tenor", &[BRASS_MUTE]),
    ("trombone_bass", &[BRASS_MUTE]),
    ("euphonium", &[BRASS_MUTE]),
    ("tuba", &[BRASS_MUTE]),
    ("cimbasso", &[BRASS_MUTE]),
    (
        "timpani",
        &[(
            FactKind::Retune {
                whole_tone: Span::Millis(400),
                drums: 4,
            },
            true,
        )],
    ),
    (
        "harp",
        &[
            (HARP_PEDALS, true),
            (FactKind::NotesPerHand { notes: 4 }, true),
        ],
    ),
    (
        "violin",
        &[(STRING_MUTE, true), (HALF_A_SECOND, true), TWO_HELD],
    ),
    (
        "viola",
        &[(STRING_MUTE, true), (HALF_A_SECOND, false), TWO_HELD],
    ),
    (
        "cello",
        &[(STRING_MUTE, true), (HALF_A_SECOND, false), TWO_HELD],
    ),
    (
        "double_bass",
        &[(STRING_MUTE, true), (HALF_A_SECOND, false), TWO_HELD],
    ),
];

/// Every record with playability facts has the researched ones, and no other record has any
/// (TST-10, TST-07).
#[test]
fn playability_facts_match_the_researched_values() {
    let mut with_facts: Vec<&str> = INSTRUMENTS
        .iter()
        .filter(|record| !record.facts().is_empty())
        .map(Instrument::id)
        .collect();
    with_facts.sort_unstable();
    let mut expected: Vec<&str> = FACTS.iter().map(|(id, _)| *id).collect();
    expected.sort_unstable();
    assert_eq!(with_facts, expected);

    for (id, facts) in FACTS {
        let found: Vec<(FactKind, bool)> = get(id)
            .facts()
            .iter()
            .map(|fact| {
                let cited = matches!(fact.provenance, Provenance::Cited { .. });
                (fact.kind, cited)
            })
            .collect();
        assert_eq!(found, facts, "{id}");
    }
}

/// The facts follow the articulations a record lists: every string and brass instrument with a
/// mute it puts on (`con_sordino` or a named mute) has the time a mute change takes, and no
/// other has; every bowed string with pizzicato has the time back to arco and the notes of a
/// chord it can hold (TST-07).
#[test]
fn mute_and_pizzicato_facts_follow_the_articulations() {
    let has = |record: &Instrument, wanted: fn(&FactKind) -> bool| {
        record.facts().iter().any(|fact| wanted(&fact.kind))
    };
    for record in &INSTRUMENTS {
        let lists = |articulations: &[Articulation]| {
            record
                .articulations()
                .iter()
                .any(|articulation| articulations.contains(articulation))
        };
        let lists_mute = lists(&[
            Articulation::ConSordino,
            Articulation::MuteCup,
            Articulation::MuteHarmon,
            Articulation::MutePlunger,
            Articulation::MuteBucket,
        ]);
        let mute_change = has(record, |kind| matches!(kind, FactKind::MuteChange { .. }));
        if matches!(record.family(), Family::Strings | Family::Brass) {
            assert_eq!(mute_change, lists_mute, "{}", record.id());
        }
        if record.family() == Family::Strings {
            assert!(lists(&[Articulation::Pizzicato]), "{}", record.id());
            assert!(
                has(record, |kind| matches!(
                    kind,
                    FactKind::ArcoPizzSwitch { .. }
                )),
                "{}",
                record.id()
            );
            assert!(
                has(record, |kind| matches!(
                    kind,
                    FactKind::SustainedStops { .. }
                )),
                "{}",
                record.id()
            );
        }
    }
    // The flugelhorn lists no mute, so it is the one brass instrument without a mute change.
    assert_eq!(get("flugelhorn").facts(), []);
}

/// The fact checks of `try_new` reject each broken fixture, the first failing check in the
/// documented order (TST-03).
#[test]
fn validator_rejects_broken_facts() {
    const TWICE: &[Fact] = &[
        MUTE,
        TO_ARCO,
        fact(FactKind::MuteChange {
            on: Span::Millis(1),
            off: Span::Millis(1),
        }),
    ];
    const BREATH_ON_A_VIOLIN: &[Fact] = &[MUTE, BREATH];
    const RETUNE_ON_A_VIOLIN: &[Fact] = &[RETUNE];
    const PEDALS_ON_A_VIOLIN: &[Fact] = &[fact(HARP_PEDALS)];
    const ZERO_MUTE: &[Fact] = &[fact(FactKind::MuteChange {
        on: Span::ModerateBeats(8),
        off: Span::ModerateBeats(0),
    })];
    const ZERO_TO_ARCO: &[Fact] = &[fact(FactKind::ArcoPizzSwitch {
        to_arco: Span::Millis(0),
    })];
    const ZERO_STOPS: &[Fact] = &[fact(FactKind::SustainedStops { notes: 0 })];
    // A misfit and a zero in one fact: the fit is checked first.
    const ZERO_BREATH: &[Fact] = &[fact(FactKind::Breath {
        held_note: Span::Millis(0),
    })];
    // A duplicate after a misfit: duplicates are checked first.
    const MISFIT_TWICE: &[Fact] = &[BREATH, STOPS, BREATH];
    for (facts, error) in [
        (TWICE, InstrumentError::DuplicateFact),
        (BREATH_ON_A_VIOLIN, InstrumentError::FactDoesNotFit),
        (RETUNE_ON_A_VIOLIN, InstrumentError::FactDoesNotFit),
        (PEDALS_ON_A_VIOLIN, InstrumentError::FactDoesNotFit),
        (ZERO_MUTE, InstrumentError::ZeroFactValue),
        (ZERO_TO_ARCO, InstrumentError::ZeroFactValue),
        (ZERO_STOPS, InstrumentError::ZeroFactValue),
        (ZERO_BREATH, InstrumentError::FactDoesNotFit),
        (MISFIT_TWICE, InstrumentError::DuplicateFact),
    ] {
        assert_eq!(violin(facts), Err(error), "{facts:?}");
    }

    // A fact rests on the articulation the record lists: a mute it puts on, or pizzicato.
    let bare_violin = |articulations: &'static [Articulation], facts: &'static [Fact]| {
        Instrument::try_new(
            InstrumentType::Violin,
            InstrumentSpec {
                articulations,
                facts,
                ..written_in_clefs(0, (55, 105), &[Clef::Treble])
            },
        )
    };
    assert_eq!(
        bare_violin(&[Articulation::Pizzicato], &[MUTE]),
        Err(InstrumentError::FactWithoutArticulation)
    );
    assert_eq!(
        bare_violin(&[Articulation::ConSordino], &[TO_ARCO]),
        Err(InstrumentError::FactWithoutArticulation)
    );
    // A hand is not a mute the player puts on: a horn that lists only `stopped`.
    assert_eq!(
        Instrument::try_new(
            InstrumentType::HornF,
            InstrumentSpec {
                articulations: &[Articulation::Stopped],
                facts: &[MUTE],
                ..written_in_clefs(-7, (34, 77), &[Clef::Treble])
            },
        ),
        Err(InstrumentError::FactWithoutArticulation)
    );
    // The fact checks come after the keyboard checks.
    assert_eq!(
        Instrument::try_new(
            InstrumentType::PianoUpright,
            InstrumentSpec {
                keyboard: Some(mechanics(20, 108)),
                facts: &[BREATH],
                ..written_in_clefs(0, (21, 108), &[Clef::Treble])
            },
        ),
        Err(InstrumentError::KeysOutsidePlayable)
    );
}

/// The harp's fact checks reject a zero and pedals that do not name each letter name once, the
/// zero first (TST-03).
#[test]
fn validator_rejects_broken_harp_facts() {
    const MISSING_LETTER: &[Fact] = &[fact(FactKind::HarpPedals {
        left: &[LetterName::D, LetterName::C],
        right: &[LetterName::E, LetterName::F, LetterName::G, LetterName::A],
        change: Span::ModerateBeats(1),
    })];
    const LETTER_TWICE: &[Fact] = &[fact(FactKind::HarpPedals {
        left: &[LetterName::D, LetterName::C, LetterName::C],
        right: &[LetterName::E, LetterName::F, LetterName::G, LetterName::A],
        change: Span::ModerateBeats(1),
    })];
    const EIGHT_PEDALS: &[Fact] = &[fact(FactKind::HarpPedals {
        left: &[LetterName::D, LetterName::C, LetterName::B],
        right: &[
            LetterName::E,
            LetterName::F,
            LetterName::G,
            LetterName::A,
            LetterName::B,
        ],
        change: Span::ModerateBeats(1),
    })];
    // A zero change time is reported before the layout.
    const ZERO_CHANGE: &[Fact] = &[fact(FactKind::HarpPedals {
        left: &[LetterName::D],
        right: &[],
        change: Span::Millis(0),
    })];
    const ZERO_HANDS: &[Fact] = &[fact(FactKind::NotesPerHand { notes: 0 })];
    for (facts, error) in [
        (MISSING_LETTER, InstrumentError::HarpPedalLayout),
        (LETTER_TWICE, InstrumentError::HarpPedalLayout),
        (EIGHT_PEDALS, InstrumentError::HarpPedalLayout),
        (ZERO_CHANGE, InstrumentError::ZeroFactValue),
        (ZERO_HANDS, InstrumentError::ZeroFactValue),
    ] {
        assert_eq!(harp(facts), Err(error), "{facts:?}");
    }
}

/// The valid counterparts of the broken fixtures: each kind on an instrument it fits, at the
/// smallest values, returned unchanged (TST-07).
#[test]
fn validator_accepts_valid_facts() {
    const STRINGS: &[Fact] = &[MUTE, TO_ARCO, STOPS];
    const HARP: &[Fact] = &[fact(HARP_PEDALS), fact(FactKind::NotesPerHand { notes: 4 })];
    const SMALLEST: &[Fact] = &[
        fact(FactKind::MuteChange {
            on: Span::Millis(1),
            off: Span::ModerateBeats(1),
        }),
        fact(FactKind::ArcoPizzSwitch {
            to_arco: Span::Millis(1),
        }),
        fact(FactKind::SustainedStops { notes: 1 }),
    ];
    for facts in [STRINGS, SMALLEST, &[]] {
        assert_eq!(violin(facts).map(|record| record.facts()), Ok(facts));
    }
    assert_eq!(harp(HARP).map(|record| record.facts()), Ok(HARP));
    assert!(
        Instrument::try_new(
            InstrumentType::Timpani,
            InstrumentSpec {
                facts: &[RETUNE],
                ..VALID
            },
        )
        .is_ok()
    );
    // A breath fits a wind, a brass and a voice part.
    for instrument_type in [
        InstrumentType::Oboe,
        InstrumentType::Tuba,
        InstrumentType::ChoirBass,
    ] {
        let PitchModel::Pitched(pitched) = get(instrument_type.id()).pitch() else {
            panic!("{instrument_type:?} plays pitches");
        };
        let range = pitched.playable.range;
        let spec = InstrumentSpec {
            facts: &[BREATH],
            ..written_in_clefs(
                pitched.transposition.semitones(),
                (range.low().get(), range.high().get()),
                &[],
            )
        };
        assert!(
            Instrument::try_new(instrument_type, spec).is_ok(),
            "{instrument_type:?}"
        );
    }
}
