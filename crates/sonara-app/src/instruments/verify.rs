//! Deep validation of a patch through a [`PatchLoader`].
//!
//! Discovery only checks that a patch file exists and can be opened. Whether the patch parses
//! and its samples are present and decodable takes reading the samples, which is slow and
//! memory-hungry, so it runs on request ([`super::InstrumentCatalog::verify`]) through this seam.
//! The same check records what the patch holds ([`PatchFacts`]) and what its decoded samples
//! show ([`SoundFacts`]), from the one decoding. Tests use a fake loader; the application uses
//! [`MediaPatchLoader`]. Neither needs an audio engine.

use std::path::Path;

use super::facts::{PatchFacts, SoundFacts};

/// Why a patch cannot be loaded, in words for the user.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct PatchLoadError(pub String);

/// Loads a patch to find out whether it is playable. Runs off the real-time thread.
pub trait PatchLoader {
    /// Loads the patch at `patch`, and returns what decoding its samples measured and discards
    /// the rest: `None` from a loader that does not measure.
    ///
    /// # Errors
    ///
    /// [`PatchLoadError`] when the patch or one of its samples is missing, malformed or cannot be
    /// decoded.
    fn load(&self, patch: &Path) -> Result<Option<SoundFacts>, PatchLoadError>;

    /// What the structural check finds in the patch at the given path, from the patch file and
    /// the headers of its samples. `None` when the patch cannot be read, and for a loader that
    /// does not inspect patches, which is what this default says: the catalog then records no
    /// facts.
    fn inspect(&self, _patch: &Path) -> Option<PatchFacts> {
        None
    }
}

/// The loader of the application: the SFZ reader and sample decoder of `sonara-media`, the one
/// the sampler uses to play the patch. It decodes every sample the patch names, so it can take
/// seconds and memory up to `sonara_media::sfz::MAX_BANK_BYTES`, measures the decoded samples
/// (`sonara_media::sfz::measure`) and drops the bank at once.
///
/// The sampler plays a patch without the regions whose sample is missing; this check is
/// stricter and refuses such a patch with the reader's notes, because one of its samples is
/// missing.
#[derive(Debug, Clone, Copy, Default)]
pub struct MediaPatchLoader;

impl PatchLoader for MediaPatchLoader {
    fn load(&self, patch: &Path) -> Result<Option<SoundFacts>, PatchLoadError> {
        let bank =
            sonara_media::sfz::load_sfz(patch).map_err(|err| PatchLoadError(err.to_string()))?;
        if bank.load_notes.is_empty() {
            Ok(Some(SoundFacts::from(&sonara_media::sfz::measure(&bank))))
        } else {
            Err(PatchLoadError(format!(
                "the patch leaves out regions it cannot load: {}",
                bank.load_notes.join("; ")
            )))
        }
    }

    /// The report of `sonara_media::sfz::inspect`, which reads the patch and the headers of its
    /// samples without decoding them.
    fn inspect(&self, patch: &Path) -> Option<PatchFacts> {
        sonara_media::sfz::inspect(patch)
            .ok()
            .map(|report| PatchFacts::from(&report))
    }
}
