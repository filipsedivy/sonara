//! The words of the orchestration grammar, and the reading of one stretch of a request into
//! tokens.
//!
//! Every phrase is stored folded, the way `sonara_midi::orchestration::fold` folds text:
//! lower-case ASCII letters and digits, words separated by one space ("pod tím" is `pod tim`,
//! "držené" is `drzene`). Like the instrument aliases, the Czech forms are listed one by one;
//! nothing is stemmed or guessed.
//!
//! Instrument names are not here: they are the aliases of `sonara_midi::orchestration`, found by
//! its `resolve` (DES-01). Where a phrase of these tables and an alias start at the same word, the
//! longer one wins, and an alias wins a tie: "bass line" is a role, "bass" alone an instrument,
//! and "piano" always the instrument.

use std::ops::Range;

use sonara_midi::orchestration::rules::{Dynamic, Role};
use sonara_midi::orchestration::{Articulation, Lang, Mention, resolve};
use sonara_midi::performance::voicing::Density;
use sonara_midi::theory::{Key, Scale};

use super::{Hairpin, RoleRequest, Size};

/// The languages a request is read in. The grammar's own words are English and Czech, so the
/// aliases of the other languages would only add readings nobody meant (the French "alto" is the
/// viola, the English one a voice).
pub(super) const LANGUAGES: [Lang; 2] = [Lang::En, Lang::Cs];

/// The most words a phrase of [`PHRASES`] or an articulation id spans.
pub(super) const MAX_PHRASE_WORDS: usize = 4;

/// What a phrase of the tables means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Word {
    /// A function in the texture: "melody", "akordy", "pod tím".
    Role(RoleRequest),
    /// An articulation, a mute or a vibrato setting; `None` is the ordinary sound ("ordinario",
    /// "arco").
    Articulation(Option<Articulation>),
    /// A notated dynamic: "pp", "softly", "potichu".
    Dynamic(Dynamic),
    /// A change of dynamic: "crescendo", "zesiluje".
    Hairpin(Hairpin),
    /// How thickly the line is doubled: "full", "plně".
    Density(Density),
    /// One player or the whole section: "solo", "sekce".
    Size(Size),
    /// How many play: "two", "čtyři". Digits are numbers of their own.
    Count(u16),
    /// Which part of a section: "first", "druhé". "1." is an ordinal too.
    Ordinal(u8),
    /// A verb that joins the instruments to what they play: "play", "hrají".
    Verb,
    /// A word that joins two items of a list: "and", "a", "i".
    Joiner,
    /// A word that adds instruments doubling the others: "with", "s".
    With,
    /// A word that ends one request and starts the next: "then", "potom".
    Then,
    /// A word that carries nothing the grammar reads: "the", "please", "v".
    Filler,
}

use Word::{Filler, Joiner, Then, Verb, With};

/// Each phrase of the grammar, folded, with what it means. A phrase is listed once.
pub(super) const PHRASES: &[(&str, Word)] = &[
    // Roles.
    ("melody", Word::Role(RoleRequest::Role(Role::Melody))),
    ("melodies", Word::Role(RoleRequest::Role(Role::Melody))),
    ("tune", Word::Role(RoleRequest::Role(Role::Melody))),
    ("tunes", Word::Role(RoleRequest::Role(Role::Melody))),
    ("theme", Word::Role(RoleRequest::Role(Role::Melody))),
    ("themes", Word::Role(RoleRequest::Role(Role::Melody))),
    ("lead", Word::Role(RoleRequest::Role(Role::Melody))),
    ("melodie", Word::Role(RoleRequest::Role(Role::Melody))),
    ("melodii", Word::Role(RoleRequest::Role(Role::Melody))),
    ("melodiemi", Word::Role(RoleRequest::Role(Role::Melody))),
    ("tema", Word::Role(RoleRequest::Role(Role::Melody))),
    ("tematu", Word::Role(RoleRequest::Role(Role::Melody))),
    ("tematem", Word::Role(RoleRequest::Role(Role::Melody))),
    ("temata", Word::Role(RoleRequest::Role(Role::Melody))),
    (
        "countermelody",
        Word::Role(RoleRequest::Role(Role::CounterMelody)),
    ),
    (
        "countermelodies",
        Word::Role(RoleRequest::Role(Role::CounterMelody)),
    ),
    (
        "counter melody",
        Word::Role(RoleRequest::Role(Role::CounterMelody)),
    ),
    (
        "counter melodies",
        Word::Role(RoleRequest::Role(Role::CounterMelody)),
    ),
    (
        "kontramelodie",
        Word::Role(RoleRequest::Role(Role::CounterMelody)),
    ),
    (
        "kontramelodii",
        Word::Role(RoleRequest::Role(Role::CounterMelody)),
    ),
    (
        "protimelodie",
        Word::Role(RoleRequest::Role(Role::CounterMelody)),
    ),
    (
        "protimelodii",
        Word::Role(RoleRequest::Role(Role::CounterMelody)),
    ),
    (
        "protihlas",
        Word::Role(RoleRequest::Role(Role::CounterMelody)),
    ),
    (
        "protihlasem",
        Word::Role(RoleRequest::Role(Role::CounterMelody)),
    ),
    ("harmony", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("harmonies", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("chord", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("chords", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("pad", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("pads", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("harmonie", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("harmonii", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("akord", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("akordy", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("akordu", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("akordem", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("akordech", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("akordum", Word::Role(RoleRequest::Role(Role::Harmony))),
    ("bass line", Word::Role(RoleRequest::Role(Role::Bass))),
    ("bass lines", Word::Role(RoleRequest::Role(Role::Bass))),
    ("bassline", Word::Role(RoleRequest::Role(Role::Bass))),
    ("basslines", Word::Role(RoleRequest::Role(Role::Bass))),
    ("basova linka", Word::Role(RoleRequest::Role(Role::Bass))),
    ("basovou linku", Word::Role(RoleRequest::Role(Role::Bass))),
    ("basove linky", Word::Role(RoleRequest::Role(Role::Bass))),
    ("basove lince", Word::Role(RoleRequest::Role(Role::Bass))),
    ("basovou linkou", Word::Role(RoleRequest::Role(Role::Bass))),
    ("pedal", Word::Role(RoleRequest::Role(Role::Pedal))),
    ("pedal point", Word::Role(RoleRequest::Role(Role::Pedal))),
    ("pedal note", Word::Role(RoleRequest::Role(Role::Pedal))),
    ("drone", Word::Role(RoleRequest::Role(Role::Pedal))),
    ("prodleva", Word::Role(RoleRequest::Role(Role::Pedal))),
    ("prodlevu", Word::Role(RoleRequest::Role(Role::Pedal))),
    ("prodlevou", Word::Role(RoleRequest::Role(Role::Pedal))),
    ("prodlevy", Word::Role(RoleRequest::Role(Role::Pedal))),
    ("rhythm", Word::Role(RoleRequest::Role(Role::Rhythm))),
    ("rhythms", Word::Role(RoleRequest::Role(Role::Rhythm))),
    ("ostinato", Word::Role(RoleRequest::Role(Role::Rhythm))),
    ("ostinati", Word::Role(RoleRequest::Role(Role::Rhythm))),
    ("ostinatem", Word::Role(RoleRequest::Role(Role::Rhythm))),
    ("rytmus", Word::Role(RoleRequest::Role(Role::Rhythm))),
    ("rytmu", Word::Role(RoleRequest::Role(Role::Rhythm))),
    ("rytmem", Word::Role(RoleRequest::Role(Role::Rhythm))),
    ("rytmy", Word::Role(RoleRequest::Role(Role::Rhythm))),
    ("colour", Word::Role(RoleRequest::Role(Role::Colour))),
    ("colours", Word::Role(RoleRequest::Role(Role::Colour))),
    ("color", Word::Role(RoleRequest::Role(Role::Colour))),
    ("colors", Word::Role(RoleRequest::Role(Role::Colour))),
    ("barva", Word::Role(RoleRequest::Role(Role::Colour))),
    ("barvu", Word::Role(RoleRequest::Role(Role::Colour))),
    ("barvou", Word::Role(RoleRequest::Role(Role::Colour))),
    ("underneath", Word::Role(RoleRequest::Underneath)),
    ("underneath it", Word::Role(RoleRequest::Underneath)),
    ("underneath the melody", Word::Role(RoleRequest::Underneath)),
    ("under", Word::Role(RoleRequest::Underneath)),
    ("under it", Word::Role(RoleRequest::Underneath)),
    ("under the melody", Word::Role(RoleRequest::Underneath)),
    ("below", Word::Role(RoleRequest::Underneath)),
    ("below it", Word::Role(RoleRequest::Underneath)),
    ("below the melody", Word::Role(RoleRequest::Underneath)),
    ("beneath", Word::Role(RoleRequest::Underneath)),
    ("beneath it", Word::Role(RoleRequest::Underneath)),
    ("beneath the melody", Word::Role(RoleRequest::Underneath)),
    ("accompany", Word::Role(RoleRequest::Underneath)),
    ("accompanies", Word::Role(RoleRequest::Underneath)),
    ("accompanying", Word::Role(RoleRequest::Underneath)),
    ("accompaniment", Word::Role(RoleRequest::Underneath)),
    ("pod", Word::Role(RoleRequest::Underneath)),
    ("pod tim", Word::Role(RoleRequest::Underneath)),
    ("pod nim", Word::Role(RoleRequest::Underneath)),
    ("pod ni", Word::Role(RoleRequest::Underneath)),
    ("pod nimi", Word::Role(RoleRequest::Underneath)),
    ("pod melodii", Word::Role(RoleRequest::Underneath)),
    ("doprovod", Word::Role(RoleRequest::Underneath)),
    ("doprovodem", Word::Role(RoleRequest::Underneath)),
    ("doprovazi", Word::Role(RoleRequest::Underneath)),
    ("doprovazeji", Word::Role(RoleRequest::Underneath)),
    ("doprovazet", Word::Role(RoleRequest::Underneath)),
    // Articulations beside the vocabulary's own ids, which are words of their own
    // ("legato", "col legno", "sul ponticello").
    ("sustained", Word::Articulation(Some(Articulation::Sustain))),
    (
        "long notes",
        Word::Articulation(Some(Articulation::Sustain)),
    ),
    (
        "held notes",
        Word::Articulation(Some(Articulation::Sustain)),
    ),
    ("drzene", Word::Articulation(Some(Articulation::Sustain))),
    (
        "drzene tony",
        Word::Articulation(Some(Articulation::Sustain)),
    ),
    (
        "drzenymi tony",
        Word::Articulation(Some(Articulation::Sustain)),
    ),
    (
        "dlouhe tony",
        Word::Articulation(Some(Articulation::Sustain)),
    ),
    (
        "dlouhymi tony",
        Word::Articulation(Some(Articulation::Sustain)),
    ),
    ("slurred", Word::Articulation(Some(Articulation::Legato))),
    ("legatem", Word::Articulation(Some(Articulation::Legato))),
    ("vazane", Word::Articulation(Some(Articulation::Legato))),
    ("vazanou", Word::Articulation(Some(Articulation::Legato))),
    (
        "vazanymi tony",
        Word::Articulation(Some(Articulation::Legato)),
    ),
    ("stacc", Word::Articulation(Some(Articulation::Staccato))),
    (
        "short notes",
        Word::Articulation(Some(Articulation::Staccato)),
    ),
    (
        "staccatem",
        Word::Articulation(Some(Articulation::Staccato)),
    ),
    ("kratce", Word::Articulation(Some(Articulation::Staccato))),
    (
        "kratke tony",
        Word::Articulation(Some(Articulation::Staccato)),
    ),
    (
        "kratkymi tony",
        Word::Articulation(Some(Articulation::Staccato)),
    ),
    ("spicc", Word::Articulation(Some(Articulation::Spiccato))),
    ("pizz", Word::Articulation(Some(Articulation::Pizzicato))),
    (
        "pizzicatem",
        Word::Articulation(Some(Articulation::Pizzicato)),
    ),
    ("trem", Word::Articulation(Some(Articulation::Tremolo))),
    ("tremolem", Word::Articulation(Some(Articulation::Tremolo))),
    ("trills", Word::Articulation(Some(Articulation::Trill))),
    ("trylek", Word::Articulation(Some(Articulation::Trill))),
    ("trylky", Word::Articulation(Some(Articulation::Trill))),
    ("trylkem", Word::Articulation(Some(Articulation::Trill))),
    (
        "flageolet",
        Word::Articulation(Some(Articulation::Harmonics)),
    ),
    (
        "flageolets",
        Word::Articulation(Some(Articulation::Harmonics)),
    ),
    (
        "flazolet",
        Word::Articulation(Some(Articulation::Harmonics)),
    ),
    (
        "flazolety",
        Word::Articulation(Some(Articulation::Harmonics)),
    ),
    (
        "flazoletem",
        Word::Articulation(Some(Articulation::Harmonics)),
    ),
    (
        "sul pont",
        Word::Articulation(Some(Articulation::SulPonticello)),
    ),
    (
        "con sord",
        Word::Articulation(Some(Articulation::ConSordino)),
    ),
    ("muted", Word::Articulation(Some(Articulation::ConSordino))),
    (
        "with mute",
        Word::Articulation(Some(Articulation::ConSordino)),
    ),
    (
        "with mutes",
        Word::Articulation(Some(Articulation::ConSordino)),
    ),
    (
        "s dusitkem",
        Word::Articulation(Some(Articulation::ConSordino)),
    ),
    (
        "s dusitky",
        Word::Articulation(Some(Articulation::ConSordino)),
    ),
    (
        "flutter",
        Word::Articulation(Some(Articulation::FlutterTongue)),
    ),
    (
        "frullato",
        Word::Articulation(Some(Articulation::FlutterTongue)),
    ),
    ("rolls", Word::Articulation(Some(Articulation::Roll))),
    ("vireni", Word::Articulation(Some(Articulation::Roll))),
    ("virenim", Word::Articulation(Some(Articulation::Roll))),
    ("accents", Word::Articulation(Some(Articulation::Accent))),
    ("accented", Word::Articulation(Some(Articulation::Accent))),
    ("akcent", Word::Articulation(Some(Articulation::Accent))),
    ("akcenty", Word::Articulation(Some(Articulation::Accent))),
    ("akcentem", Word::Articulation(Some(Articulation::Accent))),
    ("sfz", Word::Articulation(Some(Articulation::Sforzando))),
    ("gliss", Word::Articulation(Some(Articulation::Glissando))),
    (
        "senza vibrato",
        Word::Articulation(Some(Articulation::NonVibrato)),
    ),
    (
        "without vibrato",
        Word::Articulation(Some(Articulation::NonVibrato)),
    ),
    (
        "no vibrato",
        Word::Articulation(Some(Articulation::NonVibrato)),
    ),
    (
        "bez vibrata",
        Word::Articulation(Some(Articulation::NonVibrato)),
    ),
    ("vibrato", Word::Articulation(Some(Articulation::VibratoOn))),
    (
        "with vibrato",
        Word::Articulation(Some(Articulation::VibratoOn)),
    ),
    (
        "s vibratem",
        Word::Articulation(Some(Articulation::VibratoOn)),
    ),
    ("ordinario", Word::Articulation(None)),
    ("ord", Word::Articulation(None)),
    ("normale", Word::Articulation(None)),
    ("arco", Word::Articulation(None)),
    // Dynamics. "piano" stays the instrument: an alias wins a tie.
    ("ppp", Word::Dynamic(Dynamic::Pianissimo)),
    ("pp", Word::Dynamic(Dynamic::Pianissimo)),
    ("pianissimo", Word::Dynamic(Dynamic::Pianissimo)),
    ("very softly", Word::Dynamic(Dynamic::Pianissimo)),
    ("very soft", Word::Dynamic(Dynamic::Pianissimo)),
    ("very quietly", Word::Dynamic(Dynamic::Pianissimo)),
    ("velmi potichu", Word::Dynamic(Dynamic::Pianissimo)),
    ("velmi tise", Word::Dynamic(Dynamic::Pianissimo)),
    ("hodne potichu", Word::Dynamic(Dynamic::Pianissimo)),
    ("p", Word::Dynamic(Dynamic::Piano)),
    ("softly", Word::Dynamic(Dynamic::Piano)),
    ("soft", Word::Dynamic(Dynamic::Piano)),
    ("quietly", Word::Dynamic(Dynamic::Piano)),
    ("quiet", Word::Dynamic(Dynamic::Piano)),
    ("gently", Word::Dynamic(Dynamic::Piano)),
    ("potichu", Word::Dynamic(Dynamic::Piano)),
    ("tise", Word::Dynamic(Dynamic::Piano)),
    ("jemne", Word::Dynamic(Dynamic::Piano)),
    ("mp", Word::Dynamic(Dynamic::MezzoPiano)),
    ("mezzo piano", Word::Dynamic(Dynamic::MezzoPiano)),
    ("mezzopiano", Word::Dynamic(Dynamic::MezzoPiano)),
    ("mf", Word::Dynamic(Dynamic::MezzoForte)),
    ("mezzo forte", Word::Dynamic(Dynamic::MezzoForte)),
    ("mezzoforte", Word::Dynamic(Dynamic::MezzoForte)),
    ("f", Word::Dynamic(Dynamic::Forte)),
    ("forte", Word::Dynamic(Dynamic::Forte)),
    ("loud", Word::Dynamic(Dynamic::Forte)),
    ("loudly", Word::Dynamic(Dynamic::Forte)),
    ("strongly", Word::Dynamic(Dynamic::Forte)),
    ("nahlas", Word::Dynamic(Dynamic::Forte)),
    ("hlasite", Word::Dynamic(Dynamic::Forte)),
    ("silne", Word::Dynamic(Dynamic::Forte)),
    ("ff", Word::Dynamic(Dynamic::Fortissimo)),
    ("fff", Word::Dynamic(Dynamic::Fortissimo)),
    ("fortissimo", Word::Dynamic(Dynamic::Fortissimo)),
    ("very loud", Word::Dynamic(Dynamic::Fortissimo)),
    ("very loudly", Word::Dynamic(Dynamic::Fortissimo)),
    ("velmi nahlas", Word::Dynamic(Dynamic::Fortissimo)),
    ("velmi hlasite", Word::Dynamic(Dynamic::Fortissimo)),
    ("velmi silne", Word::Dynamic(Dynamic::Fortissimo)),
    // Changes of dynamic.
    ("crescendo", Word::Hairpin(Hairpin::Crescendo)),
    ("cresc", Word::Hairpin(Hairpin::Crescendo)),
    ("getting louder", Word::Hairpin(Hairpin::Crescendo)),
    ("zesiluje", Word::Hairpin(Hairpin::Crescendo)),
    ("zesiluji", Word::Hairpin(Hairpin::Crescendo)),
    ("zesilovat", Word::Hairpin(Hairpin::Crescendo)),
    ("diminuendo", Word::Hairpin(Hairpin::Diminuendo)),
    ("dim", Word::Hairpin(Hairpin::Diminuendo)),
    ("decrescendo", Word::Hairpin(Hairpin::Diminuendo)),
    ("decresc", Word::Hairpin(Hairpin::Diminuendo)),
    ("getting softer", Word::Hairpin(Hairpin::Diminuendo)),
    ("fading", Word::Hairpin(Hairpin::Diminuendo)),
    ("fade out", Word::Hairpin(Hairpin::Diminuendo)),
    ("zeslabuje", Word::Hairpin(Hairpin::Diminuendo)),
    ("zeslabuji", Word::Hairpin(Hairpin::Diminuendo)),
    ("zeslabovat", Word::Hairpin(Hairpin::Diminuendo)),
    ("ztisuje", Word::Hairpin(Hairpin::Diminuendo)),
    ("ztisuji", Word::Hairpin(Hairpin::Diminuendo)),
    ("swell", Word::Hairpin(Hairpin::Swell)),
    ("swells", Word::Hairpin(Hairpin::Swell)),
    ("swelling", Word::Hairpin(Hairpin::Swell)),
    ("messa di voce", Word::Hairpin(Hairpin::Swell)),
    // Density.
    ("full", Word::Density(Density::Full)),
    ("fully", Word::Density(Density::Full)),
    ("thick", Word::Density(Density::Full)),
    ("plne", Word::Density(Density::Full)),
    ("hutne", Word::Density(Density::Full)),
    ("thin", Word::Density(Density::Thin)),
    ("thinly", Word::Density(Density::Thin)),
    ("sparse", Word::Density(Density::Thin)),
    ("sparsely", Word::Density(Density::Thin)),
    ("ridce", Word::Density(Density::Thin)),
    ("tence", Word::Density(Density::Thin)),
    // One player or the section.
    ("solo", Word::Size(Size::Solo)),
    ("solova", Word::Size(Size::Solo)),
    ("solove", Word::Size(Size::Solo)),
    ("solovy", Word::Size(Size::Solo)),
    ("solovou", Word::Size(Size::Solo)),
    ("solovych", Word::Size(Size::Solo)),
    ("solovym", Word::Size(Size::Solo)),
    ("solovymi", Word::Size(Size::Solo)),
    ("section", Word::Size(Size::Section)),
    ("sections", Word::Size(Size::Section)),
    ("sekce", Word::Size(Size::Section)),
    ("sekci", Word::Size(Size::Section)),
    ("sekcemi", Word::Size(Size::Section)),
    // Counts. Czech has a form per case: "dva", "dvě", "dvou", "dvěma".
    ("one", Word::Count(1)),
    ("two", Word::Count(2)),
    ("three", Word::Count(3)),
    ("four", Word::Count(4)),
    ("five", Word::Count(5)),
    ("six", Word::Count(6)),
    ("seven", Word::Count(7)),
    ("eight", Word::Count(8)),
    ("nine", Word::Count(9)),
    ("ten", Word::Count(10)),
    ("eleven", Word::Count(11)),
    ("twelve", Word::Count(12)),
    ("jeden", Word::Count(1)),
    ("jedna", Word::Count(1)),
    ("jedno", Word::Count(1)),
    ("jednu", Word::Count(1)),
    ("jednoho", Word::Count(1)),
    ("jednim", Word::Count(1)),
    ("dva", Word::Count(2)),
    ("dve", Word::Count(2)),
    ("dvou", Word::Count(2)),
    ("dvema", Word::Count(2)),
    ("tri", Word::Count(3)),
    ("trech", Word::Count(3)),
    ("tremi", Word::Count(3)),
    ("ctyri", Word::Count(4)),
    ("ctyr", Word::Count(4)),
    ("ctyrech", Word::Count(4)),
    ("ctyrem", Word::Count(4)),
    ("ctyrmi", Word::Count(4)),
    ("pet", Word::Count(5)),
    ("peti", Word::Count(5)),
    ("sest", Word::Count(6)),
    ("sesti", Word::Count(6)),
    ("sedm", Word::Count(7)),
    ("sedmi", Word::Count(7)),
    ("osm", Word::Count(8)),
    ("osmi", Word::Count(8)),
    ("devet", Word::Count(9)),
    ("deviti", Word::Count(9)),
    ("deset", Word::Count(10)),
    ("deseti", Word::Count(10)),
    ("jedenact", Word::Count(11)),
    ("jedenacti", Word::Count(11)),
    ("dvanact", Word::Count(12)),
    ("dvanacti", Word::Count(12)),
    // Which part of a section.
    ("first", Word::Ordinal(1)),
    ("second", Word::Ordinal(2)),
    ("third", Word::Ordinal(3)),
    ("fourth", Word::Ordinal(4)),
    ("1st", Word::Ordinal(1)),
    ("2nd", Word::Ordinal(2)),
    ("3rd", Word::Ordinal(3)),
    ("4th", Word::Ordinal(4)),
    ("prvni", Word::Ordinal(1)),
    ("prvnich", Word::Ordinal(1)),
    ("prvnim", Word::Ordinal(1)),
    ("prvnimi", Word::Ordinal(1)),
    ("druhe", Word::Ordinal(2)),
    ("druhy", Word::Ordinal(2)),
    ("druha", Word::Ordinal(2)),
    ("druhou", Word::Ordinal(2)),
    ("druhych", Word::Ordinal(2)),
    ("druhym", Word::Ordinal(2)),
    ("druhymi", Word::Ordinal(2)),
    ("treti", Word::Ordinal(3)),
    ("tretich", Word::Ordinal(3)),
    ("tretim", Word::Ordinal(3)),
    ("tretimi", Word::Ordinal(3)),
    ("ctvrte", Word::Ordinal(4)),
    ("ctvrty", Word::Ordinal(4)),
    ("ctvrta", Word::Ordinal(4)),
    ("ctvrtou", Word::Ordinal(4)),
    ("ctvrtych", Word::Ordinal(4)),
    ("ctvrtym", Word::Ordinal(4)),
    ("ctvrtymi", Word::Ordinal(4)),
    // Verbs.
    ("play", Verb),
    ("plays", Verb),
    ("playing", Verb),
    ("played", Verb),
    ("sing", Verb),
    ("sings", Verb),
    ("singing", Verb),
    ("take", Verb),
    ("takes", Verb),
    ("carry", Verb),
    ("carries", Verb),
    ("hold", Verb),
    ("holds", Verb),
    ("have", Verb),
    ("has", Verb),
    ("get", Verb),
    ("gets", Verb),
    ("hraje", Verb),
    ("hraji", Verb),
    ("hrajou", Verb),
    ("hrat", Verb),
    ("zahraje", Verb),
    ("zahraji", Verb),
    ("zahrajou", Verb),
    ("zpiva", Verb),
    ("zpivaji", Verb),
    ("zpivat", Verb),
    ("drzi", Verb),
    ("maji", Verb),
    ("vezmou", Verb),
    ("berou", Verb),
    ("nesou", Verb),
    // Joining words. "a" is the Czech "and" and the English article: between two instruments it
    // joins them, anywhere else it parts the request like "and" does, and a part left without
    // instruments joins the one before ("strings play" + "legato melody").
    ("and", Joiner),
    ("a", Joiner),
    ("i", Joiner),
    ("plus", Joiner),
    ("with", With),
    ("s", With),
    ("se", With),
    ("then", Then),
    ("potom", Then),
    ("pak", Then),
    ("pote", Then),
    // Words that carry nothing the grammar reads.
    ("the", Filler),
    ("an", Filler),
    ("to", Filler),
    ("of", Filler),
    ("in", Filler),
    ("on", Filler),
    ("at", Filler),
    ("for", Filler),
    ("by", Filler),
    ("from", Filler),
    ("please", Filler),
    ("should", Filler),
    ("will", Filler),
    ("would", Filler),
    ("can", Filler),
    ("could", Filler),
    ("must", Filler),
    ("let", Filler),
    ("lets", Filler),
    ("make", Filler),
    ("want", Filler),
    ("now", Filler),
    ("also", Filler),
    ("too", Filler),
    ("all", Filler),
    ("both", Filler),
    ("together", Filler),
    ("their", Filler),
    ("it", Filler),
    ("its", Filler),
    ("them", Filler),
    ("they", Filler),
    ("this", Filler),
    ("that", Filler),
    ("very", Filler),
    ("line", Filler),
    ("part", Filler),
    ("parts", Filler),
    ("notes", Filler),
    ("v", Filler),
    ("ve", Filler),
    ("na", Filler),
    ("do", Filler),
    ("od", Filler),
    ("pro", Filler),
    ("z", Filler),
    ("ze", Filler),
    ("k", Filler),
    ("ke", Filler),
    ("o", Filler),
    ("po", Filler),
    ("za", Filler),
    ("ta", Filler),
    ("ty", Filler),
    ("tim", Filler),
    ("jen", Filler),
    ("taky", Filler),
    ("prosim", Filler),
    ("spolu", Filler),
    ("vsechny", Filler),
    ("velmi", Filler),
    ("bude", Filler),
    ("budou", Filler),
    ("linku", Filler),
    ("tony", Filler),
    ("noty", Filler),
];

/// The words of a stretch of bars: "bars 5-12", the Czech "v taktech 5–12".
const BAR_WORDS: &[&str] = &[
    "bar", "bars", "measure", "measures", "takt", "taktu", "takty", "taktech", "taktem", "taktum",
];

/// The words between the first and the last bar of a stretch: "to", the Czech "až".
const RANGE_WORDS: &[&str] = &["to", "through", "thru", "until", "till", "az", "do"];

/// The Czech and German note names a key with "dur" or "moll" takes, with their pitch classes:
/// `b` is B♭ and `h` is B, `fis` F♯ and `es` E♭.
const CZECH_NOTES: &[(&str, i64)] = &[
    ("c", 0),
    ("cis", 1),
    ("ces", 11),
    ("d", 2),
    ("dis", 3),
    ("des", 1),
    ("e", 4),
    ("eis", 5),
    ("es", 3),
    ("f", 5),
    ("fis", 6),
    ("fes", 4),
    ("g", 7),
    ("gis", 8),
    ("ges", 6),
    ("a", 9),
    ("ais", 10),
    ("as", 8),
    ("h", 11),
    ("his", 0),
    ("b", 10),
];

/// A folded word of the request, with where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FoldedWord {
    /// The folded word: `a-z` and `0-9` only.
    pub text: String,
    /// The index of the request's word it was folded from.
    pub raw: usize,
    /// Whether the word is a number written with a trailing full stop, "1.": the Czech and German
    /// way to write an ordinal ("1. housle", the first violins).
    pub dotted: bool,
}

/// A token: a run of folded words that the grammar reads as one item.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Token {
    /// The request's words the token was read from (indices of the words split at white space).
    pub raw: Range<usize>,
    /// The folded words of the token, separated by one space.
    pub text: String,
    /// What the token is.
    pub kind: Kind,
}

/// What a token is.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Kind {
    /// Words that name instruments, a category or a family.
    Mention(Mention),
    /// A phrase of [`PHRASES`] or an articulation id.
    Word(Word),
    /// A number written in digits, or a count word.
    Number(u32),
    /// A stretch of bars, first to last as written (not yet checked to be a range).
    Bars(u32, u32),
    /// A key: "in D minor", the Czech "d moll".
    Key(Key),
    /// A word the grammar does not know.
    Unknown,
}

/// The meaning of `phrase` (folded words) in [`PHRASES`] or as an articulation id with its
/// underscores read as spaces ("col legno").
pub(super) fn lookup(phrase: &str) -> Option<Word> {
    PHRASES
        .iter()
        .find_map(|&(text, word)| (text == phrase).then_some(word))
        .or_else(|| {
            Articulation::ALL
                .into_iter()
                .find(|articulation| is_id_phrase(articulation.id(), phrase))
                .map(|articulation| Word::Articulation(Some(articulation)))
        })
}

/// Whether `phrase` spells the articulation id `id` with spaces for its underscores.
fn is_id_phrase(id: &str, phrase: &str) -> bool {
    id.len() == phrase.len()
        && id
            .bytes()
            .zip(phrase.bytes())
            .all(|(i, p)| i == p || (i == b'_' && p == b' '))
}

/// Reads the folded words of one stretch of a request (no comma, semicolon or "then" inside)
/// into tokens.
///
/// At each word the longest reading wins among an alias that starts there, a phrase of the
/// tables, a key and a stretch of bars; an alias wins a tie. A word nothing reads is
/// [`Kind::Unknown`].
pub(super) fn tokenize(words: &[FoldedWord]) -> Vec<Token> {
    let texts: Vec<&str> = words.iter().map(|word| word.text.as_str()).collect();
    let mut mentions = mentions_from(&texts, 0);
    let mut tokens = Vec::with_capacity(texts.len());
    let mut at = 0;
    while at < texts.len() {
        let mention = mentions
            .iter()
            .position(|(range, _)| range.start == at)
            .map(|index| mentions.swap_remove(index));
        let reading = reading_at(&texts, words, at);
        let (len, kind) = match (mention, reading) {
            (Some((range, mention)), None) => (range.len(), Kind::Mention(mention)),
            (Some((range, mention)), Some((len, _))) if range.len() >= len => {
                (range.len(), Kind::Mention(mention))
            }
            (_, Some(reading)) => reading,
            (None, None) => (1, Kind::Unknown),
        };
        let end = at.saturating_add(len).min(texts.len());
        // A phrase that ends inside an alias leaves the alias's other words unread: read the rest
        // again.
        if mentions
            .iter()
            .any(|(range, _)| range.start < end && range.end > end)
        {
            mentions = mentions_from(&texts, end);
        }
        let span = words.get(at..end).unwrap_or_default();
        tokens.push(Token {
            raw: raw_span(span),
            text: texts.get(at..end).unwrap_or_default().join(" "),
            kind,
        });
        at = end.max(at.saturating_add(1));
    }
    tokens
}

/// The request's words that `words` were folded from.
fn raw_span(words: &[FoldedWord]) -> Range<usize> {
    match (words.first(), words.last()) {
        (Some(first), Some(last)) => first.raw..last.raw.saturating_add(1),
        _ => 0..0,
    }
}

/// The mentions among `texts[start..]`, with their word positions within `texts`.
fn mentions_from(texts: &[&str], start: usize) -> Vec<(Range<usize>, Mention)> {
    let rest = texts.get(start..).unwrap_or_default().join(" ");
    resolve(&rest, &LANGUAGES)
        .into_iter()
        .map(|mention| {
            let words = mention.words();
            (words.start + start..words.end + start, mention)
        })
        .collect()
}

/// The longest reading other than an alias that starts at word `at`: a key, a stretch of bars, a
/// phrase or a number, in that order on a tie.
fn reading_at(texts: &[&str], words: &[FoldedWord], at: usize) -> Option<(usize, Kind)> {
    let candidates = [
        key_at(texts, at).map(|(len, key)| (len, Kind::Key(key))),
        bars_at(texts, at).map(|(len, first, last)| (len, Kind::Bars(first, last))),
        phrase_at(texts, at).map(|(len, word)| (len, word_kind(word))),
        number_at(words, at).map(|kind| (1, kind)),
    ];
    let mut best: Option<(usize, Kind)> = None;
    for candidate in candidates.into_iter().flatten() {
        if best.as_ref().is_none_or(|(len, _)| candidate.0 > *len) {
            best = Some(candidate);
        }
    }
    best
}

/// The token kind of a phrase: a count word is a number like digits.
fn word_kind(word: Word) -> Kind {
    match word {
        Word::Count(count) => Kind::Number(u32::from(count)),
        other => Kind::Word(other),
    }
}

/// The longest phrase of the tables that starts at word `at`, with its length in words.
fn phrase_at(texts: &[&str], at: usize) -> Option<(usize, Word)> {
    (1..=MAX_PHRASE_WORDS).rev().find_map(|len| {
        let phrase = texts.get(at..at.checked_add(len)?)?.join(" ");
        lookup(&phrase).map(|word| (len, word))
    })
}

/// The number at word `at`: digits, or with a trailing full stop an ordinal ("1." is the first).
fn number_at(words: &[FoldedWord], at: usize) -> Option<Kind> {
    let word = words.get(at)?;
    let value = digits(&word.text)?;
    if word.dotted
        && let Ok(ordinal) = u8::try_from(value)
        && ordinal > 0
    {
        return Some(Kind::Word(Word::Ordinal(ordinal)));
    }
    Some(Kind::Number(value))
}

/// The value of a word of ASCII digits; `None` for any other word or a value beyond `u32`.
fn digits(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// A stretch of bars at word `at`: "bars 5 12" (from "5-12"), "bars 5 to 12", "bar 5 to bar 12",
/// "takty 5 až 12" or one bar, "bar 5". Returns its length in words and the first and last bar
/// as written.
fn bars_at(texts: &[&str], at: usize) -> Option<(usize, u32, u32)> {
    let word = |offset: usize| texts.get(at.checked_add(offset)?).copied();
    if !BAR_WORDS.contains(&word(0)?) {
        return None;
    }
    let first = digits(word(1)?)?;
    if let Some(last) = word(2).and_then(digits) {
        return Some((3, first, last));
    }
    if word(2).is_some_and(|w| RANGE_WORDS.contains(&w)) {
        let mut offset = 3;
        if word(offset).is_some_and(|w| BAR_WORDS.contains(&w)) {
            offset += 1;
        }
        if let Some(last) = word(offset).and_then(digits) {
            return Some((offset + 1, first, last));
        }
    }
    Some((2, first, first))
}

/// A key at word `at`: a Czech or German note name followed by "dur" or "moll" ("d moll",
/// "fis dur", "b moll" is B♭ minor), or "in" with an English note name and "major" or "minor"
/// ("in D minor", "in F sharp major", "in Bb major"), or "key of" with an English note name and an
/// optional mode ("key of D"). The English form needs its "in" or "key of", so that "a minor
/// melody" is not a key. Returns its length in words and the key.
fn key_at(texts: &[&str], at: usize) -> Option<(usize, Key)> {
    let word = |offset: usize| texts.get(at.checked_add(offset)?).copied();
    let czech = CZECH_NOTES
        .iter()
        .find_map(|&(name, root)| (Some(name) == word(0)).then_some(root));
    if let Some(root) = czech
        && let Some(scale) = word(1).and_then(czech_mode)
    {
        return Some((2, Key { root, scale }));
    }
    match (word(0)?, word(1)) {
        ("in", _) => {
            let (len, root) = english_note(texts, at.checked_add(1)?)?;
            let scale = word(1 + len).and_then(english_mode)?;
            Some((len + 2, Key { root, scale }))
        }
        ("key", Some("of")) => {
            let (len, root) = english_note(texts, at.checked_add(2)?)?;
            match word(2 + len).and_then(english_mode) {
                Some(scale) => Some((len + 3, Key { root, scale })),
                None => Some((
                    len + 2,
                    Key {
                        root,
                        scale: Scale::Major,
                    },
                )),
            }
        }
        _ => None,
    }
}

/// The scale of a Czech or German mode word.
fn czech_mode(word: &str) -> Option<Scale> {
    match word {
        "dur" => Some(Scale::Major),
        "moll" => Some(Scale::Minor),
        _ => None,
    }
}

/// The scale of an English mode word.
fn english_mode(word: &str) -> Option<Scale> {
    match word {
        "major" => Some(Scale::Major),
        "minor" => Some(Scale::Minor),
        _ => None,
    }
}

/// An English note name at word `at` and its length in words: a letter `a`-`g`, optionally
/// followed by "sharp" or "flat"; a letter with `b` for flat ("bb", "eb"; "B♭" folds to "bb"); or
/// a letter with `is` for sharp ("F♯" and "F#" fold to "fis").
fn english_note(texts: &[&str], at: usize) -> Option<(usize, i64)> {
    let word = texts.get(at)?.as_bytes();
    let natural = |letter: u8| -> Option<i64> {
        match letter {
            b'c' => Some(0),
            b'd' => Some(2),
            b'e' => Some(4),
            b'f' => Some(5),
            b'g' => Some(7),
            b'a' => Some(9),
            b'b' => Some(11),
            _ => None,
        }
    };
    let (&letter, rest) = word.split_first()?;
    let root = natural(letter)?;
    let shifted = |shift: i64| (root + shift).rem_euclid(12);
    match rest {
        [] => match texts.get(at.checked_add(1)?).copied() {
            Some("sharp") => Some((2, shifted(1))),
            Some("flat") => Some((2, shifted(-1))),
            _ => Some((1, root)),
        },
        b"b" => Some((1, shifted(-1))),
        b"is" => Some((1, shifted(1))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use sonara_midi::orchestration::{all_aliases, fold};

    use super::*;

    /// Every phrase is folded, listed once, no longer than [`MAX_PHRASE_WORDS`], and not an
    /// articulation id as well (TST-07).
    #[test]
    fn phrases_are_folded_and_listed_once() {
        for (index, &(phrase, _)) in PHRASES.iter().enumerate() {
            assert_eq!(fold(phrase), phrase, "{phrase:?} is folded");
            assert!(
                phrase.split(' ').count() <= MAX_PHRASE_WORDS,
                "{phrase:?} is too long"
            );
            assert!(
                !PHRASES
                    .iter()
                    .skip(index + 1)
                    .any(|&(other, _)| other == phrase),
                "{phrase:?} is listed twice"
            );
            assert!(
                !Articulation::ALL
                    .iter()
                    .any(|articulation| is_id_phrase(articulation.id(), phrase)),
                "{phrase:?} is an articulation id already"
            );
        }
        for articulation in Articulation::ALL {
            assert!(
                articulation.id().split('_').count() <= MAX_PHRASE_WORDS,
                "{articulation:?}"
            );
        }
    }

    /// No phrase is also an English or Czech alias: an alias wins a tie, so such a phrase could
    /// never be read (TST-07).
    #[test]
    fn no_phrase_is_an_alias() {
        let shared: Vec<String> = PHRASES
            .iter()
            .map(|&(phrase, _)| phrase.to_owned())
            .chain(Articulation::ALL.iter().map(|a| a.id().replace('_', " ")))
            .filter(|phrase| {
                all_aliases()
                    .any(|(_, alias)| LANGUAGES.contains(&alias.lang) && alias.text == phrase)
            })
            .collect();
        assert_eq!(shared, Vec::<String>::new());
    }

    #[test]
    fn czech_and_english_note_names_give_their_pitch_classes() {
        let key = |text: &str| {
            let folded = fold(text);
            let texts: Vec<&str> = folded.split(' ').collect();
            key_at(&texts, 0)
        };
        let cases = [
            ("d moll", 2, Scale::Minor, 2),
            ("fis dur", 6, Scale::Major, 2),
            ("b moll", 10, Scale::Minor, 2),
            ("h moll", 11, Scale::Minor, 2),
            ("es dur", 3, Scale::Major, 2),
            ("in D minor", 2, Scale::Minor, 3),
            ("in B minor", 11, Scale::Minor, 3),
            ("in B♭ major", 10, Scale::Major, 3),
            ("in Eb major", 3, Scale::Major, 3),
            ("in F sharp minor", 6, Scale::Minor, 4),
            ("in A flat major", 8, Scale::Major, 4),
            ("key of G", 7, Scale::Major, 3),
            ("key of c minor", 0, Scale::Minor, 4),
        ];
        for (text, root, scale, len) in cases {
            assert_eq!(key(text), Some((len, Key { root, scale })), "{text:?}");
        }
        for text in ["a minor", "in a hurry", "d", "in h", "key"] {
            assert_eq!(key(text), None, "{text:?}");
        }
    }

    #[test]
    fn bars_read_a_single_bar_or_a_stretch() {
        let bars = |text: &str| {
            let folded = fold(text);
            let texts: Vec<&str> = folded.split(' ').collect();
            bars_at(&texts, 0)
        };
        assert_eq!(bars("bars 5-12"), Some((3, 5, 12)));
        assert_eq!(bars("taktech 5–12"), Some((3, 5, 12)));
        assert_eq!(bars("bar 5 to bar 12"), Some((5, 5, 12)));
        assert_eq!(bars("takty 5 až 12"), Some((4, 5, 12)));
        assert_eq!(bars("bar 3"), Some((2, 3, 3)));
        assert_eq!(bars("bar 3 to"), Some((2, 3, 3)));
        assert_eq!(bars("bars"), None);
        assert_eq!(bars("five bars"), None);
    }
}
