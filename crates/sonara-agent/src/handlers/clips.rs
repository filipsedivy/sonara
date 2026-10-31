//! Clip and note tools (the prototype's "clips and notes" section): MIDI clips, their notes and
//! (agent API 1.1) their controller events, and moving, editing, splitting, duplicating and
//! deleting clips.

use std::collections::HashSet;

use serde_json::json;
use sonara_core::commands::clip::{
    AddNotes, CreateMidiClip as CreateMidiClipCommand, DeleteClips as DeleteClipsCommand,
    DeleteNotes, DuplicateClip as DuplicateClipCommand, EditNotes, MoveClip as MoveClipCommand,
    NoteEdit, ReplaceNotes, SetClipProperties, SetControlEvents, SplitClip as SplitClipCommand,
    TrimClip,
};
use sonara_core::commands::{Command, Transaction};
use sonara_core::error::DomainError;
use sonara_core::ids::{self, IdGenerator};
use sonara_core::midi::{ControlKind, MidiControlEvent, MidiNote};
use sonara_core::model::clip::{Clip, Fade, FadeCurve as ClipFadeCurve};
use sonara_midi::transforms::{self, HumanizeOptions, QuantizeOptions, VelocityScale};

use super::positions::Positions;
use super::tracks::commit_steps;
use crate::context::AgentContext;
use crate::contract::args::{
    AddMidiNotes, CreateMidiClip, DeleteClips, DuplicateClip, EditClip, EditMidiNotes, FadeCurve,
    HumanizeSpec, MoveClip, NoteEditInput, NoteInput, QuantizeSpec, SplitClip,
    WriteMidiControllers,
};
use crate::conversion;
use crate::registry::ToolResult;

/// `create_midi_clip`: a MIDI clip on an instrument track, optionally with notes (starts in
/// beats from the clip start). The notes draw their IDs before the clip.
pub(super) fn create_midi_clip(ctx: &AgentContext, args: CreateMidiClip) -> ToolResult {
    let CreateMidiClip {
        dry_run,
        track_id,
        start,
        length,
        name,
        notes,
        loop_length,
        color,
    } = args;
    let positions = Positions::new(ctx);
    let start = positions.beats(&start)?;
    let length = positions.length_beats(&length, start)?;
    let loop_length = loop_length
        .map(|loop_length| positions.length_beats(&loop_length, start))
        .transpose()?;
    let mut ids = ctx.app().ids();
    let notes = conversion::notes(&notes, &mut ids)?;
    let command = CreateMidiClipCommand {
        track_id,
        start,
        length,
        name,
        notes,
        loop_length,
        color,
        clip_id: ids.new_id(ids::CLIP),
    };
    Ok(ctx.commit(&command, dry_run)?)
}

/// `add_midi_notes`: adds notes to a MIDI clip; the result lists their new IDs.
pub(super) fn add_midi_notes(ctx: &AgentContext, args: AddMidiNotes) -> ToolResult {
    let AddMidiNotes {
        dry_run,
        clip_id,
        notes,
    } = args;
    let notes = conversion::notes(&notes, &mut ctx.app().ids())?;
    Ok(ctx.commit(&AddNotes { clip_id, notes }, dry_run)?)
}

/// `write_midi_controllers` (agent API 1.1): replaces every event of one MIDI controller (CC) in
/// a MIDI clip with the given events, as one undo step (`Edit Controllers`). Events of other
/// controllers and of other kinds (pitch bend, channel pressure) stay.
///
/// The controller is matched by its number on every channel, as `SetControlEvents` matches it;
/// the new events carry the given channel.
pub(super) fn write_midi_controllers(ctx: &AgentContext, args: WriteMidiControllers) -> ToolResult {
    let WriteMidiControllers {
        dry_run,
        clip_id,
        controller,
        events,
        channel,
    } = args;
    let events = events
        .iter()
        .map(|event| {
            MidiControlEvent::new(
                event.beat,
                ControlKind::Cc,
                i64::from(event.value),
                i64::from(controller),
                i64::from(channel),
            )
        })
        .collect::<Result<Vec<_>, DomainError>>()?;
    let command = SetControlEvents {
        clip_id,
        events,
        // The whole clip: every event of the controller is replaced.
        start: f64::NEG_INFINITY,
        end: f64::INFINITY,
    };
    Ok(ctx.commit(&command, dry_run)?)
}

/// The note edits of `edit_midi_notes` before the transforms: a replacement of every note,
/// per-note edits and deletions, each only when asked for.
struct NoteSteps {
    replace: Option<ReplaceNotes>,
    edit: Option<EditNotes>,
    delete: Option<DeleteNotes>,
}

impl NoteSteps {
    /// The steps of `clip_id`, converted in the prototype's order: the replacement notes draw
    /// their IDs before the edits' pitches are read.
    fn new(
        ctx: &AgentContext,
        clip_id: &str,
        replace_all: Option<&[NoteInput]>,
        edits: &[NoteEditInput],
        delete_note_ids: Vec<String>,
    ) -> Result<Self, DomainError> {
        let replace = match replace_all {
            None => None,
            Some(notes) => Some(ReplaceNotes {
                clip_id: clip_id.to_owned(),
                notes: conversion::notes(notes, &mut ctx.app().ids())?,
                name: "Replace Notes".to_owned(),
            }),
        };
        let edit = if edits.is_empty() {
            None
        } else {
            let edits = edits
                .iter()
                .map(|edit| {
                    Ok(NoteEdit {
                        note_id: edit.note_id.clone(),
                        pitch: edit
                            .pitch
                            .as_ref()
                            .map(conversion::pitch)
                            .transpose()?
                            .map(i64::from),
                        start: edit.start,
                        length: edit.length,
                        velocity: edit.velocity.map(i64::from),
                        channel: None,
                    })
                })
                .collect::<Result<Vec<_>, DomainError>>()?;
            Some(EditNotes {
                clip_id: clip_id.to_owned(),
                edits,
            })
        };
        let delete = (!delete_note_ids.is_empty()).then(|| DeleteNotes {
            clip_id: clip_id.to_owned(),
            note_ids: delete_note_ids,
        });
        Ok(Self {
            replace,
            edit,
            delete,
        })
    }

    /// The steps as commands, in the prototype's order.
    fn commands(&self) -> Vec<Box<dyn Command>> {
        let mut commands: Vec<Box<dyn Command>> = Vec::new();
        if let Some(replace) = &self.replace {
            commands.push(Box::new(replace.clone()));
        }
        if let Some(edit) = &self.edit {
            commands.push(Box::new(edit.clone()));
        }
        if let Some(delete) = &self.delete {
            commands.push(Box::new(delete.clone()));
        }
        commands
    }
}

/// The transforms of `edit_midi_notes`, applied in the prototype's order.
struct NoteTransforms {
    transpose: Option<i8>,
    quantize: Option<QuantizeSpec>,
    humanize: Option<HumanizeSpec>,
    velocity_scale: Option<f64>,
}

impl NoteTransforms {
    /// Whether any transform was asked for (a transposition by 0 counts).
    fn any(&self) -> bool {
        self.transpose.is_some()
            || self.quantize.is_some()
            || self.humanize.is_some()
            || self.velocity_scale.is_some()
    }

    /// `notes` transposed, quantized, humanized and velocity-scaled, as asked.
    fn apply(&self, mut notes: Vec<MidiNote>) -> Result<Vec<MidiNote>, DomainError> {
        // `if a.transpose:` – a transposition by 0 leaves the notes as they are.
        if let Some(semitones) = self.transpose.filter(|semitones| *semitones != 0) {
            notes = transforms::transpose(&notes, i64::from(semitones))?;
        }
        if let Some(quantize) = &self.quantize {
            let options = QuantizeOptions {
                strength: quantize.strength,
                swing: quantize.swing,
                ends: quantize.ends,
            };
            notes = transforms::quantize(&notes, quantize.grid, &options)?;
        }
        if let Some(humanize) = &self.humanize {
            let options = HumanizeOptions {
                timing: humanize.timing,
                velocity: i64::from(humanize.velocity),
                length: 0.0,
                seed: humanize.seed,
            };
            notes = transforms::humanize(&notes, &options)?;
        }
        if let Some(factor) = self.velocity_scale {
            let options = VelocityScale {
                factor,
                ..VelocityScale::default()
            };
            notes = transforms::scale_velocity(&notes, &options)?;
        }
        Ok(notes)
    }
}

/// `edit_midi_notes`: per-note edits, deletions, transposition, quantization, humanization,
/// velocity scaling, or a replacement of every note, as one undo step (`Edit Notes`).
///
/// The transforms apply to the selected notes (`note_ids`; every note when empty) as the other
/// edits leave them: the session simulates those edits first (`session.simulate`). The
/// transformed notes follow the others in the clip.
pub(super) fn edit_midi_notes(ctx: &AgentContext, args: EditMidiNotes) -> ToolResult {
    let EditMidiNotes {
        dry_run,
        clip_id,
        edits,
        delete_note_ids,
        transpose,
        quantize,
        humanize,
        velocity_scale,
        note_ids,
        replace_all,
    } = args;
    let project = ctx.project()?;
    if !matches!(project.clip(&clip_id)?.1, Clip::Midi(_)) {
        return Err(DomainError::invalid("edit_midi_notes needs a MIDI clip").into());
    }
    let steps = NoteSteps::new(
        ctx,
        &clip_id,
        replace_all.as_deref(),
        &edits,
        delete_note_ids,
    )?;
    let mut commands = steps.commands();
    let transforms = NoteTransforms {
        transpose,
        quantize,
        humanize,
        velocity_scale,
    };
    if transforms.any() {
        let preview = if commands.is_empty() {
            project
        } else {
            let transaction = Transaction {
                commands: steps.commands(),
                name: "Edit Notes".to_owned(),
            };
            ctx.app().session().lock().simulate(&transaction)?
        };
        let current: &[MidiNote] = match preview.clip(&clip_id)?.1 {
            Clip::Midi(clip) => &clip.notes,
            // The edits above keep the clip a MIDI clip.
            Clip::Audio(_) => &[],
        };
        // `set(a.note_ids) if a.note_ids else` every note.
        let selected: Option<HashSet<&str>> = note_ids
            .as_ref()
            .filter(|ids| !ids.is_empty())
            .map(|ids| ids.iter().map(String::as_str).collect());
        let is_selected = |note: &MidiNote| {
            selected
                .as_ref()
                .is_none_or(|selected| selected.contains(note.id.as_str()))
        };
        let (chosen, others): (Vec<MidiNote>, Vec<MidiNote>) =
            current.iter().cloned().partition(|note| is_selected(note));
        let chosen = transforms.apply(chosen)?;
        commands.push(Box::new(ReplaceNotes {
            clip_id,
            notes: others.into_iter().chain(chosen).collect(),
            name: "Transform Notes".to_owned(),
        }));
    }
    commit_steps(ctx, commands, "Edit Notes", dry_run)
}

/// `move_clip`: moves a clip in time and/or to another compatible track.
pub(super) fn move_clip(ctx: &AgentContext, args: MoveClip) -> ToolResult {
    let MoveClip {
        dry_run,
        clip_id,
        start,
        track_id,
    } = args;
    let start = start
        .map(|start| Positions::new(ctx).beats(&start))
        .transpose()?;
    let command = MoveClipCommand {
        clip_id,
        start,
        track_id,
    };
    Ok(ctx.commit(&command, dry_run)?)
}

fn fade_curve(curve: FadeCurve) -> ClipFadeCurve {
    match curve {
        FadeCurve::Linear => ClipFadeCurve::Linear,
        FadeCurve::EqualPower => ClipFadeCurve::EqualPower,
        FadeCurve::Exponential => ClipFadeCurve::Exponential,
        FadeCurve::SCurve => ClipFadeCurve::SCurve,
    }
}

/// `edit_clip`: renames, mutes, sets gain and fades (audio), trims the edges or sets a loop
/// length (MIDI), as one undo step (`Edit Clip`): the trim first, then the properties.
///
/// A loop length the call does not give leaves the clip's as it is (the prototype's `_UNSET`;
/// the call never turns looping off).
pub(super) fn edit_clip(ctx: &AgentContext, args: EditClip) -> ToolResult {
    let EditClip {
        dry_run,
        clip_id,
        name,
        muted,
        gain_db,
        fade_in_seconds,
        fade_out_seconds,
        fade_curve: curve,
        trim_start,
        trim_end,
        loop_length,
    } = args;
    let positions = Positions::new(ctx);
    let clip_start = positions.project()?.clip(&clip_id)?.1.start();
    let curve = curve.map_or(ClipFadeCurve::Linear, fade_curve);
    let fade_in = fade_in_seconds
        .map(|seconds| Fade::new(seconds, curve))
        .transpose()?;
    let fade_out = fade_out_seconds
        .map(|seconds| Fade::new(seconds, curve))
        .transpose()?;
    let loop_length = loop_length
        .map(|length| positions.length_beats(&length, clip_start))
        .transpose()?;
    let properties = SetClipProperties {
        clip_id: clip_id.clone(),
        name,
        gain_db,
        fade_in,
        fade_out,
        muted,
        color: None,
        loop_length: loop_length.map(Some),
    };
    let has_properties = properties.name.is_some()
        || properties.muted.is_some()
        || properties.gain_db.is_some()
        || properties.fade_in.is_some()
        || properties.fade_out.is_some()
        || properties.loop_length.is_some();
    let mut commands: Vec<Box<dyn Command>> = Vec::new();
    if trim_start.is_some() || trim_end.is_some() {
        let start = trim_start
            .map(|start| positions.beats(&start))
            .transpose()?;
        let end = trim_end.map(|end| positions.beats(&end)).transpose()?;
        commands.push(Box::new(TrimClip {
            clip_id,
            start,
            end,
        }));
    }
    if has_properties {
        commands.push(Box::new(properties));
    }
    commit_steps(ctx, commands, "Edit Clip", dry_run)
}

/// `split_clip`: splits a clip in two at a position.
pub(super) fn split_clip(ctx: &AgentContext, args: SplitClip) -> ToolResult {
    let SplitClip {
        dry_run,
        clip_id,
        at,
    } = args;
    let at = Positions::new(ctx).beats(&at)?;
    let command = SplitClipCommand {
        clip_id,
        at,
        new_clip_id: ctx.app().ids().new_id(ids::CLIP),
    };
    Ok(ctx.commit(&command, dry_run)?)
}

/// `duplicate_clip`: copies a clip one or more times, back to back, as one undo step
/// (`Duplicate Clip`). The result lists the copies (`clip_ids`).
pub(super) fn duplicate_clip(ctx: &AgentContext, args: DuplicateClip) -> ToolResult {
    let DuplicateClip {
        dry_run,
        clip_id,
        times,
        start,
        track_id,
    } = args;
    let positions = Positions::new(ctx);
    let (_, clip) = positions.project()?.clip(&clip_id)?;
    let length = clip.length();
    let first = match &start {
        None => clip.end(),
        Some(start) => positions.beats(start)?,
    };
    let mut ids = ctx.app().ids();
    let copies: Vec<DuplicateClipCommand> = (0..times)
        .map(|i| DuplicateClipCommand {
            clip_id: clip_id.clone(),
            start: Some(first + f64::from(i) * length),
            track_id: track_id.clone(),
            new_clip_id: ids.new_id(ids::CLIP),
        })
        .collect();
    let clip_ids: Vec<String> = copies.iter().map(|c| c.new_clip_id.clone()).collect();
    let commands: Vec<Box<dyn Command>> = copies
        .into_iter()
        .map(|c| Box::new(c) as Box<dyn Command>)
        .collect();
    let transaction = Transaction {
        commands,
        name: "Duplicate Clip".to_owned(),
    };
    let mut out = ctx.commit(&transaction, dry_run)?;
    out["clip_ids"] = json!(clip_ids);
    Ok(out)
}

/// `delete_clips`: deletes clips; their media stays in the project.
pub(super) fn delete_clips(ctx: &AgentContext, args: DeleteClips) -> ToolResult {
    let command = DeleteClipsCommand {
        clip_ids: args.clip_ids,
    };
    Ok(ctx.commit(&command, args.dry_run)?)
}
