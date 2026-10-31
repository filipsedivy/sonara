//! The versions of the agent API and what each adds to the recorded contract (CTR-01).
//!
//! The tool contract was recorded at version 1.0 (`golden/contracts/tools.json`,
//! `golden/contracts/mcp-tools-list.json`, the IPC recording and the golden transcripts), and
//! the recording is never re-recorded (GLD-01). A later version is additive: it adds tools and
//! optional fields of input models, and keeps every tool, field, default, description, result
//! and error code of 1.0, so a call that was valid in 1.0 stays valid and means the same.
//!
//! [`ADDITIONS`] records what [`API_VERSION`] adds, one entry per added tool or field. The golden
//! tests take exactly these additions out of what this build publishes ([`recorded_view`],
//! [`recorded_tools`]), compare the rest with the recording, and check that every recorded
//! addition is published ([`missing_additions`]). So the published contract is the recording
//! plus exactly the recorded additions: an unrecorded change, or a record without its change,
//! fails those tests.
//!
//! A change that is not an addition (a changed default or description, a new required field, a
//! removed tool) cannot be recorded here: it needs a new major version, a decision of its own.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::tools::find;

/// The agent API version this build speaks: reported by `get_capabilities`, by the IPC `hello`
/// and `info` replies, and in the tool contract.
pub const API_VERSION: &str = "1.1";

/// The API version of the recorded contract: the version the golden contract, the IPC
/// recording and the golden transcripts carry.
pub const RECORDED_API_VERSION: &str = "1.0";

/// One addition of [`API_VERSION`] to the recorded contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Addition {
    /// A new tool, by name.
    Tool(&'static str),
    /// A new optional field of an input model, wherever the model appears: as a tool's input
    /// (the top-level properties of its schema) or nested (under `$defs`, for example
    /// `NoteInput`).
    Field {
        /// The model's class name ([`Model::name`](super::Model)), for example `NoteInput`.
        model: &'static str,
        /// The field name.
        field: &'static str,
    },
}

/// What [`API_VERSION`] adds to [`RECORDED_API_VERSION`], in the order the additions were made.
/// A tool or field added to the contract is recorded here in the same change.
pub static ADDITIONS: &[Addition] = &[];

/// Whether `name` is a tool that [`API_VERSION`] adds.
#[must_use]
pub fn is_added_tool(name: &str) -> bool {
    added_tool(ADDITIONS, name)
}

/// `document` as the recorded version publishes it, for the documents that carry the version or
/// the tools: the tool contract (`{api_version, tools}`), the MCP listing, the IPC `hello` and
/// `info` results and the `get_capabilities` result.
///
/// - A top-level `api_version` equal to [`API_VERSION`] becomes [`RECORDED_API_VERSION`].
/// - A top-level tool count (`tools`, a number) loses the added tools.
/// - A top-level tool list (`tools`, an array) becomes [`recorded_tools`] of it.
///
/// Every other value is kept as it is, so the view equals the recording only when the document
/// differs from it by exactly the recorded additions.
#[must_use]
pub fn recorded_view(document: &Value) -> Value {
    view(document, ADDITIONS)
}

/// A tool listing as the recorded version publishes it: without the added tools, and with the
/// added fields taken out of every other tool's input schema (`input_schema` in the tool
/// contract and the IPC `tools/list`, `inputSchema` in the MCP listing). A nested model that only
/// an added field used goes with it. Everything else is kept as it is.
#[must_use]
pub fn recorded_tools(tools: &[Value]) -> Vec<Value> {
    strip(tools, ADDITIONS)
}

/// The recorded additions that `tools` (a listing in either form of [`recorded_tools`]) does not
/// show: an added tool that is not listed, or an added field that no listed tool's schema has.
/// Anything here is recorded in [`ADDITIONS`] but not published.
#[must_use]
pub fn missing_additions(tools: &[Value]) -> Vec<Addition> {
    missing(tools, ADDITIONS)
}

fn added_tool(additions: &[Addition], name: &str) -> bool {
    additions
        .iter()
        .any(|addition| matches!(addition, Addition::Tool(tool) if *tool == name))
}

fn view(document: &Value, additions: &[Addition]) -> Value {
    let Value::Object(map) = document else {
        return document.clone();
    };
    let mut out = map.clone();
    if out.get("api_version").and_then(Value::as_str) == Some(API_VERSION) {
        out.insert("api_version".to_owned(), Value::from(RECORDED_API_VERSION));
    }
    match out.get_mut("tools") {
        Some(Value::Array(tools)) => *tools = strip(tools, additions),
        Some(Value::Number(count)) => {
            let added = additions
                .iter()
                .filter(|addition| matches!(addition, Addition::Tool(_)))
                .count();
            if let Some(recorded) = count
                .as_u64()
                .zip(u64::try_from(added).ok())
                .and_then(|(count, added)| count.checked_sub(added))
            {
                *count = recorded.into();
            }
        }
        _ => {}
    }
    Value::Object(out)
}

fn strip(tools: &[Value], additions: &[Addition]) -> Vec<Value> {
    tools
        .iter()
        .filter(|tool| {
            !tool
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| added_tool(additions, name))
        })
        .map(|tool| {
            let mut tool = tool.clone();
            strip_fields(&mut tool, additions);
            tool
        })
        .collect()
}

/// The class name of the input model of the tool `tool` (a listing entry), when it is a tool of
/// the contract.
fn input_model(tool: &Value) -> Option<&'static str> {
    tool.get("name")
        .and_then(Value::as_str)
        .and_then(find)
        .map(|spec| spec.input.name)
}

/// The input schema of a listing entry: `input_schema` (tool contract) or `inputSchema` (MCP).
fn schema_mut(tool: &mut Value) -> Option<&mut Map<String, Value>> {
    let tool = tool.as_object_mut()?;
    let key = if tool.contains_key("input_schema") {
        "input_schema"
    } else {
        "inputSchema"
    };
    tool.get_mut(key).and_then(Value::as_object_mut)
}

/// The input schema of a listing entry, read only.
fn schema(tool: &Value) -> Option<&Map<String, Value>> {
    tool.get("input_schema")
        .or_else(|| tool.get("inputSchema"))
        .and_then(Value::as_object)
}

/// Whether the properties of `object` (a schema or a `$defs` entry) include `field`.
fn has_property(object: &Map<String, Value>, field: &str) -> bool {
    object
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|properties| properties.contains_key(field))
}

/// Takes `field` out of the properties of `object`; whether it was there.
fn remove_property(object: &mut Map<String, Value>, field: &str) -> bool {
    object
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .is_some_and(|properties| properties.shift_remove(field).is_some())
}

/// The `$defs` entry of the model `model` in `schema`.
fn definition<'a>(schema: &'a Map<String, Value>, model: &str) -> Option<&'a Map<String, Value>> {
    schema
        .get("$defs")
        .and_then(|defs| defs.get(model))
        .and_then(Value::as_object)
}

/// Whether `schema` has the field `field` of the model `model`: among its top-level properties
/// when the schema is that model's (`top`), or among the properties of its `$defs` entry.
fn has_field(schema: &Map<String, Value>, top: Option<&str>, model: &str, field: &str) -> bool {
    (top == Some(model) && has_property(schema, field))
        || definition(schema, model).is_some_and(|definition| has_property(definition, field))
}

/// Takes the added fields out of the input schema of the listing entry `tool`.
fn strip_fields(tool: &mut Value, additions: &[Addition]) {
    let top = input_model(tool);
    let Some(schema) = schema_mut(tool) else {
        return;
    };
    let mut stripped = false;
    for addition in additions {
        let Addition::Field { model, field } = *addition else {
            continue;
        };
        if top == Some(model) {
            stripped |= remove_property(schema, field);
        }
        if let Some(definition) = schema
            .get_mut("$defs")
            .and_then(|defs| defs.get_mut(model))
            .and_then(Value::as_object_mut)
        {
            stripped |= remove_property(definition, field);
        }
    }
    if stripped {
        prune_defs(schema);
    }
}

/// Removes the `$defs` entries that nothing references any more, and `$defs` itself when it is
/// left empty. The schema generator only writes referenced models, so after an added field is
/// taken out, an unreferenced entry is a model that only that field used.
fn prune_defs(schema: &mut Map<String, Value>) {
    loop {
        let mut used = BTreeSet::new();
        for (key, value) in schema.iter() {
            if key != "$defs" {
                references(value, &mut used);
            }
        }
        let Some(defs) = schema.get_mut("$defs").and_then(Value::as_object_mut) else {
            return;
        };
        for value in defs.values() {
            references(value, &mut used);
        }
        let unused: Vec<String> = defs
            .keys()
            .filter(|name| !used.contains(name.as_str()))
            .cloned()
            .collect();
        if unused.is_empty() {
            return;
        }
        for name in &unused {
            defs.shift_remove(name);
        }
        if defs.is_empty() {
            schema.shift_remove("$defs");
            return;
        }
    }
}

/// Collects the model names of every `{"$ref": "#/$defs/<name>"}` in `value`.
fn references(value: &Value, used: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(name) = map
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.strip_prefix("#/$defs/"))
            {
                used.insert(name.to_owned());
            }
            for item in map.values() {
                references(item, used);
            }
        }
        Value::Array(items) => {
            for item in items {
                references(item, used);
            }
        }
        _ => {}
    }
}

fn missing(tools: &[Value], additions: &[Addition]) -> Vec<Addition> {
    additions
        .iter()
        .filter(|addition| match **addition {
            Addition::Tool(name) => !tools.iter().any(|tool| tool["name"] == name),
            Addition::Field { model, field } => !tools.iter().any(|tool| {
                schema(tool)
                    .is_some_and(|schema| has_field(schema, input_model(tool), model, field))
            }),
        })
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::{
        ADDITIONS, API_VERSION, Addition, RECORDED_API_VERSION, is_added_tool, missing, strip, view,
    };
    use crate::contract::model::{Kind, Model};
    use crate::contract::{TOOLS, tool_contract};
    use crate::mcp;

    /// The additions of a made-up next version: a tool, an optional field of a tool's input
    /// that brings two nested models of its own, and an optional field of a nested model that
    /// several tools use. The names are made up, so that they never meet a real addition.
    const SYNTHETIC: [Addition; 3] = [
        Addition::Tool("made_up_tool"),
        Addition::Field {
            model: "CreateTrack",
            field: "made_up_part",
        },
        Addition::Field {
            model: "NoteInput",
            field: "made_up_articulation",
        },
    ];

    /// The tools of the contract (`input_schema`) and of the MCP listing (`inputSchema`), each
    /// without the real additions: the tool lists of the recorded version.
    fn listings() -> [(Vec<Value>, &'static str); 2] {
        let contract = tool_contract()["tools"]
            .as_array()
            .expect("a tool list")
            .clone();
        [
            (strip(&contract, ADDITIONS), "input_schema"),
            (strip(&mcp::tools(), ADDITIONS), "inputSchema"),
        ]
    }

    fn properties(object: &mut Map<String, Value>) -> &mut Map<String, Value> {
        object
            .get_mut("properties")
            .and_then(Value::as_object_mut)
            .expect("properties")
    }

    /// `tools` with the additions of [`SYNTHETIC`] made, as the schema generator would write
    /// them.
    fn extended(tools: &[Value], key: &str) -> Vec<Value> {
        let mut out = tools.to_vec();
        let mut nested = 0;
        for tool in &mut out {
            let is_create_track = tool["name"] == "create_track";
            let schema = tool[key].as_object_mut().expect("an input schema");
            if is_create_track {
                properties(schema).insert(
                    "made_up_part".to_owned(),
                    json!({"anyOf": [{"$ref": "#/$defs/MadeUpPart"}, {"type": "null"}],
                           "default": null}),
                );
                assert!(
                    schema.get("$defs").is_none(),
                    "create_track has no nested model"
                );
                schema.insert(
                    "$defs".to_owned(),
                    json!({
                        "MadeUpPart": {"additionalProperties": false, "type": "object",
                                       "title": "MadeUpPart",
                                       "properties": {"family": {"$ref": "#/$defs/MadeUpFamily"},
                                                      "number": {"title": "Number",
                                                                 "type": "integer"}}},
                        "MadeUpFamily": {"enum": ["strings", "brass"], "title": "MadeUpFamily",
                                         "type": "string"},
                    }),
                );
            }
            if let Some(note) = schema
                .get_mut("$defs")
                .and_then(|defs| defs.get_mut("NoteInput"))
                .and_then(Value::as_object_mut)
            {
                properties(note).insert(
                    "made_up_articulation".to_owned(),
                    json!({"anyOf": [{"type": "string"}, {"type": "null"}], "default": null,
                           "title": "Made Up Articulation"}),
                );
                nested += 1;
            }
        }
        assert_eq!(
            nested, 3,
            "create_midi_clip, add_midi_notes and edit_midi_notes"
        );
        let mut tool = Map::new();
        tool.insert("name".to_owned(), json!("made_up_tool"));
        tool.insert("title".to_owned(), json!("Made-up tool"));
        tool.insert(key.to_owned(), json!({"type": "object", "properties": {}}));
        out.push(Value::Object(tool));
        out
    }

    #[test]
    fn taking_the_additions_out_gives_back_the_listing() {
        for (tools, key) in listings() {
            let extended = extended(&tools, key);
            assert_ne!(extended, tools);
            assert_eq!(strip(&extended, &SYNTHETIC), tools, "{key}");
            // The models only `made_up_part` used are gone with it, and `$defs` with them.
            let create_track = strip(&extended, &SYNTHETIC)
                .into_iter()
                .find(|tool| tool["name"] == "create_track")
                .expect("create_track");
            assert!(create_track[key].get("$defs").is_none());
        }
    }

    #[test]
    fn an_unrecorded_change_stays_in_the_view() {
        for (tools, key) in listings() {
            let mut changed = extended(&tools, key);
            let create_track = changed
                .iter_mut()
                .find(|tool| tool["name"] == "create_track")
                .expect("create_track");
            let schema = create_track[key].as_object_mut().expect("a schema");
            properties(schema).insert("made_up_voice".to_owned(), json!({"type": "string"}));
            properties(schema)["kind"]["default"] = json!("audio");
            let view = strip(&changed, &SYNTHETIC);
            assert_ne!(view, tools, "{key}");
            let create_track = view
                .iter()
                .find(|tool| tool["name"] == "create_track")
                .expect("create_track");
            assert_eq!(
                create_track[key]["properties"]["made_up_voice"],
                json!({"type": "string"})
            );
            assert_eq!(create_track[key]["properties"]["kind"]["default"], "audio");
        }
    }

    #[test]
    fn a_field_is_taken_out_of_its_own_model_only() {
        let velocity = [Addition::Field {
            model: "NoteInput",
            field: "velocity",
        }];
        let tools = tool_contract()["tools"]
            .as_array()
            .expect("a tool list")
            .clone();
        let view = strip(&tools, &velocity);
        let edit = view
            .iter()
            .find(|tool| tool["name"] == "edit_midi_notes")
            .expect("edit_midi_notes");
        let defs = &edit["input_schema"]["$defs"];
        assert!(defs["NoteInput"]["properties"].get("velocity").is_none());
        assert!(defs["NoteInput"]["properties"].get("pitch").is_some());
        assert!(
            defs["NoteEditInput"]["properties"]
                .get("velocity")
                .is_some()
        );
        // A top-level field of another model with the same name stays too.
        let generate = view
            .iter()
            .find(|tool| tool["name"] == "generate_midi")
            .expect("generate_midi");
        assert!(
            generate["input_schema"]["properties"]
                .get("velocity")
                .is_some()
        );
    }

    #[test]
    fn a_recorded_addition_that_is_not_published_is_missing() {
        for (tools, key) in listings() {
            assert_eq!(missing(&tools, &SYNTHETIC), SYNTHETIC.to_vec(), "{key}");
            assert_eq!(missing(&extended(&tools, key), &SYNTHETIC), [], "{key}");
            // A field of another model with the same name does not count.
            let elsewhere = [Addition::Field {
                model: "NoteInput",
                field: "note_id",
            }];
            assert_eq!(missing(&tools, &elsewhere), elsewhere.to_vec(), "{key}");
            let own = [Addition::Field {
                model: "NoteEditInput",
                field: "note_id",
            }];
            assert_eq!(missing(&tools, &own), [], "{key}");
        }
    }

    #[test]
    fn the_view_puts_back_the_recorded_version_and_tool_count() {
        let hello = json!({"server": "sonara", "protocol": 1, "api_version": API_VERSION,
                           "project": null, "tools": 58});
        assert_eq!(
            view(&hello, &SYNTHETIC).to_string(),
            json!({"server": "sonara", "protocol": 1, "api_version": RECORDED_API_VERSION,
                   "project": null, "tools": 57})
            .to_string(),
            "the keys keep their order"
        );
        let capabilities = json!({"ok": true, "api_version": API_VERSION, "limits": {}});
        assert_eq!(
            view(&capabilities, &[]),
            json!({"ok": true, "api_version": RECORDED_API_VERSION, "limits": {}})
        );
        // Another version is not this build's, and stays as it is.
        let other = json!({"api_version": "2.0", "tools": 3});
        assert_eq!(view(&other, &[]), other);
        // A count smaller than the added tools is left alone.
        assert_eq!(view(&json!({"tools": 0}), &SYNTHETIC), json!({"tools": 0}));
        let [_, (recorded, key)] = listings();
        let listing = json!({"api_version": API_VERSION, "tools": extended(&recorded, key)});
        assert_eq!(
            view(&listing, &SYNTHETIC),
            json!({"api_version": RECORDED_API_VERSION, "tools": recorded})
        );
    }

    /// Every model a tool's input reaches, the inputs included.
    fn models() -> Vec<&'static Model> {
        fn walk(kind: &'static Kind, out: &mut Vec<&'static Model>) {
            match kind {
                Kind::Model(model) => visit(model, out),
                Kind::List(inner, _)
                | Kind::Dict(inner)
                | Kind::Nullable(inner)
                | Kind::Described(inner, _) => walk(inner, out),
                Kind::Union(members) => members.iter().for_each(|member| walk(member, out)),
                _ => {}
            }
        }
        fn visit(model: &'static Model, out: &mut Vec<&'static Model>) {
            if out.iter().any(|seen| std::ptr::eq(*seen, model)) {
                return;
            }
            out.push(model);
            for field in model.fields {
                walk(&field.kind, out);
            }
        }
        let mut out = Vec::new();
        for tool in &TOOLS {
            visit(tool.input, &mut out);
        }
        out
    }

    #[test]
    fn the_version_is_an_additive_step_from_the_recorded_one() {
        let parts = |version: &str| -> (u32, u32) {
            let (major, minor) = version.split_once('.').expect("major.minor");
            (
                major.parse().expect("a major version"),
                minor.parse().expect("a minor version"),
            )
        };
        let (major, minor) = parts(API_VERSION);
        let (recorded_major, recorded_minor) = parts(RECORDED_API_VERSION);
        assert_eq!(major, recorded_major, "an addition keeps the major version");
        assert!(minor > recorded_minor);
    }

    #[test]
    fn every_recorded_addition_is_additive() {
        let models = models();
        assert!(models.iter().any(|model| model.name == "NoteInput"));
        for (i, addition) in ADDITIONS.iter().enumerate() {
            assert!(
                !ADDITIONS[..i].contains(addition),
                "{addition:?} is recorded twice"
            );
            match *addition {
                Addition::Tool(name) => {
                    assert!(TOOLS.iter().any(|tool| tool.name == name), "{name}");
                    assert!(is_added_tool(name));
                }
                Addition::Field { model, field } => {
                    let model = models
                        .iter()
                        .find(|candidate| candidate.name == model)
                        .unwrap_or_else(|| panic!("no tool uses the model {model}"));
                    let field = model
                        .field(field)
                        .unwrap_or_else(|| panic!("{} has no field {field}", model.name));
                    assert!(
                        !field.is_required(),
                        "{}.{} must be optional: a 1.0 call leaves it out",
                        model.name,
                        field.name
                    );
                }
            }
        }
        assert!(!is_added_tool("create_project"));
    }
}
