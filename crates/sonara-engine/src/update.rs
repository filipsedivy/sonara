//! Real-time: [`apply`] runs on the audio render thread (see docs/audio-engine.md);
//! [`SetupTicket::prepare`] is setup and runs on the builder thread.
//!
//! What the builder thread hands to the render thread, and back (RT-09).
//! The player owns one `sonara_rt::handoff` channel of [`Update`]s:
//!
//! ```text
//! builder thread                          render thread
//!   GraphBuilder::fresh / rebuild ──Update::Graph──► apply: adopt, swap with the current graph
//!   SetupTicket::prepare          ──Update::Setup──► apply: install into the processor
//!   drop what comes back          ◄── the same Update, now holding the old graph, the
//!                                     displaced setup, or what was not applied
//!   SetupTicket ◄──────────── sonara_rt::queue (Copy) ── RenderGraph::post_setup_requests
//! ```
//!
//! `Receiver::receive` reserves the return slot before it takes an update, and [`apply`] only
//! moves values in and out of it, so nothing is allocated or dropped on the render thread.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use sonara_dsp::processor::{PreparedSetup, SetupRequest};

use crate::graph::{InstanceId, RenderGraph};

/// A processor's setup request on its way to the builder thread: a `Copy` value for a real-time
/// queue ([`RenderGraph::post_setup_requests`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetupTicket {
    /// The processor that waits.
    pub instance: InstanceId,
    /// What it waits for.
    pub request: SetupRequest,
}

impl SetupTicket {
    /// Does the setup work (for the reverb, synthesizes and partitions the impulse response)
    /// and addresses the result to the processor.
    ///
    /// Setup: allocates; runs on the builder thread.
    #[must_use]
    pub fn prepare(&self) -> SetupDelivery {
        SetupDelivery {
            instance: self.instance,
            setup: self.request.prepare(),
        }
    }
}

/// Prepared setup addressed to a processor, or, once applied, what its install displaced.
#[derive(Debug)]
pub struct SetupDelivery {
    /// The processor it is for.
    pub instance: InstanceId,
    /// The setup, or what replaced it.
    pub setup: PreparedSetup,
}

/// What the builder thread hands to the render thread through `sonara_rt::handoff`. After
/// [`apply`] it holds what goes back, to be dropped on the builder thread.
#[derive(Debug)]
pub enum Update {
    /// A graph to play from now on (`None` takes the current graph away, for example before
    /// the stream stops). After [`apply`]: the graph it replaced, or itself when it was not
    /// adopted.
    Graph(Option<Box<RenderGraph>>),
    /// Prepared setup for a processor of the current graph. After [`apply`]: what the install
    /// displaced, or the unused setup.
    Setup(SetupDelivery),
}

/// What [`apply`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The graph replaced the current one, which went into the update.
    Swapped,
    /// The graph was planned against another graph ([`crate::StalePlan`]); the current graph
    /// plays on and the rejected one goes back.
    Rejected,
    /// The setup was installed.
    Installed,
    /// The setup's processor is gone or did not take it; it goes back.
    Unused,
}

/// Applies `update` to the render thread's `current` graph: a new graph adopts the current
/// one's state ([`RenderGraph::adopt`]) and replaces it; a setup is installed into its
/// processor ([`RenderGraph::install_setup`]). Afterwards `update` holds what must be dropped
/// off the real-time thread.
///
/// Real-time: moves boxes and small values; allocates and frees nothing.
#[cfg_attr(feature = "rtsan", rtsan_standalone::nonblocking)]
pub fn apply(current: &mut Option<Box<RenderGraph>>, update: &mut Update) -> Applied {
    match update {
        Update::Graph(next) => {
            let adopted = match (next.as_deref_mut(), current.as_deref_mut()) {
                (Some(next), Some(current)) => next.adopt(current).is_ok(),
                // A rebuilt graph needs the graph it was planned against.
                (Some(next), None) => !next.awaits_adoption(),
                (None, _) => true,
            };
            if !adopted {
                return Applied::Rejected;
            }
            std::mem::swap(current, next);
            Applied::Swapped
        }
        Update::Setup(delivery) => {
            let installed = current
                .as_deref_mut()
                .is_some_and(|graph| graph.install_setup(delivery));
            if installed {
                Applied::Installed
            } else {
                Applied::Unused
            }
        }
    }
}
