//! The orchestration grammar: reads a request that gives instruments something to play, in English
//! or Czech, into clauses, and lists the questions the request leaves open.
//!
//! "strings play a legato melody, horns and timpani underneath" and "smyčce hrají legato melodii,
//! lesní rohy a tympány pod tím" both read as two clauses: the strings with the role melody and the
//! articulation legato, then the horns and the timpani with the role "underneath".
//!
//! # What a clause holds
//!
//! - **Instruments** are the aliases `sonara_midi::orchestration::resolve` finds, in English and
//!   Czech: every listed form, Czech cases included ("lesních rohů", "tympány"), with every target
//!   an ambiguous word names. The grammar folds nothing itself (DES-01). A group of instruments
//!   can carry a count ("two horns", "čtyři lesní rohy", "4 horns"), the parts of a section
//!   ("first violins", "1. housle", "horn 3") and "solo" or "section" ("sekce"). Instruments
//!   after "with" ("s", "se") double the others.
//! - **A role**: melody, counter-melody, harmony ("chords", "akordy"), bass line, pedal, rhythm or
//!   colour, or "underneath" ("pod tím"), which leaves the role to the instrument: harmony for
//!   sustaining instruments, the bass or a pedal for the timpani.
//! - **How to play**: an articulation, a mute and a vibrato as an articulation request (the
//!   vocabulary's ids are words of their own, "col legno"; "pizz", "držené" and "s dusítkem" are
//!   listed beside them), a dynamic ("pp", "softly", "potichu"), a change of dynamic
//!   ("crescendo", "zesiluje"), how thickly to double ("full", "plně"), a stretch of bars ("in
//!   bars 5-12", "v taktech 5–12") and a key ("in D minor", "v d moll").
//!
//! # How a request is cut into clauses
//!
//! A semicolon and "then" ("potom", "pak") end a clause. A comma, and "and" ("a", "i") anywhere
//! but between two instruments, part the request too, but a part that only lists instruments joins
//! the part after it ("flutes, oboes and clarinets play the melody"), and a part that names no
//! instrument but adds a role or a manner of playing continues the part before it ("strings play
//! the melody, softly"). So "horns and timpani underneath" is one clause, and "a" works both as the
//! Czech "and" and as the English article ("strings play" and "legato melody" join again).
//!
//! A clause that names no instrument and gives no role or manner of playing ("set the tempo to
//! 96") is left to the planner's other request kinds: [`Clause::assignment`] is `None`.
//!
//! # Questions
//!
//! Nothing is to be changed while a [`Question`] is open:
//!
//! - words in an instrument's place that name no instrument, with "did you mean" suggestions of
//!   aliases a few edits away (`strsim`; suggestions only, never a match: "viola" and "violin" are
//!   one edit apart);
//! - words that name several targets ("bass"), and a category without a usual member ("bells");
//! - "horns" in a request that names nothing else orchestral: French horns, or a brass and
//!   saxophone section;
//! - a clause with no instruments, two different values of one kind in a clause, a stretch of bars
//!   that is not a range, and a count of no players.
//!
//! The reading is deterministic: the same request always gives the same clauses and questions, in
//! the order written (TST-04). It reads no project and calls no tool.

mod question;
mod words;

use std::num::NonZeroU32;
use std::ops::Range;

use sonara_midi::orchestration::rules::{Dynamic, Role};
use sonara_midi::orchestration::{
    Articulation, Axis, EnsembleId, InstrumentType, Mute, Number, Request, Selection, Target,
    Vibrato, fold,
};
use sonara_midi::performance::voicing::Density;
use sonara_midi::theory::Key;

pub use self::question::{Aspect, MAX_SUGGESTIONS, Question, Suggestion, target_name};
use self::words::{FoldedWord, Kind, Token, Word};

/// How many of an instrument's players a group asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    /// One player: "solo violin", the Czech "sólové housle".
    Solo,
    /// The whole section: "violin section", the Czech "sekce".
    Section,
}

/// The function a clause gives its instruments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoleRequest {
    /// A role of the orchestration rules: "melody", "chords" (harmony), "bass line", the Czech
    /// "akordy".
    Role(Role),
    /// Below the melody: "underneath", "under", "accompany", the Czech "pod tím" and "doprovod".
    /// Which role that is depends on the instrument: harmony for sustaining instruments, the bass
    /// or a pedal for the timpani.
    Underneath,
}

/// A change of dynamic over a passage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hairpin {
    /// Louder: "crescendo", the Czech "zesiluje".
    Crescendo,
    /// Softer: "diminuendo", "decrescendo", the Czech "zeslabuje".
    Diminuendo,
    /// Louder, then softer: "swell", "messa di voce".
    Swell,
}

/// A stretch of bars, counted from 1, first to last inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bars {
    first: u32,
    last: u32,
}

impl Bars {
    /// Bars `first` to `last`; `None` unless `1 <= first <= last`.
    #[must_use]
    pub const fn new(first: u32, last: u32) -> Option<Self> {
        if first >= 1 && last >= first {
            Some(Self { first, last })
        } else {
            None
        }
    }

    /// The first bar, from 1.
    #[must_use]
    pub const fn first(self) -> u32 {
        self.first
    }

    /// The last bar, at least [`Bars::first`].
    #[must_use]
    pub const fn last(self) -> u32 {
        self.last
    }
}

/// Instruments one alias names, with what the words around it say about them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The alias the request used, folded: `lesni rohy`, `strings`.
    pub words: &'static str,
    /// Every instrument, category or family the alias names, in data order. More than one means
    /// the words are ambiguous, and the parse has a [`Question::Ambiguous`] for them.
    pub targets: Vec<Target>,
    /// The grammatical number of the alias: "horn" is singular, "horns" plural, the Czech
    /// "housle" both. [`Number::Unmarked`] also when the targets' forms disagree.
    pub number: Number,
    /// How many play, when the request says: "two horns", "čtyři lesní rohy", "4 horns". A count
    /// of 0 is never stored; it is a [`Question::InvalidCount`].
    pub count: Option<NonZeroU32>,
    /// The parts of the section the request names, ascending: `[1]` for "first violins" or the
    /// Czech "1. housle", `[1, 2]` for "horns 1 and 2". Empty when it names none.
    pub parts: Vec<u8>,
    /// Whether the request asks for one player or the section.
    pub size: Option<Size>,
}

/// What one clause asks of its instruments. Every field the clause does not set is empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    /// The instruments that play, in the order written. Empty only when the parse has a question
    /// about them.
    pub groups: Vec<Group>,
    /// The instruments named after "with" ("s", "se"), which double the others: "violins with
    /// flute".
    pub doubled_by: Vec<Group>,
    /// The role the clause gives them.
    pub role: Option<RoleRequest>,
    /// The articulation, mute and vibrato the clause asks for, when it names any. A request whose
    /// primary is `None` asks for the ordinary sound ("ordinario", "arco") or names only a mute or
    /// a vibrato.
    pub articulation: Option<Request>,
    /// The notated dynamic.
    pub dynamic: Option<Dynamic>,
    /// The change of dynamic.
    pub hairpin: Option<Hairpin>,
    /// How thickly to double the line.
    pub density: Option<Density>,
    /// The bars the clause is about. `None` also when the bars it names are not a range (see
    /// [`Question::InvalidBars`]).
    pub bars: Option<Bars>,
    /// The key the clause names.
    pub key: Option<Key>,
    /// The words the grammar read past, folded, in the order written: words it does not know that
    /// stand where no instrument belongs ("beautiful" in "a beautiful melody").
    pub ignored: Vec<String>,
}

/// One clause of a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clause {
    /// The clause as written in the request.
    pub text: String,
    /// What the clause asks of instruments; `None` for a clause that names none and gives no role
    /// or manner of playing, such as "set the tempo to 96".
    pub assignment: Option<Assignment>,
}

/// A request read by [`parse`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    /// The clauses, in the order written.
    pub clauses: Vec<Clause>,
    /// What the request leaves open, clause by clause in the order written, then the questions
    /// about the request as a whole. Not empty means nothing is to be changed yet.
    pub questions: Vec<Question>,
}

impl Parsed {
    /// Whether the request leaves nothing open.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.questions.is_empty()
    }
}

/// Reads `request` (see the module documentation).
#[must_use]
pub fn parse(request: &str) -> Parsed {
    let raws = raw_words(request);
    let mut fragments = Vec::new();
    for piece in pieces(request, &raws) {
        let tokens = words::tokenize(&piece.words);
        split_at_joiners(tokens, piece.soft_before, &mut fragments);
    }
    let mut parsed = Parsed {
        clauses: Vec::new(),
        questions: Vec::new(),
    };
    for fragment in merge(fragments) {
        let text = clause_text(request, &raws, &fragment.tokens);
        if text.is_empty() {
            // Nothing but joining words, such as the "and" of "and then".
            continue;
        }
        let draft = scan(&fragment.tokens);
        if !draft.is_assignment() {
            parsed.clauses.push(Clause {
                text,
                assignment: None,
            });
            continue;
        }
        if draft.groups.is_empty() && draft.unknown_instruments.is_empty() {
            parsed.questions.push(Question::NoInstruments {
                clause: text.clone(),
            });
        }
        let (assignment, questions) = draft.finish();
        parsed.questions.extend(questions);
        parsed.clauses.push(Clause {
            text,
            assignment: Some(assignment),
        });
    }
    if let Some(question) = horn_sense(&parsed.clauses) {
        parsed.questions.push(question);
    }
    parsed
}

// -- cutting the request ---------------------------------------------------------------------

/// A word of the request as written, or a comma or semicolon.
struct RawWord {
    /// Its bytes in the request.
    span: Range<usize>,
    /// `Some` for a comma (soft) or a semicolon (hard).
    separator: Option<Boundary>,
}

/// How strongly a separator parts a request.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Boundary {
    /// A comma, or "and" away from a list: the parts may still join.
    Soft,
    /// A semicolon or "then": the parts never join.
    Hard,
}

/// The request's words, split at white space, with every comma and semicolon a word of its own.
fn raw_words(request: &str) -> Vec<RawWord> {
    let mut words = Vec::new();
    let mut start = None;
    for (at, c) in request.char_indices() {
        let separator = match c {
            ',' => Some(Boundary::Soft),
            ';' => Some(Boundary::Hard),
            _ => None,
        };
        if c.is_whitespace() || separator.is_some() {
            if let Some(start) = start.take() {
                words.push(RawWord {
                    span: start..at,
                    separator: None,
                });
            }
            if separator.is_some() {
                words.push(RawWord {
                    span: at..at + c.len_utf8(),
                    separator,
                });
            }
        } else if start.is_none() {
            start = Some(at);
        }
    }
    if let Some(start) = start {
        words.push(RawWord {
            span: start..request.len(),
            separator: None,
        });
    }
    words
}

/// A stretch of the request between separators, folded.
struct Piece {
    words: Vec<FoldedWord>,
    /// Whether the separator before it was soft.
    soft_before: bool,
}

/// The request cut at commas, semicolons and "then", each piece folded word by word.
fn pieces(request: &str, raws: &[RawWord]) -> Vec<Piece> {
    let mut pieces = vec![Piece {
        words: Vec::new(),
        soft_before: false,
    }];
    let cut = |pieces: &mut Vec<Piece>, boundary: Boundary| match pieces.last_mut() {
        // Two separators in a row: the stronger one counts.
        Some(last) if last.words.is_empty() => last.soft_before &= boundary == Boundary::Soft,
        _ => pieces.push(Piece {
            words: Vec::new(),
            soft_before: boundary == Boundary::Soft,
        }),
    };
    for (index, raw) in raws.iter().enumerate() {
        if let Some(boundary) = raw.separator {
            cut(&mut pieces, boundary);
            continue;
        }
        let text = request.get(raw.span.clone()).unwrap_or_default();
        let folded = fold(&prepare(text));
        let dotted = is_dotted(text) && !folded.contains(' ');
        for word in folded.split(' ').filter(|word| !word.is_empty()) {
            if words::lookup(word) == Some(Word::Then) {
                cut(&mut pieces, Boundary::Hard);
                continue;
            }
            if let Some(piece) = pieces.last_mut() {
                piece.words.push(FoldedWord {
                    text: word.to_owned(),
                    raw: index,
                    dotted,
                });
            }
        }
    }
    pieces.retain(|piece| !piece.words.is_empty());
    pieces
}

/// A word as written, made ready to fold: `#` reads as `♯`, so "F#" folds like "F♯" to `fis`, and
/// an English possessive "'s" loses its apostrophe, so that its `s` is not read as the Czech "s"
/// (with).
fn prepare(text: &str) -> String {
    let text = text
        .strip_suffix("'s")
        .or_else(|| text.strip_suffix("’s"))
        .map_or_else(|| text.to_owned(), |stem| format!("{stem}s"));
    text.replace('#', "♯")
}

/// Whether a word as written is a number with a trailing full stop, "1.".
fn is_dotted(text: &str) -> bool {
    text.strip_suffix('.')
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// A run of tokens that may become a clause.
struct Fragment {
    tokens: Vec<Token>,
    /// Whether the boundary before it was soft.
    soft_before: bool,
}

/// Cuts a piece's tokens at each joiner that does not stand between two instruments, keeping the
/// joiner at the start of the part after it.
fn split_at_joiners(tokens: Vec<Token>, soft_before: bool, fragments: &mut Vec<Fragment>) {
    let spans: Vec<Range<usize>> = group_reads(&tokens)
        .into_iter()
        .map(|read| read.span)
        .collect();
    let cuts: Vec<bool> = (0..tokens.len())
        .map(|at| {
            is_kind(&tokens, at, |kind| matches!(kind, Kind::Word(Word::Joiner)))
                && !spans.iter().any(|span| span.contains(&at))
                && !is_list_joiner(&tokens, &spans, at)
        })
        .collect();
    let mut current = Fragment {
        tokens: Vec::new(),
        soft_before,
    };
    for (token, cut) in tokens.into_iter().zip(cuts) {
        if cut && !current.tokens.is_empty() {
            fragments.push(current);
            current = Fragment {
                tokens: Vec::new(),
                soft_before: true,
            };
        }
        current.tokens.push(token);
    }
    if !current.tokens.is_empty() {
        fragments.push(current);
    }
}

/// Whether the joiner at `at` joins items of a list of instruments, fillers aside: it stands
/// between the end of one group and the start of another ("horns and timpani", "lesní rohy a
/// tympány"), or between a group and unknown words that then stand where an instrument belongs
/// ("horns and trumpts").
fn is_list_joiner(tokens: &[Token], spans: &[Range<usize>], at: usize) -> bool {
    let unknown = |index: usize| is_kind(tokens, index, |kind| *kind == Kind::Unknown);
    let before = previous_word(tokens, at);
    let after = next_word(tokens, at);
    let group_before = before.is_some_and(|b| spans.iter().any(|span| span.end == b + 1));
    let group_after = after.is_some_and(|a| spans.iter().any(|span| span.start == a));
    if group_before {
        group_after || after.is_some_and(unknown)
    } else {
        group_after && before.is_some_and(unknown)
    }
}

/// Joins the fragments that belong together across a soft boundary: a bare list of instruments,
/// or a fragment of nothing but fillers and verbs, heads the fragment after it; and a fragment
/// that names no instrument but adds a role or a manner of playing (or only fillers) continues
/// the clause before it.
fn merge(fragments: Vec<Fragment>) -> Vec<Fragment> {
    let mut forward: Vec<Fragment> = Vec::with_capacity(fragments.len());
    let mut head: Option<Fragment> = None;
    for mut fragment in fragments {
        if let Some(mut list) = head.take() {
            if fragment.soft_before {
                list.tokens.append(&mut fragment.tokens);
                fragment.tokens = list.tokens;
                fragment.soft_before = list.soft_before;
            } else {
                forward.push(list);
            }
        }
        let draft = scan(&fragment.tokens);
        if draft.is_bare_list || draft.is_empty {
            head = Some(fragment);
        } else {
            forward.push(fragment);
        }
    }
    forward.extend(head);
    let mut merged: Vec<Fragment> = Vec::with_capacity(forward.len());
    for fragment in forward {
        let draft = scan(&fragment.tokens);
        let continues = fragment.soft_before
            && draft.groups.is_empty()
            && draft.unknown_instruments.is_empty()
            && (draft.has_manner() || draft.is_empty)
            && merged
                .last()
                .is_some_and(|before| scan(&before.tokens).is_assignment());
        match merged.last_mut() {
            Some(before) if continues => before.tokens.extend(fragment.tokens),
            _ => merged.push(fragment),
        }
    }
    merged
}

/// The clause's text as written: from its first to its last word, joining words at either end
/// left out.
fn clause_text(request: &str, raws: &[RawWord], tokens: &[Token]) -> String {
    let is_joiner = |token: &&Token| matches!(token.kind, Kind::Word(Word::Joiner));
    let first = tokens.iter().find(|token| !is_joiner(token));
    let last = tokens.iter().rev().find(|token| !is_joiner(token));
    let (Some(first), Some(last)) = (first, last) else {
        return String::new();
    };
    let start = raws.get(first.raw.start).map(|raw| raw.span.start);
    let end = last
        .raw
        .end
        .checked_sub(1)
        .and_then(|index| raws.get(index))
        .map(|raw| raw.span.end);
    match (start, end) {
        (Some(start), Some(end)) => request.get(start..end).unwrap_or_default().to_owned(),
        _ => String::new(),
    }
}

// -- reading a clause ------------------------------------------------------------------------

/// A group read from the tokens, with the tokens it covers.
struct GroupRead {
    group: Group,
    span: Range<usize>,
    /// Whether the request gave the group a count of 0.
    zero_count: bool,
}

/// Every group of instruments in `tokens`, left to right.
fn group_reads(tokens: &[Token]) -> Vec<GroupRead> {
    let mut reads = Vec::new();
    let mut at = 0;
    while at < tokens.len() {
        match read_group(tokens, at) {
            Some(read) => {
                at = read.span.end;
                reads.push(read);
            }
            None => at += 1,
        }
    }
    reads
}

/// The group that starts at token `start`: ordinals ("first and second", "1. a 2."), or a count,
/// then "solo" or "section", the instrument, "solo" or "section" again, and part numbers ("horn
/// 3", "horns 1 and 2"). `None` unless an instrument follows.
fn read_group(tokens: &[Token], start: usize) -> Option<GroupRead> {
    let kind = |at: usize| tokens.get(at).map(|token| &token.kind);
    let mut at = start;
    let mut parts = Vec::new();
    while let Some(Kind::Word(Word::Ordinal(ordinal))) = kind(at) {
        parts.push(*ordinal);
        at += 1;
        if matches!(kind(at), Some(Kind::Word(Word::Joiner)))
            && matches!(kind(at + 1), Some(Kind::Word(Word::Ordinal(_))))
        {
            at += 1;
        } else {
            break;
        }
    }
    let mut count = None;
    if parts.is_empty()
        && let Some(Kind::Number(number)) = kind(at)
    {
        count = Some(*number);
        at += 1;
    }
    let mut size = None;
    if let Some(Kind::Word(Word::Size(written))) = kind(at) {
        size = Some(*written);
        at += 1;
    }
    let Some(Kind::Mention(mention)) = kind(at) else {
        return None;
    };
    at += 1;
    if let Some(Kind::Word(Word::Size(written))) = kind(at) {
        size = size.or(Some(*written));
        at += 1;
    }
    if parts.is_empty() {
        let part = |at: usize| match kind(at) {
            Some(Kind::Number(number)) if !starts_group_word(kind(at + 1)) => {
                u8::try_from(*number).ok().filter(|&part| part > 0)
            }
            _ => None,
        };
        while let Some(number) = part(at) {
            parts.push(number);
            at += 1;
            if matches!(kind(at), Some(Kind::Word(Word::Joiner))) && part(at + 1).is_some() {
                at += 1;
            } else {
                break;
            }
        }
    }
    parts.sort_unstable();
    parts.dedup();
    let mut numbers = mention.matches().iter().map(|(_, alias)| alias.number);
    let first = numbers.next().unwrap_or(Number::Unmarked);
    let number = if numbers.all(|other| other == first) {
        first
    } else {
        Number::Unmarked
    };
    Some(GroupRead {
        group: Group {
            words: mention.text(),
            targets: mention.targets(),
            number,
            count: count.and_then(NonZeroU32::new),
            parts,
            size,
        },
        span: start..at,
        zero_count: count == Some(0),
    })
}

/// Whether a token kind continues a group, so that a number before it is a count, not a part.
fn starts_group_word(kind: Option<&Kind>) -> bool {
    matches!(kind, Some(Kind::Mention(_) | Kind::Word(Word::Size(_))))
}

/// A clause's tokens, read.
#[derive(Default)]
struct Draft {
    groups: Vec<Group>,
    doubled_by: Vec<Group>,
    role: Option<(RoleRequest, String)>,
    primary: Option<(Option<Articulation>, String)>,
    mute: Option<(Mute, String)>,
    vibrato: Option<(Vibrato, String)>,
    dynamic: Option<(Dynamic, String)>,
    hairpin: Option<(Hairpin, String)>,
    density: Option<(Density, String)>,
    bars: Option<((u32, u32), String)>,
    key: Option<(Key, String)>,
    ignored: Vec<String>,
    /// The questions, each with the token it is about, so they come in the order written.
    questions: Vec<(usize, Question)>,
    /// The runs of unknown words that stand where an instrument belongs, each with its first
    /// token.
    unknown_instruments: Vec<(usize, String)>,
    /// Whether every token outside the groups is a joiner or a filler, and there is a group.
    is_bare_list: bool,
    /// Whether the clause holds no group and nothing but joiners, fillers and verbs.
    is_empty: bool,
}

impl Draft {
    /// Whether the clause gives a role, a manner of playing or doubling instruments.
    fn has_manner(&self) -> bool {
        self.role.is_some()
            || self.primary.is_some()
            || self.mute.is_some()
            || self.vibrato.is_some()
            || self.dynamic.is_some()
            || self.hairpin.is_some()
            || self.density.is_some()
            || self.bars.is_some()
            || self.key.is_some()
            || !self.doubled_by.is_empty()
    }

    /// Whether the clause asks something of instruments: it names some, has words in an
    /// instrument's place, or gives a role or a manner of playing.
    fn is_assignment(&self) -> bool {
        !self.groups.is_empty() || !self.unknown_instruments.is_empty() || self.has_manner()
    }

    /// Records `value` in `slot`; a different value already there is a conflict.
    fn note<T: PartialEq>(
        slot: &mut Option<(T, String)>,
        value: T,
        said: &str,
        at: usize,
        aspect: Aspect,
        questions: &mut Vec<(usize, Question)>,
    ) {
        match slot {
            None => *slot = Some((value, said.to_owned())),
            Some((kept, first)) if *kept != value => questions.push((
                at,
                Question::Conflict {
                    aspect,
                    said: vec![first.clone(), said.to_owned()],
                },
            )),
            Some(_) => {}
        }
    }

    /// Adds a group read at token `at`, with the questions its words raise.
    fn add_group(&mut self, read: &GroupRead, at: usize, doubling: bool) {
        let group = read.group.clone();
        if read.zero_count {
            self.questions
                .push((at, Question::InvalidCount { words: group.words }));
        }
        match group.targets.as_slice() {
            [Target::Category(category)] if category.get().selection() == Selection::Ask => {
                self.questions.push((
                    at,
                    Question::ChooseMember {
                        words: group.words,
                        category: *category,
                    },
                ));
            }
            [] | [_] => {}
            targets => self.questions.push((
                at,
                Question::Ambiguous {
                    words: group.words,
                    targets: targets.to_vec(),
                },
            )),
        }
        if doubling {
            self.doubled_by.push(group);
        } else {
            self.groups.push(group);
        }
    }

    /// Records an articulation word: a mute or a vibrato id fills its own slot, any other id and
    /// the ordinary sound the primary one.
    fn articulation(&mut self, word: Option<Articulation>, said: &str, at: usize) {
        let questions = &mut self.questions;
        match word.map(Articulation::axis) {
            None | Some(Axis::Primary) => Self::note(
                &mut self.primary,
                word,
                said,
                at,
                Aspect::Articulation,
                questions,
            ),
            Some(Axis::Mute(mute)) => {
                Self::note(&mut self.mute, mute, said, at, Aspect::Mute, questions);
            }
            Some(Axis::Vibrato(vibrato)) => Self::note(
                &mut self.vibrato,
                vibrato,
                said,
                at,
                Aspect::Vibrato,
                questions,
            ),
        }
    }

    /// Records a token that belongs to no group. Returns whether the instruments read next double
    /// the others ("with" starts that, a verb, a role or a manner of playing ends it), or `None`
    /// when the token leaves that as it was.
    fn read_word(&mut self, token: &Token, at: usize) -> Option<bool> {
        let said = token.text.as_str();
        match &token.kind {
            // A mention always starts a group, so only numbers, ordinals, sizes and unknown words
            // that belong to no group reach this arm with the joining words and fillers.
            Kind::Word(
                Word::Joiner
                | Word::Filler
                | Word::Then
                | Word::Size(_)
                | Word::Ordinal(_)
                | Word::Count(_),
            )
            | Kind::Number(_)
            | Kind::Mention(_)
            | Kind::Unknown => return None,
            Kind::Word(Word::Verb) => return Some(false),
            Kind::Word(Word::With) => return Some(true),
            Kind::Word(Word::Role(role)) => {
                Self::note(
                    &mut self.role,
                    *role,
                    said,
                    at,
                    Aspect::Role,
                    &mut self.questions,
                );
            }
            Kind::Word(Word::Articulation(articulation)) => {
                self.articulation(*articulation, said, at);
            }
            Kind::Word(Word::Dynamic(dynamic)) => Self::note(
                &mut self.dynamic,
                *dynamic,
                said,
                at,
                Aspect::Dynamic,
                &mut self.questions,
            ),
            Kind::Word(Word::Hairpin(hairpin)) => Self::note(
                &mut self.hairpin,
                *hairpin,
                said,
                at,
                Aspect::Hairpin,
                &mut self.questions,
            ),
            Kind::Word(Word::Density(density)) => Self::note(
                &mut self.density,
                *density,
                said,
                at,
                Aspect::Density,
                &mut self.questions,
            ),
            Kind::Bars(first, last) => {
                if Bars::new(*first, *last).is_none() {
                    self.questions.push((
                        at,
                        Question::InvalidBars {
                            first: *first,
                            last: *last,
                        },
                    ));
                }
                Self::note(
                    &mut self.bars,
                    (*first, *last),
                    said,
                    at,
                    Aspect::Bars,
                    &mut self.questions,
                );
            }
            Kind::Key(key) => {
                Self::note(
                    &mut self.key,
                    *key,
                    said,
                    at,
                    Aspect::Key,
                    &mut self.questions,
                );
            }
        }
        Some(false)
    }

    /// The assignment and the clause's questions in the order written.
    fn finish(mut self) -> (Assignment, Vec<Question>) {
        let articulation = (self.primary.is_some()
            || self.mute.is_some()
            || self.vibrato.is_some())
        .then(|| {
            Request::new(
                self.primary.and_then(|(primary, _)| primary),
                self.mute.map_or(Mute::Open, |(mute, _)| mute),
                self.vibrato
                    .map_or(Vibrato::Ordinary, |(vibrato, _)| vibrato),
            )
            .expect("the primary slot holds only ids whose axis is primary (Draft::articulation)")
        });
        // Suggestions are looked for only now, once per clause: a clause is read more than once
        // while the request is cut.
        for (at, words) in self.unknown_instruments {
            let suggestions = question::suggestions(&words);
            self.questions
                .push((at, Question::UnknownInstrument { words, suggestions }));
        }
        self.questions.sort_by_key(|&(at, _)| at);
        let assignment = Assignment {
            groups: self.groups,
            doubled_by: self.doubled_by,
            role: self.role.map(|(role, _)| role),
            articulation,
            dynamic: self.dynamic.map(|(dynamic, _)| dynamic),
            hairpin: self.hairpin.map(|(hairpin, _)| hairpin),
            density: self.density.map(|(density, _)| density),
            bars: self
                .bars
                .and_then(|((first, last), _)| Bars::new(first, last)),
            key: self.key.map(|(key, _)| key),
            ignored: self.ignored,
        };
        let questions = self
            .questions
            .into_iter()
            .map(|(_, question)| question)
            .collect();
        (assignment, questions)
    }
}

/// Reads a clause's tokens: its groups, role and manner of playing, and the questions they raise.
fn scan(tokens: &[Token]) -> Draft {
    let reads = group_reads(tokens);
    let mut draft = Draft::default();
    let mut doubling = false;
    let mut only_list_words = true;
    let mut only_empty_words = true;
    let mut at = 0;
    while let Some(token) = tokens.get(at) {
        if let Some(read) = reads.iter().find(|read| read.span.start == at) {
            draft.add_group(read, at, doubling);
            only_empty_words = false;
            at = read.span.end;
            continue;
        }
        if let Some(with) = draft.read_word(token, at) {
            doubling = with;
        }
        let list_word = matches!(token.kind, Kind::Word(Word::Joiner | Word::Filler));
        only_list_words &= list_word;
        only_empty_words &= list_word || matches!(token.kind, Kind::Word(Word::Verb));
        at += 1;
    }
    draft.is_bare_list = only_list_words && !draft.groups.is_empty();
    draft.is_empty = only_empty_words;
    read_unknown_words(tokens, &reads, &mut draft);
    draft
}

/// Sorts the runs of unknown words of a clause into words where an instrument belongs, which
/// become questions with suggestions, and words the grammar reads past.
///
/// An instrument belongs after a count or an ordinal that starts no group ("two saxhorns"), after
/// "with", beside a joiner whose other side is a group ("horns and trumpts", "trumpts and
/// horns"), and, in a clause that names no instrument, right before its verb or "underneath"
/// ("saxhorns play the melody", "saxhorns underneath"). A word before a role noun is read past:
/// "beautiful" in "a beautiful melody". Fillers between do not count.
fn read_unknown_words(tokens: &[Token], reads: &[GroupRead], draft: &mut Draft) {
    let in_group = |at: usize| reads.iter().any(|read| read.span.contains(&at));
    let ends_group = |at: usize| reads.iter().any(|read| read.span.end == at + 1);
    let starts_group = |at: usize| reads.iter().any(|read| read.span.start == at);
    let kind_is = |at: usize, test: fn(&Kind) -> bool| is_kind(tokens, at, test);
    let no_instruments = draft.groups.is_empty();
    let mut at = 0;
    while at < tokens.len() {
        if !kind_is(at, |kind| matches!(kind, Kind::Unknown)) || in_group(at) {
            at += 1;
            continue;
        }
        let start = at;
        while at < tokens.len() && kind_is(at, |kind| matches!(kind, Kind::Unknown)) {
            at += 1;
        }
        let before = previous_word(tokens, start);
        let after = next_word(tokens, at - 1);
        let after_count = before.is_some_and(|b| {
            !in_group(b)
                && kind_is(b, |kind| {
                    matches!(kind, Kind::Number(_) | Kind::Word(Word::Ordinal(_)))
                })
        });
        let after_with = before.is_some_and(|b| kind_is(b, |kind| *kind == Kind::Word(Word::With)));
        let is_joiner = |at: usize| kind_is(at, |kind| *kind == Kind::Word(Word::Joiner));
        let listed = before
            .is_some_and(|b| is_joiner(b) && previous_word(tokens, b).is_some_and(ends_group))
            || after
                .is_some_and(|a| is_joiner(a) && next_word(tokens, a).is_some_and(starts_group));
        let subject = no_instruments
            && after.is_some_and(|a| {
                kind_is(a, |kind| {
                    matches!(
                        kind,
                        Kind::Word(Word::Verb | Word::Role(RoleRequest::Underneath))
                    )
                })
            });
        let words = tokens
            .get(start..at)
            .unwrap_or_default()
            .iter()
            .map(|token| token.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if after_count || after_with || listed || subject {
            draft.unknown_instruments.push((start, words));
        } else {
            draft.ignored.push(words);
        }
    }
}

/// Whether the token at `at` exists and its kind passes `test`.
fn is_kind(tokens: &[Token], at: usize, test: fn(&Kind) -> bool) -> bool {
    tokens.get(at).is_some_and(|token| test(&token.kind))
}

/// The nearest token before `at` that is not a filler.
fn previous_word(tokens: &[Token], at: usize) -> Option<usize> {
    (0..at).rev().find(|&index| !is_filler(tokens, index))
}

/// The nearest token after `at` that is not a filler.
fn next_word(tokens: &[Token], at: usize) -> Option<usize> {
    (at + 1..tokens.len()).find(|&index| !is_filler(tokens, index))
}

fn is_filler(tokens: &[Token], at: usize) -> bool {
    is_kind(tokens, at, |kind| *kind == Kind::Word(Word::Filler))
}

// -- the request as a whole ------------------------------------------------------------------

/// The orchestras of the ensemble templates. Their instruments are what "orchestral" means for
/// [`Question::HornSense`]; the big band and the chamber groups are left out.
const ORCHESTRAS: [EnsembleId; 8] = [
    EnsembleId::ClassicalOrchestra,
    EnsembleId::BeethovenOrchestra,
    EnsembleId::RomanticOrchestra,
    EnsembleId::LateRomanticOrchestra,
    EnsembleId::FilmOrchestra,
    EnsembleId::ChamberOrchestra,
    EnsembleId::StringOrchestra,
    EnsembleId::ChoirAndOrchestra,
];

/// [`Question::HornSense`] when the request names the horn only as "horn" or "horns" and names
/// nothing else an orchestra plays.
fn horn_sense(clauses: &[Clause]) -> Option<Question> {
    let groups: Vec<&Group> = clauses
        .iter()
        .filter_map(|clause| clause.assignment.as_ref())
        .flat_map(|assignment| assignment.groups.iter().chain(&assignment.doubled_by))
        .collect();
    let is_bare_horn = |group: &Group| {
        group.targets == [Target::Instrument(InstrumentType::HornF)]
            && matches!(group.words, "horn" | "horns")
    };
    let horn = groups.iter().find(|group| is_bare_horn(group))?;
    let orchestral = groups
        .iter()
        .filter(|group| !is_bare_horn(group))
        .any(|group| group.targets.iter().any(|&target| is_orchestral(target)));
    (!orchestral).then_some(Question::HornSense { words: horn.words })
}

/// Whether an orchestra template has a part for the target: for a category or a family, for any
/// of its instruments.
fn is_orchestral(target: Target) -> bool {
    let in_orchestra = |instrument: InstrumentType| {
        ORCHESTRAS.iter().any(|orchestra| {
            orchestra
                .get()
                .parts()
                .iter()
                .any(|part| part.instrument == instrument)
        })
    };
    match target {
        Target::Instrument(instrument) => in_orchestra(instrument),
        Target::Category(category) => category.get().members().any(in_orchestra),
        Target::Family(family) => InstrumentType::ALL
            .into_iter()
            .filter(|instrument| instrument.family() == family)
            .any(in_orchestra),
    }
}
