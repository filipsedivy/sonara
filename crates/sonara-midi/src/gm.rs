//! The General MIDI percussion map (channel 10) used by the drum kit and the generators
//! (`sonara.midi.gm`).

/// GM percussion notes and the sound each plays. Several notes share a sound name.
pub const GM_MAP: [(u8, &str); 34] = [
    (35, "kick"),
    (36, "kick"),
    (37, "rim"),
    (38, "snare"),
    (39, "clap"),
    (40, "snare"),
    (41, "tom_floor_low"),
    (42, "hat_closed"),
    (43, "tom_floor_high"),
    (44, "hat_pedal"),
    (45, "tom_low"),
    (46, "hat_open"),
    (47, "tom_mid_low"),
    (48, "tom_mid_high"),
    (49, "crash"),
    (50, "tom_high"),
    (51, "ride"),
    (52, "china"),
    (53, "ride_bell"),
    (54, "tambourine"),
    (55, "splash"),
    (56, "cowbell"),
    (57, "crash"),
    (59, "ride"),
    (60, "bongo_high"),
    (61, "bongo_low"),
    (62, "conga_mute"),
    (63, "conga_high"),
    (64, "conga_low"),
    (69, "shaker"),
    (70, "shaker"),
    (75, "claves"),
    (76, "block_high"),
    (77, "block_low"),
];

/// The GM note of a sound for the drum kit and the generators (`DRUM_NAMES`).
pub const DRUM_NAMES: [(&str, i64); 20] = [
    ("kick", KICK),
    ("snare", SNARE),
    ("clap", CLAP),
    ("rim", RIM),
    ("hat_closed", HAT_CLOSED),
    ("hat_pedal", HAT_PEDAL),
    ("hat_open", HAT_OPEN),
    ("crash", CRASH),
    ("ride", RIDE),
    ("ride_bell", 53),
    ("tom_low", TOM_LOW),
    ("tom_mid", TOM_MID),
    ("tom_high", TOM_HIGH),
    ("tom_floor", TOM_FLOOR),
    ("tambourine", 54),
    ("cowbell", 56),
    ("shaker", 70),
    ("claves", 75),
    ("conga", 63),
    ("bongo", 60),
];

/// GM note of the kick drum.
pub const KICK: i64 = 36;
/// GM note of the snare drum.
pub const SNARE: i64 = 38;
/// GM note of the hand clap.
pub const CLAP: i64 = 39;
/// GM note of the rim shot.
pub const RIM: i64 = 37;
/// GM note of the closed hi-hat.
pub const HAT_CLOSED: i64 = 42;
/// GM note of the pedal hi-hat.
pub const HAT_PEDAL: i64 = 44;
/// GM note of the open hi-hat.
pub const HAT_OPEN: i64 = 46;
/// GM note of the crash cymbal.
pub const CRASH: i64 = 49;
/// GM note of the ride cymbal.
pub const RIDE: i64 = 51;
/// GM note of the low tom.
pub const TOM_LOW: i64 = 45;
/// GM note of the mid tom.
pub const TOM_MID: i64 = 47;
/// GM note of the high tom.
pub const TOM_HIGH: i64 = 50;
/// GM note of the floor tom.
pub const TOM_FLOOR: i64 = 41;

/// The GM note of the sound `name` (`DRUM_NAMES[name]`), or `None` for an unknown name.
#[must_use]
pub fn drum_note(name: &str) -> Option<i64> {
    DRUM_NAMES
        .iter()
        .find(|(sound, _)| *sound == name)
        .map(|&(_, note)| note)
}

/// The sound a GM percussion `note` plays (`GM_MAP[note]`), or `None` when the note is unmapped.
#[must_use]
pub fn drum_sound(note: u8) -> Option<&'static str> {
    GM_MAP
        .iter()
        .find(|(mapped, _)| *mapped == note)
        .map(|&(_, sound)| sound)
}

#[cfg(test)]
mod tests {
    use super::{CRASH, GM_MAP, KICK, SNARE, TOM_FLOOR, drum_note, drum_sound};

    #[test]
    fn names_resolve_to_the_generators_constants() {
        assert_eq!(drum_note("kick"), Some(KICK));
        assert_eq!(drum_note("snare"), Some(SNARE));
        assert_eq!(drum_note("crash"), Some(CRASH));
        assert_eq!(drum_note("tom_floor"), Some(TOM_FLOOR));
        assert_eq!(drum_note("cowbell"), Some(56));
        assert_eq!(drum_note("vuvuzela"), None);
    }

    #[test]
    fn notes_resolve_to_sounds() {
        assert_eq!(drum_sound(35), Some("kick"));
        assert_eq!(drum_sound(42), Some("hat_closed"));
        assert_eq!(drum_sound(58), None);
        let mut notes: Vec<u8> = GM_MAP.iter().map(|&(note, _)| note).collect();
        notes.dedup();
        assert_eq!(notes.len(), GM_MAP.len(), "notes are unique and ascending");
    }
}
