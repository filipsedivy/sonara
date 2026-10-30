//! The instrument catalog (SON-P02): the instrument presets found in the user's sample
//! libraries, their metadata and their availability on this machine.
//!
//! The catalog is a read model of the application layer. It reads directory entries, optional
//! library manifests and, on request, a patch through a [`PatchLoader`]; it never starts an audio
//! engine, so it is testable without one, and an unusable library is reported as data, never as a
//! panic. The audio engine plays a preset by its patch path (ADR 0010); nothing here changes the
//! plugin registry, the agent tools or the `.sonara` format (CTR-01).
//!
//! **Discovery** ([`InstrumentCatalog::discover`]) takes the library directories of the
//! `sample_libraries` setting ([`crate::samples`]) and merges three sources:
//!
//! - a **manifest** (`sonara-library.json`, see [`MANIFEST_FILE`]) that a library may carry: it
//!   declares the library's name and licence and, per patch, the display name, family and
//!   articulations. Declared metadata is trusted and marked as such;
//! - the **identity** of a library profile Sonara ships (VSCO 2 CE, VCSL): a library whose
//!   manifest declares no `id` and that holds every file the profile names to detect it takes
//!   the profile's `id` and name, so its preset IDs do not change when its folder is renamed;
//! - a **scan** for `.sfz` files, which finds every patch whether or not the manifest knows it.
//!   A patch the manifest does not list gets a name from its file name and a family *inferred*
//!   from the words of its path ([`InstrumentFamily`]), marked as inferred. Articulations are
//!   never guessed: without a manifest the list is empty, which means "not verified". The scan
//!   leaves out folders named `includes`, which hold fragments that other patches include
//!   ([`crate::samples`]); a manifest entry that names a file in one is still listed.
//!
//! A manifest entry whose file is gone is kept as [`Availability::Missing`], so the user sees
//! what to reinstall. Discovery opens no patch, so a file with no region is listed as available
//! until [`InstrumentCatalog::verify`] loads it and marks it [`Availability::Invalid`]. SFZ has
//! no bank or program numbers, so a preset has none; a future SF2 source would add them.

mod discovery;
mod family;
mod manifest;
mod model;
mod profiles;
mod verify;

pub use discovery::InstrumentCatalog;
pub use family::{FamilyClaim, InstrumentFamily};
pub use manifest::{MANIFEST_FILE, MANIFEST_VERSION};
pub use model::{
    Availability, CatalogError, InstrumentId, InstrumentPreset, InvalidLibraryId, Library,
    LibraryId, LibraryIssue,
};
pub use verify::{MediaPatchLoader, PatchLoadError, PatchLoader};
