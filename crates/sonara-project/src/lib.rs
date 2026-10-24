//! The `.sonara` project format: the serde model of `project.json` and the manifest, schema
//! validation, migrations, bundle and ZIP store, atomic saves, backups, autosave and integrity
//! checks.
#![forbid(unsafe_code)]

pub mod atomic;
pub mod canonical;
pub mod migrations;
mod pyjson;
pub mod schema;
pub mod serializer;
pub mod store;
