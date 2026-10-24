//! The application's single source of object IDs.
//!
//! The prototype draws every ID, from tracks to operations, from one process-wide source
//! (`sonara.domain.ids.new_id`), which the golden recorder replaces with a counter. The golden
//! transcripts therefore interleave the numbers of commands and operations (`med_…30`,
//! `op_…32`, `med_…33`). [`IdSource`] is that one source: the context hands clones of it to the
//! job manager, the project service and every interface that builds commands.

use std::fmt;
use std::sync::{Arc, Mutex};

use sonara_core::ids::{IdGenerator, RandomIds};

use crate::sync::lock;

/// A shared, thread-safe [`IdGenerator`]; clones draw from the same sequence.
#[derive(Clone)]
pub struct IdSource(Arc<Mutex<Box<dyn IdGenerator>>>);

impl IdSource {
    /// A source drawing from `generator`.
    #[must_use]
    pub fn new(generator: Box<dyn IdGenerator>) -> Self {
        Self(Arc::new(Mutex::new(generator)))
    }

    /// Random IDs, for normal operation.
    #[must_use]
    pub fn random() -> Self {
        Self::new(Box::new(RandomIds))
    }
}

impl Default for IdSource {
    fn default() -> Self {
        Self::random()
    }
}

impl IdGenerator for IdSource {
    fn new_id(&mut self, prefix: &str) -> String {
        lock(&self.0).new_id(prefix)
    }
}

impl fmt::Debug for IdSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdSource").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use sonara_core::ids::{IdGenerator, SequentialIds};

    use super::IdSource;

    #[test]
    fn clones_share_one_sequence() {
        let mut a = IdSource::new(Box::new(SequentialIds::new(0)));
        let mut b = a.clone();
        assert_eq!(a.new_id("trk"), "trk_000000000001");
        assert_eq!(b.new_id("op"), "op_000000000002");
        assert_eq!(a.new_id("med"), "med_000000000003");
    }
}
