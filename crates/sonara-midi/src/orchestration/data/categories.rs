//! The categories, and the words that name a category or a family as a whole.
//!
//! A category's members are the records that name it; its record holds the selection rule and
//! the words that mean the whole category ("clarinet", the Czech "klarinety"). A member's own
//! words, such as "bass clarinet", are aliases of its record. A word that is a member's display
//! name stays with that member: the Czech "varhany" is the pipe organ's name, so only the English
//! "organ" and the Italian "organo" name the category.

use super::INSTRUMENTS;
use crate::orchestration::id::{Category, CategoryId, InstrumentType, Selection};
use crate::orchestration::instrument::{Family, Provenance};
use crate::orchestration::names::{Alias, alias_fault};

const CLARINET: Category = Category::new(
    CategoryId::Clarinet,
    Selection::Default(InstrumentType::ClarinetBb),
    Provenance::Estimate {
        reason: "the clarinet of most orchestral parts; the others are named as such (E♭ clarinet, \
            clarinet in A, bass clarinet)",
    },
    aliases![
        En Singular: "clarinet";
        En Plural: "clarinets";
        Cs Singular: "klarinet", "klarinetem";
        Cs Unmarked: "klarinetu";
        Cs Plural: "klarinety", "klarinetum", "klarinetech";
        De Singular: "klarinette";
        De Plural: "klarinetten";
        Fr Singular: "clarinette";
        Fr Plural: "clarinettes";
        It Singular: "clarinetto";
        It Plural: "clarinetti";
    ],
);

const SAXOPHONE: Category = Category::new(
    CategoryId::Saxophone,
    Selection::Default(InstrumentType::SaxophoneAlto),
    Provenance::Estimate {
        reason: "the saxophone orchestral scores use most",
    },
    aliases![
        En Singular: "saxophone", "sax";
        En Plural: "saxophones", "saxes";
        Cs Singular: "saxofon", "saxofonem";
        Cs Unmarked: "saxofonu";
        Cs Plural: "saxofony", "saxofonum", "saxofonech";
        De Singular: "saxophon";
        De Plural: "saxophone";
        Fr Singular: "saxophone";
        Fr Plural: "saxophones";
        It Singular: "sassofono";
        It Plural: "sassofoni";
    ],
);

const WAGNER_TUBA: Category = Category::new(
    CategoryId::WagnerTuba,
    Selection::All,
    Provenance::Estimate {
        reason: "Wagner tubas are written as a group of tenors and basses",
    },
    aliases![
        En Singular: "wagner tuba";
        En Plural: "wagner tubas";
        Cs Singular: "wagnerova tuba", "wagnerove tube", "wagnerovu tubu", "wagnerovou tubou";
        Cs Unmarked: "wagnerovy tuby";
        Cs Plural: "wagnerovych tub", "wagnerovym tubam", "wagnerovych tubach",
            "wagnerovymi tubami";
        De Singular: "wagnertuba";
        De Plural: "wagnertuben";
        Fr Singular: "tuba wagnerien";
        Fr Plural: "tubas wagneriens";
        It Singular: "tuba wagneriana";
        It Plural: "tube wagneriane";
    ],
);

const TRUMPET: Category = Category::new(
    CategoryId::Trumpet,
    Selection::Default(InstrumentType::TrumpetBb),
    Provenance::Estimate {
        reason: "the trumpet of most modern parts and sample libraries",
    },
    aliases![
        En Singular: "trumpet";
        En Plural: "trumpets";
        En Unmarked: "tpt";
        Cs Singular: "trubka", "trubce", "trubku", "trubkou";
        Cs Unmarked: "trubky";
        Cs Plural: "trubek", "trubkam", "trubkach", "trubkami";
        De Singular: "trompete";
        De Plural: "trompeten";
        Fr Singular: "trompette";
        Fr Plural: "trompettes";
        It Singular: "tromba";
        It Plural: "trombe";
    ],
);

const TROMBONE: Category = Category::new(
    CategoryId::Trombone,
    Selection::Default(InstrumentType::TromboneTenor),
    Provenance::Estimate {
        reason: "the ordinary orchestral trombone; the bass trombone is named as such",
    },
    aliases![
        En Singular: "trombone";
        En Plural: "trombones";
        En Unmarked: "tbn";
        Cs Singular: "pozoun", "pozounem", "trombon", "trombonem";
        Cs Unmarked: "pozounu", "trombonu";
        Cs Plural: "pozouny", "pozounum", "pozounech", "trombony", "trombonum", "trombonech";
        De Singular: "posaune";
        De Plural: "posaunen";
        Fr Singular: "trombone";
        Fr Plural: "trombones";
        It Singular: "trombone";
        It Plural: "tromboni";
    ],
);

const CYMBALS: Category = Category::new(
    CategoryId::Cymbals,
    Selection::Default(InstrumentType::ClashCymbals),
    Provenance::Estimate {
        reason: "cymbals in a score are the clash pair unless a stick or a mallet asks for the \
            suspended cymbal",
    },
    aliases![
        En Singular: "cymbal";
        En Plural: "cymbals";
        En Unmarked: "cym";
        Cs Singular: "cinel", "cinelu", "cinelem";
        Fr Singular: "cymbale";
        It Singular: "piatto";
    ],
);

const BELLS: Category = Category::new(
    CategoryId::Bells,
    Selection::Ask,
    Provenance::Estimate {
        reason: "the word names the tubular bells, the glockenspiel (orchestral bells), the \
            crotales or a carillon, and none of them is the usual meaning",
    },
    aliases![
        En Singular: "bell";
        En Plural: "bells";
    ],
);

const PIANO: Category = Category::new(
    CategoryId::Piano,
    Selection::Default(InstrumentType::PianoConcertGrand),
    Provenance::Estimate {
        reason: "the piano of the orchestra and the concert hall; a request for a piano names no \
            size",
    },
    aliases![
        En Singular: "piano", "pianoforte", "grand piano";
        En Plural: "pianos", "grand pianos";
        En Unmarked: "pno";
        Cs Singular: "klavir", "klavirem";
        Cs Unmarked: "klaviru";
        Cs Plural: "klaviry", "klavirum", "klavirech";
        De Singular: "klavier";
        De Plural: "klaviere";
        De Unmarked: "flugel";
        Fr Singular: "piano";
        Fr Plural: "pianos";
        It Singular: "pianoforte";
        It Plural: "pianoforti";
    ],
);

const ELECTRIC_PIANO: Category = Category::new(
    CategoryId::ElectricPiano,
    Selection::Default(InstrumentType::ElectricPianoRhodes),
    Provenance::Estimate {
        reason: "the electric piano most recordings and parts mean",
    },
    aliases![
        En Singular: "electric piano", "e piano";
        En Plural: "electric pianos";
        Cs Singular: "elektricke piano", "elektrickeho piana", "elektrickemu pianu",
            "elektrickem pianu", "elektrickym pianem";
        Cs Plural: "elektricka piana", "elektrickych pian", "elektrickym pianum",
            "elektrickych pianech", "elektrickymi piany";
        De Singular: "e piano";
        De Plural: "e pianos";
        Fr Singular: "piano electrique";
        Fr Plural: "pianos electriques";
        It Singular: "pianoforte elettrico", "piano elettrico";
        It Plural: "pianoforti elettrici";
    ],
);

const ORGAN: Category = Category::new(
    CategoryId::Organ,
    Selection::Default(InstrumentType::OrganPipe),
    Provenance::Estimate {
        reason: "the organ of orchestral and choral scores",
    },
    aliases![
        En Singular: "organ";
        En Plural: "organs";
        En Unmarked: "org";
        It Singular: "organo";
        It Plural: "organi";
    ],
);

const SOLO_VOICE: Category = Category::new(
    CategoryId::SoloVoice,
    Selection::Ask,
    Provenance::Estimate {
        reason: "no voice type is the usual one",
    },
    aliases![
        En Singular: "solo voice", "singer", "vocalist";
        En Plural: "solo voices", "singers", "vocalists";
        Cs Singular: "solovy hlas", "soloveho hlasu", "solovemu hlasu", "solovem hlasu",
            "solovym hlasem", "zpevak", "zpevaka", "zpevakovi", "zpevakem", "zpevacka",
            "zpevacce", "zpevacku", "zpevackou";
        Cs Plural: "solove hlasy", "solovych hlasu", "solovym hlasum", "solovych hlasech",
            "solovymi hlasy", "zpevaci", "zpevaku", "zpevakum", "zpevaky", "zpevacich",
            "zpevacek", "zpevackam", "zpevackach", "zpevackami";
        Cs Unmarked: "zpevacky";
        De Singular: "solostimme", "sangerin";
        De Unmarked: "sanger";
        De Plural: "solostimmen", "sangerinnen";
        Fr Singular: "voix soliste", "chanteur", "chanteuse";
        Fr Plural: "chanteurs", "chanteuses";
        It Singular: "voce solista", "cantante";
        It Plural: "voci soliste", "cantanti";
    ],
);

// Aliases: "chorus" is left out, since in a song it is the refrain.
const CHOIR: Category = Category::new(
    CategoryId::Choir,
    Selection::All,
    Provenance::Estimate {
        reason: "a choir sings in four sections: sopranos, altos, tenors and basses",
    },
    aliases![
        En Singular: "choir";
        En Plural: "choirs";
        Cs Singular: "sbor", "sborem";
        Cs Unmarked: "sboru";
        Cs Plural: "sbory", "sborum", "sborech";
        De Singular: "chor";
        De Plural: "chore";
        Fr Singular: "choeur";
        Fr Plural: "choeurs";
        It Singular: "coro";
        It Plural: "cori";
    ],
);

/// Every category's record, at its id's place in [`CategoryId::ALL`].
static CATEGORIES: [Category; CategoryId::ALL.len()] = [
    CLARINET,
    SAXOPHONE,
    WAGNER_TUBA,
    TRUMPET,
    TROMBONE,
    CYMBALS,
    BELLS,
    PIANO,
    ELECTRIC_PIANO,
    ORGAN,
    SOLO_VOICE,
    CHOIR,
];

// Compile-time checks of the categories (PRI-04):
// - each record is at its id's place, so a lookup by place returns the right one;
// - its aliases are folded, have three letters or more and are listed once per language;
// - its provenance gives a reason;
// - a category has two members or more, and a default is one of them.
const _: () = {
    let mut i = 0;
    while i < CATEGORIES.len() {
        let category = &CATEGORIES[i];
        assert!(
            category.id() as usize == i,
            "CATEGORIES holds each category at its id's place"
        );
        assert!(
            alias_fault(category.aliases()).is_none(),
            "a category's aliases are folded, long enough and listed once"
        );
        let Provenance::Estimate { reason } = category.provenance() else {
            panic!("a category's selection rule is an estimate with a reason");
        };
        assert!(
            !reason.trim_ascii().is_empty(),
            "a category's selection rule gives its reason"
        );
        let mut members = 0;
        let mut j = 0;
        while j < INSTRUMENTS.len() {
            if let Some(id) = INSTRUMENTS[j].category()
                && id as usize == i
            {
                members += 1;
            }
            j += 1;
        }
        assert!(members >= 2, "a category has two members or more");
        if let Selection::Default(member) = category.selection() {
            let Some(id) = INSTRUMENTS[member as usize].category() else {
                panic!("a category's default is one of its members");
            };
            assert!(
                id as usize == i,
                "a category's default is one of its members"
            );
        }
        i += 1;
    }
};

/// The record of `id`.
pub(in crate::orchestration) fn category(id: CategoryId) -> &'static Category {
    // The check above places each category at its id's index, so the index is in bounds.
    &CATEGORIES[id as usize]
}

// No family is named by "voices" or its translations, which also name the parts of a chord.
const STRINGS_FAMILY: &[Alias] = aliases![
    En Plural: "strings", "string section", "string instruments";
    Cs Plural: "smycce", "smyccu", "smyccum", "smyccich", "smycci", "smyccove nastroje",
        "smyccovych nastroju", "smyccovym nastrojum", "smyccovych nastrojich",
        "smyccovymi nastroji";
    De Unmarked: "streicher";
    De Plural: "streichinstrumente";
    Fr Plural: "cordes";
    It Plural: "archi";
];

const WOODWINDS_FAMILY: &[Alias] = aliases![
    En Plural: "woodwinds", "woodwind section", "woodwind instruments";
    En Unmarked: "woodwind";
    Cs Plural: "dreva", "drev", "drevum", "drevech", "drevy", "drevene dechove nastroje",
        "drevenych dechovych nastroju", "drevenym dechovym nastrojum",
        "drevenych dechovych nastrojich", "drevenymi dechovymi nastroji";
    De Unmarked: "holzblaser";
    De Plural: "holzblasinstrumente";
    Fr Plural: "bois";
    It Plural: "legni";
];

const BRASS_FAMILY: &[Alias] = aliases![
    En Unmarked: "brass";
    En Plural: "brass section", "brass instruments";
    Cs Plural: "zeste", "zestu", "zestove nastroje", "zestovych nastroju",
        "zestovym nastrojum", "zestovych nastrojich", "zestovymi nastroji";
    De Unmarked: "blechblaser";
    De Plural: "blechblasinstrumente";
    Fr Plural: "cuivres";
    It Plural: "ottoni";
];

const PERCUSSION_FAMILY: &[Alias] = aliases![
    En Unmarked: "percussion";
    En Plural: "percussion section", "percussion instruments";
    Cs Plural: "bici", "bicich", "bicim", "bicimi", "bici nastroje", "bicich nastroju",
        "bicim nastrojum", "bicich nastrojich", "bicimi nastroji";
    De Singular: "schlagzeug", "schlagwerk";
    De Plural: "schlaginstrumente";
    Fr Plural: "percussions";
    Fr Unmarked: "percussion";
    It Plural: "percussioni";
];

const KEYBOARDS_FAMILY: &[Alias] = aliases![
    En Plural: "keyboards", "keyboard instruments";
    Cs Plural: "klavesy", "klaves", "klavesam", "klavesach", "klavesami",
        "klavesove nastroje", "klavesovych nastroju", "klavesovym nastrojum",
        "klavesovych nastrojich", "klavesovymi nastroji";
    De Plural: "tasteninstrumente";
    Fr Plural: "claviers", "instruments a clavier";
    It Plural: "tastiere", "strumenti a tastiera";
];

const PLUCKED_FAMILY: &[Alias] = aliases![
    En Plural: "plucked strings", "plucked instruments";
    Cs Plural: "drnkaci nastroje", "drnkacich nastroju", "drnkacim nastrojum",
        "drnkacich nastrojich", "drnkacimi nastroji";
    De Plural: "zupfinstrumente";
    Fr Plural: "cordes pincees";
    It Plural: "corde pizzicate", "strumenti a pizzico";
];

const CHOIR_FAMILY: &[Alias] = aliases![
    En Plural: "vocals";
    Cs Singular: "zpev", "zpevu", "zpevem";
    De Singular: "gesang";
    Fr Singular: "chant";
    It Singular: "canto";
];

/// The words that name each family as a whole, in the order of [`Family`]'s variants. `Other`
/// has none.
pub(in crate::orchestration) static FAMILY_ALIASES: [(Family, &[Alias]); 7] = [
    (Family::Strings, STRINGS_FAMILY),
    (Family::Woodwinds, WOODWINDS_FAMILY),
    (Family::Brass, BRASS_FAMILY),
    (Family::Percussion, PERCUSSION_FAMILY),
    (Family::Keyboards, KEYBOARDS_FAMILY),
    (Family::Plucked, PLUCKED_FAMILY),
    (Family::Choir, CHOIR_FAMILY),
];

/// The words that name `family` as a whole.
pub(in crate::orchestration) const fn family_aliases(family: Family) -> &'static [Alias] {
    match family {
        Family::Strings => STRINGS_FAMILY,
        Family::Woodwinds => WOODWINDS_FAMILY,
        Family::Brass => BRASS_FAMILY,
        Family::Percussion => PERCUSSION_FAMILY,
        Family::Keyboards => KEYBOARDS_FAMILY,
        Family::Plucked => PLUCKED_FAMILY,
        Family::Choir => CHOIR_FAMILY,
        Family::Other => &[],
    }
}

// Compile-time check: every family's aliases are folded, long enough and listed once (PRI-04).
const _: () = {
    let mut i = 0;
    while i < FAMILY_ALIASES.len() {
        assert!(
            alias_fault(FAMILY_ALIASES[i].1).is_none(),
            "a family's aliases are folded, long enough and listed once"
        );
        i += 1;
    }
};
