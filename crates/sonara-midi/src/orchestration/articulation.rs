//! The articulation vocabulary: stable identifiers of general musical articulations.
//!
//! An identifier names a musical idea, not a sample patch, key switch or controller: how a library
//! produces it is outside this metadata. The slugs match the spelling in a sample library's
//! `sonara-library.json` manifest (`sustain`, `pizzicato`), so a later mapping is a string match.
//!
//! - **The ordinary sound has no id.** A note without an articulation (`None` where an
//!   `Option<Articulation>` is expected) is the instrument's ordinary sound, ordinario. There is
//!   no `ordinario` id, and `sustain` is not the ordinary sound but the long, held note.
//! - **Mutes and vibrato are passage states.** The mute ids (`con_sordino`, `mute_cup`, …) and the
//!   vibrato ids (`vibrato_on`, `non_vibrato`) stay in the vocabulary, because libraries name them
//!   as strings, but [`Articulation::axis`] marks them as a [`Mute`] or a [`Vibrato`] that holds
//!   over a passage, never as a note's primary articulation.
//! - **Refinements name their parent.** A more specific id (`trill_half`) names the general id it
//!   refines (`trill`) through [`Articulation::parent`], so a library without the refinement can
//!   play the parent instead.
//! - **Rolls.** A mallet instrument's roll is `tremolo`; `roll` is the roll of the timpani, a drum,
//!   a cymbal or other unpitched percussion.
//!
//! [`Articulation::ALL`] is append-only: ids are never removed, renamed or moved.

use thiserror::Error;

use super::id::{is_snake_case, same_text};

/// A general musical articulation.
///
/// [`Articulation::description`] gives each one's definition and [`Articulation::kind`] what it
/// does to a note. The variants are declared in the order of [`Articulation::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Articulation {
    /// Sustain (`sustain`).
    Sustain,
    /// Legato (`legato`).
    Legato,
    /// Staccato (`staccato`).
    Staccato,
    /// Spiccato (`spiccato`).
    Spiccato,
    /// Tenuto (`tenuto`).
    Tenuto,
    /// Marcato (`marcato`).
    Marcato,
    /// Pizzicato (`pizzicato`).
    Pizzicato,
    /// Tremolo, including a mallet instrument's roll (`tremolo`).
    Tremolo,
    /// Trill (`trill`).
    Trill,
    /// Col legno (`col_legno`).
    ColLegno,
    /// Harmonics (`harmonics`).
    Harmonics,
    /// Sul ponticello (`sul_ponticello`).
    SulPonticello,
    /// Con sordino, the mute of no named type (`con_sordino`).
    ConSordino,
    /// Flutter tongue (`flutter_tongue`).
    FlutterTongue,
    /// Roll of the timpani, a drum or other unpitched percussion (`roll`).
    Roll,
    /// Accent (`accent`).
    Accent,
    /// Air tone (`air_tone`).
    AirTone,
    /// Bartók pizzicato (`bartok_pizzicato`).
    BartokPizzicato,
    /// Bisbigliando (`bisbigliando`).
    Bisbigliando,
    /// Choke (`choke`).
    Choke,
    /// Col legno battuto (`col_legno_battuto`).
    ColLegnoBattuto,
    /// Col legno tratto (`col_legno_tratto`).
    ColLegnoTratto,
    /// Crescendo swell (`crescendo_swell`).
    CrescendoSwell,
    /// Cuivré (`cuivre`).
    Cuivre,
    /// Damped (`damped`).
    Damped,
    /// Dead stroke (`dead_stroke`).
    DeadStroke,
    /// Doit (`doit`).
    Doit,
    /// Drag (`drag`).
    Drag,
    /// Fall (`fall`).
    Fall,
    /// Falsetto (`falsetto`).
    Falsetto,
    /// Flam (`flam`).
    Flam,
    /// Flautando (`flautando`).
    Flautando,
    /// Fortepiano (`fortepiano`).
    Fortepiano,
    /// Glissando (`glissando`).
    Glissando,
    /// Growl (`growl`).
    Growl,
    /// Half valve (`half_valve`).
    HalfValve,
    /// Artificial harmonics (`harmonics_artificial`).
    HarmonicsArtificial,
    /// Natural harmonics (`harmonics_natural`).
    HarmonicsNatural,
    /// Humming (`humming`).
    Humming,
    /// Key click (`key_click`).
    KeyClick,
    /// Let ring (`let_ring`).
    LetRing,
    /// Multiphonic (`multiphonic`).
    Multiphonic,
    /// Non vibrato (`non_vibrato`).
    NonVibrato,
    /// Palm mute (`palm_mute`).
    PalmMute,
    /// Portamento (`portamento`).
    Portamento,
    /// Portato (`portato`).
    Portato,
    /// Près de la table (`pres_de_la_table`).
    PresDeLaTable,
    /// Rim (`rim`).
    Rim,
    /// Rimshot (`rimshot`).
    Rimshot,
    /// Rip (`rip`).
    Rip,
    /// Sforzando (`sforzando`).
    Sforzando,
    /// Shake (`shake`).
    Shake,
    /// Slap tongue (`slap_tongue`).
    SlapTongue,
    /// Staccatissimo (`staccatissimo`).
    Staccatissimo,
    /// Stopped (`stopped`).
    Stopped,
    /// Sul tasto (`sul_tasto`).
    SulTasto,
    /// Measured tremolo (`tremolo_measured`).
    TremoloMeasured,
    /// Half-step trill (`trill_half`).
    TrillHalf,
    /// Whole-step trill (`trill_whole`).
    TrillWhole,
    /// Vibrato on (`vibrato_on`).
    VibratoOn,
    /// Vowel (`vowel`).
    Vowel,
    /// Xylophonic (`xylophonic`).
    Xylophonic,
    /// Bells up (`bells_up`).
    BellsUp,
    /// Bowed (`bowed`).
    Bowed,
    /// Multiple tonguing (`multi_tongue`).
    MultiTongue,
    /// Bucket mute (`mute_bucket`).
    MuteBucket,
    /// Cup mute (`mute_cup`).
    MuteCup,
    /// Harmon mute (`mute_harmon`).
    MuteHarmon,
    /// Plunger mute (`mute_plunger`).
    MutePlunger,
    /// Scoop (`scoop`).
    Scoop,
    /// Scrape (`scrape`).
    Scrape,
    /// Shake stroke (`shake_stroke`).
    ShakeStroke,
    /// Snares off (`snares_off`).
    SnaresOff,
    /// Su una corda (`su_una_corda`).
    SuUnaCorda,
    /// Text (`text`).
    Text,
    /// Wah-wah (`wah_wah`).
    WahWah,
    /// Whisper (`whisper`).
    Whisper,
}

impl Articulation {
    /// Every articulation of the vocabulary, in its frozen order.
    ///
    /// The fifteen original ids come first, in their original order. The 47 established technique
    /// terms follow, then the 15 newer terms that at least one sample library records, each group
    /// in alphabetical order. The order is append-only: a choice list built from it is automated by
    /// index, so an id is never removed, renamed or moved, and a new id goes at the end.
    pub const ALL: [Self; 77] = [
        Self::Sustain,
        Self::Legato,
        Self::Staccato,
        Self::Spiccato,
        Self::Tenuto,
        Self::Marcato,
        Self::Pizzicato,
        Self::Tremolo,
        Self::Trill,
        Self::ColLegno,
        Self::Harmonics,
        Self::SulPonticello,
        Self::ConSordino,
        Self::FlutterTongue,
        Self::Roll,
        Self::Accent,
        Self::AirTone,
        Self::BartokPizzicato,
        Self::Bisbigliando,
        Self::Choke,
        Self::ColLegnoBattuto,
        Self::ColLegnoTratto,
        Self::CrescendoSwell,
        Self::Cuivre,
        Self::Damped,
        Self::DeadStroke,
        Self::Doit,
        Self::Drag,
        Self::Fall,
        Self::Falsetto,
        Self::Flam,
        Self::Flautando,
        Self::Fortepiano,
        Self::Glissando,
        Self::Growl,
        Self::HalfValve,
        Self::HarmonicsArtificial,
        Self::HarmonicsNatural,
        Self::Humming,
        Self::KeyClick,
        Self::LetRing,
        Self::Multiphonic,
        Self::NonVibrato,
        Self::PalmMute,
        Self::Portamento,
        Self::Portato,
        Self::PresDeLaTable,
        Self::Rim,
        Self::Rimshot,
        Self::Rip,
        Self::Sforzando,
        Self::Shake,
        Self::SlapTongue,
        Self::Staccatissimo,
        Self::Stopped,
        Self::SulTasto,
        Self::TremoloMeasured,
        Self::TrillHalf,
        Self::TrillWhole,
        Self::VibratoOn,
        Self::Vowel,
        Self::Xylophonic,
        Self::BellsUp,
        Self::Bowed,
        Self::MultiTongue,
        Self::MuteBucket,
        Self::MuteCup,
        Self::MuteHarmon,
        Self::MutePlunger,
        Self::Scoop,
        Self::Scrape,
        Self::ShakeStroke,
        Self::SnaresOff,
        Self::SuUnaCorda,
        Self::Text,
        Self::WahWah,
        Self::Whisper,
    ];

    /// The stable identifier, for example `col_legno`: lower-case `snake_case`, never renamed.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sustain => "sustain",
            Self::Legato => "legato",
            Self::Staccato => "staccato",
            Self::Spiccato => "spiccato",
            Self::Tenuto => "tenuto",
            Self::Marcato => "marcato",
            Self::Pizzicato => "pizzicato",
            Self::Tremolo => "tremolo",
            Self::Trill => "trill",
            Self::ColLegno => "col_legno",
            Self::Harmonics => "harmonics",
            Self::SulPonticello => "sul_ponticello",
            Self::ConSordino => "con_sordino",
            Self::FlutterTongue => "flutter_tongue",
            Self::Roll => "roll",
            Self::Accent => "accent",
            Self::AirTone => "air_tone",
            Self::BartokPizzicato => "bartok_pizzicato",
            Self::Bisbigliando => "bisbigliando",
            Self::Choke => "choke",
            Self::ColLegnoBattuto => "col_legno_battuto",
            Self::ColLegnoTratto => "col_legno_tratto",
            Self::CrescendoSwell => "crescendo_swell",
            Self::Cuivre => "cuivre",
            Self::Damped => "damped",
            Self::DeadStroke => "dead_stroke",
            Self::Doit => "doit",
            Self::Drag => "drag",
            Self::Fall => "fall",
            Self::Falsetto => "falsetto",
            Self::Flam => "flam",
            Self::Flautando => "flautando",
            Self::Fortepiano => "fortepiano",
            Self::Glissando => "glissando",
            Self::Growl => "growl",
            Self::HalfValve => "half_valve",
            Self::HarmonicsArtificial => "harmonics_artificial",
            Self::HarmonicsNatural => "harmonics_natural",
            Self::Humming => "humming",
            Self::KeyClick => "key_click",
            Self::LetRing => "let_ring",
            Self::Multiphonic => "multiphonic",
            Self::NonVibrato => "non_vibrato",
            Self::PalmMute => "palm_mute",
            Self::Portamento => "portamento",
            Self::Portato => "portato",
            Self::PresDeLaTable => "pres_de_la_table",
            Self::Rim => "rim",
            Self::Rimshot => "rimshot",
            Self::Rip => "rip",
            Self::Sforzando => "sforzando",
            Self::Shake => "shake",
            Self::SlapTongue => "slap_tongue",
            Self::Staccatissimo => "staccatissimo",
            Self::Stopped => "stopped",
            Self::SulTasto => "sul_tasto",
            Self::TremoloMeasured => "tremolo_measured",
            Self::TrillHalf => "trill_half",
            Self::TrillWhole => "trill_whole",
            Self::VibratoOn => "vibrato_on",
            Self::Vowel => "vowel",
            Self::Xylophonic => "xylophonic",
            Self::BellsUp => "bells_up",
            Self::Bowed => "bowed",
            Self::MultiTongue => "multi_tongue",
            Self::MuteBucket => "mute_bucket",
            Self::MuteCup => "mute_cup",
            Self::MuteHarmon => "mute_harmon",
            Self::MutePlunger => "mute_plunger",
            Self::Scoop => "scoop",
            Self::Scrape => "scrape",
            Self::ShakeStroke => "shake_stroke",
            Self::SnaresOff => "snares_off",
            Self::SuUnaCorda => "su_una_corda",
            Self::Text => "text",
            Self::WahWah => "wah_wah",
            Self::Whisper => "whisper",
        }
    }

    /// The articulation with identifier `id`, or `None` when it is not in the vocabulary. The match
    /// is exact: a display name (`Pizzicato`), an abbreviation (`pizz`) or another spelling of an
    /// id is not an id.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|articulation| articulation.id() == id)
    }

    /// Whether this is one of the fifteen ids the vocabulary started with, the first fifteen of
    /// [`Articulation::ALL`] (`sustain` to `roll`). Library manifests could declare only these
    /// before the vocabulary grew. None of them refines another id.
    #[must_use]
    pub const fn is_stable(self) -> bool {
        matches!(
            self,
            Self::Sustain
                | Self::Legato
                | Self::Staccato
                | Self::Spiccato
                | Self::Tenuto
                | Self::Marcato
                | Self::Pizzicato
                | Self::Tremolo
                | Self::Trill
                | Self::ColLegno
                | Self::Harmonics
                | Self::SulPonticello
                | Self::ConSordino
                | Self::FlutterTongue
                | Self::Roll
        )
    }

    /// What the articulation does to a note. An id that straddles two kinds has the kind of its
    /// main use.
    #[must_use]
    pub const fn kind(self) -> Kind {
        match self {
            Self::Sustain
            | Self::Tremolo
            | Self::FlutterTongue
            | Self::Roll
            | Self::Bisbigliando
            | Self::TremoloMeasured => Kind::LongNote,
            Self::Staccato
            | Self::Spiccato
            | Self::Tenuto
            | Self::Pizzicato
            | Self::BartokPizzicato
            | Self::Choke
            | Self::DeadStroke
            | Self::Portato
            | Self::Staccatissimo
            | Self::MultiTongue
            | Self::ShakeStroke => Kind::ShortNote,
            Self::Trill
            | Self::Drag
            | Self::Flam
            | Self::Shake
            | Self::TrillHalf
            | Self::TrillWhole => Kind::Ornament,
            Self::Legato
            | Self::Doit
            | Self::Fall
            | Self::Glissando
            | Self::HalfValve
            | Self::Portamento
            | Self::Rip
            | Self::Scoop => Kind::Transition,
            Self::ColLegno
            | Self::Harmonics
            | Self::AirTone
            | Self::ColLegnoBattuto
            | Self::ColLegnoTratto
            | Self::Growl
            | Self::HarmonicsArtificial
            | Self::HarmonicsNatural
            | Self::KeyClick
            | Self::Multiphonic
            | Self::Rim
            | Self::Rimshot
            | Self::SlapTongue
            | Self::Bowed
            | Self::Scrape
            | Self::WahWah => Kind::Effect,
            Self::SulPonticello
            | Self::Cuivre
            | Self::Flautando
            | Self::NonVibrato
            | Self::PresDeLaTable
            | Self::SulTasto
            | Self::VibratoOn
            | Self::BellsUp
            | Self::SnaresOff
            | Self::SuUnaCorda => Kind::TimbreModifier,
            Self::ConSordino
            | Self::PalmMute
            | Self::Stopped
            | Self::Xylophonic
            | Self::MuteBucket
            | Self::MuteCup
            | Self::MuteHarmon
            | Self::MutePlunger => Kind::Mute,
            Self::Marcato
            | Self::Accent
            | Self::CrescendoSwell
            | Self::Fortepiano
            | Self::Sforzando => Kind::DynamicShape,
            Self::Damped | Self::LetRing => Kind::KeyboardControl,
            Self::Falsetto | Self::Humming | Self::Vowel | Self::Text | Self::Whisper => {
                Kind::Vocal
            }
        }
    }

    /// Where the id belongs in a performance: [`Axis::Primary`] for a note's own articulation, or
    /// the passage state the id names. The eight mute ids name the [`Mute`] values other than
    /// [`Mute::Open`] one to one, and `vibrato_on` and `non_vibrato` name [`Vibrato::On`] and
    /// [`Vibrato::Off`]; [`Mute::articulation`] and [`Vibrato::articulation`] map back.
    #[must_use]
    pub const fn axis(self) -> Axis {
        match self {
            Self::ConSordino => Axis::Mute(Mute::Generic),
            Self::MuteCup => Axis::Mute(Mute::Cup),
            Self::MuteHarmon => Axis::Mute(Mute::Harmon),
            Self::MutePlunger => Axis::Mute(Mute::Plunger),
            Self::MuteBucket => Axis::Mute(Mute::Bucket),
            Self::Stopped => Axis::Mute(Mute::Stopped),
            Self::PalmMute => Axis::Mute(Mute::Palm),
            Self::Xylophonic => Axis::Mute(Mute::Xylophonic),
            Self::VibratoOn => Axis::Vibrato(Vibrato::On),
            Self::NonVibrato => Axis::Vibrato(Vibrato::Off),
            _ => Axis::Primary,
        }
    }

    /// The more general articulation this one refines, which a library without this one can play
    /// instead (`trill_half` refines `trill`), or `None` for an id that refines none. A parent is
    /// on the same axis as its child, and no original id has one.
    #[must_use]
    pub const fn parent(self) -> Option<Self> {
        match self {
            Self::TrillHalf | Self::TrillWhole | Self::Shake => Some(Self::Trill),
            Self::HarmonicsNatural | Self::HarmonicsArtificial => Some(Self::Harmonics),
            Self::TremoloMeasured => Some(Self::Tremolo),
            Self::BartokPizzicato => Some(Self::Pizzicato),
            Self::MuteCup
            | Self::MuteHarmon
            | Self::MutePlunger
            | Self::MuteBucket
            | Self::Stopped => Some(Self::ConSordino),
            Self::Xylophonic => Some(Self::PalmMute),
            Self::Staccatissimo => Some(Self::Staccato),
            Self::Flautando => Some(Self::SulTasto),
            Self::Choke => Some(Self::Damped),
            Self::Accent => Some(Self::Marcato),
            Self::ColLegnoBattuto => Some(Self::ColLegno),
            _ => None,
        }
    }

    /// A one-sentence definition, for reference texts and tooltips.
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per articulation of the vocabulary"
    )]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Sustain => {
                "A held note with no special attack, a library's long patch; the ordinary sound is \
                 no articulation at all."
            }
            Self::Legato => {
                "Notes joined without a new attack; libraries play it on one voice, triggered by \
                 overlapping notes."
            }
            Self::Staccato => "Short, detached notes.",
            Self::Spiccato => "A controlled, bounced bow stroke, one stroke per note.",
            Self::Tenuto => "A note held for its full value, with a slight weight.",
            Self::Marcato => "A strongly accented, heavy attack.",
            Self::Pizzicato => "A bowed string plucked with a finger.",
            Self::Tremolo => {
                "Rapid, unmeasured repetition of one note or chord by bow, plectrum or bellows; on \
                 a mallet instrument, its roll."
            }
            Self::Trill => "Rapid alternation with the note above, at an unspecified interval.",
            Self::ColLegno => "A string struck with the wood of the bow.",
            Self::Harmonics => {
                "Natural or artificial harmonics; on a flute or saxophone, overblown partials over \
                 a low fingering."
            }
            Self::SulPonticello => {
                "Bowing, or on a guitar plucking, near the bridge: a thin, glassy tone."
            }
            Self::ConSordino => {
                "Played with a mute of no named type: the straight mute on brass, a covered head \
                 on percussion."
            }
            Self::FlutterTongue => "A wind or brass tone with a rolled or guttural R (frullato).",
            Self::Roll => {
                "A sustained roll of rapid strokes on the timpani, a drum, a cymbal or other \
                 unpitched percussion; on a rattle, a continuous shake."
            }
            Self::Accent => "An accented attack, lighter than marcato or sforzando.",
            Self::AirTone => "Breath noise with little or no pitch.",
            Self::BartokPizzicato => {
                "A string pulled so that it snaps back against the fingerboard."
            }
            Self::Bisbigliando => {
                "A soft, whispering shimmer on one pitch or chord: a harp alternates strings, a \
                 woodwind alternates fingerings."
            }
            Self::Choke => "A ringing cymbal grabbed right after the stroke, cutting it off.",
            Self::ColLegnoBattuto => {
                "The struck col legno, named explicitly as battuto beside the drawn col legno \
                 tratto."
            }
            Self::ColLegnoTratto => "The wood of the bow drawn across the string.",
            Self::CrescendoSwell => {
                "A swell on a held note: a crescendo, or a crescendo and diminuendo."
            }
            Self::Cuivre => "A forced, brassy, cutting tone; on the horn, overblown stopped notes.",
            Self::Damped => {
                "The ring stopped at the note's written end by a damper, a pedal or a hand."
            }
            Self::DeadStroke => "A stroke damped from its attack, so the note has almost no ring.",
            Self::Doit => "An upward slide off the end of a note.",
            Self::Drag => "A stroke preceded by several grace strokes.",
            Self::Fall => "A descending slide off the end of a note.",
            Self::Falsetto => "A male voice in falsetto or head voice.",
            Self::Flam => "A stroke preceded by one grace stroke.",
            Self::Flautando => {
                "A light, fast bow over the fingerboard: a flute-like tone, apart from sul tasto."
            }
            Self::Fortepiano => "A loud attack that turns soft at once (fp).",
            Self::Glissando => {
                "A deliberate slide through a range of pitches, continuous or stepwise."
            }
            Self::Growl => {
                "Humming, singing or a flutter of the throat into the instrument while playing."
            }
            Self::HalfValve => {
                "Valves pressed part way: a choked, squeezed tone, and the valved brass's slide."
            }
            Self::HarmonicsArtificial => {
                "A stopped note with a lightly touched node, sounding a harmonic of the stopped \
                 string."
            }
            Self::HarmonicsNatural => {
                "Partials of an open string, sounded by lightly touching a node."
            }
            Self::Humming => "Sung with the mouth closed.",
            Self::KeyClick => "The key noise or key slaps of a woodwind, with or without blowing.",
            Self::LetRing => {
                "The note rings on past its written end: the pedal held, laissez vibrer."
            }
            Self::Multiphonic => {
                "Two or more pitches at once, by a special fingering or by humming while playing."
            }
            Self::NonVibrato => "A straight tone without vibrato.",
            Self::PalmMute => {
                "A string sounded while a hand rests on it: dry and short, but still pitched."
            }
            Self::Portamento => {
                "A continuous slide in pitch without a new attack, between two notes or as a bend \
                 of one."
            }
            Self::Portato => "Slightly separated, gently weighted notes.",
            Self::PresDeLaTable => {
                "A harp plucked near the soundboard: a metallic, guitar-like tone."
            }
            Self::Rim => "A stick on the rim only (rim click, cross-stick).",
            Self::Rimshot => "Head and rim struck together.",
            Self::Rip => "A fast upward sweep into a loud note.",
            Self::Sforzando => "A sudden, strong accent (sfz).",
            Self::Shake => {
                "A jazz lip shake: a wide, fast oscillation between neighbouring harmonics."
            }
            Self::SlapTongue => {
                "The tongue slapped off the reed or the embouchure for a percussive attack."
            }
            Self::Staccatissimo => "Shorter and sharper than staccato.",
            Self::Stopped => {
                "A hand-stopped horn note: muted and nasal, its pitch raised a semitone."
            }
            Self::SulTasto => {
                "Bowing, or on a guitar plucking, over the fingerboard: a soft, veiled tone."
            }
            Self::TremoloMeasured => {
                "Repetition at a notated, metered subdivision that the players synchronize."
            }
            Self::TrillHalf => "A trill to the semitone above.",
            Self::TrillWhole => "A trill to the whole tone above.",
            Self::VibratoOn => {
                "With vibrato: the player's own, or a mechanism such as a vibraphone's motor."
            }
            Self::Vowel => "A sustained, sung vowel.",
            Self::Xylophonic => {
                "A harp string damped at the soundboard by one hand while the other plucks: dry and \
                 xylophone-like."
            }
            Self::BellsUp => "A horn played with the bell raised: a more direct, brassier sound.",
            Self::Bowed => "A bar, plate, cymbal or tam-tam sounded with a bass bow.",
            Self::MultiTongue => {
                "Fast repeated or running notes articulated by double or triple tonguing."
            }
            Self::MuteBucket => "A bucket mute clipped to the bell: a covered, horn-like sound.",
            Self::MuteCup => "A cup mute: darker and more subdued than the straight mute.",
            Self::MuteHarmon => "A harmon mute, stem in (penetrating) or out (subdued).",
            Self::MutePlunger => "A plunger held still in front of the bell.",
            Self::Scoop => "The note starts below pitch and bends up into it.",
            Self::Scrape => "A beater drawn across the surface of a cymbal or tam-tam.",
            Self::ShakeStroke => {
                "One short shake of a jingle or rattle instrument, as opposed to a sustained roll."
            }
            Self::SnaresOff => "A snare drum played with its snares disengaged (sans timbre).",
            Self::SuUnaCorda => "A passage kept on one named string for its colour (sul G).",
            Self::Text => "Sung words or syllables.",
            Self::WahWah => {
                "A plunger, or a hand over a harmon mute, opened and closed while the note sounds."
            }
            Self::Whisper => "An unvoiced, whispered voice.",
        }
    }
}

/// What an articulation does to a note: the ten kinds of the vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// How a held sound is produced or kept going, repetition included (tremolo, roll).
    LongNote,
    /// The length and separation of detached notes, and short plucked or struck sounds.
    ShortNote,
    /// Extra notes or strokes around the main note.
    Ornament,
    /// Pitch movement into, between or out of notes, legato included.
    Transition,
    /// A special sound production, usually a sample patch of its own.
    Effect,
    /// A colour change that can combine with other articulations (contact point, vibrato, bell).
    TimbreModifier,
    /// A device or a hand damping the sound throughout the note.
    Mute,
    /// An attack accent or a loudness contour within the note.
    DynamicShape,
    /// How the note ends or how keys are struck: the damper, the pedal or the hand.
    KeyboardControl,
    /// A sound production of the voice only.
    Vocal,
}

/// Where an articulation id belongs in a performance ([`Articulation::axis`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    /// A note's own articulation: at most one per note.
    Primary,
    /// A mute, a state that holds over a passage until it changes.
    Mute(Mute),
    /// A vibrato setting, a state that holds over a passage until it changes.
    Vibrato(Vibrato),
}

/// The mute of a passage. [`Mute::Open`], the default, has no articulation id; every other value
/// is named by exactly one ([`Mute::articulation`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mute {
    /// No mute: the open, ordinary sound.
    #[default]
    Open,
    /// A mute of no named type (`con_sordino`): the straight mute on brass.
    Generic,
    /// A cup mute (`mute_cup`).
    Cup,
    /// A harmon mute (`mute_harmon`).
    Harmon,
    /// A plunger held in front of the bell (`mute_plunger`).
    Plunger,
    /// A bucket mute (`mute_bucket`).
    Bucket,
    /// A hand-stopped horn (`stopped`).
    Stopped,
    /// A hand resting on the sounding string (`palm_mute`).
    Palm,
    /// A harp string damped at the soundboard (`xylophonic`).
    Xylophonic,
}

impl Mute {
    /// Every mute state, the default first.
    pub const ALL: [Self; 9] = [
        Self::Open,
        Self::Generic,
        Self::Cup,
        Self::Harmon,
        Self::Plunger,
        Self::Bucket,
        Self::Stopped,
        Self::Palm,
        Self::Xylophonic,
    ];

    /// The articulation id that names this mute, or `None` for [`Mute::Open`], which no id names.
    #[must_use]
    pub const fn articulation(self) -> Option<Articulation> {
        match self {
            Self::Open => None,
            Self::Generic => Some(Articulation::ConSordino),
            Self::Cup => Some(Articulation::MuteCup),
            Self::Harmon => Some(Articulation::MuteHarmon),
            Self::Plunger => Some(Articulation::MutePlunger),
            Self::Bucket => Some(Articulation::MuteBucket),
            Self::Stopped => Some(Articulation::Stopped),
            Self::Palm => Some(Articulation::PalmMute),
            Self::Xylophonic => Some(Articulation::Xylophonic),
        }
    }
}

/// The vibrato of a passage. [`Vibrato::Ordinary`], the default, leaves vibrato to the player or
/// the patch and has no articulation id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Vibrato {
    /// The instrument's ordinary vibrato, as the player or the patch has it.
    #[default]
    Ordinary,
    /// With vibrato (`vibrato_on`).
    On,
    /// Without vibrato (`non_vibrato`).
    Off,
}

impl Vibrato {
    /// Every vibrato state, the default first.
    pub const ALL: [Self; 3] = [Self::Ordinary, Self::On, Self::Off];

    /// The articulation id that names this state, or `None` for [`Vibrato::Ordinary`], which no id
    /// names.
    #[must_use]
    pub const fn articulation(self) -> Option<Articulation> {
        match self {
            Self::Ordinary => None,
            Self::On => Some(Articulation::VibratoOn),
            Self::Off => Some(Articulation::NonVibrato),
        }
    }
}

// Compile-time checks of the vocabulary, so a broken edit does not build (PRI-04):
// - `ALL` holds every variant once, in declaration order, so a variant's position in `ALL` is its
//   discriminant and the append-only order is the declaration order;
// - every id is non-empty lower-case `snake_case`, no two articulations share an id, and every
//   articulation has a description.
const _: () = {
    let all = Articulation::ALL;
    let mut i = 0;
    while i < all.len() {
        let articulation = all[i];
        assert!(
            articulation as usize == i,
            "Articulation::ALL lists every variant once, in declaration order"
        );
        assert!(
            is_snake_case(articulation.id()),
            "an articulation id is non-empty lower-case snake_case"
        );
        assert!(
            !articulation.description().is_empty(),
            "every articulation has a description"
        );
        let mut j = i + 1;
        while j < all.len() {
            assert!(
                !same_text(articulation.id(), all[j].id()),
                "two articulations share an id"
            );
            j += 1;
        }
        i += 1;
    }
};

/// Whether an instrument can play an articulation, as far as is known.
///
/// Only [`ArticulationAvailability::Supported`] means a library can play it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArticulationAvailability {
    /// The library declared it for this instrument.
    Supported,
    /// The instrument plays it, but no library declaration confirms it.
    IdiomaticUnverified,
    /// The instrument does not normally play it and no library declared it.
    NotIdiomatic,
    /// The identifier is not in the vocabulary.
    UnknownArticulation,
}

/// What a passage asks of an instrument: at most one primary articulation per note, played under
/// a mute and a vibrato that hold over the passage.
///
/// A primary of `None` is the instrument's ordinary sound, ordinario. The fields are private so
/// that a request never holds a passage state as its primary: [`Request::new`] rejects a mute id
/// (`con_sordino`, `mute_cup`, …) or a vibrato id (`vibrato_on`, `non_vibrato`) there, because
/// those are the [`Mute`] and [`Vibrato`] of the request (PRI-04). The default request is the
/// ordinary sound, open, with the ordinary vibrato.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Request {
    primary: Option<Articulation>,
    mute: Mute,
    vibrato: Vibrato,
}

impl Request {
    /// A request for `primary` (`None` for the ordinary sound) under `mute` and `vibrato`.
    ///
    /// # Errors
    /// [`RequestError::NotPrimary`] when `primary` is an id whose [`Articulation::axis`] is not
    /// [`Axis::Primary`]: a mute or vibrato id, which a request states as its `mute` or `vibrato`
    /// instead.
    pub const fn new(
        primary: Option<Articulation>,
        mute: Mute,
        vibrato: Vibrato,
    ) -> Result<Self, RequestError> {
        if let Some(articulation) = primary
            && !matches!(articulation.axis(), Axis::Primary)
        {
            return Err(RequestError::NotPrimary(articulation));
        }
        Ok(Self {
            primary,
            mute,
            vibrato,
        })
    }

    /// The note's articulation, or `None` for the ordinary sound. Never a mute or vibrato id.
    #[must_use]
    pub const fn primary(self) -> Option<Articulation> {
        self.primary
    }

    /// The mute the passage is played with.
    #[must_use]
    pub const fn mute(self) -> Mute {
        self.mute
    }

    /// The vibrato the passage is played with.
    #[must_use]
    pub const fn vibrato(self) -> Vibrato {
        self.vibrato
    }
}

/// Why a [`Request`] cannot be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RequestError {
    /// The primary articulation names a passage state, a mute or a vibrato, not a note's own
    /// articulation.
    #[error(
        "articulation `{}` names a passage state, not a note's primary articulation",
        .0.id()
    )]
    NotPrimary(Articulation),
}

/// Whether an instrument can play each part of a [`Request`], as far as is known
/// ([`Instrument::request_availability`](super::Instrument::request_availability)).
///
/// Each field answers for the id of its part as [`ArticulationAvailability`] does for a single
/// id. A part at its default, which has no id (the ordinary sound, [`Mute::Open`],
/// [`Vibrato::Ordinary`]), is [`ArticulationAvailability::Supported`]: playing it needs no
/// library declaration. A request holds only ids of the vocabulary, so no field is
/// [`ArticulationAvailability::UnknownArticulation`]. Whether a library recorded the parts
/// together, such as a muted staccato, is not answered here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestAvailability {
    /// The availability of the primary articulation.
    pub primary: ArticulationAvailability,
    /// The availability of the mute.
    pub mute: ArticulationAvailability,
    /// The availability of the vibrato.
    pub vibrato: ArticulationAvailability,
}

#[cfg(test)]
mod tests {
    use super::Articulation;

    #[test]
    fn ids_are_unique_and_round_trip() {
        for articulation in Articulation::ALL {
            assert_eq!(Articulation::from_id(articulation.id()), Some(articulation));
        }
        let mut ids: Vec<&str> = Articulation::ALL.iter().map(|a| a.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Articulation::ALL.len());
    }

    #[test]
    fn unknown_ids_are_not_guessed() {
        assert_eq!(Articulation::from_id("Pizzicato"), None);
        assert_eq!(Articulation::from_id("pizz"), None);
        assert_eq!(Articulation::from_id(""), None);
        assert_eq!(
            Articulation::from_id("pizzicato"),
            Some(Articulation::Pizzicato)
        );
    }
}
