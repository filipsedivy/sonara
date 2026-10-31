//! The orchestration grammar against English and Czech requests: instruments with their counts,
//! parts and sizes, roles, articulations, dynamics, bars and keys, and the questions a request
//! leaves open, with "did you mean" suggestions for words that name no instrument.

use std::num::NonZeroU32;

use sonara_agent::orchestration::grammar::{
    Aspect, Assignment, Bars, Hairpin, Parsed, Question, RoleRequest, Size, parse,
};
use sonara_midi::orchestration::rules::{Dynamic, Role};
use sonara_midi::orchestration::{
    Articulation, CategoryId, Family, InstrumentType, Mute, Number, Request, Target, Vibrato,
};
use sonara_midi::performance::voicing::Density;
use sonara_midi::theory::{Key, Scale};

const HORN: Target = Target::Instrument(InstrumentType::HornF);
const TIMPANI: Target = Target::Instrument(InstrumentType::Timpani);
const VIOLIN: Target = Target::Instrument(InstrumentType::Violin);
const STRINGS: Target = Target::Family(Family::Strings);

/// The assignment of a request that reads as exactly one clause.
fn one(request: &str) -> Assignment {
    let parsed = parse(request);
    assert_eq!(parsed.clauses.len(), 1, "{request:?}: {parsed:#?}");
    parsed.clauses[0]
        .assignment
        .clone()
        .unwrap_or_else(|| panic!("{request:?} asks nothing of instruments"))
}

/// The assignment of a request that reads as one clause and leaves nothing open.
fn clear(request: &str) -> Assignment {
    let parsed = parse(request);
    assert!(parsed.is_clear(), "{request:?}: {:?}", parsed.questions);
    one(request)
}

/// The targets of each group of an assignment.
fn targets(assignment: &Assignment) -> Vec<Vec<Target>> {
    assignment
        .groups
        .iter()
        .map(|group| group.targets.clone())
        .collect()
}

/// The request for `primary`, open and with the ordinary vibrato.
fn articulation(primary: Articulation) -> Request {
    Request::new(Some(primary), Mute::Open, Vibrato::Ordinary)
        .expect("the test passes a primary articulation")
}

/// One clause's targets, role and articulation.
type Outline = (Vec<Vec<Target>>, Option<RoleRequest>, Option<Request>);

/// Each clause's targets, role and articulation: what the walkthrough request asks.
fn outline(parsed: &Parsed) -> Vec<Outline> {
    parsed
        .clauses
        .iter()
        .map(|clause| {
            let assignment = clause
                .assignment
                .as_ref()
                .expect("every clause of the walkthrough asks something of instruments");
            (
                targets(assignment),
                assignment.role,
                assignment.articulation,
            )
        })
        .collect()
}

#[test]
fn english_and_czech_walkthrough_requests_read_alike() {
    let english = parse("strings play a legato melody, horns and timpani underneath");
    let czech = parse("smyčce hrají legato melodii, lesní rohy a tympány pod tím");
    let expected = vec![
        (
            vec![vec![STRINGS]],
            Some(RoleRequest::Role(Role::Melody)),
            Some(articulation(Articulation::Legato)),
        ),
        (
            vec![vec![HORN], vec![TIMPANI]],
            Some(RoleRequest::Underneath),
            None,
        ),
    ];
    for parsed in [&english, &czech] {
        assert!(parsed.is_clear(), "{:?}", parsed.questions);
        assert_eq!(outline(parsed), expected);
    }
    let texts: Vec<&str> = czech.clauses.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "smyčce hrají legato melodii",
            "lesní rohy a tympány pod tím"
        ]
    );
    assert_eq!(english.clauses[0].text, "strings play a legato melody");
    for clause in english.clauses.iter().chain(&czech.clauses) {
        assert_eq!(
            clause.assignment.as_ref().unwrap().ignored,
            Vec::<String>::new()
        );
    }
}

/// TST-06: the planner's own clause split parts a request at every "and", so "horns and timpani
/// underneath" became two clauses; the grammar keeps a list of instruments together.
#[test]
fn horns_and_timpani_underneath_is_one_clause() {
    for request in [
        "horns and timpani underneath",
        "lesní rohy a tympány pod tím",
    ] {
        let assignment = clear(request);
        assert_eq!(
            targets(&assignment),
            [vec![HORN], vec![TIMPANI]],
            "{request:?}"
        );
        assert_eq!(
            assignment.role,
            Some(RoleRequest::Underneath),
            "{request:?}"
        );
    }
    // "and" away from a list still parts the request.
    let parsed = parse("strings play the melody and horns play chords");
    assert_eq!(parsed.clauses.len(), 2);
    assert_eq!(parsed.clauses[1].text, "horns play chords");
}

/// The Czech "pod tím" folds to `pod tim`, one edit from the alias "timp": it is a role, never an
/// unknown word with a suggestion (TST-06).
#[test]
fn tim_in_pod_tim_is_not_a_suggestion() {
    let parsed = parse("lesní rohy hrají pod tím");
    assert!(parsed.is_clear(), "{:?}", parsed.questions);
    let assignment = one("lesní rohy hrají pod tím");
    assert_eq!(assignment.role, Some(RoleRequest::Underneath));
    assert_eq!(assignment.ignored, Vec::<String>::new());
    // A word of three letters gets no suggestions even where an instrument belongs: "tmp" is one
    // edit from "timp".
    let parsed = parse("two tmp play the melody");
    assert_eq!(
        parsed.questions,
        [Question::UnknownInstrument {
            words: "tmp".to_owned(),
            suggestions: Vec::new(),
        }]
    );
}

#[test]
fn counts_read_in_both_languages_and_every_case() {
    let count = |n: u32| NonZeroU32::new(n);
    let cases = [
        ("čtyři lesní rohy hrají akordy", HORN, 4),
        ("4 horns play chords", HORN, 4),
        (
            "two oboes play the melody",
            Target::Instrument(InstrumentType::Oboe),
            2,
        ),
        ("dva tympány hrají prodlevu", TIMPANI, 2),
        (
            "dvě flétny hrají melodii",
            Target::Instrument(InstrumentType::Flute),
            2,
        ),
    ];
    for (request, target, n) in cases {
        let assignment = one(request);
        assert_eq!(targets(&assignment), [vec![target]], "{request:?}");
        assert_eq!(assignment.groups[0].count, count(n), "{request:?}");
    }
    // A count in the instrumental case after "s" (with): five horns double the strings.
    let assignment = clear("smyčce hrají melodii s pěti lesními rohy");
    assert_eq!(targets(&assignment), [vec![STRINGS]]);
    assert_eq!(assignment.doubled_by.len(), 1);
    assert_eq!(assignment.doubled_by[0].targets, [HORN]);
    assert_eq!(assignment.doubled_by[0].count, count(5));
    assert_eq!(assignment.doubled_by[0].words, "lesnimi rohy");
    assert_eq!(assignment.doubled_by[0].number, Number::Plural);
}

#[test]
fn parts_and_sizes_of_a_section() {
    let cases: [(&str, Target, &[u8], Option<Size>); 7] = [
        (
            "first and second violins play the melody",
            VIOLIN,
            &[1, 2],
            None,
        ),
        ("1. housle hrají melodii", VIOLIN, &[1], None),
        ("druhé housle hrají harmonii", VIOLIN, &[2], None),
        ("horn 3 plays the bass line", HORN, &[3], None),
        ("lesní rohy 1 a 2 hrají akordy", HORN, &[1, 2], None),
        (
            "solo violin plays the melody",
            VIOLIN,
            &[],
            Some(Size::Solo),
        ),
        (
            "violin section plays the melody",
            VIOLIN,
            &[],
            Some(Size::Section),
        ),
    ];
    for (request, target, parts, size) in cases {
        let assignment = one(request);
        assert_eq!(targets(&assignment), [vec![target]], "{request:?}");
        assert_eq!(assignment.groups[0].parts, parts, "{request:?}");
        assert_eq!(assignment.groups[0].size, size, "{request:?}");
        assert_eq!(assignment.groups[0].count, None, "{request:?}");
    }
    // A full stop that ends the sentence keeps the last part number a part.
    assert_eq!(one("horns 1 and 2.").groups[0].parts, [1, 2]);
    assert_eq!(one("Hraje lesní roh 3.").groups[0].parts, [3]);
    // "sólové housle": the Czech size word before the instrument.
    assert_eq!(
        clear("sólové housle hrají melodii").groups[0].size,
        Some(Size::Solo)
    );
}

#[test]
fn roles_read_in_both_languages() {
    let cases = [
        ("violas play the countermelody", Role::CounterMelody),
        ("violy hrají protimelodii", Role::CounterMelody),
        ("cellos play the bass line", Role::Bass),
        ("violoncella hrají basovou linku", Role::Bass),
        ("lesní rohy drží akordy", Role::Harmony),
        ("trombones play chords", Role::Harmony),
        ("timpani play a pedal", Role::Pedal),
        ("tympány hrají prodlevu", Role::Pedal),
        ("cellos play an ostinato", Role::Rhythm),
        ("the harp adds colour", Role::Colour),
        ("flutes play the tune", Role::Melody),
    ];
    for (request, role) in cases {
        let parsed = parse(request);
        // "bass line" is a role: its "bass" is not the ambiguous instrument.
        assert!(parsed.is_clear(), "{request:?}: {:?}", parsed.questions);
        let assignment = one(request);
        assert_eq!(
            assignment.role,
            Some(RoleRequest::Role(role)),
            "{request:?}"
        );
        assert_eq!(targets(&assignment).len(), 1, "{request:?}");
    }
    assert_eq!(
        clear("lesní rohy hrají pod melodií").role,
        Some(RoleRequest::Underneath)
    );
    assert_eq!(
        clear("trombones accompany").role,
        Some(RoleRequest::Underneath)
    );
}

#[test]
fn articulation_words_build_one_request() {
    let request = |primary, mute, vibrato| Some(Request::new(primary, mute, vibrato).unwrap());
    let cases = [
        (
            "strings play pizz",
            request(Some(Articulation::Pizzicato), Mute::Open, Vibrato::Ordinary),
        ),
        (
            "smyčce hrají pizzicato",
            request(Some(Articulation::Pizzicato), Mute::Open, Vibrato::Ordinary),
        ),
        (
            "smyčce hrají držené tóny",
            request(Some(Articulation::Sustain), Mute::Open, Vibrato::Ordinary),
        ),
        (
            "violins play col legno",
            request(Some(Articulation::ColLegno), Mute::Open, Vibrato::Ordinary),
        ),
        (
            "flétny hrají trylky",
            request(Some(Articulation::Trill), Mute::Open, Vibrato::Ordinary),
        ),
        (
            "solo violin plays con sordino senza vibrato",
            request(None, Mute::Generic, Vibrato::Off),
        ),
        (
            "trubky hrají legato s dusítkem",
            request(Some(Articulation::Legato), Mute::Generic, Vibrato::Ordinary),
        ),
        (
            "violins play ordinario",
            request(None, Mute::Open, Vibrato::Ordinary),
        ),
    ];
    for (text, expected) in cases {
        let parsed = parse(text);
        assert!(parsed.is_clear(), "{text:?}: {:?}", parsed.questions);
        assert_eq!(one(text).articulation, expected, "{text:?}");
    }
    assert_eq!(clear("strings play the melody").articulation, None);
}

#[test]
fn dynamics_and_their_changes_read_in_both_languages() {
    let cases = [
        ("strings play pp", Some(Dynamic::Pianissimo), None),
        // Markings beyond the six read as their nearest neighbour (`Dynamic`).
        ("strings play ppp", Some(Dynamic::Pianissimo), None),
        ("trombones play fff", Some(Dynamic::Fortissimo), None),
        ("strings play softly", Some(Dynamic::Piano), None),
        ("smyčce hrají potichu", Some(Dynamic::Piano), None),
        (
            "smyčce hrají velmi potichu",
            Some(Dynamic::Pianissimo),
            None,
        ),
        (
            "trombones play mezzo forte",
            Some(Dynamic::MezzoForte),
            None,
        ),
        (
            "lesní rohy hrají velmi nahlas",
            Some(Dynamic::Fortissimo),
            None,
        ),
        (
            "strings play pp crescendo",
            Some(Dynamic::Pianissimo),
            Some(Hairpin::Crescendo),
        ),
        ("smyčce zesilují", None, Some(Hairpin::Crescendo)),
        ("strings play a diminuendo", None, Some(Hairpin::Diminuendo)),
        ("smyčce zeslabují", None, Some(Hairpin::Diminuendo)),
        ("trombones swell", None, Some(Hairpin::Swell)),
    ];
    for (request, dynamic, hairpin) in cases {
        let parsed = parse(request);
        assert!(parsed.is_clear(), "{request:?}: {:?}", parsed.questions);
        let assignment = one(request);
        assert_eq!(assignment.dynamic, dynamic, "{request:?}");
        assert_eq!(assignment.hairpin, hairpin, "{request:?}");
    }
    // "piano" names the instrument; an alias wins over the dynamic.
    let assignment = one("strings and piano play the melody");
    assert_eq!(assignment.dynamic, None);
    assert_eq!(
        targets(&assignment),
        [vec![STRINGS], vec![Target::Category(CategoryId::Piano)]]
    );
}

#[test]
fn bars_keys_and_density_read_in_both_languages() {
    let english = clear("first violins play the melody fully in bars 5-12 in D minor");
    let czech = clear("1. housle hrají melodii plně v taktech 5–12 v d moll");
    for assignment in [&english, &czech] {
        assert_eq!(assignment.bars, Bars::new(5, 12));
        assert_eq!(
            assignment.key,
            Some(Key {
                root: 2,
                scale: Scale::Minor,
            })
        );
        assert_eq!(assignment.density, Some(Density::Full));
    }
    assert_eq!(clear("violins play bar 3").bars, Bars::new(3, 3));
    assert_eq!(
        clear("violins play thinly in F# minor").key,
        Some(Key {
            root: 6,
            scale: Scale::Minor,
        })
    );
    assert_eq!(clear("violins play thinly").density, Some(Density::Thin));
    assert_eq!(
        clear("housle hrají v b dur").key.map(|key| key.root),
        Some(10)
    );
    assert_eq!(
        clear("housle hrají v h moll").key.map(|key| key.root),
        Some(11)
    );
    // "a minor melody" is no key: English keys need "in".
    assert_eq!(clear("violins play a minor melody").key, None);
}

#[test]
fn ambiguous_words_list_every_target() {
    let parsed = parse("the bass plays the melody");
    assert_eq!(
        parsed.questions,
        [Question::Ambiguous {
            words: "bass",
            targets: vec![
                Target::Instrument(InstrumentType::VoiceBass),
                Target::Instrument(InstrumentType::DoubleBass),
            ],
        }]
    );
    assert_eq!(
        parsed.questions[0].text(),
        "'bass' can mean Bass (voice_bass) or Double bass (double_bass). Which one do you mean?"
    );
    let parsed = parse("melodii zpívá basu");
    assert!(
        parsed.questions.contains(&Question::Ambiguous {
            words: "basu",
            targets: vec![
                Target::Instrument(InstrumentType::VoiceBass),
                Target::Instrument(InstrumentType::ChoirBass),
            ],
        }),
        "{:?}",
        parsed.questions
    );
}

#[test]
fn a_category_without_a_usual_member_asks_which() {
    let parsed = parse("bells play the melody");
    assert_eq!(
        parsed.questions,
        [Question::ChooseMember {
            words: "bells",
            category: CategoryId::Bells,
        }]
    );
    let members: Vec<String> = CategoryId::Bells
        .get()
        .members()
        .map(|member| member.get().name().to_owned())
        .collect();
    assert!(members.len() > 1);
    for name in &members {
        assert!(parsed.questions[0].text().contains(name.as_str()), "{name}");
    }
    // A category with a usual member asks nothing: "clarinets" are the clarinets in B♭.
    assert_eq!(
        targets(&clear("clarinets play the melody")),
        [vec![Target::Category(CategoryId::Clarinet)]]
    );
}

/// The one unknown-instrument question of `request`: its words and each suggestion's target,
/// alias and distance.
fn suggested(request: &str) -> (String, Vec<(Target, &'static str, usize)>) {
    let parsed = parse(request);
    let unknown: Vec<_> = parsed
        .questions
        .iter()
        .filter_map(|question| match question {
            Question::UnknownInstrument { words, suggestions } => Some((
                words.clone(),
                suggestions
                    .iter()
                    .map(|s| (s.target, s.alias.text, s.distance))
                    .collect(),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(unknown.len(), 1, "{request:?}: {:?}", parsed.questions);
    unknown
        .into_iter()
        .next()
        .expect("the length was checked above")
}

#[test]
fn unknown_instruments_get_suggestions() {
    // Beside an instrument in a list, before or after it.
    assert_eq!(
        suggested("trombones and trumpts play chords"),
        (
            "trumpts".to_owned(),
            vec![(Target::Category(CategoryId::Trumpet), "trumpets", 1)]
        )
    );
    assert_eq!(
        suggested("violns and cellos play chords"),
        (
            "violns".to_owned(),
            vec![
                (VIOLIN, "violins", 1),
                (Target::Instrument(InstrumentType::Viola), "violas", 1),
            ]
        )
    );
    // After a count, after "with", and before the verb of a clause without instruments.
    assert_eq!(
        suggested("dva tympány a tři trubkz").1,
        [(Target::Category(CategoryId::Trumpet), "trubka", 1)]
    );
    assert_eq!(
        suggested("violins with flutez play the melody").1[0],
        (Target::Instrument(InstrumentType::Flute), "flute", 1)
    );
    assert_eq!(
        suggested("timpanni play the bass line").1,
        [(TIMPANI, "timpani", 1)]
    );
    assert_eq!(
        suggested("lesní rohi hrají melodii").1,
        [(HORN, "lesni roh", 1)]
    );
    // A word near no alias still asks.
    let parsed = parse("saxhorns play the melody");
    assert_eq!(
        parsed.questions,
        [Question::UnknownInstrument {
            words: "saxhorns".to_owned(),
            suggestions: Vec::new(),
        }]
    );
    assert_eq!(
        parsed.questions[0].text(),
        "I don't know the instrument 'saxhorns'. Which instrument do you mean?"
    );
    assert_eq!(
        parse("trombones and trumpts play chords").questions[0].text(),
        "I don't know the instrument 'trumpts'. Did you mean 'trumpets' (trumpet)?"
    );
}

#[test]
fn unknown_words_away_from_instruments_are_read_past() {
    let assignment = clear("the strings play a beautiful melody");
    assert_eq!(assignment.ignored, ["beautiful"]);
    assert_eq!(assignment.role, Some(RoleRequest::Role(Role::Melody)));
}

#[test]
fn horns_alone_ask_which_horns() {
    let horn_sense = Question::HornSense { words: "horns" };
    for request in [
        "horns play the melody",
        "horns and saxophones play the melody",
    ] {
        assert!(
            parse(request).questions.contains(&horn_sense),
            "{request:?}"
        );
    }
    for request in [
        "french horns play the melody",
        "lesní rohy hrají melodii",
        "horns and timpani underneath",
        "strings play the melody, horns underneath",
    ] {
        let parsed = parse(request);
        assert!(parsed.is_clear(), "{request:?}: {:?}", parsed.questions);
    }
}

#[test]
fn two_values_of_one_kind_are_a_question() {
    let cases = [
        (
            "violins with flute play the melody legato staccato",
            Aspect::Articulation,
            ["legato", "staccato"],
        ),
        ("strings play pp ff", Aspect::Dynamic, ["pp", "ff"]),
        (
            "strings play the melody and chords",
            Aspect::Role,
            ["melody", "chords"],
        ),
        (
            "smyčce hrají v d moll v f dur",
            Aspect::Key,
            ["d moll", "f dur"],
        ),
    ];
    for (request, aspect, said) in cases {
        let parsed = parse(request);
        assert_eq!(parsed.clauses.len(), 1, "{request:?}");
        assert_eq!(
            parsed.questions,
            [Question::Conflict {
                aspect,
                said: said.map(str::to_owned).to_vec(),
            }],
            "{request:?}"
        );
    }
    assert_eq!(
        parse("strings play pp ff").questions[0].text(),
        "'pp' and 'ff' ask for different dynamics. Which one do you want?"
    );
    // The first value is kept, and the doubling stays apart from the players.
    let assignment = one("violins with flute play the melody legato staccato");
    assert_eq!(
        assignment.articulation,
        Some(articulation(Articulation::Legato))
    );
    assert_eq!(targets(&assignment), [vec![VIOLIN]]);
    assert_eq!(
        assignment.doubled_by[0].targets,
        [Target::Instrument(InstrumentType::Flute)]
    );
    // The same word twice is no conflict.
    assert!(parse("strings play legato, legato").is_clear());
}

#[test]
fn bars_that_are_not_a_range_and_counts_of_none_are_questions() {
    let parsed = parse("strings play bars 12 to 5");
    assert_eq!(
        parsed.questions,
        [Question::InvalidBars { first: 12, last: 5 }]
    );
    assert_eq!(one("strings play bars 12 to 5").bars, None);
    assert_eq!(
        parse("strings play bar 0").questions,
        [Question::InvalidBars { first: 0, last: 0 }]
    );
    let parsed = parse("0 trumpets play chords");
    assert_eq!(
        parsed.questions,
        [Question::InvalidCount { words: "trumpets" }]
    );
    assert_eq!(one("0 trumpets play chords").groups[0].count, None);
}

#[test]
fn a_clause_without_instruments_asks_for_them() {
    let parsed = parse("play a legato melody");
    assert_eq!(
        parsed.questions,
        [Question::NoInstruments {
            clause: "play a legato melody".to_owned(),
        }]
    );
    // After a comma, such a clause continues the one before it.
    let assignment = clear("strings play the melody, softly");
    assert_eq!(assignment.dynamic, Some(Dynamic::Piano));
    assert_eq!(assignment.role, Some(RoleRequest::Role(Role::Melody)));
    // A semicolon ends the clause, so it continues nothing.
    let parsed = parse("strings play the melody; softly");
    assert_eq!(parsed.clauses.len(), 2);
    assert_eq!(
        parsed.questions,
        [Question::NoInstruments {
            clause: "softly".to_owned(),
        }]
    );
}

#[test]
fn lists_join_across_commas_and_then_ends_a_clause() {
    let assignment = clear("flutes, oboes and clarinets play the melody softly");
    assert_eq!(targets(&assignment).len(), 3);
    assert_eq!(assignment.dynamic, Some(Dynamic::Piano));
    let parsed = parse("smyčce hrají melodii, potom lesní rohy hrají akordy");
    assert!(parsed.is_clear(), "{:?}", parsed.questions);
    let texts: Vec<&str> = parsed.clauses.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(texts, ["smyčce hrají melodii", "lesní rohy hrají akordy"]);
}

#[test]
fn requests_for_no_instrument_are_left_to_the_planner() {
    let parsed = parse("set the tempo to 96.5 and delete track Drums");
    assert!(parsed.is_clear());
    let clauses: Vec<(&str, bool)> = parsed
        .clauses
        .iter()
        .map(|c| (c.text.as_str(), c.assignment.is_some()))
        .collect();
    assert_eq!(
        clauses,
        [
            ("set the tempo to 96.5", false),
            ("delete track Drums", false)
        ]
    );
    let parsed = parse("set tempo to 100 and strings play legato");
    let kinds: Vec<bool> = parsed
        .clauses
        .iter()
        .map(|c| c.assignment.is_some())
        .collect();
    assert_eq!(kinds, [false, true]);
}

/// The same request always reads the same, and every clause is a stretch of the request as
/// written, in order and without overlap (TST-04, TST-07).
#[test]
fn reading_is_deterministic_and_clauses_quote_the_request() {
    let requests = [
        "strings play a legato melody, horns and timpani underneath",
        "smyčce hrají legato melodii, lesní rohy a tympány pod tím",
        "flutes, oboes and clarinets play the melody softly; then horns play chords",
        "1. a 2. housle hrají melodii v d moll, violy a violoncella akordy",
        "violns and horns play chords and the bass plays the bass line",
        "set the tempo to 96 and delete track Drums, please",
        "",
        "   ,;  and then ",
    ];
    for request in requests {
        let first = parse(request);
        assert_eq!(first, parse(request), "{request:?}");
        let mut from = 0;
        for clause in &first.clauses {
            assert!(!clause.text.is_empty(), "{request:?}");
            let at = request
                .get(from..)
                .and_then(|rest| rest.find(clause.text.as_str()))
                .unwrap_or_else(|| panic!("{:?} is not in {request:?} after {from}", clause.text));
            from += at + clause.text.len();
        }
    }
    assert_eq!(parse("").clauses, []);
    assert!(parse("").is_clear());
}
