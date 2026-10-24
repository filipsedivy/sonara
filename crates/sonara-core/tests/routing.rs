//! The routing graph follows the prototype (`tests/unit/test_routing.py`): edges, issues and
//! their messages, cycles, processing order, reachability and solo logic.

use std::collections::BTreeSet;

use serde_json::json;
use sonara_core::error::ErrorCode;
use sonara_core::ids::MASTER_ID;
use sonara_core::model::JsonMap;
use sonara_core::model::device::{Device, DeviceData, Params};
use sonara_core::model::project::{Project, ProjectData};
use sonara_core::model::track::{Send as TrackSend, Track, TrackData, TrackKind};
use sonara_core::routing::{self, Edge, EdgeKind, RoutingIssue};

/// A track named after its upper-cased ID, as the prototype's `_track` builds it.
fn track(id: &str, kind: TrackKind, change: impl FnOnce(&mut TrackData)) -> Track {
    let mut data = TrackData::new(id, id.to_uppercase(), kind);
    change(&mut data);
    Track::new(data).expect("valid track")
}

fn audio(id: &str) -> Track {
    track(id, TrackKind::Audio, |_| {})
}

fn bus(id: &str) -> Track {
    track(id, TrackKind::Bus, |_| {})
}

fn project(tracks: Vec<Track>) -> Project {
    Project::new(ProjectData::new("prj_r", "R"))
        .and_then(|p| p.edit(|d| d.tracks = tracks))
        .expect("valid project")
}

fn send(id: &str, target: &str) -> TrackSend {
    TrackSend::new(id, target, 0.0, false, true).expect("valid send")
}

fn sidechained(id: &str, source: &str) -> Device {
    Device::new(DeviceData {
        id: id.to_owned(),
        plugin_id: "sonara.dynamics.compressor".to_owned(),
        name: "C".to_owned(),
        params: Params::new(),
        enabled: true,
        sidechain_id: Some(source.to_owned()),
        preset: None,
        state: JsonMap::new(),
        extra: JsonMap::new(),
    })
    .expect("valid device")
}

fn edge(source: &str, target: &str, kind: EdgeKind, via: Option<&str>) -> Edge {
    Edge {
        source: source.to_owned(),
        target: target.to_owned(),
        kind,
        via: via.map(str::to_owned),
    }
}

fn issue(code: ErrorCode, message: &str, object_id: &str) -> RoutingIssue {
    RoutingIssue {
        code,
        message: message.to_owned(),
        object_id: object_id.to_owned(),
    }
}

fn invalid(message: &str, object_id: &str) -> RoutingIssue {
    issue(ErrorCode::InvalidRouting, message, object_id)
}

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn edge_kinds_render_as_the_prototype_values() {
    assert_eq!(EdgeKind::Output.to_string(), "output");
    assert_eq!(EdgeKind::Send.to_string(), "send");
    assert_eq!(EdgeKind::Sidechain.as_str(), "sidechain");
}

#[test]
fn edges_cover_outputs_sends_sidechains() {
    let kick = audio("trk_kick");
    let bass = track("trk_bass", TrackKind::Audio, |d| {
        d.inserts = vec![sidechained("dev_c", "trk_kick")];
        d.sends = vec![send("snd_1", "trk_bus")];
    });
    let folder = track("trk_f", TrackKind::Folder, |_| {});
    let p = project(vec![bus("trk_bus"), kick, bass, folder]);
    // Outputs and sends in track order, then sidechains; folders carry no audio.
    assert_eq!(
        routing::edges(&p),
        [
            edge("trk_bus", MASTER_ID, EdgeKind::Output, None),
            edge("trk_kick", MASTER_ID, EdgeKind::Output, None),
            edge("trk_bass", MASTER_ID, EdgeKind::Output, None),
            edge("trk_bass", "trk_bus", EdgeKind::Send, Some("snd_1")),
            edge("trk_kick", "trk_bass", EdgeKind::Sidechain, Some("dev_c")),
        ]
    );
    assert_eq!(routing::issues(&p), []);
    assert!(routing::validate(&p).is_ok());
}

#[test]
fn master_sidechains_are_edges() {
    let master = Track::master()
        .edit(|d| d.inserts = vec![sidechained("dev_m", "trk_a")])
        .expect("valid master");
    let p = project(vec![audio("trk_a")])
        .edit(|d| d.master = master)
        .expect("valid project");
    assert_eq!(
        routing::edges(&p).last(),
        Some(&edge(
            "trk_a",
            MASTER_ID,
            EdgeKind::Sidechain,
            Some("dev_m")
        ))
    );
}

#[test]
fn invalid_targets_reported() {
    let p = project(vec![
        audio("trk_a"),
        track("trk_b", TrackKind::Audio, |d| {
            d.output_id = Some("trk_a".into());
        }),
    ]);
    assert_eq!(
        routing::issues(&p),
        [invalid(
            "'TRK_B' can only output to a bus or the master",
            "trk_b"
        )]
    );
    // Audio tracks are no routing targets, but the edge still orders the tracks.
    assert_eq!(
        routing::processing_order(&p).expect("acyclic"),
        ids(&["trk_b", "trk_a", MASTER_ID])
    );

    let p = project(vec![track("trk_a", TrackKind::Audio, |d| {
        d.output_id = Some("trk_missing".into());
    })]);
    assert_eq!(
        routing::issues(&p),
        [invalid(
            "output target 'trk_missing' of 'TRK_A' does not exist",
            "trk_a"
        )]
    );

    let p = project(vec![
        audio("trk_a"),
        track("trk_b", TrackKind::Audio, |d| {
            d.sends = vec![send("snd_1", "trk_a")];
        }),
    ]);
    assert_eq!(
        routing::issues(&p),
        [invalid("send from 'TRK_B' must target a bus", "snd_1")]
    );

    let p = project(vec![track("trk_a", TrackKind::Audio, |d| {
        d.output_id = None;
    })]);
    assert_eq!(
        routing::issues(&p),
        [invalid("'TRK_A' has no output", "trk_a")]
    );

    let p = project(vec![track("trk_a", TrackKind::Audio, |d| {
        d.inserts = vec![sidechained("dev_1", MASTER_ID)];
    })]);
    assert_eq!(
        routing::issues(&p),
        [invalid("'Master' cannot be a sidechain source", "dev_1")]
    );
    // The sidechain from the master closes a loop, which `issues` does not search for here.
    assert_eq!(
        routing::find_cycle(&p),
        Some(ids(&["trk_a", MASTER_ID, "trk_a"]))
    );
    assert!(routing::processing_order(&p).is_err());
    let err = routing::validate(&p).expect_err("invalid sidechain");
    assert_eq!(err.code, ErrorCode::InvalidRouting);
    assert_eq!(err.message, "'Master' cannot be a sidechain source");
    // The prototype's keyword order.
    assert_eq!(
        err.details.keys().collect::<Vec<_>>(),
        ["object_id", "issue_count"]
    );
    assert_eq!(
        serde_json::Value::Object(err.details),
        json!({"object_id": "dev_1", "issue_count": 1})
    );
}

#[test]
fn missing_endpoints_name_the_send_or_device() {
    let p = project(vec![
        track("trk_a", TrackKind::Audio, |d| {
            d.sends = vec![send("snd_1", "trk_gone")];
            d.inserts = vec![sidechained("dev_1", "trk_ghost")];
        }),
        track("trk_b", TrackKind::Audio, |d| {
            d.sends = vec![send("", "trk_a")];
        }),
    ]);
    assert_eq!(
        routing::issues(&p),
        [
            invalid("send target 'trk_gone' of 'TRK_A' does not exist", "snd_1"),
            // An empty send ID is falsy in the prototype's `e.via or src.id`.
            invalid("send from 'TRK_B' must target a bus", "trk_b"),
            invalid("sidechain source 'trk_ghost' does not exist", "dev_1"),
        ]
    );
    let err = routing::validate(&p).expect_err("invalid routing");
    assert_eq!(err.details["object_id"], "snd_1");
    assert_eq!(err.details["issue_count"], 3);
}

#[test]
fn self_route_is_cycle() {
    let p = project(vec![track("trk_bus", TrackKind::Bus, |d| {
        d.output_id = Some("trk_bus".into());
    })]);
    assert_eq!(
        routing::issues(&p),
        [issue(
            ErrorCode::RoutingCycle,
            "'TRK_BUS' cannot route to itself",
            "trk_bus"
        )]
    );
    assert_eq!(routing::find_cycle(&p), Some(ids(&["trk_bus", "trk_bus"])));
}

fn bus_into(id: &str, target: &str) -> Track {
    track(id, TrackKind::Bus, |d| d.output_id = Some(target.into()))
}

#[test]
fn bus_cycle_detected() {
    let p = project(vec![bus_into("trk_a", "trk_b"), bus_into("trk_b", "trk_a")]);
    assert_eq!(
        routing::find_cycle(&p),
        Some(ids(&["trk_a", "trk_b", "trk_a"]))
    );
    assert_eq!(
        routing::issues(&p),
        [issue(
            ErrorCode::RoutingCycle,
            "routing feedback loop: TRK_A → TRK_B → TRK_A",
            "trk_a"
        )]
    );
    let err = routing::processing_order(&p).expect_err("cycle");
    assert_eq!(err.code, ErrorCode::RoutingCycle);
    assert_eq!(err.message, "routing graph contains a feedback loop");
    assert!(err.details.is_empty());
    let err = routing::validate(&p).expect_err("cycle");
    assert_eq!(err.code, ErrorCode::RoutingCycle);
    assert_eq!(err.details["object_id"], "trk_a");
}

#[test]
fn send_cycle_detected() {
    let a = track("trk_a", TrackKind::Bus, |d| {
        d.sends = vec![send("snd_1", "trk_b")];
    });
    let p = project(vec![a, bus_into("trk_b", "trk_a")]);
    // The search finishes the master (the first target of trk_a) before following the send.
    assert_eq!(
        routing::find_cycle(&p),
        Some(ids(&["trk_a", "trk_b", "trk_a"]))
    );
}

#[test]
fn cycle_path_starts_where_it_closes() {
    // trk_a feeds the loop trk_b → trk_c → trk_b without being part of it.
    let p = project(vec![
        bus_into("trk_a", "trk_b"),
        bus_into("trk_b", "trk_c"),
        bus_into("trk_c", "trk_b"),
    ]);
    assert_eq!(
        routing::find_cycle(&p),
        Some(ids(&["trk_b", "trk_c", "trk_b"]))
    );
    assert_eq!(
        routing::issues(&p),
        [issue(
            ErrorCode::RoutingCycle,
            "routing feedback loop: TRK_B → TRK_C → TRK_B",
            "trk_b"
        )]
    );
}

#[test]
fn sidechain_participates_in_cycles() {
    // The bus feeds the master; track A outputs to the bus; the bus keys a compressor on A.
    let a = track("trk_a", TrackKind::Audio, |d| {
        d.inserts = vec![sidechained("dev_c", "trk_bus")];
        d.output_id = Some("trk_bus".into());
    });
    let p = project(vec![a, bus("trk_bus")]);
    assert_eq!(
        routing::find_cycle(&p),
        Some(ids(&["trk_a", "trk_bus", "trk_a"]))
    );
}

#[test]
fn long_chains_need_no_recursion() {
    // 512 buses in a chain, the last one feeding the first.
    let count = 512;
    let id = |i: usize| format!("trk_{i:03}");
    let chain: Vec<Track> = (0..count)
        .map(|i| bus_into(&id(i), &id((i + 1) % count)))
        .collect();
    let p = project(chain);
    let cycle = routing::find_cycle(&p).expect("cycle");
    assert_eq!(cycle.len(), count + 1);
    assert_eq!(cycle.first(), Some(&id(0)));
    assert_eq!(cycle.last(), Some(&id(0)));
    assert!(routing::processing_order(&p).is_err());

    let open = p
        .edit(|d| {
            if let Some(last) = d.tracks.last_mut() {
                *last = bus(&id(count - 1));
            }
        })
        .expect("valid project");
    assert_eq!(routing::find_cycle(&open), None);
    let order = routing::processing_order(&open).expect("acyclic");
    assert_eq!(order.len(), count + 1);
    assert_eq!(order.first(), Some(&id(0)));
    assert_eq!(order.last().map(String::as_str), Some(MASTER_ID));
}

#[test]
fn processing_order_is_topological_and_deterministic() {
    let tracks = vec![
        audio("trk_z"),
        bus("trk_bus"),
        track("trk_a", TrackKind::Audio, |d| {
            d.output_id = Some("trk_bus".into());
        }),
        track("trk_m", TrackKind::Audio, |d| {
            d.inserts = vec![sidechained("dev_s", "trk_z")];
        }),
        track("trk_f", TrackKind::Folder, |_| {}),
    ];
    let p = project(tracks);
    let order = routing::processing_order(&p).expect("acyclic");
    // Ties follow track order: z, a, m are all sources; bus waits for a; folders are not nodes.
    assert_eq!(
        order,
        ids(&["trk_z", "trk_a", "trk_m", "trk_bus", MASTER_ID])
    );
    assert_eq!(routing::processing_order(&p).expect("acyclic"), order);
}

#[test]
fn processing_order_ignores_missing_endpoints() {
    let p = project(vec![
        track("trk_a", TrackKind::Audio, |d| {
            d.output_id = Some("trk_gone".into());
        }),
        audio("trk_b"),
    ]);
    assert_eq!(
        routing::processing_order(&p).expect("acyclic"),
        ids(&["trk_a", "trk_b", MASTER_ID])
    );
}

#[test]
fn downstream_upstream() {
    let p = project(vec![
        track("trk_a", TrackKind::Audio, |d| {
            d.output_id = Some("trk_grp".into());
        }),
        bus_into("trk_grp", "trk_sub"),
        bus("trk_sub"),
    ]);
    assert_eq!(
        routing::downstream(&p, "trk_a"),
        set(&["trk_grp", "trk_sub", MASTER_ID])
    );
    assert_eq!(routing::upstream(&p, "trk_sub"), set(&["trk_grp", "trk_a"]));
    assert_eq!(routing::upstream(&p, "trk_nowhere"), set(&[]));
}

#[test]
fn reachability_ignores_sidechains() {
    let p = project(vec![
        audio("trk_kick"),
        track("trk_bass", TrackKind::Audio, |d| {
            d.inserts = vec![sidechained("dev_c", "trk_kick")];
        }),
    ]);
    assert_eq!(routing::downstream(&p, "trk_kick"), set(&[MASTER_ID]));
    assert_eq!(routing::upstream(&p, "trk_bass"), set(&[]));
}

fn mixed(t: &Track, mute: bool, solo: bool, solo_safe: bool) -> Track {
    t.edit(|d| {
        d.mixer.mute = mute;
        d.mixer.solo = solo;
        d.mixer.solo_safe = solo_safe;
    })
    .expect("valid track")
}

#[test]
fn audible_without_solo_respects_mute() {
    let p = project(vec![
        audio("trk_a"),
        mixed(&audio("trk_b"), true, false, false),
    ]);
    assert_eq!(routing::audible_nodes(&p), set(&["trk_a", MASTER_ID]));
}

#[test]
fn solo_track_opens_its_path_only() {
    let a = track("trk_a", TrackKind::Audio, |d| {
        d.sends = vec![send("snd_1", "trk_verb")];
    });
    let b = track("trk_b", TrackKind::Audio, |d| {
        d.sends = vec![send("snd_2", "trk_verb")];
    });
    let p = project(vec![mixed(&a, false, true, false), b, bus("trk_verb")]);
    assert_eq!(
        routing::audible_nodes(&p),
        set(&["trk_a", "trk_verb", MASTER_ID])
    );
}

#[test]
fn solo_bus_solos_its_sources() {
    let into_grp = |id: &str| {
        track(id, TrackKind::Audio, |d| {
            d.output_id = Some("trk_grp".into());
        })
    };
    let p = project(vec![
        into_grp("trk_a"),
        audio("trk_b"),
        mixed(&bus("trk_grp"), false, true, false),
        mixed(&into_grp("trk_c"), true, false, false),
    ]);
    assert_eq!(
        routing::audible_nodes(&p),
        set(&["trk_grp", "trk_a", MASTER_ID])
    );
}

#[test]
fn solo_safe_stays_audible_with_its_path() {
    let into_fx = |id: &str| {
        track(id, TrackKind::Audio, |d| {
            d.output_id = Some("trk_fx".into());
        })
    };
    let p = project(vec![
        bus("trk_fx"),
        mixed(&into_fx("trk_safe"), false, false, true),
        into_fx("trk_other"),
        mixed(&audio("trk_s"), false, true, false),
    ]);
    assert_eq!(
        routing::audible_nodes(&p),
        set(&["trk_s", "trk_safe", "trk_fx", MASTER_ID])
    );
}

#[test]
fn muted_master_silences_everything() {
    let master = Track::master()
        .edit(|d| d.mixer.mute = true)
        .expect("valid master");
    let p = project(vec![audio("trk_a")])
        .edit(|d| d.master = master)
        .expect("valid project");
    assert_eq!(routing::audible_nodes(&p), set(&["trk_a"]));
    let soloed = p
        .edit(|d| d.tracks = vec![mixed(&audio("trk_a"), false, true, false)])
        .expect("valid project");
    assert_eq!(routing::audible_nodes(&soloed), set(&["trk_a"]));
}

#[test]
fn audible_nodes_skip_missing_targets() {
    // Deviation from the prototype, which raises NOT_FOUND here (see `audible_nodes`).
    let a = track("trk_a", TrackKind::Audio, |d| {
        d.output_id = Some("trk_gone".into());
    });
    let p = project(vec![mixed(&a, false, true, false)]);
    assert_eq!(routing::audible_nodes(&p), set(&["trk_a", MASTER_ID]));
}

fn folder_in(id: &str, parent: Option<&str>) -> Track {
    track(id, TrackKind::Folder, |d| {
        d.parent_id = parent.map(str::to_owned);
    })
}

#[test]
fn folder_parent_rules_and_cycles() {
    let p = project(vec![
        folder_in("trk_f1", Some("trk_f2")),
        folder_in("trk_f2", Some("trk_f1")),
    ]);
    assert_eq!(
        routing::issues(&p),
        [
            invalid("folder nesting of 'TRK_F1' is circular", "trk_f1"),
            invalid("folder nesting of 'TRK_F2' is circular", "trk_f2"),
        ]
    );

    let bad_parent = project(vec![
        bus("trk_bus"),
        track("trk_a", TrackKind::Audio, |d| {
            d.parent_id = Some("trk_bus".into());
        }),
    ]);
    assert_eq!(
        routing::issues(&bad_parent),
        [invalid("parent of 'TRK_A' must be a folder track", "trk_a")]
    );

    let missing_parent = project(vec![track("trk_a", TrackKind::Audio, |d| {
        d.parent_id = Some("trk_none".into());
    })]);
    assert_eq!(
        routing::issues(&missing_parent),
        [invalid("parent of 'TRK_A' must be a folder track", "trk_a")]
    );

    let ok = project(vec![
        folder_in("trk_f", None),
        track("trk_a", TrackKind::Audio, |d| {
            d.parent_id = Some("trk_f".into());
        }),
    ]);
    assert_eq!(routing::issues(&ok), []);
}

#[test]
fn a_track_below_a_folder_loop_reports_too() {
    let p = project(vec![
        folder_in("trk_f1", Some("trk_f2")),
        folder_in("trk_f2", Some("trk_f1")),
        folder_in("trk_f3", Some("trk_f1")),
    ]);
    let objects: Vec<String> = routing::issues(&p)
        .into_iter()
        .map(|i| i.object_id)
        .collect();
    assert_eq!(objects, ids(&["trk_f1", "trk_f2", "trk_f3"]));
}

#[test]
fn cycle_search_waits_for_other_issues_but_folder_loops_do_not() {
    let p = project(vec![
        bus_into("trk_a", "trk_b"),
        bus_into("trk_b", "trk_a"),
        track("trk_c", TrackKind::Audio, |d| d.output_id = None),
        folder_in("trk_f1", Some("trk_f2")),
        folder_in("trk_f2", Some("trk_f1")),
    ]);
    assert_eq!(
        routing::issues(&p),
        [
            invalid("'TRK_C' has no output", "trk_c"),
            invalid("folder nesting of 'TRK_F1' is circular", "trk_f1"),
            invalid("folder nesting of 'TRK_F2' is circular", "trk_f2"),
        ]
    );
    let err = routing::validate(&p).expect_err("invalid routing");
    assert_eq!(err.message, "'TRK_C' has no output");
    assert_eq!(err.details["issue_count"], 3);

    let only_loops = project(vec![
        bus_into("trk_a", "trk_b"),
        bus_into("trk_b", "trk_a"),
        folder_in("trk_f1", Some("trk_f1")),
    ]);
    assert_eq!(
        routing::issues(&only_loops),
        [
            issue(
                ErrorCode::RoutingCycle,
                "routing feedback loop: TRK_A → TRK_B → TRK_A",
                "trk_a"
            ),
            invalid("folder nesting of 'TRK_F1' is circular", "trk_f1"),
        ]
    );
}
