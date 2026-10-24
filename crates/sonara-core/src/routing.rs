//! The signal-routing graph, derived from tracks (`sonara.domain.routing`).
//!
//! Nodes are every audio-producing track plus the master. Directed edges carry audio:
//!
//! - `output`: a track's main output into a bus or the master;
//! - `send`: an auxiliary send into a bus;
//! - `sidechain`: a track feeding the sidechain input of a device on another track.
//!
//! Feedback loops are **not allowed**: any command that would create a cycle is rejected with
//! [`ErrorCode::RoutingCycle`]. Disabled sends still count, so enabling a send can never create a
//! cycle.
//!
//! The graph code is a hand port, not `petgraph` (QG-13): the prototype's results depend on its
//! edge order (the path that names a feedback loop, the track-position tie-break of
//! [`processing_order`]) and its nodes include IDs that name no track, which `petgraph`'s
//! `toposort` and cycle search do not reproduce. The prototype's recursive depth-first search is
//! an explicit stack here, so no chain of tracks can overflow the thread stack.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt;
use std::iter;

use crate::error::{DomainError, ErrorCode, Result};
use crate::ids;
use crate::model::project::Project;
use crate::model::track::{Track, TrackKind};
use crate::pyfmt::str_repr;

/// What carries the audio along an [`Edge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// A track's main output into a bus or the master.
    Output,
    /// An auxiliary send into a bus.
    Send,
    /// A track feeding the sidechain input of a device on another track.
    Sidechain,
}

impl EdgeKind {
    /// The kind as the prototype names it: `"output"`, `"send"` or `"sidechain"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Output => "output",
            Self::Send => "send",
            Self::Sidechain => "sidechain",
        }
    }
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A directed edge of the routing graph: audio flows from `source` to `target`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    /// The node the audio comes from (a track ID, which may name no track).
    pub source: String,
    /// The node the audio goes to (a track ID, which may name no track).
    pub target: String,
    /// What carries the audio.
    pub kind: EdgeKind,
    /// The send ID of a send, the device ID of a sidechain; `None` for an output.
    pub via: Option<String>,
}

impl Edge {
    /// `via`, or `fallback` without one (Python's `e.via or fallback`, where `""` counts as
    /// none).
    fn via_or<'a>(&'a self, fallback: &'a str) -> &'a str {
        self.via
            .as_deref()
            .filter(|via| !via.is_empty())
            .unwrap_or(fallback)
    }
}

/// One problem of the routing graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingIssue {
    /// [`ErrorCode::InvalidRouting`] or [`ErrorCode::RoutingCycle`].
    pub code: ErrorCode,
    /// The message, worded as in the prototype.
    pub message: String,
    /// The object to fix: a track, send or device ID.
    pub object_id: String,
}

impl RoutingIssue {
    fn new(code: ErrorCode, message: String, object_id: &str) -> Self {
        Self {
            code,
            message,
            object_id: object_id.to_owned(),
        }
    }

    fn invalid(message: String, object_id: &str) -> Self {
        Self::new(ErrorCode::InvalidRouting, message, object_id)
    }
}

/// The edges of the routing graph: the outputs and sends of the audio-producing tracks, in track
/// order, then the sidechains of every track and the master, in device order.
#[must_use]
pub fn edges(project: &Project) -> Vec<Edge> {
    let mut out = Vec::new();
    for track in project.tracks.iter().filter(|t| t.kind.produces_audio()) {
        if let Some(output_id) = &track.output_id {
            out.push(Edge {
                source: track.id.clone(),
                target: output_id.clone(),
                kind: EdgeKind::Output,
                via: None,
            });
        }
        out.extend(track.sends.iter().map(|send| Edge {
            source: track.id.clone(),
            target: send.target_id.clone(),
            kind: EdgeKind::Send,
            via: Some(send.id.clone()),
        }));
    }
    for track in project.all_tracks() {
        out.extend(track.devices().filter_map(|device| {
            device.sidechain_id.as_ref().map(|source| Edge {
                source: source.clone(),
                target: track.id.clone(),
                kind: EdgeKind::Sidechain,
                via: Some(device.id.clone()),
            })
        }));
    }
    out
}

/// All routing problems in `project`, in the prototype's order; empty when the graph is valid.
///
/// The edges are checked first, then the outputs and folder parents of the tracks. The cycle
/// search runs only when nothing else was found; circular folder nesting is always reported.
#[must_use]
pub fn issues(project: &Project) -> Vec<RoutingIssue> {
    let by_id: HashMap<&str, &Track> = project.all_tracks().map(|t| (t.id.as_str(), t)).collect();
    let mut found: Vec<RoutingIssue> = edges(project)
        .iter()
        .filter_map(|edge| edge_issue(edge, &by_id))
        .collect();
    for track in &project.tracks {
        if track.kind.produces_audio() && track.output_id.is_none() {
            found.push(RoutingIssue::invalid(
                format!("{} has no output", str_repr(&track.name)),
                &track.id,
            ));
        }
        if let Some(parent_id) = &track.parent_id {
            let is_folder = by_id
                .get(parent_id.as_str())
                .is_some_and(|parent| parent.kind == TrackKind::Folder);
            if !is_folder {
                found.push(RoutingIssue::invalid(
                    format!("parent of {} must be a folder track", str_repr(&track.name)),
                    &track.id,
                ));
            }
        }
    }
    if found.is_empty()
        && let Some(cycle) = find_cycle(project)
        && let Some(first) = cycle.first()
    {
        // Every endpoint exists here (missing ones were reported above), so the fallback to the
        // ID is never used.
        let names: Vec<&str> = cycle
            .iter()
            .map(|id| {
                by_id
                    .get(id.as_str())
                    .map_or(id.as_str(), |t| t.name.as_str())
            })
            .collect();
        found.push(RoutingIssue::new(
            ErrorCode::RoutingCycle,
            format!("routing feedback loop: {}", names.join(" → ")),
            first,
        ));
    }
    found.extend(folder_cycles(project));
    found
}

/// The problem of one edge, if any. The kind checks exclude each other, so an edge has at most
/// one problem.
fn edge_issue(edge: &Edge, by_id: &HashMap<&str, &Track>) -> Option<RoutingIssue> {
    let Some(src) = by_id.get(edge.source.as_str()) else {
        return Some(RoutingIssue::invalid(
            format!(
                "{} source {} does not exist",
                edge.kind,
                str_repr(&edge.source)
            ),
            edge.via_or(&edge.target),
        ));
    };
    let name = str_repr(&src.name);
    let Some(dst) = by_id.get(edge.target.as_str()) else {
        return Some(RoutingIssue::invalid(
            format!(
                "{} target {} of {name} does not exist",
                edge.kind,
                str_repr(&edge.target)
            ),
            edge.via_or(&src.id),
        ));
    };
    if edge.source == edge.target {
        return Some(RoutingIssue::new(
            ErrorCode::RoutingCycle,
            format!("{name} cannot route to itself"),
            edge.via_or(&src.id),
        ));
    }
    match edge.kind {
        EdgeKind::Output if !dst.kind.is_routing_target() => Some(RoutingIssue::invalid(
            format!("{name} can only output to a bus or the master"),
            &src.id,
        )),
        EdgeKind::Send if dst.kind != TrackKind::Bus => Some(RoutingIssue::invalid(
            format!("send from {name} must target a bus"),
            edge.via_or(&src.id),
        )),
        EdgeKind::Sidechain if src.kind == TrackKind::Master || !src.kind.produces_audio() => {
            Some(RoutingIssue::invalid(
                format!("{name} cannot be a sidechain source"),
                edge.via_or(&src.id),
            ))
        }
        _ => None,
    }
}

/// Fails with the first routing issue, if any.
///
/// # Errors
///
/// The first issue of [`issues`] as a [`DomainError`] with its code and message, and the
/// details `object_id` and `issue_count` (the number of issues).
pub fn validate(project: &Project) -> Result<()> {
    let problems = issues(project);
    let count = problems.len();
    match problems.into_iter().next() {
        None => Ok(()),
        Some(first) => Err(DomainError::new(first.code, first.message)
            .with_detail("object_id", first.object_id)
            .with_detail("issue_count", count)),
    }
}

/// The node IDs of one cycle, the first node repeated at the end, or `None` without a cycle.
///
/// A depth-first search starts from each track and the master in order, follows the edges in
/// edge order and returns the first cycle it closes.
#[must_use]
pub fn find_cycle(project: &Project) -> Option<Vec<String>> {
    let edges = edges(project);
    let adj = adjacency(edges.iter().map(|e| (e.source.as_str(), e.target.as_str())));
    let mut states: HashMap<&str, Visit> = HashMap::new();
    for track in project.all_tracks() {
        if !states.contains_key(track.id.as_str())
            && let Some(cycle) = visit(&track.id, &adj, &mut states)
        {
            return Some(cycle);
        }
    }
    None
}

/// Depth-first search state of a node; an unvisited node has none (the prototype's white).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Visit {
    /// On the current path (grey).
    Open,
    /// Finished without closing a cycle (black).
    Done,
}

/// The prototype's recursive `visit`, with an explicit stack: each path entry is a node and the
/// index of its next target, so the search resumes there when a target finishes.
fn visit<'a>(
    start: &'a str,
    adj: &Adjacency<'a>,
    states: &mut HashMap<&'a str, Visit>,
) -> Option<Vec<String>> {
    let mut path: Vec<(&str, usize)> = vec![(start, 0)];
    states.insert(start, Visit::Open);
    while let Some(top) = path.last_mut() {
        let (node, index) = *top;
        top.1 = index + 1;
        let Some(&next) = targets(adj, node).get(index) else {
            path.pop();
            states.insert(node, Visit::Done);
            continue;
        };
        match states.get(next) {
            Some(Visit::Open) => {
                // `next` is on the path: the cycle runs from its first occurrence back to it.
                let cycle = path
                    .iter()
                    .map(|(id, _)| *id)
                    .skip_while(|id| *id != next)
                    .chain(iter::once(next))
                    .map(str::to_owned)
                    .collect();
                return Some(cycle);
            }
            Some(Visit::Done) => {}
            None => {
                states.insert(next, Visit::Open);
                path.push((next, 0));
            }
        }
    }
    None
}

/// The audio-producing tracks and then the master, in topological order: every node comes after
/// the nodes that feed it. Ties follow track order, so the result is deterministic.
///
/// # Errors
///
/// [`ErrorCode::RoutingCycle`]: `routing graph contains a feedback loop`.
pub fn processing_order(project: &Project) -> Result<Vec<String>> {
    let nodes: Vec<&str> = project
        .tracks
        .iter()
        .filter(|t| t.kind.produces_audio())
        .map(|t| t.id.as_str())
        .chain(iter::once(ids::MASTER_ID))
        .collect();
    // A node's position in `nodes` is its index in the vectors below.
    let position: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let mut indegree = vec![0_usize; nodes.len()];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for edge in &edges(project) {
        if let (Some(&source), Some(&target)) = (
            position.get(edge.source.as_str()),
            position.get(edge.target.as_str()),
        ) {
            adj[source].push(target);
            indegree[target] += 1;
        }
    }
    // Kahn's algorithm. The initial nodes are in position order; nodes that become ready
    // together are queued in position order.
    let mut ready: VecDeque<usize> = (0..nodes.len()).filter(|&n| indegree[n] == 0).collect();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(node) = ready.pop_front() {
        order.push(nodes[node].to_owned());
        let mut newly = Vec::new();
        for &next in &adj[node] {
            // Each entry of `adj` added one to the target's in-degree, so this never underflows.
            indegree[next] -= 1;
            if indegree[next] == 0 {
                newly.push(next);
            }
        }
        newly.sort_unstable();
        ready.extend(newly);
    }
    if order.len() != nodes.len() {
        return Err(DomainError::new(
            ErrorCode::RoutingCycle,
            "routing graph contains a feedback loop",
        ));
    }
    Ok(order)
}

/// All nodes reachable from `node_id` through outputs and sends (`node_id` itself only when it
/// lies on a cycle).
#[must_use]
pub fn downstream(project: &Project, node_id: &str) -> BTreeSet<String> {
    let edges = edges(project);
    owned(&reachable(&signal_adjacency(&edges, false), node_id))
}

/// All nodes that feed `node_id` through outputs and sends (`node_id` itself only when it lies
/// on a cycle).
#[must_use]
pub fn upstream(project: &Project, node_id: &str) -> BTreeSet<String> {
    let edges = edges(project);
    owned(&reachable(&signal_adjacency(&edges, true), node_id))
}

/// Nodes that are audible given the mute and solo state.
///
/// Without any solo, every unmuted node is audible. With solos, a node is audible when it is
/// soloed or solo-safe, when it feeds a soloed node (soloing a bus solos its sources), or when it
/// lies downstream of a soloed node (the signal path to the master stays open). Muted nodes are
/// never audible.
///
/// An output or send to an ID that names no track adds no node, where the prototype raises
/// `NOT_FOUND`. Commands never create such a route ([`validate`] rejects it); only a project
/// file that already holds one reaches this case.
#[must_use]
pub fn audible_nodes(project: &Project) -> BTreeSet<String> {
    let tracks: Vec<&Track> = project
        .all_tracks()
        .filter(|t| t.kind.produces_audio())
        .collect();
    let soloed: Vec<&str> = tracks
        .iter()
        .filter(|t| t.mixer.solo)
        .map(|t| t.id.as_str())
        .collect();
    if soloed.is_empty() {
        return tracks
            .iter()
            .filter(|t| !t.mixer.mute)
            .map(|t| t.id.clone())
            .collect();
    }
    let edges = edges(project);
    let feeds = signal_adjacency(&edges, false);
    let fed_by = signal_adjacency(&edges, true);
    let mut audible: BTreeSet<&str> = soloed.iter().copied().collect();
    for id in &soloed {
        audible.extend(reachable(&feeds, id));
        audible.extend(reachable(&fed_by, id));
    }
    // Solo-safe nodes stay audible, and so does their path to the master (opening a bus does
    // not unmute its other inputs: those are judged individually).
    for track in tracks.iter().filter(|t| t.mixer.solo_safe) {
        audible.insert(&track.id);
        audible.extend(reachable(&feeds, &track.id));
    }
    audible.insert(ids::MASTER_ID);
    audible
        .into_iter()
        .filter(|id| project.find_track(id).is_some_and(|t| !t.mixer.mute))
        .map(str::to_owned)
        .collect()
}

/// One issue for each track whose chain of folder parents comes back to a folder already seen.
fn folder_cycles(project: &Project) -> Vec<RoutingIssue> {
    let parents: HashMap<&str, Option<&str>> = project
        .tracks
        .iter()
        .map(|t| (t.id.as_str(), t.parent_id.as_deref()))
        .collect();
    let parent_of = |id: &str| parents.get(id).copied().flatten();
    let mut found = Vec::new();
    for track in &project.tracks {
        let mut seen = HashSet::from([track.id.as_str()]);
        let mut parent = parent_of(&track.id);
        while let Some(id) = parent {
            if !seen.insert(id) {
                found.push(RoutingIssue::invalid(
                    format!("folder nesting of {} is circular", str_repr(&track.name)),
                    &track.id,
                ));
                break;
            }
            parent = parent_of(id);
        }
    }
    found
}

/// Each node's targets in edge order (the prototype's `defaultdict(list)`).
type Adjacency<'a> = HashMap<&'a str, Vec<&'a str>>;

fn adjacency<'a>(pairs: impl Iterator<Item = (&'a str, &'a str)>) -> Adjacency<'a> {
    let mut adj = Adjacency::new();
    for (from, to) in pairs {
        adj.entry(from).or_default().push(to);
    }
    adj
}

/// The output and send edges (no sidechains), from source to target or, `reversed`, from target
/// to source.
fn signal_adjacency(edges: &[Edge], reversed: bool) -> Adjacency<'_> {
    adjacency(
        edges
            .iter()
            .filter(|e| e.kind != EdgeKind::Sidechain)
            .map(|e| {
                let (source, target) = (e.source.as_str(), e.target.as_str());
                if reversed {
                    (target, source)
                } else {
                    (source, target)
                }
            }),
    )
}

/// The targets of `node`; none for a node without edges.
fn targets<'m, 'a>(adj: &'m Adjacency<'a>, node: &str) -> &'m [&'a str] {
    adj.get(node).map_or(&[], Vec::as_slice)
}

/// Every node reachable from `start` along `adj`.
fn reachable<'a>(adj: &Adjacency<'a>, start: &'a str) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::new();
    let mut todo = vec![start];
    while let Some(node) = todo.pop() {
        for &next in targets(adj, node) {
            if seen.insert(next) {
                todo.push(next);
            }
        }
    }
    seen
}

fn owned(ids: &BTreeSet<&str>) -> BTreeSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}
