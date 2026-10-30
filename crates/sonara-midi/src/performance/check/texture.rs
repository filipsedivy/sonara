//! The texture of a whole piece: characteristic colours overused (TEX-01) and percussion all
//! together or brass and percussion never resting (TEX-02).
//!
//! A piece runs from the bar of its first note to the bar of its last note's end, in the score's
//! metre; a part sounds in a bar when one of its notes sounds in it.

use sonara_core::midi::MidiNote;

use super::{BEAT_EPSILON, Context, Hit, HitBuilder, Part};
use crate::orchestration::Family;
use crate::orchestration::InstrumentType;
use crate::orchestration::rules::{
    ALL_PERCUSSION, CHARACTERISTIC_COLOUR_SHARE, RuleId, UNRESTING_BARS,
};

/// The colours too characteristic to be used over frequently: the celesta, glockenspiel,
/// xylophone, castanets and the piano, of whatever make.
const CHARACTERISTIC_COLOURS: [InstrumentType; 10] = [
    InstrumentType::Celesta,
    InstrumentType::Glockenspiel,
    InstrumentType::Xylophone,
    InstrumentType::Castanets,
    InstrumentType::PianoConcertGrand,
    InstrumentType::PianoStudioGrand,
    InstrumentType::PianoUpright,
    InstrumentType::PianoFelt,
    InstrumentType::PianoPrepared,
    InstrumentType::PianoHonkyTonk,
];

/// TEX-01: a characteristic colour, all its parts together, sounding in more than
/// [`CHARACTERISTIC_COLOUR_SHARE`] of the piece's bars.
pub(super) fn characteristic_colours(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Tex01);
    let Some(piece) = Piece::of(context) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for colour in CHARACTERISTIC_COLOURS {
        let parts: Vec<&Part<'_>> = context
            .parts_where(|instrument| instrument == colour)
            .collect();
        let notes: Vec<&MidiNote> = parts
            .iter()
            .flat_map(|part| part.notes.iter().copied())
            .collect();
        let sounding = piece.bars_sounding(context, &notes);
        let bars = sounding.iter().filter(|&&sounds| sounds).count();
        let share = ratio(bars, piece.bars);
        if share > f64::from(CHARACTERISTIC_COLOUR_SHARE.value) {
            let detail = format!(
                "the {} sounds in {bars} of {} bars",
                colour.get().name(),
                piece.bars,
            );
            hits.extend(builder.notes(builder.fixed(), &parts, &notes, detail));
        }
    }
    hits
}

/// TEX-02: [`ALL_PERCUSSION`] percussion parts or more all sounding at once, each span once; and
/// the brass and the percussion both sounding in every bar of a piece of [`UNRESTING_BARS`] bars
/// or more.
pub(super) fn percussion_and_brass_rests(context: &Context<'_>) -> Vec<Hit> {
    let builder = HitBuilder::new(RuleId::Tex02);
    let percussion: Vec<&Part<'_>> = context
        .parts_where(|instrument| instrument.family() == Family::Percussion)
        .collect();
    let mut hits = Vec::new();
    if percussion.len() >= usize::from(ALL_PERCUSSION.value) {
        for (start, end) in all_sounding(&percussion) {
            let notes: Vec<&MidiNote> = percussion
                .iter()
                .flat_map(|part| part.notes.iter().copied())
                .filter(|note| note.start < end - BEAT_EPSILON && note.end() > start + BEAT_EPSILON)
                .collect();
            let detail = format!("all {} percussion parts play at once", percussion.len());
            hits.push(builder.span(builder.fixed(), &percussion, &notes, start, end, detail));
        }
    }
    let brass: Vec<&Part<'_>> = context
        .parts_where(|instrument| instrument.family() == Family::Brass)
        .collect();
    if let Some(piece) = Piece::of(context)
        && piece.bars >= usize::from(UNRESTING_BARS.value)
        && !brass.is_empty()
        && !percussion.is_empty()
    {
        let throughout = |parts: &[&Part<'_>]| {
            let notes: Vec<&MidiNote> = parts
                .iter()
                .flat_map(|part| part.notes.iter().copied())
                .collect();
            piece
                .bars_sounding(context, &notes)
                .into_iter()
                .all(|sounds| sounds)
        };
        if throughout(&brass) && throughout(&percussion) {
            let parts: Vec<&Part<'_>> = brass.iter().chain(&percussion).copied().collect();
            let detail = format!(
                "the brass and the percussion sound in every one of {} bars",
                piece.bars
            );
            hits.push(builder.span(builder.fixed(), &parts, &[], piece.start, piece.end, detail));
        }
    }
    hits
}

/// The bars of a piece.
struct Piece {
    /// The 0-based bar the piece starts in.
    first_bar: f64,
    /// How many bars it spans.
    bars: usize,
    /// Where it starts, in content beats: the first note's start.
    start: f64,
    /// Where it ends, in content beats: the last note's end.
    end: f64,
}

impl Piece {
    /// The piece of every note of `context`, or `None` when there is no note.
    fn of(context: &Context<'_>) -> Option<Self> {
        let notes = context.parts.iter().flat_map(|part| part.notes.iter());
        let start = notes.clone().map(|note| note.start).reduce(f64::min)?;
        let end = notes.map(|note| note.end()).reduce(f64::max)?;
        let first_bar = bar_floor(context, start);
        let last_bar = bar_ceil(context, end);
        let bars = to_count(last_bar - first_bar);
        Some(Self {
            first_bar,
            bars,
            start,
            end,
        })
    }

    /// For each bar of the piece, whether one of `notes` sounds in it.
    fn bars_sounding(&self, context: &Context<'_>, notes: &[&MidiNote]) -> Vec<bool> {
        let mut sounding = vec![false; self.bars];
        for note in notes {
            let from = to_count(bar_floor(context, note.start) - self.first_bar);
            let to = to_count(bar_ceil(context, note.end()) - self.first_bar);
            for bar in sounding.iter_mut().take(to).skip(from) {
                *bar = true;
            }
        }
        sounding
    }
}

/// The 0-based bar that holds content beat `beat`.
fn bar_floor(context: &Context<'_>, beat: f64) -> f64 {
    let signatures = context.score.signatures();
    (signatures.beat_to_bar(context.score.origin() + beat) + BEAT_EPSILON).floor()
}

/// The 0-based bar after the one that holds the moment just before content beat `beat`.
fn bar_ceil(context: &Context<'_>, beat: f64) -> f64 {
    let signatures = context.score.signatures();
    (signatures.beat_to_bar(context.score.origin() + beat) - BEAT_EPSILON).ceil()
}

/// A whole number of bars as a count; a negative or non-finite one is 0.
fn to_count(bars: f64) -> usize {
    if bars.is_finite() && bars > 0.0 {
        // A piece's bar count is far below 2^52, so the conversion is exact.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is a whole, positive, finite number of bars"
        )]
        let count = bars.round() as usize;
        count
    } else {
        0
    }
}

/// `part` divided by `whole`; 0 for an empty whole.
fn ratio(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        #[expect(clippy::cast_precision_loss, reason = "bar counts are far below 2^52")]
        let ratio = part as f64 / whole as f64;
        ratio
    }
}

/// The spans, in content beats, in which every one of `parts` sounds a note; spans that touch are
/// joined.
fn all_sounding(parts: &[&Part<'_>]) -> Vec<(f64, f64)> {
    let mut events: Vec<(f64, bool, usize)> = parts
        .iter()
        .enumerate()
        .flat_map(|(index, part)| {
            part.notes
                .iter()
                .flat_map(move |note| [(note.start, true, index), (note.end(), false, index)])
        })
        .collect();
    // At one beat a note's end comes before another's start: the two do not sound together.
    events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut sounding = vec![0_usize; parts.len()];
    let mut spans: Vec<(f64, f64)> = Vec::new();
    let mut opened: Option<f64> = None;
    for (beat, starts, index) in events {
        if let Some(count) = sounding.get_mut(index) {
            *count = if starts {
                count.saturating_add(1)
            } else {
                count.saturating_sub(1)
            };
        }
        let all = sounding.iter().all(|&count| count > 0);
        match (all, opened) {
            (true, None) => opened = Some(beat),
            (false, Some(start)) => {
                match spans.last_mut() {
                    Some(last) if (start - last.1).abs() < BEAT_EPSILON => last.1 = beat,
                    _ => spans.push((start, beat)),
                }
                opened = None;
            }
            _ => {}
        }
    }
    spans.retain(|(start, end)| end - start > BEAT_EPSILON);
    spans
}
