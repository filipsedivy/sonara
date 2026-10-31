//! Real-time: runs on the audio render thread (see docs/audio-engine.md).
//!
//! `sonara.sampler`: a polyphonic sample player for the instruments of an SFZ patch
//! ([`SampleBank`]). It is Sonara's own engine (ADR 0010): there is no prototype to match, so
//! the golden data does not cover it; `tests/sampler.rs` pins its behaviour.
//!
//! - **Layers and articulations.** A key press starts every region whose key range, velocity
//!   range, keyswitch, random range and round-robin position match. A press of one of the
//!   patch's keyswitch keys makes it the active switch; the patch's default switch is active
//!   until then, also when the patch has no keyswitch keys. A keyswitch event
//!   ([`EventKind::KeySwitch`]) of a keyswitch key makes it the active switch too, without
//!   starting the regions mapped to that key or holding it down. All notes off makes the
//!   default switch active again: the engine sends it when playback jumps (start, seek, loop
//!   wrap), followed by a keyswitch event for the last keyswitch note before the new position,
//!   so the articulation there is the one that playing up to it selects.
//! - **The `articulation` parameter.** `auto`, its default, forces nothing: the patch's default
//!   switch and the keyswitches of the clips decide, as above. Any other choice applies its
//!   target in the preset's mapping ([`SamplerMapping::target`]) to the notes that start after
//!   it; sounding notes ring on. A keyswitch target makes its key the active switch without
//!   sounding it, also when the patch lost its keyswitch keys, since a region compares its
//!   switch with the key itself. A velocity zone target moves each new note's velocity into
//!   the zone, `lo + (velocity - 1) · (hi - lo) / 126`, so some dynamic range remains; the
//!   moved velocity picks the layer, sets the gain and selects the note's release regions. A
//!   controller target sets the controller's value on every channel for what the regions read
//!   (gates, crossfades and level changes), without starting the regions an event of the
//!   controller starts. A choice without a target, such as `ordinario` on a preset about which
//!   nothing is claimed, plays as `auto`.
//!
//!   The target is applied when the choice changes, and again on a reset and at all notes off,
//!   so a seek or a loop wrap keeps it. A keyswitch or an event of the target's controller that
//!   comes from a clip after it wins until the target is applied again, so the engine's chase,
//!   which sends the clip's last keyswitch and controller values after its all notes off,
//!   restores what the clips selected before the new position. Back at `auto`, the clips' last
//!   keyswitch and controller values apply again.
//!
//!   An automated change inside a block arrives as a choice event
//!   ([`EventKind::ParamChoice`]) at its own frame, rather than with the parameters at the
//!   next block start: the notes from that frame on play it, a note that starts on that frame
//!   included, since the engine sends the choice before the frame's note-ons. It changes the
//!   choice exactly as a new parameter set does, so a choice event of the choice already in
//!   force applies nothing again and leaves what the clips selected since alone; and it becomes
//!   the stored choice, so the parameters of a later block start (after a seek, say) apply
//!   their choice whenever it differs from the one the event left. The same holds for the
//!   `mute` and `vibrato` parameters (below); a choice event of another parameter changes
//!   nothing.
//! - **The `mute` and `vibrato` parameters** state the mute and the vibrato of a passage. The
//!   target the articulation choice plays is looked up under their choices
//!   ([`SamplerMapping::target_under`]): the preset's mapping holds, for every combination, the
//!   recording the library made with that mute or vibrato, or the nearest it has, so a state
//!   the preset does not record plays as `auto`. A change of the articulation choice applies
//!   its target as above; a change of the mute or the vibrato applies the target it gives the
//!   articulation choice only when that differs from the target in force, so a state that
//!   changes nothing leaves what the clips selected since alone. At `auto`, both force nothing
//!   of their own, and the sampler plays exactly as without them.
//! - **A note's own articulation** ([`InstrumentEvent::articulation`]) takes the place of the
//!   parameter's choice for that note alone, at the note's own frame, under the `mute` and
//!   `vibrato` choices in force: the note plays its choice's target as the parameter would play
//!   it (its keyswitch, its velocity zone, its controller value; a target without one plays the
//!   clips' last keyswitch and the controllers as they are). It changes nothing that later notes play: the parameter's target and the
//!   clips' last keyswitch stay as they were, so the next note without an articulation of its
//!   own plays them again. The note's release regions play with the note's own keyswitch, or
//!   the clips' last one at the release when its target forces none, and the controller values
//!   its target forces; its voices read those values for as long as they sound, also when a
//!   clip's event of the controller comes after them. A note whose choice has no target in the
//!   preset's mapping, such as any choice of a preset about which nothing is claimed, plays the
//!   parameter's choice. The engine's chase re-sends a note that sounds at the new position
//!   with its articulation, so a seek into it plays it as before.
//! - **Multi-patch sets.** A preset may be a set of patches of one instrument, which one bank
//!   holds together ([`SampleBank::member_keyswitches`]; each region knows its patch,
//!   [`Region::member`]). A key press plays the regions of one patch only: the patch its
//!   articulation target names ([`ArticulationTarget::member`]), the parameter's or the note's
//!   own, else, at `auto` and for a choice without a target, the patch of the preset's
//!   ordinary sound ([`SamplerMapping::default_member`]). So a change of the articulation
//!   switches patches for the notes that start after it, a lookup without allocation, and
//!   sounding notes ring on in theirs, as with a keyswitch. A note's release regions are those
//!   of the patch the note played. Each patch keeps its own keyswitches and its own last
//!   keyswitch: a clip's keyswitch selects the switch in every patch whose keyswitch key it
//!   is, a target's keyswitch applies in the patch the target names, and all notes off brings
//!   every patch back to its default switch. The controllers, the pedal, the round robin and
//!   the voices are the device's, shared by its patches. A bank of one patch is patch 0 and
//!   plays as described above.
//! - **Key offset.** The preset's key offset moves every note and keyswitch event of the clips
//!   onto the patch's keys before anything else reads it, for a patch keyed away from the pitch
//!   it sounds ([`SamplerMapping::patch_key`]): a note moved onto a keyswitch key selects that
//!   switch, and the engine's chase finds the keyswitch notes the same way
//!   ([`SamplerSetup::switch_notes`]). A note the offset moves out of the MIDI keys 0 to 127 is
//!   dropped, with its release. An articulation's keyswitch target names the patch's key and is
//!   not moved.
//! - **The `dynamics` parameter** (0 to 1, default 1) acts as the preset's mapping says
//!   ([`SamplerMapping::dynamics`]):
//!   - **Velocity** (the default): a key press at velocity `v` plays at
//!     `max(1, round(v · dynamics))`, which picks the layer and sets the gain. A sounding voice
//!     then follows later changes as a gain, `(dynamics / dynamics at its start)²`, both taken
//!     as at least 1/127, at most 4 (+12 dB): a swell on a held chord changes its level, and its
//!     timbre changes from the next note on. At 1 every note plays exactly as without the
//!     parameter.
//!   - **Fixed velocity**: every key press plays at the preset's velocity in place of its own,
//!     scaled and followed as with velocity, for patches whose layers should not follow the
//!     note's velocity.
//!   - **Controller**: the parameter sets the controller to `round(dynamics · 127)` on every
//!     channel, as a clip's event of it would (for volume, 7, and expression, 11, the level they
//!     set too, but no region starts and the sustain pedal does not move), so the patch's
//!     controller crossfades and gates play the dynamics; velocities and levels stay as they are.
//!     It writes the value when the parameter changes, on a reset and at all notes off, and a
//!     clip's event of the controller wins after it, as the chase's restored value does after a
//!     jump. The default 1 sets 127, the loudest layer.
//!
//!   The dynamics act before an articulation's velocity zone, so a softer note stays in the
//!   zone. A note's release regions play at the velocity it was pressed with and follow the
//!   parameter from the dynamics it was pressed with. A voice that a controller starts follows
//!   the parameter from its start. A change reaches sounding voices at the next block start,
//!   ramped linearly over the block with their controller gains.
//! - **Legato triggers.** A `trigger=first` region starts only on a press while no other key is
//!   down, and a `trigger=legato` region only on a press while one is, as the SFZ format and the
//!   reference player sfizz read them ([`Trigger::starts_on_press`]). Sonatina's legato patches
//!   pair them with a timed exclusive-group fade, so a phrase starts with the attack sample and
//!   each joined note overlaps the one before it. Unlike sfizz, which counts every key down, a
//!   held keyswitch key does not count, so an articulation chosen with a held switch still
//!   starts its phrase with a `first` note. A key released under the sustain pedal is not down,
//!   and a key pressed again while it is still down counts itself, so the second press is
//!   joined as in sfizz. All notes off releases every key, so the next press is a `first` note.
//! - **Overlap legato.** A preset whose library says it plays legato by overlapping its notes
//!   ([`SamplerMapping::legato`]) joins the notes that play its legato target
//!   ([`ArticulationTarget::legato`], the `legato` choice), as the parameter's choice or as a
//!   note's own articulation. Such a note, pressed while the note pressed before it is still
//!   held or at most the preset's gap after its release, crossfades from it over the preset's
//!   crossfade time (at least 5 ms): the old note fades out from where it is, a release under
//!   way included, and the new one fades in from the preset's start offset into its sample,
//!   past the sample's attack, in place of the region's attack. The new note's articulation
//!   decides, as with a library's legato patch: a note that plays another one joins nothing,
//!   and a legato note after it joins it all the same.
//!
//!   Both fades follow an equal-power curve, the level heard being `sin(level · π/2)`, so two
//!   uncorrelated sounds keep their power mid-way instead of dipping by 3 dB; a voice still in
//!   its attack starts its fade where it is heard, and a joined note at full level plays and
//!   releases as any note. A start offset at or past the loop end of a looping region starts
//!   at the loop start, and one at or past the end of a region that does not loop is not
//!   taken, so the note still sounds.
//!
//!   The legato is monophonic: a chord played legato keeps its last note. A press of the last
//!   note's own key plays its attack beside it, as without the legato; so does a press with no
//!   note held or just released, and the first press after all notes off, so a seek or a loop
//!   wrap starts the phrase with an attack. A note left this way starts no release regions at
//!   its release, and neither an exclusive group nor the sustain pedal changes its fade; a fade
//!   that an earlier transition started is shortened to 10 ms, so a run faster than the
//!   crossfade keeps at most two notes sounding beyond those short tails. Keyswitch keys play
//!   no part in it. It is an emulation from sustained samples, not recorded transitions.
//! - **Velocity.** A region's gain follows the note's velocity as its `amp_veltrack` says: the
//!   curve `(velocity / 127)²` at the default 100 %, constant at 0 %, inverted below 0. Its
//!   velocity crossfade (`xfin_*vel`, `xfout_*vel`, `xf_velcurve`) scales it too, so dynamic
//!   layers that overlap in velocity blend instead of both sounding at full gain
//!   ([`Region::velocity_gain`]). The gain is set when the voice starts and costs nothing per
//!   sample. A region that a note would start at zero gain, such as a layer outside its
//!   crossfade, starts no voice.
//! - **Round robin.** `seq_length`/`seq_position` follow one counter that advances on each
//!   key press that matched a round-robin region. `lorand`/`hirand` use a fixed-seed generator,
//!   so a render is deterministic (TST-04).
//! - **Release.** Key release fades the voice out over the region's release time, and starts
//!   the `trigger=release` and `trigger=release_key` regions that match the released key and
//!   its original velocity. Those voices are the note's tail: they play on, and a later release
//!   of the same key does not fade them.
//! - **Sustain pedal.** A key released while the pedal (CC 64) is down keeps sounding until the
//!   pedal lifts. Its `trigger=release` regions start then, chosen as at the release: by the
//!   velocity the key was pressed with and the keyswitch active when it was released. A key
//!   released twice under one pedal starts them once, for the later release. Its
//!   `trigger=release_key` regions ignore the pedal (SFZ: "ignores sustain pedal") and start at
//!   the release itself, as Sonatina's timpani roll endings need.
//! - **Exclusive groups.** A key press that starts a region of `group` switches off the voices
//!   whose region names that group in `off_by`. They fade out as their region's `off_mode` says
//!   ([`OffMode`]): quickly (10 ms) by default, over the region's release (`normal`), or over
//!   its `off_time` (`time`), so a legato patch's old note overlaps the new one. A choke only
//!   ever shortens a fade already under way, and the sustain pedal no longer holds a voice it
//!   has switched off.
//! - **Playback.** Linear interpolation, constant memory: 64 preallocated voices, the oldest
//!   one is stolen when all sound (a releasing one first).
//! - **Controls.** Sustain pedal (CC 64), volume (CC 7), expression (CC 11), pitch bend
//!   (±2 semitones), keyswitch events and all notes off.
//! - **Controller table.** The sampler keeps the last value of every controller on each of the
//!   16 MIDI channels, preallocated, starting where the patch sets them
//!   ([`SampleBank::controllers`]) and back there on a reset. All notes off lifts the sustain
//!   pedal, so it sets controller 64 to 0 on every channel; the other values stay. After it,
//!   the engine's chase re-sends the last modulation (1), expression (11) and sustain (64)
//!   values before the new position as ordinary controller events, so a region that such an
//!   event starts (a pedal noise) starts again there. Sustain, volume and expression still act
//!   on every channel, and start up and full whatever the patch sets.
//! - **Controller gates.** A region with `loccN`/`hiccN` starts only while the controllers of
//!   the note's channel lie in its ranges ([`Region::gates_open`]), read when the region would
//!   start: at the key press, at the key release (for a release region under the pedal, at the
//!   pedal's lift) or at the controller event. A voice that sounds keeps sounding when a
//!   controller leaves the range, as in the reference player sfizz.
//! - **Controller triggers.** A region with `on_loccN`/`on_hiccN` starts on every controller
//!   event of the range whose gates and keyswitch let it, as the SFZ format reads them:
//!   Salamander's pedal noises start when the pedal goes down and up. It plays at its
//!   `pitch_keycenter`, without a velocity gain, chooses by `lorand`/`hirand` (a random number is
//!   drawn only for an event that starts a region, so controllers do not change the random
//!   choices of patches without such regions) and ignores the round robin. No key release ends
//!   it: it plays to its sample's end or until all notes off.
//! - **Controller crossfades and gains.** `xfin_loccN`, `xfout_loccN` and their kin fade a
//!   region in and out with a controller, `volume_onccN` (`gain_ccN`) changes its level in dB and
//!   `amplitude_onccN` scales it ([`Region::controller_gain`]), on the voice's channel and while
//!   it sounds: a swell on controller 1 crossfades the dynamic layers of a held chord. At each
//!   block start every voice aims at the gain the controllers give it then and reaches it on the
//!   block's last frame with a linear ramp, so a controller step never clicks; a controller that
//!   moves within a block reaches sounding voices from the next one. A voice starts at the gain
//!   of the controllers at its start, and starts even when that gain is 0, since the controller
//!   may rise while it is held.
//!
//! Not supported, and not offered as parameters: filters, per-region pan, sample streaming,
//! high-resolution controllers (`lohdccN` and the like). A patch that uses them plays without
//! them (the loader reports the opcodes it ignored). A region that no key starts
//! ([`Region::on_key`] is false: a key of -1, or a region a controller starts) is never started
//! by a key press or release.

use std::f64::consts::FRAC_PI_2;
use std::sync::Arc;

use super::DEFAULT_SPEC;
use super::params::{choice, float};
use crate::bank::{
    ArticulationTarget, Controllers, DynamicsDrive, Keyswitches, LegatoSpec, LoopMode, MAX_MEMBERS,
    OffMode, Passage, Region, Sample, SampleBank, SamplerMapping, SamplerSetup, Selector, Trigger,
};
use crate::filters::db_to_amp;
use crate::processor::{
    BlockContext, DeviceState, EventKind, InstrumentEvent, ParamSlot, ParamSlots, ProcessSpec,
    Processor, Report,
};

/// The plugin this processor implements.
pub(crate) const PLUGIN_ID: &str = "sonara.sampler";

// Slot indices, in the descriptor's parameter order. The graph reads `instrument` (3) and the
// application `instrument_type` (4); the processor reads neither.
const VOLUME_DB: usize = 0;
const PAN: usize = 1;
const RELEASE_SCALE: usize = 2;
const ARTICULATION: usize = 5;
const DYNAMICS: usize = 6;
const MUTE: usize = 7;
const VIBRATO: usize = 8;

/// Voices that can sound at once.
const MAX_VOICES: usize = 64;
/// MIDI channels, each with its own controller values.
const CHANNELS: usize = 16;
/// The sustain pedal's controller.
const SUSTAIN: u8 = 64;
/// Shortest fade-in, in seconds: prevents clicks on regions without an attack.
const MIN_ATTACK: f64 = 0.0005;
/// Shortest fade-out, in seconds: prevents clicks on regions without a release.
const MIN_RELEASE: f64 = 0.005;
/// The fade-out, in seconds, of a voice that an exclusive group switches off with the default
/// `off_mode=fast`, and of every voice on all notes off.
const CHOKE_RELEASE: f64 = 0.01;
/// Pitch bend range in semitones.
const BEND_RANGE: f64 = 2.0;
/// The seed of the generator behind `lorand`/`hirand`.
const RNG_SEED: u32 = 0x2545_F491;
/// The highest MIDI key and velocity.
const MIDI_MAX: u8 = 127;
/// The lowest `dynamics` a sounding note's level is measured against: the value at which a
/// note of velocity 127 plays at velocity 1. A lower value plays as this one, so no ratio of
/// two dynamics divides by zero.
const MIN_DYNAMICS: f64 = 1.0 / 127.0;
/// The most the `dynamics` parameter raises a sounding note's level above the level it started
/// with: 4, +12 dB.
const MAX_DYNAMICS_GAIN: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Settings {
    gain: f64,
    pan: f64,
    release_scale: f64,
}

impl Settings {
    fn from_slots(slots: &ParamSlots) -> Option<Self> {
        Some(Self {
            gain: db_to_amp(float(slots, VOLUME_DB)?),
            pan: float(slots, PAN)?.clamp(-1.0, 1.0),
            release_scale: float(slots, RELEASE_SCALE)?.clamp(0.1, 4.0),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct Voice {
    active: bool,
    region: usize,
    sample: usize,
    /// The key that started the voice; `None` for a voice a controller started, which no key
    /// release ends.
    key: Option<u8>,
    channel: u8,
    /// Position in source frames.
    pos: f64,
    /// Source frames per output frame, before pitch bend.
    step: f64,
    /// Region gain times the gain the note's velocity gives the region.
    gain: f64,
    /// The `dynamics` the voice started with, at least [`MIN_DYNAMICS`]: what the parameter's
    /// later values are measured against ([`dynamics_gain`]).
    dynamics: f64,
    /// What the articulation of the voice's note forces, when the note has its own
    /// ([`InstrumentEvent::articulation`]): the voice reads the controllers with it in place of
    /// the parameter's while it sounds. `None` for a voice that follows the parameter's.
    articulation: Option<Forced>,
    /// The gain that follows the voice while it sounds: the gain the controllers give the
    /// region ([`Region::controller_gain`]) times the gain the `dynamics` parameter gives the
    /// voice ([`dynamics_gain`]), ramped linearly from block to block.
    live_gain: f64,
    /// The change of `live_gain` per frame in this block.
    live_step: f64,
    /// The value `live_gain` reaches on this block's last frame.
    live_target: f64,
    level: f64,
    /// How the level is heard. A voice that a legato crossfade fades in leaves the equal-power
    /// curve when it reaches full level, so its release is the same as any note's.
    curve: Curve,
    attack_step: f64,
    release_step: f64,
    releasing: bool,
    /// Key released while the pedal was down: releases when the pedal comes up.
    held_by_pedal: bool,
    /// What a transition of the overlap legato does with the voice.
    transition: Transition,
    off_by: u32,
    age: u64,
}

impl Voice {
    const IDLE: Self = Self {
        active: false,
        region: 0,
        sample: 0,
        key: None,
        channel: 0,
        pos: 0.0,
        step: 0.0,
        gain: 0.0,
        dynamics: 1.0,
        articulation: None,
        live_gain: 1.0,
        live_step: 0.0,
        live_target: 1.0,
        level: 0.0,
        curve: Curve::Linear,
        attack_step: 0.0,
        release_step: 0.0,
        releasing: false,
        held_by_pedal: false,
        transition: Transition::Untouched,
        off_by: 0,
        age: 0,
    };

    /// The level as it is heard: along the equal-power curve during a legato crossfade.
    fn heard(&self) -> f64 {
        match self.curve {
            Curve::Linear => self.level,
            Curve::EqualPower => (self.level * FRAC_PI_2).sin(),
        }
    }

    /// Fades the voice out over `seconds` along the equal-power curve, from where it is heard,
    /// as a transition of the overlap legato leaves its note: a fade of its release under way
    /// starts again over the crossfade, and the sustain pedal no longer holds it.
    fn leave(&mut self, sample_rate: u32, seconds: f64) {
        if self.curve == Curve::Linear {
            // The level on the curve that sounds as the level does now, so the fade starts
            // where the voice is heard, also in its attack or its release.
            if self.level < 1.0 {
                self.level = self.level.clamp(0.0, 1.0).asin() / FRAC_PI_2;
            }
            self.curve = Curve::EqualPower;
        }
        self.held_by_pedal = false;
        self.releasing = false;
        self.transition = Transition::Leaving;
        start_release(self, sample_rate, seconds);
    }
}

/// How a voice's level is heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Curve {
    /// As it is: a linear attack and release.
    Linear,
    /// Along the equal-power curve, `sin(level · π/2)`, as in a legato crossfade: the powers of
    /// a note that fades out this way and of one that fades in add up to the same all through
    /// the crossfade, where linear fades of two uncorrelated sounds dip by 3 dB mid-way.
    EqualPower,
}

/// What a transition of the overlap legato does with a voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transition {
    /// Nothing: a release region's voice, a controller's, or one that an exclusive group or all
    /// notes off switched off.
    Untouched,
    /// The sound of a key press, held or fading with its key's release: a transition from its
    /// key fades it out over the crossfade.
    Joinable,
    /// Fading out under a transition: a later one shortens the fade to [`CHOKE_RELEASE`], and
    /// no exclusive group switches it off.
    Leaving,
}

/// How a key press that the overlap legato joins to the note before it starts its regions.
#[derive(Debug, Clone, Copy)]
struct Join {
    /// The crossfade in seconds: the fade-in of the new note, in place of its regions' attack.
    crossfade: f64,
    /// How far into its sample the new note starts, in seconds past the region's offset.
    skip: f64,
}

/// The last key press of a note, the one that a press of the overlap legato joins.
#[derive(Debug, Clone, Copy)]
struct LastNote {
    key: u8,
    channel: u8,
    /// The frame of the key's release since the press ([`Sampler::frame`]); `None` while the key
    /// is held.
    released_at: Option<u64>,
}

/// What starts a voice.
#[derive(Debug, Clone, Copy)]
enum Start {
    /// A key press or release: the key, the velocity it was pressed with, its channel, the
    /// `dynamics` it was pressed with (at least [`MIN_DYNAMICS`]), what the note's own
    /// articulation forces, if it has one, and how the overlap legato joins a press to the note
    /// before it, if it does.
    Key {
        key: u8,
        velocity: u8,
        channel: u8,
        dynamics: f64,
        articulation: Option<Forced>,
        joined: Option<Join>,
    },
    /// A controller event on a channel: the region plays at its `pitch_keycenter`, and no
    /// velocity changes its gain.
    Controller { channel: u8 },
}

/// What a choice of the `articulation` parameter forces over what the clips select: its target
/// in the preset's mapping ([`SamplerMapping::target`]), nothing for `auto` and for a choice
/// without a target. The parameter's choice forces it for every new note; a note's own
/// articulation forces it in place of the parameter's for that note alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Forced {
    /// The patch of a multi-patch set that new notes play ([`ArticulationTarget::member`]);
    /// `None` when nothing is forced, and the patch of the preset's ordinary sound plays
    /// ([`SamplerMapping::default_member`]).
    member: Option<u8>,
    /// The active keyswitch for new notes in that patch, until a keyswitch from a clip replaces
    /// it.
    switch: Option<u8>,
    /// The velocity zone new notes play in, `(lo, hi)` with 1 ≤ `lo` ≤ `hi` ≤ 127.
    velocity: Option<(u8, u8)>,
    /// A controller number and the value the regions read for it on every channel, until an
    /// event of that controller replaces it.
    controller: Option<(u8, u8)>,
    /// Whether new notes are joined by the preset's overlap legato
    /// ([`ArticulationTarget::legato`]).
    legato: bool,
}

impl Forced {
    /// Nothing forced: the patch and the clips decide.
    const NONE: Self = Self {
        member: None,
        switch: None,
        velocity: None,
        controller: None,
        legato: false,
    };

    /// What `target` forces: its patch, its selector and its legato; nothing without a target.
    fn of(target: Option<ArticulationTarget>) -> Self {
        target.map_or(Self::NONE, |target| Self {
            member: Some(target.member),
            legato: target.legato,
            ..Self::select(target.selector)
        })
    }

    /// What `selector` forces; nothing for a target without one, a patch that is the
    /// articulation. Values outside the MIDI range are brought into it.
    fn select(selector: Option<Selector>) -> Self {
        match selector {
            None => Self::NONE,
            Some(Selector::Keyswitch(key)) => Self {
                switch: Some(key),
                ..Self::NONE
            },
            Some(Selector::Velocity { lo, hi }) => {
                let lo = lo.clamp(1, MIDI_MAX);
                Self {
                    velocity: Some((lo, hi.clamp(lo, MIDI_MAX))),
                    ..Self::NONE
                }
            }
            Some(Selector::Controller { number, value }) => Self {
                controller: Some((number, value.min(MIDI_MAX))),
                ..Self::NONE
            },
        }
    }

    /// The velocity a note pressed at `velocity` plays with: moved into the zone, if one is
    /// forced, by `lo + (velocity - 1) · (hi - lo) / 126`, so the softest note plays at `lo`,
    /// the loudest at `hi`, and some dynamic range remains.
    fn velocity(self, velocity: u8) -> u8 {
        let Some((lo, hi)) = self.velocity else {
            return velocity;
        };
        let pressed = u16::from(velocity.clamp(1, MIDI_MAX).saturating_sub(1));
        let span = u16::from(hi.saturating_sub(lo));
        // Both factors are at most 126, so the product fits; 126 is the range of a velocity.
        let moved = pressed.saturating_mul(span) / u16::from(MIDI_MAX - 1);
        lo.saturating_add(u8::try_from(moved).unwrap_or(MIDI_MAX))
            .min(hi)
    }

    /// `controllers` as the regions read them: with the forced controller's value.
    fn read(self, mut controllers: Controllers) -> Controllers {
        if let Some((number, value)) = self.controller {
            controllers.set(number, value);
        }
        controllers
    }
}

/// What selects the `trigger=release` regions of a key release, and what they play with.
#[derive(Debug, Clone, Copy)]
struct KeyRelease {
    /// The velocity the key was pressed with.
    velocity: u8,
    /// The patch of a multi-patch set that the note played; its release regions are that
    /// patch's.
    member: u8,
    /// The keyswitch of that patch active when the key was released, or the one the note's own
    /// articulation forces.
    switch: Option<u8>,
    /// The `dynamics` the key was pressed with, so the note's tail follows the parameter as
    /// the note did.
    dynamics: f64,
    /// What the note's own articulation forces, if it had one: its tail reads the controllers
    /// with it in place of the parameter's.
    articulation: Option<Forced>,
}

/// The sample player.
pub(crate) struct Sampler {
    spec: ProcessSpec,
    params: ParamSlots,
    settings: Option<Settings>,
    /// The instrument: the loaded patch and the mapping of its preset.
    setup: Option<Arc<SamplerSetup>>,
    voices: [Voice; MAX_VOICES],
    /// The velocity each key was pressed with, for release-trigger regions.
    key_velocity: [u8; 128],
    /// The `dynamics` each key was pressed with, for its release-trigger regions; `None` for a
    /// key not pressed since the last reset.
    key_dynamics: [Option<f64>; 128],
    /// What the own articulation of the note last pressed on each key forces, for its release;
    /// `None` for a key whose note follows the parameter's choice.
    key_articulation: [Option<Forced>; 128],
    /// The patch of a multi-patch set that the note last pressed on each key played, for its
    /// release; 0 for a bank of one patch.
    key_member: [u8; 128],
    key_down: [bool; 128],
    /// The keys released while the sustain pedal was down: their release regions start when
    /// the pedal lifts.
    released_under_pedal: [Option<KeyRelease>; 128],
    /// The keys that a transition of the overlap legato left while they were held: their
    /// release starts no release regions.
    key_left: [bool; 128],
    /// The last key press of a note (not of a keyswitch key), which the next press of the
    /// overlap legato joins when it is held or just released; `None` after a reset and all
    /// notes off.
    last_note: Option<LastNote>,
    /// The frame of the event being applied, counted from the last reset (wrapping): what the
    /// gap of the overlap legato is measured with.
    frame: u64,
    /// The last value of every controller, per MIDI channel.
    controllers: [Controllers; CHANNELS],
    /// The last keyswitch the clips selected in each patch, or the patch's default switch, by
    /// member number ([`Region::member`]): of a bank of one patch, only the first is used.
    switches: [Option<u8>; MAX_MEMBERS],
    /// The choice of the `articulation` parameter in force, by index.
    articulation: u16,
    /// The choices of the `mute` and `vibrato` parameters in force.
    passage: Passage,
    /// The target last applied: that of the articulation choice under the passage, in the
    /// mapping of the preset.
    target: Option<ArticulationTarget>,
    /// What that target forces over what the clips select.
    forced: Forced,
    /// The value of the `dynamics` parameter, 0 to 1.
    dynamics: f64,
    round_robin: u32,
    rng: u32,
    bend: f64,
    volume_cc: f64,
    expression_cc: f64,
    pedal: bool,
    clock: u64,
}

impl Sampler {
    /// Setup: an unprepared processor without an instrument (it plays silence).
    pub(crate) fn new() -> Self {
        Self {
            spec: DEFAULT_SPEC,
            params: ParamSlots::EMPTY,
            settings: None,
            setup: None,
            voices: [Voice::IDLE; MAX_VOICES],
            key_velocity: [0; 128],
            key_dynamics: [None; 128],
            key_articulation: [None; 128],
            key_member: [0; 128],
            key_down: [false; 128],
            released_under_pedal: [None; 128],
            key_left: [false; 128],
            last_note: None,
            frame: 0,
            controllers: [Controllers::INITIAL; CHANNELS],
            switches: [None; MAX_MEMBERS],
            articulation: SamplerMapping::AUTO,
            passage: Passage::AUTO,
            target: None,
            forced: Forced::NONE,
            dynamics: 1.0,
            round_robin: 0,
            rng: RNG_SEED,
            bend: 0.0,
            volume_cc: 1.0,
            expression_cc: 1.0,
            pedal: false,
            clock: 0,
        }
    }

    fn rest(&mut self) {
        self.voices = [Voice::IDLE; MAX_VOICES];
        self.key_velocity = [0; 128];
        self.key_dynamics = [None; 128];
        self.key_articulation = [None; 128];
        self.key_member = [0; 128];
        self.key_down = [false; 128];
        self.released_under_pedal = [None; 128];
        self.key_left = [false; 128];
        self.last_note = None;
        self.frame = 0;
        self.controllers = [self.initial_controllers(); CHANNELS];
        self.switches = self.default_switches();
        self.round_robin = 0;
        self.rng = RNG_SEED;
        self.bend = 0.0;
        self.volume_cc = 1.0;
        self.expression_cc = 1.0;
        self.pedal = false;
        self.force();
        self.drive_controller();
    }

    /// Applies the target of the `articulation` choice in force under the `mute` and
    /// `vibrato` choices in force, from the mapping of the preset the sampler plays.
    fn force(&mut self) {
        self.target = self.lookup();
        self.forced = Forced::of(self.target);
    }

    /// The target of the `articulation` choice in force under the passage in force: a lookup,
    /// which neither allocates nor searches (RT-02).
    fn lookup(&self) -> Option<ArticulationTarget> {
        self.setup
            .as_deref()
            .and_then(|setup| setup.mapping.target_under(self.articulation, self.passage))
    }

    /// Takes the `articulation` choice `articulation` and the `mute` and `vibrato` choices of
    /// `passage`. A new articulation choice applies its target. A new passage applies the
    /// target it gives the articulation choice only when that differs from the target in force,
    /// so a mute or vibrato the preset does not record leaves what the clips selected since
    /// alone; the same choices apply nothing again.
    fn choose(&mut self, articulation: u16, passage: Passage) {
        let new_choice = articulation != self.articulation;
        self.articulation = articulation;
        self.passage = passage;
        if new_choice || self.lookup() != self.target {
            self.force();
        }
    }

    /// What the articulation of a note-on, the `articulation` choice with index `choice`,
    /// forces in place of the parameter's choice: its target under the `mute` and `vibrato`
    /// choices in force, in the mapping of the preset the sampler plays, a lookup like
    /// [`Self::lookup`]. `None` for a note without an articulation of its own and for a choice
    /// the mapping has no target for (`auto` among them), so the note plays the parameter's
    /// choice.
    fn own_articulation(&self, choice: Option<u16>) -> Option<Forced> {
        let target = self
            .setup
            .as_deref()?
            .mapping
            .target_under(choice?, self.passage)?;
        Some(Forced::of(Some(target)))
    }

    /// The patch of a multi-patch set that a note plays when `forced` is what its articulation
    /// forces: the patch its target names, else the patch of the preset's ordinary sound
    /// ([`SamplerMapping::default_member`]); 0 for a bank of one patch.
    fn member_with(&self, forced: Forced) -> u8 {
        forced.member.unwrap_or_else(|| {
            self.setup
                .as_deref()
                .map_or(0, |setup| setup.mapping.default_member())
        })
    }

    /// The keyswitch a note of the patch `member` plays with when `forced` is what its
    /// articulation forces: the forced one, when the forced target plays that patch, else the
    /// clips' last one in the patch.
    fn switch_in(&self, member: u8, forced: Forced) -> Option<u8> {
        forced
            .switch
            .filter(|_| self.member_with(forced) == member)
            .or_else(|| self.switches.get(usize::from(member)).copied().flatten())
    }

    /// What the `dynamics` parameter drives in the preset the sampler plays: the velocity
    /// without an instrument.
    fn drive(&self) -> DynamicsDrive {
        self.setup
            .as_deref()
            .map_or(DynamicsDrive::Velocity, |setup| setup.mapping.dynamics)
    }

    /// The velocity a key pressed at `velocity` plays with, before an articulation's velocity
    /// zone moves it: scaled by the `dynamics` parameter ([`scaled_velocity`]), from the
    /// preset's fixed velocity in place of the note's own when it has one. A preset whose
    /// dynamics drive a controller keeps the note's velocity.
    fn dynamic_velocity(&self, velocity: u8) -> u8 {
        match self.drive() {
            DynamicsDrive::Velocity => scaled_velocity(velocity, self.dynamics),
            DynamicsDrive::Fixed(fixed) => scaled_velocity(fixed.clamp(1, MIDI_MAX), self.dynamics),
            DynamicsDrive::Controller(_) => velocity,
        }
    }

    /// The `dynamics` a voice that starts now remembers: the parameter's value, at least
    /// [`MIN_DYNAMICS`].
    fn start_dynamics(&self) -> f64 {
        self.dynamics.max(MIN_DYNAMICS)
    }

    /// For a preset whose dynamics drive a controller, sets that controller to the `dynamics`
    /// parameter's value, `round(dynamics · 127)`, on every channel, as an event of the
    /// controller from a clip would, and so for volume (7) and expression (11) the level they
    /// set too. It starts no region and does not move the sustain pedal. A later event of the
    /// controller from a clip replaces the value, until this runs again: when the parameter
    /// changes, on a reset and at all notes off.
    fn drive_controller(&mut self) {
        let DynamicsDrive::Controller(number) = self.drive() else {
            return;
        };
        let value = controller_value(self.dynamics);
        for controllers in &mut self.controllers {
            controllers.set(number, value);
        }
        let level = f64::from(value) / 127.0;
        match number {
            7 => self.volume_cc = level,
            11 => self.expression_cc = level,
            _ => {}
        }
    }

    /// `event`, a note on, a note off or a keyswitch event of a clip, on the key of the patch:
    /// moved by the preset's key offset ([`SamplerMapping::patch_key`]). `None` when the offset
    /// moves the key out of the MIDI keys, so the event is dropped.
    fn moved(&self, event: &InstrumentEvent) -> Option<InstrumentEvent> {
        let Some(setup) = self.setup.as_deref() else {
            return Some(*event);
        };
        let pitch = setup.mapping.patch_key(event.pitch)?;
        Some(InstrumentEvent { pitch, ..*event })
    }

    /// A handle on the loaded patch, for the methods that start voices while they read it.
    /// Cloning the [`Arc`] only counts a reference, and the setup this processor holds keeps the
    /// bank alive, so dropping the handle never frees it (RT-02).
    fn shared_bank(&self) -> Option<Arc<SampleBank>> {
        self.setup.as_ref().map(|setup| Arc::clone(&setup.bank))
    }

    /// The switch of each patch that is active before any keyswitch, by member number: the
    /// patch's `sw_default`.
    fn default_switches(&self) -> [Option<u8>; MAX_MEMBERS] {
        let mut switches = [None; MAX_MEMBERS];
        if let Some(bank) = bank_of(self.setup.as_ref()) {
            for (member, switch) in (0..=u8::MAX).zip(&mut switches) {
                *switch = bank.keyswitches_of(member).default;
            }
        }
        switches
    }

    /// Where the controllers of every channel start: as the patch sets them.
    fn initial_controllers(&self) -> Controllers {
        bank_of(self.setup.as_ref()).map_or(Controllers::INITIAL, |bank| bank.controllers)
    }

    /// The controller values of `channel` as the regions read them, with the value that
    /// `forced`, what an articulation forces, sets; the initial ones for a channel above 15,
    /// which is no MIDI channel.
    fn controllers_of(&self, channel: u8, forced: Forced) -> Controllers {
        let controllers = self
            .controllers
            .get(usize::from(channel))
            .copied()
            .unwrap_or_else(|| self.initial_controllers());
        forced.read(controllers)
    }

    /// A uniform number in `[0, 1)` (xorshift32).
    fn random(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        f64::from(x) / (f64::from(u32::MAX) + 1.0)
    }

    fn apply(&mut self, event: &InstrumentEvent) {
        match event.kind {
            EventKind::NoteOn if event.velocity > 0 => {
                if let Some(event) = self.moved(event) {
                    self.note_on(&event);
                }
            }
            EventKind::NoteOn | EventKind::NoteOff => {
                if let Some(event) = self.moved(event) {
                    self.note_off(&event);
                }
            }
            EventKind::PitchBend => self.bend = f64::from(event.value) / 8192.0 * BEND_RANGE,
            EventKind::Cc => self.control_change(event),
            EventKind::ChannelPressure => {}
            EventKind::ParamChoice => self.change_choice(event),
            EventKind::KeySwitch => {
                // Only a keyswitch key of the patch (of a set's patches) selects an
                // articulation, as with a press.
                if let Some(event) = self.moved(event) {
                    self.select_switch(event.pitch);
                }
            }
            EventKind::AllNotesOff => {
                // Back to the default articulation of every patch and the articulation
                // parameter's target: the engine's chase follows with the last keyswitch and
                // controller values before the new position, which win after it.
                self.switches = self.default_switches();
                self.force();
                self.pedal = false;
                self.key_down = [false; 128];
                // Everything stops: the releases under the pedal start no tail, and the next
                // legato press starts a phrase with its attack.
                self.released_under_pedal = [None; 128];
                self.key_left = [false; 128];
                self.last_note = None;
                // The pedal is up, and the controllers say so too.
                for controllers in &mut self.controllers {
                    controllers.set(SUSTAIN, 0);
                }
                // The `dynamics` value again for a preset whose dynamics drive a controller;
                // the chased value of the controller, if any, follows and wins.
                self.drive_controller();
                for voice in &mut self.voices {
                    if voice.active {
                        start_release(voice, self.spec.sample_rate, CHOKE_RELEASE);
                        voice.transition = Transition::Untouched;
                    }
                }
            }
        }
    }

    /// A choice event: the `articulation`, `mute` or `vibrato` parameter takes the choice
    /// `event.value` from this frame on, as [`Self::apply_params`] takes a new choice
    /// ([`Self::choose`]), and the stored parameters hold it, so the next parameter set
    /// compares with what is in force. An event of another parameter, or of a value that is no
    /// choice index, changes nothing.
    fn change_choice(&mut self, event: &InstrumentEvent) {
        let slot = usize::from(event.number);
        let Ok(value) = u16::try_from(event.value) else {
            return;
        };
        let (articulation, passage) = match slot {
            ARTICULATION => (value, self.passage),
            MUTE => (
                self.articulation,
                Passage {
                    mute: value,
                    ..self.passage
                },
            ),
            VIBRATO => (
                self.articulation,
                Passage {
                    vibrato: value,
                    ..self.passage
                },
            ),
            _ => return,
        };
        self.params.set(slot, ParamSlot::Choice(value));
        self.choose(articulation, passage);
    }

    fn control_change(&mut self, event: &InstrumentEvent) {
        let number = event.number;
        let clamped = event.value.clamp(0, 127);
        // Written first, so the gates of the regions this event starts read the new value.
        let cc = u8::try_from(clamped).unwrap_or(127);
        if let Some(controllers) = self.controllers.get_mut(usize::from(event.channel)) {
            controllers.set(number, cc);
        }
        // An event of the controller an articulation forces wins after it.
        if self
            .forced
            .controller
            .is_some_and(|(forced, _)| forced == number)
        {
            self.forced.controller = None;
        }
        let value = f64::from(clamped) / 127.0;
        match number {
            7 => self.volume_cc = value,
            11 => self.expression_cc = value,
            SUSTAIN => {
                let down = event.value >= 64;
                if self.pedal && !down {
                    let rate = self.spec.sample_rate;
                    let scale = self.settings.map_or(1.0, |s| s.release_scale);
                    for voice in &mut self.voices {
                        if voice.active && voice.held_by_pedal {
                            voice.held_by_pedal = false;
                            let release =
                                release_time(bank_of(self.setup.as_ref()), voice.region, scale);
                            start_release(voice, rate, release);
                        }
                    }
                    self.start_pedal_releases(event.channel);
                }
                self.pedal = down;
            }
            _ => {}
        }
        self.start_controller_regions(event.channel, number, cc);
    }

    /// Starts the regions that an event setting controller `number` to `value` on `channel`
    /// starts ([`Region::starts_on_controller`]), whose gates and keyswitch let them and whose
    /// random range holds the number drawn for this event. The number is drawn only once a
    /// region qualifies, so a patch without controller triggers keeps its random sequence.
    fn start_controller_regions(&mut self, channel: u8, number: u8, value: u8) {
        let Some(bank) = self.shared_bank() else {
            return;
        };
        let controllers = self.controllers_of(channel, self.forced);
        // The patch new notes play, with its switch.
        let member = self.member_with(self.forced);
        let switch = self.switch_in(member, self.forced);
        let mut roll = None;
        for (index, region) in bank.regions.iter().enumerate() {
            if region.member != member
                || !region.starts_on_controller(number, value)
                || !region.gates_open(&controllers)
                || !in_switch(region, switch)
            {
                continue;
            }
            let roll = *roll.get_or_insert_with(|| self.random());
            if !(region.rand_lo..region.rand_hi).contains(&roll) {
                continue;
            }
            self.choke(&bank, region.group);
            self.start_voice(&bank, index, region, Start::Controller { channel });
        }
    }

    fn note_on(&mut self, event: &InstrumentEvent) {
        let Some(bank) = self.shared_bank() else {
            return;
        };
        let key = event.pitch;
        // The note's own articulation, if it has one with a target, plays in place of the
        // parameter's, for this note alone: `self.forced` stays as it is.
        let own = self.own_articulation(event.articulation);
        // The dynamics first, then the articulation's zone, so a softer note stays in the zone.
        let velocity = own
            .unwrap_or(self.forced)
            .velocity(self.dynamic_velocity(event.velocity));
        let dynamics = self.start_dynamics();
        // The patch the note plays, of a multi-patch set: its own articulation's, else the
        // parameter's.
        let member = self.member_with(own.unwrap_or(self.forced));
        let keyswitches = bank.keyswitches_of(member);
        // Counted before this key is marked down, so a press of a key that is still down
        // counts that key (sfizz counts both presses too).
        let another_key_down = self.another_key_down(keyswitches);
        let switch_key = keyswitches.contains(key);
        // A keyswitch key of the patch, or of another patch of a set, selects its switch there.
        self.select_switch(key);
        // A keyswitch key plays no part in the overlap legato.
        let joined = if switch_key {
            None
        } else {
            self.press_legato(key, event.channel, own.unwrap_or(self.forced).legato)
        };
        if let Some(down) = self.key_down.get_mut(usize::from(key)) {
            *down = true;
        }
        if let Some(left) = self.key_left.get_mut(usize::from(key)) {
            *left = false;
        }
        if let Some(vel) = self.key_velocity.get_mut(usize::from(key)) {
            *vel = velocity;
        }
        if let Some(pressed) = self.key_dynamics.get_mut(usize::from(key)) {
            *pressed = Some(dynamics);
        }
        if let Some(articulation) = self.key_articulation.get_mut(usize::from(key)) {
            *articulation = own;
        }
        if let Some(played) = self.key_member.get_mut(usize::from(key)) {
            *played = member;
        }
        let roll = self.random();
        let counter = self.round_robin;
        // Read after a press of a keyswitch key has selected its switch.
        let forced = own.unwrap_or(self.forced);
        let controllers = self.controllers_of(event.channel, forced);
        let switch = self.switch_in(member, forced);
        let start = Start::Key {
            key,
            velocity,
            channel: event.channel,
            dynamics,
            articulation: own,
            joined,
        };
        let mut advanced = false;
        for (index, region) in bank.regions.iter().enumerate() {
            if region.member != member
                || !region.trigger.starts_on_press(another_key_down)
                || !matches_note(region, key, velocity, switch)
                || !region.gates_open(&controllers)
                || !(region.rand_lo..region.rand_hi).contains(&roll)
                || !in_sequence(region, counter)
            {
                continue;
            }
            advanced |= region.seq_length > 1;
            self.choke(&bank, region.group);
            self.start_voice(&bank, index, region, start);
        }
        if advanced {
            self.round_robin = self.round_robin.wrapping_add(1);
        }
    }

    /// Makes `key`, a key that a clip pressed or sent, the active switch of every patch whose
    /// keyswitch key it is (of a bank of one patch, of the patch): it wins over the switch an
    /// articulation forces in that patch, until that is applied again. A key that is no
    /// keyswitch key changes nothing.
    fn select_switch(&mut self, key: u8) {
        let forced_member = self.member_with(self.forced);
        let Some(bank) = bank_of(self.setup.as_ref()) else {
            return;
        };
        for (member, switch) in (0..=u8::MAX)
            .zip(&mut self.switches)
            .take(bank.member_count())
        {
            if bank.keyswitches_of(member).contains(key) {
                *switch = Some(key);
            }
        }
        if bank.keyswitches_of(forced_member).contains(key) {
            self.forced.switch = None;
        }
    }

    /// The overlap legato of a press of `key` on `channel`, a note that plays the legato target
    /// when `legato` is true: `None` for a note that plays its attack. A legato press on another
    /// key than the last note's, while that one is held or at most the preset's gap after its
    /// release, leaves it ([`Self::leave_note`]) and is joined to it. Every press becomes the
    /// last note; a press that does not play the legato target, or one of a preset without an
    /// overlap legato, joins nothing.
    fn press_legato(&mut self, key: u8, channel: u8, legato: bool) -> Option<Join> {
        let pressed = LastNote {
            key,
            channel,
            released_at: None,
        };
        let previous = self.last_note.replace(pressed);
        if !legato {
            return None;
        }
        let spec = self.setup.as_deref()?.mapping.legato?;
        let from = previous.filter(|note| note.key != key && self.within_gap(note, spec))?;
        let crossfade = (f64::from(spec.crossfade_ms) / 1000.0).max(MIN_RELEASE);
        self.leave_note(from, crossfade);
        Some(Join {
            crossfade,
            skip: f64::from(spec.start_offset_ms) / 1000.0,
        })
    }

    /// Whether a legato press now joins `note`: it is held, or it was released at most the
    /// gap of `spec` ago, a release on the frame of the press included.
    fn within_gap(&self, note: &LastNote, spec: LegatoSpec) -> bool {
        // At most 100 ms at 384 kHz in a valid mapping, far from the limit of a u64.
        let gap = u64::from(spec.gap_ms).saturating_mul(u64::from(self.spec.sample_rate)) / 1000;
        note.released_at
            .is_none_or(|released| self.frame.wrapping_sub(released) <= gap)
    }

    /// A transition of the overlap legato leaves `note`: its voices fade out over `crossfade`
    /// seconds from where they are heard ([`Voice::leave`]), and the fades of earlier
    /// transitions are shortened to [`CHOKE_RELEASE`], so a fast run keeps at most two notes
    /// sounding beyond those short tails. The note starts no release regions that have not
    /// started yet: none at the release of its key while it is held, and none waiting for the
    /// sustain pedal.
    fn leave_note(&mut self, note: LastNote, crossfade: f64) {
        let rate = self.spec.sample_rate;
        for voice in self.voices.iter_mut().filter(|voice| voice.active) {
            match voice.transition {
                Transition::Leaving => start_release(voice, rate, CHOKE_RELEASE),
                Transition::Joinable
                    if voice.key == Some(note.key) && voice.channel == note.channel =>
                {
                    voice.leave(rate, crossfade);
                }
                Transition::Joinable | Transition::Untouched => {}
            }
        }
        if note.released_at.is_none()
            && let Some(left) = self.key_left.get_mut(usize::from(note.key))
        {
            *left = true;
        }
        if let Some(waiting) = self.released_under_pedal.get_mut(usize::from(note.key)) {
            *waiting = None;
        }
    }

    /// Whether a key other than a key of `keyswitches` (those of the patch a press plays) is
    /// down: what selects between the `first` and `legato` regions of a press. A key the
    /// sustain pedal holds after its release is not down.
    fn another_key_down(&self, keyswitches: &Keyswitches) -> bool {
        (0..=127_u8)
            .zip(&self.key_down)
            .any(|(key, down)| *down && !keyswitches.contains(key))
    }

    fn note_off(&mut self, event: &InstrumentEvent) {
        let key = event.pitch;
        if let Some(down) = self.key_down.get_mut(usize::from(key)) {
            *down = false;
        }
        // The last note's release: a legato press within the gap after it still joins it.
        if let Some(note) = self.last_note.as_mut()
            && note.key == key
            && note.channel == event.channel
            && note.released_at.is_none()
        {
            note.released_at = Some(self.frame);
        }
        let left = self
            .key_left
            .get_mut(usize::from(key))
            .is_some_and(std::mem::take);
        let rate = self.spec.sample_rate;
        let pedal = self.pedal;
        let scale = self.settings.map_or(1.0, |s| s.release_scale);
        let bank = self.shared_bank();
        for voice in &mut self.voices {
            if !voice.active
                || voice.releasing
                || voice.key != Some(key)
                || voice.channel != event.channel
            {
                continue;
            }
            let Some(region) = bank.as_ref().and_then(|b| b.regions.get(voice.region)) else {
                continue;
            };
            if region.loop_mode == LoopMode::OneShot || region.trigger.is_release() {
                continue;
            }
            if pedal {
                voice.held_by_pedal = true;
            } else {
                start_release(voice, rate, region.release.max(MIN_RELEASE) * scale);
            }
        }
        if left {
            // A note that a legato transition left has faded already, and its release starts
            // no release regions, now or when the pedal lifts.
            return;
        }
        // A note with an articulation of its own releases with its keyswitch, else with the
        // switch active now, in the patch the note played.
        let own = self
            .key_articulation
            .get(usize::from(key))
            .copied()
            .flatten();
        let member = self.key_member.get(usize::from(key)).copied().unwrap_or(0);
        let release = KeyRelease {
            velocity: self
                .key_velocity
                .get(usize::from(key))
                .copied()
                .unwrap_or(event.velocity),
            member,
            switch: self.switch_in(member, own.unwrap_or(self.forced)),
            articulation: own,
            dynamics: self
                .key_dynamics
                .get(usize::from(key))
                .copied()
                .flatten()
                .unwrap_or_else(|| self.start_dynamics()),
        };
        if let Some(bank) = bank {
            // `trigger=release_key` "ignores sustain pedal" and starts now in any case; a
            // `trigger=release` region starts now only when the pedal is up.
            let starts: fn(Trigger) -> bool = if pedal {
                |trigger| trigger == Trigger::ReleaseKey
            } else {
                Trigger::is_release
            };
            self.start_release_regions(&bank, key, event.channel, release, starts);
        }
        if pedal {
            // SFZ `trigger=release` plays "on note-off or sustain pedal off": the regions wait
            // for the pedal. The release is captured now, so a re-press of the key before the
            // pedal lifts does not change it; a later release of the key replaces it.
            if let Some(slot) = self.released_under_pedal.get_mut(usize::from(key)) {
                *slot = Some(release);
            }
        }
    }

    /// Starts the `trigger=release` regions of the keys released while the pedal was down, in
    /// key order, and forgets those releases. Their `release_key` regions started at the
    /// release.
    fn start_pedal_releases(&mut self, channel: u8) {
        let pending = std::mem::replace(&mut self.released_under_pedal, [None; 128]);
        let Some(bank) = self.shared_bank() else {
            return;
        };
        for (key, release) in (0..=127_u8).zip(pending) {
            if let Some(release) = release {
                self.start_release_regions(&bank, key, channel, release, |trigger| {
                    trigger == Trigger::Release
                });
            }
        }
    }

    /// Starts the regions of a trigger that `starts` accepts (the release triggers) that
    /// `release` of `key` selects, in the patch the note played, and whose gates the
    /// controllers of `channel` open now.
    fn start_release_regions(
        &mut self,
        bank: &SampleBank,
        key: u8,
        channel: u8,
        release: KeyRelease,
        starts: fn(Trigger) -> bool,
    ) {
        let controllers = self.controllers_of(channel, release.articulation.unwrap_or(self.forced));
        let start = Start::Key {
            key,
            velocity: release.velocity,
            channel,
            dynamics: release.dynamics,
            articulation: release.articulation,
            joined: None,
        };
        for (index, region) in bank.regions.iter().enumerate() {
            if region.member == release.member
                && starts(region.trigger)
                && matches_note(region, key, release.velocity, release.switch)
                && region.gates_open(&controllers)
            {
                self.start_voice(bank, index, region, start);
            }
        }
    }

    /// Silences the voices that exclusive group `group` switches off, each over the fade its
    /// region's `off_mode` gives ([`off_fade`]). A switched-off voice is off: the sustain pedal
    /// no longer holds it, so lifting the pedal does not cut a long `off_time` fade short, and
    /// no legato transition fades it again. A voice that a legato transition fades out keeps
    /// its crossfade.
    fn choke(&mut self, bank: &SampleBank, group: u32) {
        if group == 0 {
            return;
        }
        let rate = self.spec.sample_rate;
        let scale = self.settings.map_or(1.0, |s| s.release_scale);
        for voice in &mut self.voices {
            if voice.active && voice.off_by == group && voice.transition != Transition::Leaving {
                voice.held_by_pedal = false;
                voice.transition = Transition::Untouched;
                start_release(voice, rate, off_fade(bank, voice.region, scale));
            }
        }
    }

    fn start_voice(&mut self, bank: &SampleBank, index: usize, region: &Region, start: Start) {
        let Some(sample) = bank.samples.get(region.sample) else {
            return;
        };
        if sample.frames() == 0 || self.spec.sample_rate == 0 {
            return;
        }
        let (key, pitch, velocity_gain, channel, dynamics, articulation, joined) = match start {
            Start::Key {
                key,
                velocity,
                channel,
                dynamics,
                articulation,
                joined,
            } => (
                Some(key),
                key,
                region.velocity_gain(velocity),
                channel,
                dynamics,
                articulation,
                joined,
            ),
            Start::Controller { channel } => (
                None,
                region.pitch_keycenter,
                1.0,
                channel,
                self.start_dynamics(),
                None,
                None,
            ),
        };
        let rate = f64::from(self.spec.sample_rate);
        let cents =
            (f64::from(pitch) - f64::from(region.pitch_keycenter)) * 100.0 + region.tune_cents;
        let step = f64::from(sample.rate) / rate * (cents / 1200.0).exp2();
        let gain = db_to_amp(region.volume_db) * (region.amplitude / 100.0) * velocity_gain;
        if gain.is_nan() || gain <= 0.0 {
            // A voice that would stay silent is not started. A key press has already switched
            // off the region's exclusive group and counted its round robin (`note_on`). The
            // controllers are not asked: they can still raise a voice they start silent.
            return;
        }
        // The dynamics gain is exactly 1 for a voice a key press starts now; a release region
        // starts where the note's level has moved since its press.
        let controllers = self.controllers_of(channel, articulation.unwrap_or(self.forced));
        let live_gain = region.controller_gain(&controllers)
            * dynamics_gain(self.drive(), self.dynamics, dynamics);
        // A note the overlap legato joins fades in over the crossfade, along the equal-power
        // curve, from past its sample's attack.
        let (pos, attack, curve) = match joined {
            Some(join) => (
                joined_position(region, sample, join.skip),
                join.crossfade,
                Curve::EqualPower,
            ),
            None => (
                frames_f64(region.offset),
                region.attack.max(MIN_ATTACK),
                Curve::Linear,
            ),
        };
        // A key press's sound is what a legato transition leaves; a release region is the
        // note's tail and plays on, as it does past a later release of its key.
        let transition = if key.is_some() && !region.trigger.is_release() {
            Transition::Joinable
        } else {
            Transition::Untouched
        };
        self.clock = self.clock.wrapping_add(1);
        let voice = Voice {
            active: true,
            region: index,
            sample: region.sample,
            key,
            channel,
            pos,
            step,
            gain,
            dynamics,
            articulation,
            live_gain,
            live_step: 0.0,
            live_target: live_gain,
            level: 0.0,
            curve,
            attack_step: 1.0 / (attack * rate),
            release_step: 0.0,
            releasing: false,
            held_by_pedal: false,
            transition,
            off_by: region.off_by,
            age: self.clock,
        };
        let slot = self.free_voice();
        if let Some(slot) = self.voices.get_mut(slot) {
            *slot = voice;
        }
    }

    /// The voice to start on: an idle one, else the oldest releasing one, else the oldest.
    fn free_voice(&self) -> usize {
        let mut best = (0, (true, u64::MAX));
        for (i, voice) in self.voices.iter().enumerate() {
            if !voice.active {
                return i;
            }
            let rank = (!voice.releasing, voice.age);
            if rank < best.1 {
                best = (i, rank);
            }
        }
        best.0
    }

    fn render(&mut self, out: &mut [[f64; 2]], settings: Settings) {
        let Some(bank) = bank_of(self.setup.as_ref()) else {
            return;
        };
        let bend = (self.bend / 12.0).exp2();
        let master = settings.gain * self.volume_cc * self.expression_cc;
        // Equal-power balance, unity at the centre.
        let angle = (settings.pan + 1.0) * std::f64::consts::FRAC_PI_4;
        let (pan_l, pan_r) = (
            angle.cos() * std::f64::consts::SQRT_2,
            angle.sin() * std::f64::consts::SQRT_2,
        );
        for voice in &mut self.voices {
            if !voice.active {
                continue;
            }
            let (Some(region), Some(sample)) = (
                bank.regions.get(voice.region),
                bank.samples.get(voice.sample),
            ) else {
                voice.active = false;
                continue;
            };
            let end = play_end(region, sample);
            let looping = loops(region, sample);
            let step = voice.step * bend;
            for frame in out.iter_mut() {
                if voice.releasing {
                    voice.level -= voice.release_step;
                    if voice.level <= 0.0 {
                        voice.active = false;
                        break;
                    }
                } else if voice.level < 1.0 {
                    voice.level = (voice.level + voice.attack_step).min(1.0);
                    // At full level the curve and the level agree: a joined note plays and
                    // releases as any note from here.
                    if voice.level >= 1.0 {
                        voice.curve = Curve::Linear;
                    }
                }
                if looping && voice.pos >= frames_f64(region.loop_end) {
                    voice.pos -= frames_f64(region.loop_end.saturating_sub(region.loop_start));
                }
                if voice.pos >= frames_f64(end) {
                    voice.active = false;
                    break;
                }
                let [l, r] = read_frame(
                    sample.channels,
                    &sample.data,
                    voice.pos,
                    if looping {
                        Some((region.loop_start, region.loop_end))
                    } else {
                        None
                    },
                    end,
                );
                voice.live_gain += voice.live_step;
                let amp = voice.gain * voice.live_gain * voice.heard() * master;
                frame[0] += l * amp * pan_l;
                frame[1] += r * amp * pan_r;
                voice.pos += step;
            }
        }
    }
}

/// The loaded patch of `setup`, borrowed through the field alone, so the caller can change the
/// voices meanwhile.
fn bank_of(setup: Option<&Arc<SamplerSetup>>) -> Option<&SampleBank> {
    setup.map(|setup| &*setup.bank)
}

/// The `dynamics` parameter in `slots`, 0 to 1: its default 1 when the slot is missing or not
/// a number.
fn dynamics_of(slots: &ParamSlots) -> f64 {
    float(slots, DYNAMICS)
        .filter(|dynamics| !dynamics.is_nan())
        .map_or(1.0, |dynamics| dynamics.clamp(0.0, 1.0))
}

/// The velocity a key pressed at `velocity` plays with at `dynamics` (0 to 1):
/// `max(1, round(velocity · dynamics))`, so 1 keeps every velocity and 0 plays the softest.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the product lies between 1 and the velocity, a u8, once rounded and raised to 1"
)]
fn scaled_velocity(velocity: u8, dynamics: f64) -> u8 {
    (f64::from(velocity) * dynamics).round().max(1.0) as u8
}

/// The controller value of `dynamics` (0 to 1): `round(dynamics · 127)`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to 0..=127 first"
)]
fn controller_value(dynamics: f64) -> u8 {
    (dynamics * 127.0).round().clamp(0.0, 127.0) as u8
}

/// The gain the `dynamics` parameter at `now` gives a sounding voice that started at `start`
/// (at least [`MIN_DYNAMICS`]): `(now / start)²`, with `now` at least [`MIN_DYNAMICS`] too and
/// the gain at most [`MAX_DYNAMICS_GAIN`]. So an unchanged value gives exactly 1, and a swell
/// changes the level of a held note, whose layer its velocity chose at the press. 1 for a
/// preset whose dynamics drive a controller, which the controller's crossfades follow instead.
fn dynamics_gain(drive: DynamicsDrive, now: f64, start: f64) -> f64 {
    if let DynamicsDrive::Controller(_) = drive {
        return 1.0;
    }
    let ratio = now.max(MIN_DYNAMICS) / start;
    (ratio * ratio).min(MAX_DYNAMICS_GAIN)
}

/// The release time of region `region` in seconds, scaled.
fn release_time(bank: Option<&SampleBank>, region: usize, scale: f64) -> f64 {
    let release = bank
        .and_then(|bank| bank.regions.get(region))
        .map_or(0.0, |r| r.release);
    release.max(MIN_RELEASE) * scale
}

/// The fade-out in seconds of a voice of region `region` that an exclusive group switches off,
/// as the region's `off_mode` says in the SFZ format: `fast` is the fixed choke, `normal` the
/// region's release exactly as a key release plays it (scaled by the release parameter), and
/// `time` its `off_time`, which the format makes independent of the release, so the release
/// parameter does not scale it either. Every fade lasts at least the shortest fade-out.
fn off_fade(bank: &SampleBank, region: usize, scale: f64) -> f64 {
    match bank.regions.get(region).map(|region| region.off_mode) {
        Some(OffMode::Normal) => release_time(Some(bank), region, scale),
        Some(OffMode::Time(seconds)) => seconds.max(MIN_RELEASE),
        Some(OffMode::Fast) | None => CHOKE_RELEASE,
    }
}

/// A frame count or position as a float; sample lengths stay far below 2^52.
#[expect(
    clippy::cast_precision_loss,
    reason = "frame counts of a sample are far below 2^52"
)]
fn frames_f64(frames: usize) -> f64 {
    frames as f64
}

/// A count of frames in a block as the sampler's frame clock counts them; a block holds far
/// fewer than `u64::MAX` frames.
fn frame_count(frames: usize) -> u64 {
    u64::try_from(frames).unwrap_or(u64::MAX)
}

/// The whole frame below a non-negative position.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the position is non-negative and below the sample length"
)]
fn floor_index(pos: f64) -> usize {
    pos as usize
}

/// Whether `region` plays for a press (or, for a release region, a release) of `key` at
/// `velocity` with `switch` as the last keyswitch. A region that no key starts never does.
fn matches_note(region: &Region, key: u8, velocity: u8, switch: Option<u8>) -> bool {
    region.on_key
        && (region.key_lo..=region.key_hi).contains(&key)
        && (region.vel_lo..=region.vel_hi).contains(&velocity)
        && in_switch(region, switch)
}

/// Whether `region` plays with `switch` as the last keyswitch: it names none, or that one.
fn in_switch(region: &Region, switch: Option<u8>) -> bool {
    region.sw_last.is_none_or(|sw| Some(sw) == switch)
}

/// Whether `region` is the round-robin position that `counter` selects.
fn in_sequence(region: &Region, counter: u32) -> bool {
    region.seq_length <= 1
        || counter.checked_rem(region.seq_length).map(|n| n + 1) == Some(region.seq_position)
}

/// The frame where `region` stops playing `sample`: its `end`, within the sample, or the
/// sample's end when it gives none.
fn play_end(region: &Region, sample: &Sample) -> usize {
    let frames = sample.frames();
    if region.end == 0 {
        frames
    } else {
        region.end.min(frames)
    }
}

/// Whether `region` loops in `sample`: a loop mode that loops, around a loop that lies in the
/// sample.
fn loops(region: &Region, sample: &Sample) -> bool {
    matches!(region.loop_mode, LoopMode::Continuous | LoopMode::Sustain)
        && region.loop_end > region.loop_start
        && region.loop_end <= sample.frames()
}

/// Where a note that the overlap legato joins starts in `region`'s `sample`, in source frames:
/// `skip` seconds past the region's offset, past the sample's attack. A start at or past the
/// loop end of a looping region starts at the loop start, where the sustain goes on; one at or
/// past the end of a region that does not loop starts at the offset, without the skip, so the
/// note still sounds.
fn joined_position(region: &Region, sample: &Sample, skip: f64) -> f64 {
    let offset = frames_f64(region.offset);
    let skipped = offset + skip * f64::from(sample.rate);
    if loops(region, sample) {
        if skipped >= frames_f64(region.loop_end) {
            return frames_f64(region.loop_start);
        }
    } else if skipped >= frames_f64(play_end(region, sample)) {
        return offset;
    }
    skipped
}

/// Starts a linear fade-out of `seconds` from the voice's current level.
fn start_release(voice: &mut Voice, sample_rate: u32, seconds: f64) {
    if voice.releasing {
        // A choke shortens a release in progress, never lengthens it.
        let quick = voice.level / (seconds * f64::from(sample_rate)).max(1.0);
        voice.release_step = voice.release_step.max(quick);
        return;
    }
    voice.releasing = true;
    voice.release_step =
        voice.level.max(f64::EPSILON) / (seconds * f64::from(sample_rate)).max(1.0);
}

/// The interpolated stereo frame at source position `pos`.
fn read_frame(
    channels: usize,
    data: &[f32],
    pos: f64,
    looping: Option<(usize, usize)>,
    end: usize,
) -> [f64; 2] {
    let first = floor_index(pos);
    let frac = pos - frames_f64(first);
    let mut next = first + 1;
    match looping {
        Some((start, stop)) if next >= stop => next = start + (next - stop),
        _ if next >= end => next = first,
        _ => {}
    }
    let at = |frame: usize, channel: usize| -> f64 {
        data.get(frame * channels + channel)
            .map_or(0.0, |v| f64::from(*v))
    };
    let left = at(first, 0) * (1.0 - frac) + at(next, 0) * frac;
    if channels < 2 {
        return [left, left];
    }
    [left, at(first, 1) * (1.0 - frac) + at(next, 1) * frac]
}

impl Processor for Sampler {
    fn prepare(&mut self, spec: ProcessSpec) {
        self.spec = spec;
        self.rest();
        self.apply_params();
    }

    fn set_params(&mut self, params: &ParamSlots) {
        if super::store_params(&mut self.params, params) {
            self.apply_params();
        }
    }

    fn set_state(&mut self, state: &DeviceState) {
        self.setup.clone_from(&state.sampler);
        self.rest();
    }

    fn process(&mut self, buffer: &mut [[f64; 2]], ctx: &BlockContext<'_>) {
        // An instrument ignores its input.
        buffer.fill([0.0; 2]);
        let Some(settings) = self.settings else {
            return;
        };
        let n = buffer.len();
        self.aim_live_gains(n);
        let events = ctx.events;
        let first = self.frame;
        let (mut next, mut pos) = (0, 0);
        while pos < n {
            self.frame = first.wrapping_add(frame_count(pos));
            while let Some(event) = events.get(next)
                && event.offset <= pos
            {
                self.apply(event);
                next += 1;
            }
            let until = events.get(next).map_or(n, |event| event.offset);
            let end = until.max(pos + 1).min(n);
            if let Some(segment) = buffer.get_mut(pos..end) {
                self.render(segment, settings);
            }
            pos = end;
        }
        self.frame = first.wrapping_add(frame_count(n));
        self.settle_live_gains();
    }

    fn reset(&mut self) {
        self.rest();
    }

    fn tail_seconds(&self) -> f64 {
        bank_of(self.setup.as_ref()).map_or(0.0, |bank| bank.tail_seconds)
    }

    fn report(&self) -> Report {
        let voices = self.voices.iter().filter(|v| v.active).count();
        Report::new(&[(
            "voices",
            f64::from(u32::try_from(voices).unwrap_or(u32::MAX)),
        )])
    }
}

impl Sampler {
    /// Block start: aims the live gain of every sounding voice at the gain the controllers of
    /// its channel give its region now, with the value its articulation forces, times the gain
    /// the `dynamics` parameter gives it now, reached linearly on the last of the block's
    /// `frames`. The parameter's ramp reuses the controllers' (DES-01): one ramp per voice,
    /// exact at the block's end, and a change of either never clicks.
    fn aim_live_gains(&mut self, frames: usize) {
        let (drive, now) = (self.drive(), self.dynamics);
        let Some(bank) = bank_of(self.setup.as_ref()) else {
            return;
        };
        let frames = frames_f64(frames.max(1));
        let forced = self.forced;
        for voice in self.voices.iter_mut().filter(|voice| voice.active) {
            let Some(region) = bank.regions.get(voice.region) else {
                continue;
            };
            let controllers = self
                .controllers
                .get(usize::from(voice.channel))
                .unwrap_or(&bank.controllers);
            // A voice of a note with an articulation of its own reads the controllers with it.
            let forced = voice.articulation.unwrap_or(forced);
            voice.live_target = region.controller_gain(&forced.read(*controllers))
                * dynamics_gain(drive, now, voice.dynamics);
            voice.live_step = (voice.live_target - voice.live_gain) / frames;
        }
    }

    /// Block end: every voice's live gain is exactly at its target, without the rounding of the
    /// ramp.
    fn settle_live_gains(&mut self) {
        for voice in &mut self.voices {
            voice.live_gain = voice.live_target;
            voice.live_step = 0.0;
        }
    }

    /// Takes the stored parameters: the settings, the `articulation`, `mute` and `vibrato`
    /// choices, whose target is applied only when they change it ([`Self::choose`]), and the
    /// `dynamics` value, which a preset whose dynamics drive a controller writes into it only
    /// when the value changes. So a change of another parameter (an automated volume) leaves
    /// what the clips selected since alone.
    fn apply_params(&mut self) {
        if let Some(settings) = Settings::from_slots(&self.params) {
            self.settings = Some(settings);
        }
        let articulation = choice(&self.params, ARTICULATION).unwrap_or(SamplerMapping::AUTO);
        let passage = Passage {
            mute: choice(&self.params, MUTE).unwrap_or(0),
            vibrato: choice(&self.params, VIBRATO).unwrap_or(0),
        };
        self.choose(articulation, passage);
        let dynamics = dynamics_of(&self.params);
        if dynamics.to_bits() != self.dynamics.to_bits() {
            self.dynamics = dynamics;
            self.drive_controller();
        }
    }
}
