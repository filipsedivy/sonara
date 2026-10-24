//! Project templates: starting points built from ordinary commands
//! (`sonara.application.services.templates`).
//!
//! A template builds its commands with fresh IDs in the prototype's construction order, so a
//! sequential ID source reproduces the prototype's projects exactly.

use sonara_core::commands::device::AddDevice;
use sonara_core::commands::track::{AddSend, CreateTrack};
use sonara_core::commands::{Command, CommandContext, run};
use sonara_core::error::{DomainError, ErrorCode, Result};
use sonara_core::ids::{self, IdGenerator};
use sonara_core::model::JsonMap;
use sonara_core::model::project::Project;
use sonara_core::model::track::TrackKind;
use sonara_core::pyfmt::str_repr;

/// A project template.
#[derive(Debug, Clone, Copy)]
pub struct Template {
    /// Stable ID (`empty`, `electronic`, `band`, `orchestral`).
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// What the template sets up.
    pub description: &'static str,
    build: fn(&mut dyn IdGenerator) -> Vec<Box<dyn Command>>,
}

/// The templates, in the prototype's order.
pub const TEMPLATES: [Template; 4] = [
    Template {
        id: "empty",
        name: "Empty",
        description: "No tracks.",
        build: |_| Vec::new(),
    },
    Template {
        id: "electronic",
        name: "Electronic",
        description: "Drums, bass (sidechained to the kick), chords, lead, reverb/delay buses, \
                      master limiter.",
        build: electronic,
    },
    Template {
        id: "band",
        name: "Band",
        description: "Drums, bass, keys, guitar and vocal audio tracks with vocal chain, plate \
                      reverb.",
        build: band,
    },
    Template {
        id: "orchestral",
        name: "Orchestral",
        description: "Strings, brass, choir and cinematic percussion with a large hall.",
        build: orchestral,
    },
];

/// The template with `id`.
///
/// # Errors
///
/// [`ErrorCode::NotFound`]: `unknown template '…'`, with the available IDs.
pub fn template(id: &str) -> Result<&'static Template> {
    TEMPLATES.iter().find(|t| t.id == id).ok_or_else(|| {
        let mut available: Vec<&str> = TEMPLATES.iter().map(|t| t.id).collect();
        available.sort_unstable();
        DomainError::new(
            ErrorCode::NotFound,
            format!("unknown template {}", str_repr(id)),
        )
        .with_detail("available", available)
    })
}

/// `project` with the template's commands applied.
///
/// # Errors
///
/// [`ErrorCode::NotFound`] for an unknown template, or a command error.
pub fn apply_template(
    project: &Project,
    template_id: &str,
    ctx: &CommandContext<'_>,
    ids: &mut dyn IdGenerator,
) -> Result<Project> {
    let template = template(template_id)?;
    let mut project = project.clone();
    for command in (template.build)(ids) {
        project = run(command.as_ref(), &project, ctx)?.project;
    }
    Ok(project)
}

// -- building blocks --------------------------------------------------------------------

/// A new track; like the prototype, it takes an instrument ID even when it has no instrument.
fn track(
    ids: &mut dyn IdGenerator,
    name: &str,
    kind: TrackKind,
    instrument: Option<(&str, &str)>,
    color: &str,
) -> CreateTrack {
    let track_id = ids.new_id(ids::TRACK);
    let instrument_id = ids.new_id(ids::DEVICE);
    CreateTrack {
        name: name.to_owned(),
        kind,
        index: None,
        color: Some(color.to_owned()),
        output_id: ids::MASTER_ID.to_owned(),
        instrument_plugin: instrument.map(|(plugin, _)| plugin.to_owned()),
        instrument_preset: instrument.map(|(_, preset)| preset.to_owned()),
        parent_id: None,
        track_id,
        instrument_id,
    }
}

fn instrument_track(
    ids: &mut dyn IdGenerator,
    name: &str,
    plugin: &str,
    preset: &str,
    color: &str,
) -> CreateTrack {
    track(
        ids,
        name,
        TrackKind::Instrument,
        Some((plugin, preset)),
        color,
    )
}

/// A new effect on `track_id`, with a preset and optional explicit parameters.
fn effect(
    ids: &mut dyn IdGenerator,
    track_id: &str,
    plugin: &str,
    preset: &str,
    params: &[(&str, f64)],
    sidechain_id: Option<&str>,
) -> AddDevice {
    let params: JsonMap = params
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).into()))
        .collect();
    AddDevice {
        track_id: track_id.to_owned(),
        plugin_id: plugin.to_owned(),
        index: None,
        params,
        preset: Some(preset.to_owned()),
        sidechain_id: sidechain_id.map(str::to_owned),
        name: None,
        device_id: ids.new_id(ids::DEVICE),
    }
}

fn send(ids: &mut dyn IdGenerator, track_id: &str, target_id: &str, level_db: f64) -> AddSend {
    AddSend {
        track_id: track_id.to_owned(),
        target_id: target_id.to_owned(),
        level_db,
        pre_fader: false,
        send_id: ids.new_id(ids::SEND),
    }
}

/// Reverb (and optionally delay) return buses: the commands, the reverb bus and the delay bus.
fn fx_buses(
    ids: &mut dyn IdGenerator,
    reverb_preset: &str,
    delay: bool,
) -> (Vec<Box<dyn Command>>, String, Option<String>) {
    let verb = track(ids, "Reverb", TrackKind::Bus, None, "#5b7db8");
    let verb_id = verb.track_id.clone();
    let verb_fx = effect(
        ids,
        &verb_id,
        "sonara.reverb",
        reverb_preset,
        &[("mix", 1.0)],
        None,
    );
    let mut commands: Vec<Box<dyn Command>> = vec![Box::new(verb), Box::new(verb_fx)];
    let mut delay_id = None;
    if delay {
        let bus = track(ids, "Delay", TrackKind::Bus, None, "#5ba8b8");
        let id = bus.track_id.clone();
        let fx = effect(
            ids,
            &id,
            "sonara.delay",
            "Dotted Eighth",
            &[("mix", 1.0)],
            None,
        );
        commands.push(Box::new(bus));
        commands.push(Box::new(fx));
        delay_id = Some(id);
    }
    (commands, verb_id, delay_id)
}

fn electronic(ids: &mut dyn IdGenerator) -> Vec<Box<dyn Command>> {
    let drums = instrument_track(ids, "Drums", "sonara.drums", "Punchy 909", "#d9644a");
    let bass = instrument_track(ids, "Bass", "sonara.synth.poly", "Analog Bass", "#d9a24a");
    let chords = instrument_track(ids, "Chords", "sonara.synth.poly", "Warm Pad", "#5bb86b");
    let lead = instrument_track(ids, "Lead", "sonara.synth.poly", "Supersaw Lead", "#a45bb8");
    let (fx, verb, delay) = fx_buses(ids, "Concert Hall", true);
    let pump = effect(
        ids,
        &bass.track_id,
        "sonara.dynamics.compressor",
        "Sidechain Pump",
        &[],
        Some(&drums.track_id),
    );
    let chords_verb = send(ids, &chords.track_id, &verb, -8.0);
    let lead_verb = send(ids, &lead.track_id, &verb, -10.0);
    let lead_delay = send(
        ids,
        &lead.track_id,
        delay.as_deref().unwrap_or(&verb),
        -12.0,
    );
    let limiter = effect(
        ids,
        ids::MASTER_ID,
        "sonara.dynamics.limiter",
        "Master -1 dBTP",
        &[],
        None,
    );
    let mut commands: Vec<Box<dyn Command>> = vec![
        Box::new(drums),
        Box::new(bass),
        Box::new(chords),
        Box::new(lead),
    ];
    commands.extend(fx);
    commands.extend([
        Box::new(pump) as Box<dyn Command>,
        Box::new(chords_verb),
        Box::new(lead_verb),
        Box::new(lead_delay),
        Box::new(limiter),
    ]);
    commands
}

fn band(ids: &mut dyn IdGenerator) -> Vec<Box<dyn Command>> {
    let drums = instrument_track(ids, "Drums", "sonara.drums", "Tight House", "#d9644a");
    let bass = instrument_track(ids, "Bass", "sonara.synth.poly", "Sub Bass", "#d9a24a");
    let keys = instrument_track(ids, "Keys", "sonara.synth.poly", "Electric Keys", "#5bb86b");
    let guitar = track(ids, "Guitar", TrackKind::Audio, None, "#b8925b");
    let vocals = track(ids, "Vocals", TrackKind::Audio, None, "#b85b8f");
    let (fx, verb, _) = fx_buses(ids, "Vocal Plate", false);
    let eq = effect(
        ids,
        &vocals.track_id,
        "sonara.eq.parametric",
        "Vocal Presence",
        &[],
        None,
    );
    let comp = effect(
        ids,
        &vocals.track_id,
        "sonara.dynamics.compressor",
        "Vocal Leveler",
        &[],
        None,
    );
    let vocals_verb = send(ids, &vocals.track_id, &verb, -10.0);
    let keys_verb = send(ids, &keys.track_id, &verb, -14.0);
    let mut commands: Vec<Box<dyn Command>> = vec![
        Box::new(drums),
        Box::new(bass),
        Box::new(keys),
        Box::new(guitar),
        Box::new(vocals),
    ];
    commands.extend(fx);
    commands.extend([
        Box::new(eq) as Box<dyn Command>,
        Box::new(comp),
        Box::new(vocals_verb),
        Box::new(keys_verb),
    ]);
    commands
}

fn orchestral(ids: &mut dyn IdGenerator) -> Vec<Box<dyn Command>> {
    let strings = instrument_track(ids, "Strings", "sonara.synth.poly", "Strings", "#8f5bb8");
    let brass = instrument_track(ids, "Brass", "sonara.synth.poly", "Brass", "#d9a24a");
    let choir = instrument_track(ids, "Choir", "sonara.synth.poly", "Choir Pad", "#5b9bb8");
    let perc = instrument_track(
        ids,
        "Percussion",
        "sonara.drums",
        "Cinematic Toms",
        "#d9644a",
    );
    let (fx, verb, _) = fx_buses(ids, "Cathedral", false);
    let sends = [
        send(ids, &strings.track_id, &verb, -6.0),
        send(ids, &brass.track_id, &verb, -8.0),
        send(ids, &choir.track_id, &verb, -5.0),
        send(ids, &perc.track_id, &verb, -9.0),
    ];
    let limiter = effect(
        ids,
        ids::MASTER_ID,
        "sonara.dynamics.limiter",
        "Master -1 dBTP",
        &[],
        None,
    );
    let mut commands: Vec<Box<dyn Command>> = vec![
        Box::new(strings),
        Box::new(brass),
        Box::new(choir),
        Box::new(perc),
    ];
    commands.extend(fx);
    commands.extend(sends.into_iter().map(|s| Box::new(s) as Box<dyn Command>));
    commands.push(Box::new(limiter));
    commands
}
