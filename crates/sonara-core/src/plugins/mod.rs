//! Plugin descriptors: identity, capabilities, parameter schemas and presets
//! (`sonara.plugins`), and the catalog of built-in instruments and effects.
//!
//! The descriptors are metadata only. The processors behind the built-in plugins are ported in
//! `sonara-dsp` and looked up by plugin ID.

mod builtin;
mod descriptor;

pub use builtin::{
    SAMPLER_AUTO, SAMPLER_ID, SAMPLER_ORDINARIO, SamplerChoices, builtin_registry,
    sampler_descriptor,
};
pub use descriptor::{
    ParamKind, ParamScale, ParamSpec, PluginDescriptor, PluginFormat, PluginKind, PluginRegistry,
    Preset,
};
