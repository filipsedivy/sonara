//! Real-time: the values cross real-time queues and [`Prepared`] is moved on the render thread;
//! [`Request::prepare`] is setup and runs on the builder thread.
//!
//! The deferred setup of live graphs ([`crate::processor`], "Deferred setup"): what a
//! processor in deferred mode can wait for, and the prepared result. Only the reverb waits for
//! setup: its impulse response, synthesized and partitioned exactly as its own `set_params`
//! does offline, so an install at a block equals an offline parameter change at that block.

use std::fmt;

use super::reverb::{self, IrKey};
use crate::convolution::{PreparedIr, Retired};

/// What a processor waits for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Request {
    /// The reverb's impulse response for `key`, partitioned for blocks of `block_size` frames.
    ReverbIr { key: IrKey, block_size: usize },
}

impl Request {
    /// Does the work. Setup: allocates.
    pub(crate) fn prepare(&self) -> Prepared {
        match *self {
            Self::ReverbIr { key, block_size } => reverb::prepare_ir(&key, block_size)
                .map_or(Prepared::Empty, |ir| Prepared::ReverbIr { key, ir }),
        }
    }
}

/// Prepared setup, or what an install displaced.
pub(crate) enum Prepared {
    /// Nothing: the work produced nothing to install (an unsupported sample rate or block
    /// size), or an install took the content and displaced nothing.
    Empty,
    /// The reverb's partitioned impulse response for `key`.
    ReverbIr { key: IrKey, ir: PreparedIr<f32> },
    /// The impulse responses a reverb's install displaced.
    Displaced(Retired<f32>),
}

impl Prepared {
    /// Whether it holds setup to install.
    pub(crate) fn is_pending(&self) -> bool {
        matches!(self, Self::ReverbIr { .. })
    }
}

impl fmt::Debug for Prepared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::ReverbIr { key, ir } => f
                .debug_struct("ReverbIr")
                .field("key", key)
                .field("ir", ir)
                .finish(),
            Self::Displaced(retired) => f.debug_tuple("Displaced").field(retired).finish(),
        }
    }
}
