//! The orchestration ids a project stores, checked against the orchestration metadata of
//! `sonara_midi::orchestration`: a track part's instrument and a note's articulation (project
//! schema version 2).
//!
//! The project format checks only the form of these ids, because it cannot see the metadata
//! (ARC-11). An id this build does not know is kept as written and reported as a warning when
//! the project opens, so a build that knows it still finds it, and nothing is lost on save.

use std::collections::BTreeMap;

use sonara_core::model::project::Project;
use sonara_core::pyfmt::str_repr;
use sonara_midi::orchestration::{Articulation, InstrumentType};

/// One warning per track whose part names an instrument type the metadata does not know, then
/// one per unknown note articulation (with how many notes use it, sorted by id). Empty when
/// every id is known.
#[must_use]
pub fn warnings(project: &Project) -> Vec<String> {
    let mut out: Vec<String> = project
        .all_tracks()
        .filter_map(|track| {
            let part = track.part.as_ref()?;
            InstrumentType::from_id(&part.instrument)
                .is_none()
                .then(|| {
                    format!(
                        "track {} names an unknown instrument {}; it is kept",
                        str_repr(&track.name),
                        str_repr(&part.instrument)
                    )
                })
        })
        .collect();
    let mut unknown: BTreeMap<&str, usize> = BTreeMap::new();
    for track in project.all_tracks() {
        for clip in track.midi_clips() {
            for id in clip.notes.iter().filter_map(|n| n.articulation.as_deref()) {
                if Articulation::from_id(id).is_none() {
                    *unknown.entry(id).or_default() += 1;
                }
            }
        }
    }
    out.extend(unknown.into_iter().map(|(id, notes)| {
        format!(
            "{notes} note(s) name an unknown articulation {}; it is kept",
            str_repr(id)
        )
    }));
    out
}

#[cfg(test)]
mod tests {
    use sonara_core::ids::{SequentialIds, is_vocabulary_id};
    use sonara_core::midi::MidiNote;
    use sonara_core::model::clip::{Clip, MidiClip};
    use sonara_core::model::part::{self, Part};
    use sonara_core::model::project::Project;
    use sonara_core::model::track::{Track, TrackKind};
    use sonara_midi::orchestration::{Articulation, Clef, InstrumentType};

    use super::warnings;

    /// A track: its name, its part's instrument, and the articulations of its notes.
    type TrackSpec<'a> = (&'a str, Option<&'a str>, &'a [Option<&'a str>]);

    /// A project with one instrument track per spec, each with one clip of notes.
    fn project(tracks: &[TrackSpec<'_>]) -> Project {
        let mut ids = SequentialIds::new(0);
        let mut project = Project::create(&mut ids, "Score", 48_000, 120.0).expect("valid");
        for (name, instrument, articulations) in tracks {
            let notes = articulations
                .iter()
                .enumerate()
                .map(|(i, articulation)| {
                    MidiNote::create(
                        &mut ids,
                        60,
                        f64::from(u32::try_from(i).expect("small")),
                        1.0,
                        100,
                        0,
                    )
                    .and_then(|n| n.with_articulation(articulation.map(str::to_owned)))
                    .expect("valid note")
                })
                .collect();
            let clip = MidiClip::create(&mut ids, "M", 0.0, 8.0, notes).expect("valid clip");
            let track = Track::create(&mut ids, *name, TrackKind::Instrument, |t| {
                t.part = instrument.map(|id| Part::new(id).expect("an id"));
                t.clips = vec![Clip::Midi(clip)];
            })
            .expect("valid track");
            project = project
                .edit(|p| p.tracks.push(track))
                .expect("valid project");
        }
        project
    }

    #[test]
    fn known_ids_raise_no_warning() {
        let score = project(&[
            ("Violins I", Some("violin"), &[None, Some("pizzicato")]),
            ("Horn 1", Some("horn_f"), &[Some("staccato")]),
            ("Pad", None, &[None]),
        ]);
        assert_eq!(warnings(&score), Vec::<String>::new());
    }

    #[test]
    fn unknown_ids_are_reported_and_kept() {
        let score = project(&[
            (
                "Violins I",
                Some("violn"),
                &[Some("stacato"), Some("pizzicato")],
            ),
            (
                "Theremin",
                Some("theremin"),
                &[Some("stacato"), Some("glissandoo")],
            ),
            ("Horn 1", Some("horn_f"), &[None]),
        ]);
        assert_eq!(
            warnings(&score),
            [
                "track 'Violins I' names an unknown instrument 'violn'; it is kept",
                "track 'Theremin' names an unknown instrument 'theremin'; it is kept",
                "1 note(s) name an unknown articulation 'glissandoo'; it is kept",
                "2 note(s) name an unknown articulation 'stacato'; it is kept",
            ]
        );
        let part = score.tracks[0].part.as_ref().expect("kept");
        assert_eq!(part.instrument, "violn");
    }

    /// Every id of the metadata has the form the project schema accepts, so any of them can be
    /// stored (CTR-01).
    #[test]
    fn every_metadata_id_has_the_schema_form() {
        for instrument in InstrumentType::ALL {
            assert!(is_vocabulary_id(instrument.id()), "{}", instrument.id());
        }
        for articulation in Articulation::ALL {
            assert!(is_vocabulary_id(articulation.id()), "{}", articulation.id());
        }
    }

    /// The clefs a part can be notated in are the clefs of the metadata, in the same order.
    #[test]
    fn part_clefs_are_the_metadata_clefs() {
        assert_eq!(
            part::Clef::ALL.map(part::Clef::as_str),
            Clef::ALL.map(Clef::slug)
        );
    }
}
