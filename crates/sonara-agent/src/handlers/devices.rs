//! Device tools (the prototype's "devices" section): the plugin list.

use serde_json::json;
use sonara_core::plugins::PluginKind;

use crate::context::AgentContext;
use crate::contract::args::{self, ListPlugins};
use crate::registry::ToolResult;

/// `list_plugins`: the instruments and effects with their presets, or one plugin with its full
/// parameter schema.
pub(super) fn list_plugins(ctx: &AgentContext, args: ListPlugins) -> ToolResult {
    let ListPlugins { kind, plugin_id } = args;
    let plugins = ctx.app().plugins();
    // `if a.plugin_id:` – an empty ID lists every plugin.
    if let Some(plugin_id) = plugin_id.as_deref().filter(|id| !id.is_empty()) {
        return Ok(json!({"plugin": plugins.get(plugin_id)?.to_json(true)}));
    }
    let kind = kind.map(|kind| match kind {
        args::PluginKind::Effect => PluginKind::Effect,
        args::PluginKind::Instrument => PluginKind::Instrument,
    });
    let listed: Vec<_> = plugins
        .all(kind)
        .into_iter()
        .map(|descriptor| descriptor.to_json(false))
        .collect();
    Ok(json!({
        "plugins": listed,
        "third_party_hosting": "not yet supported (see docs/plugin-hosting.md)",
    }))
}
