//! The layout of a live graph, which the builder thread keeps after sending the graph, and the
//! plan that lets the next rebuilt graph take over its state.
//!
//! Setup code: it runs on the builder thread and allocates.
//!
//! The builder cannot look into the graph the render thread plays, so it plans from the
//! layout of the last graph it built. The plan pairs, in the prototype's way
//! (`RenderGraph.__init__(…, previous)` in `audio/engine/graph.py`):
//!
//! - **processors** by device ID, when the plugin ID, the device state and the setup-relevant
//!   parameters are unchanged (the reuse decision is made while building, see
//!   `Builder::slot`);
//! - **nodes** by track ID: the volume, pan and mute smoothers, pending live events and the
//!   notes to release;
//! - **sends** by send ID within the same track: the send-level smoothers;
//! - **compensation delay lines** whose edge (the same source and target) and length are
//!   unchanged, which the prototype empties instead.

use std::collections::HashMap;
use std::sync::Arc;

use sonara_dsp::processor::{NoteSet, ParamSlots, ProcessSpec};
use sonara_media::FullAudio;

use super::{DeviceIndex, GraphId, InstanceId, NodeIndex};

/// Where a processor sits in a graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DeviceAddr {
    /// The node's position in processing order.
    pub(crate) node: usize,
    pub(crate) slot: SlotPosition,
}

/// A processor's place in its node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SlotPosition {
    Instrument,
    /// The position among the node's inserts.
    Insert(usize),
}

/// The builder thread's record of a live graph: what the next rebuild plans from, and the IDs
/// behind the graph's [`NodeIndex`] (meters) and [`DeviceIndex`] (reports).
///
/// A [`GraphBuilder`](crate::GraphBuilder) keeps the layout of the last graph it built
/// ([`GraphBuilder::layout`](crate::GraphBuilder::layout)); the control thread can clone it to
/// map the meters and reports of the graph that plays.
#[derive(Debug, Clone)]
pub struct GraphLayout {
    pub(super) id: GraphId,
    pub(super) spec: ProcessSpec,
    pub(super) nodes: Vec<NodeLayout>,
    pub(super) devices: Vec<DeviceLayout>,
}

/// A node of a [`GraphLayout`].
#[derive(Debug, Clone)]
pub(crate) struct NodeLayout {
    pub(crate) track_id: String,
    /// The output target's track ID and the length of the output's compensation delay (0
    /// without one); `None` for the master and a track without an output in the graph.
    pub(crate) output: Option<(String, usize)>,
    pub(crate) sends: Vec<SendLayout>,
}

/// A send of a [`NodeLayout`].
#[derive(Debug, Clone)]
pub(crate) struct SendLayout {
    pub(crate) send_id: String,
    pub(crate) target_id: String,
    /// The length of the send's compensation delay, 0 without one.
    pub(crate) delay: usize,
}

/// A device of a [`GraphLayout`], in report order.
#[derive(Debug, Clone)]
pub(crate) struct DeviceLayout {
    pub(crate) device_id: String,
    pub(crate) plugin_id: String,
    pub(crate) address: DeviceAddr,
    /// The device's parameter values, which its processor was given at the build.
    pub(crate) params: ParamSlots,
    /// The loaded impulse response it was given (identity by pointer; the media provider
    /// caches loads by asset and rate).
    pub(crate) ir_source: Option<Arc<FullAudio>>,
    pub(crate) playable_notes: NoteSet,
    pub(crate) instance: InstanceId,
    /// Where its processor comes from in the previous graph, when it is reused.
    pub(crate) reused: Option<DeviceAddr>,
}

impl GraphLayout {
    /// The graph's ID ([`RenderGraph::id`](crate::RenderGraph::id)).
    #[must_use]
    pub fn id(&self) -> GraphId {
        self.id
    }

    /// The number of nodes, the length of [`RenderGraph::meters`](crate::RenderGraph::meters).
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The number of devices with a processor.
    #[must_use]
    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    /// The track ID of a node.
    #[must_use]
    pub fn track_id(&self, node: NodeIndex) -> Option<&str> {
        self.nodes.get(node.0).map(|n| n.track_id.as_str())
    }

    /// The node of a track.
    #[must_use]
    pub fn node_index(&self, track_id: &str) -> Option<NodeIndex> {
        self.nodes
            .iter()
            .position(|n| n.track_id == track_id)
            .map(NodeIndex)
    }

    /// The device ID of a device.
    #[must_use]
    pub fn device_id(&self, device: DeviceIndex) -> Option<&str> {
        self.devices.get(device.0).map(|d| d.device_id.as_str())
    }
}

/// How a rebuilt live graph takes over the state of the graph it replaces, as index pairs
/// `(new, old)`; [`RenderGraph::adopt`](crate::RenderGraph::adopt) applies it on the render
/// thread with moves only.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    /// The graph the plan was made against.
    pub(crate) previous: GraphId,
    pub(crate) processors: Vec<(DeviceAddr, DeviceAddr)>,
    pub(crate) nodes: Vec<(usize, usize)>,
    /// `((node, send), (node, send))`.
    pub(crate) sends: Vec<((usize, usize), (usize, usize))>,
    /// Nodes whose output compensation delay is kept.
    pub(crate) output_lines: Vec<(usize, usize)>,
    /// Sends whose compensation delay is kept.
    pub(crate) send_lines: Vec<((usize, usize), (usize, usize))>,
}

impl Plan {
    /// The plan for `new`, built against `old`, the layout of the graph it replaces.
    pub(crate) fn new(old: &GraphLayout, new: &GraphLayout) -> Self {
        let mut plan = Self {
            previous: old.id,
            processors: new
                .devices
                .iter()
                .filter_map(|d| d.reused.map(|from| (d.address, from)))
                .collect(),
            nodes: Vec::new(),
            sends: Vec::new(),
            output_lines: Vec::new(),
            send_lines: Vec::new(),
        };
        let old_nodes: HashMap<&str, usize> = old
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.track_id.as_str(), i))
            .collect();
        for (i, node) in new.nodes.iter().enumerate() {
            let Some((j, old_node)) = old_nodes
                .get(node.track_id.as_str())
                .and_then(|&j| old.nodes.get(j).map(|n| (j, n)))
            else {
                continue;
            };
            plan.nodes.push((i, j));
            if let (Some((target, delay)), Some((old_target, old_delay))) =
                (&node.output, &old_node.output)
                && target == old_target
                && delay == old_delay
                && *delay > 0
            {
                plan.output_lines.push((i, j));
            }
            for (k, send) in node.sends.iter().enumerate() {
                let Some((l, old_send)) = old_node
                    .sends
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.send_id == send.send_id)
                else {
                    continue;
                };
                plan.sends.push(((i, k), (j, l)));
                if send.target_id == old_send.target_id
                    && send.delay == old_send.delay
                    && send.delay > 0
                {
                    plan.send_lines.push(((i, k), (j, l)));
                }
            }
        }
        plan
    }
}
