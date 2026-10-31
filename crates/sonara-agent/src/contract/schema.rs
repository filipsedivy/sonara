//! JSON Schemas of the tool inputs in pydantic v2's layout, generated from the table
//! (`GenerateJsonSchema` of pydantic 2.14, as the prototype publishes them).
//!
//! `schemars` cannot produce this layout: it titles and orders definitions differently, wraps
//! optional values in its own way and does not reproduce pydantic's rules for titles,
//! descriptions and nested `anyOf`. The schemas are part of the agent contract (CTR-01) and must
//! equal the golden ones, so they are generated here (QG-13). The rules:
//!
//! - every field gets `title` from its name (`time_signature` → `"Time Signature"`), except a
//!   field whose schema is a reference to a nested model (`$ref`, also when nullable);
//! - optional values are `anyOf` with `{"type": "null"}`; a member that is itself only an
//!   `anyOf` is flattened into it, so `int | str | None` has three members but `Position | None`
//!   keeps `Position`'s own `anyOf` and description as one member;
//! - literal defaults appear as `default`; default factories do not;
//! - `required` lists the required fields in field order;
//! - nested models go to `$defs` under their class name, with their own `title`;
//! - every model has `additionalProperties: false`; the top-level `title` is removed
//!   (`schema_of`).

use serde_json::{Map, Value, json};

use super::model::{
    Constant, Field, FieldDefault, Kind, LengthBounds, Model, Number, NumberBounds, StrRules,
};
use super::tools::{TOOLS, ToolSpec};
use super::versions::API_VERSION;

impl Model {
    /// The model's JSON Schema as the prototype publishes it for a tool (`schema_of`):
    /// `model_json_schema()` without the top-level `title`, with nested models in `$defs`.
    #[must_use]
    pub fn input_schema(&self) -> Value {
        let mut defs = Map::new();
        let mut schema = model_schema(self, &mut defs);
        schema.remove("title");
        if !defs.is_empty() {
            schema.insert("$defs".to_owned(), Value::Object(defs));
        }
        Value::Object(schema)
    }
}

impl ToolSpec {
    /// The tool as it appears in the tool contract (`ToolSpec.to_json`):
    /// `{name, title, description, safety, input_schema}`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description(),
            "safety": self.safety.as_str(),
            "input_schema": self.input.input_schema(),
        })
    }
}

/// The tool contract: `{api_version, tools}` with every tool in registration order. It is
/// `golden/contracts/tools.json` plus the additions of [`versions::ADDITIONS`].
///
/// [`versions::ADDITIONS`]: super::versions::ADDITIONS
#[must_use]
pub fn tool_contract() -> Value {
    json!({
        "api_version": API_VERSION,
        "tools": TOOLS.iter().map(ToolSpec::to_json).collect::<Vec<_>>(),
    })
}

fn model_schema(model: &Model, defs: &mut Map<String, Value>) -> Map<String, Value> {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for field in model.fields {
        properties.insert(
            field.name.to_owned(),
            Value::Object(field_schema(field, defs)),
        );
        if field.is_required() {
            required.push(Value::from(field.name));
        }
    }
    let mut schema = Map::new();
    schema.insert("additionalProperties".to_owned(), Value::Bool(false));
    schema.insert("properties".to_owned(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".to_owned(), Value::Array(required));
    }
    schema.insert("title".to_owned(), Value::from(model.name));
    schema.insert("type".to_owned(), Value::from("object"));
    schema
}

fn field_schema(field: &Field, defs: &mut Map<String, Value>) -> Map<String, Value> {
    let mut schema = kind_schema(&field.kind, defs);
    if let FieldDefault::Value(constant) = field.default {
        schema.insert("default".to_owned(), constant_json(constant));
    }
    // A field description replaces the description of an annotated type at the same level.
    if let Some(description) = field.description {
        schema.insert("description".to_owned(), Value::from(description));
    }
    if !schema.contains_key("title") && title_should_be_set(&field.kind) {
        schema.insert("title".to_owned(), Value::from(title_from_name(field.name)));
    }
    schema
}

/// pydantic's `field_title_should_be_set`: no title for a field whose schema is a reference to a
/// model, looking through the nullable wrapper.
fn title_should_be_set(kind: &Kind) -> bool {
    match kind {
        Kind::Model(_) => false,
        Kind::Nullable(inner) | Kind::Described(inner, _) => title_should_be_set(inner),
        _ => true,
    }
}

/// pydantic's `get_title_from_name`: `name.title().replace("_", " ").strip()`.
fn title_from_name(name: &str) -> String {
    // Python's `str.title`: a cased character after a cased character is lowercased, any other
    // one is uppercased.
    let mut titled = String::with_capacity(name.len());
    let mut previous_is_cased = false;
    for c in name.chars() {
        if previous_is_cased {
            titled.extend(c.to_lowercase());
        } else {
            titled.extend(c.to_uppercase());
        }
        previous_is_cased = c.is_lowercase() || c.is_uppercase();
    }
    titled.replace('_', " ").trim().to_owned()
}

fn kind_schema(kind: &Kind, defs: &mut Map<String, Value>) -> Map<String, Value> {
    match kind {
        Kind::Bool => typed("boolean"),
        Kind::Int(bounds) => number_schema("integer", bounds),
        Kind::Float(bounds) => number_schema("number", bounds),
        Kind::Str(rules) => str_schema(rules),
        Kind::StrLiteral(values) => {
            literal_schema(values.iter().map(|v| Value::from(*v)), "string")
        }
        Kind::IntLiteral(values) => {
            literal_schema(values.iter().map(|v| Value::from(*v)), "integer")
        }
        Kind::List(item, length) => {
            let mut schema = Map::new();
            schema.insert("items".to_owned(), Value::Object(kind_schema(item, defs)));
            schema.insert("type".to_owned(), Value::from("array"));
            insert_length(&mut schema, length, "minItems", "maxItems");
            schema
        }
        Kind::Dict(value) => {
            let mut schema = Map::new();
            schema.insert(
                "additionalProperties".to_owned(),
                Value::Object(kind_schema(value, defs)),
            );
            schema.insert("type".to_owned(), Value::from("object"));
            schema
        }
        Kind::Union(members) => {
            let schemas = members.iter().map(|m| kind_schema(m, defs)).collect();
            flattened_any_of(schemas)
        }
        Kind::Nullable(inner) => {
            let inner = kind_schema(inner, defs);
            flattened_any_of(vec![inner, typed("null")])
        }
        Kind::Model(model) => {
            if !defs.contains_key(model.name) {
                let schema = model_schema(model, defs);
                defs.insert(model.name.to_owned(), Value::Object(schema));
            }
            let mut schema = Map::new();
            schema.insert(
                "$ref".to_owned(),
                Value::from(format!("#/$defs/{}", model.name)),
            );
            schema
        }
        Kind::Described(inner, description) => {
            let mut schema = kind_schema(inner, defs);
            schema.insert("description".to_owned(), Value::from(*description));
            schema
        }
    }
}

fn typed(name: &str) -> Map<String, Value> {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), Value::from(name));
    schema
}

fn number_schema(name: &str, bounds: &NumberBounds) -> Map<String, Value> {
    let mut schema = typed(name);
    let keys = [
        ("minimum", bounds.ge),
        ("exclusiveMinimum", bounds.gt),
        ("maximum", bounds.le),
        ("exclusiveMaximum", bounds.lt),
    ];
    for (key, bound) in keys {
        if let Some(bound) = bound {
            schema.insert(key.to_owned(), number_json(bound));
        }
    }
    schema
}

fn str_schema(rules: &StrRules) -> Map<String, Value> {
    let mut schema = typed("string");
    insert_length(&mut schema, &rules.length, "minLength", "maxLength");
    if let Some(pattern) = rules.pattern {
        schema.insert("pattern".to_owned(), Value::from(pattern.source()));
    }
    schema
}

fn insert_length(schema: &mut Map<String, Value>, length: &LengthBounds, min: &str, max: &str) {
    if let Some(value) = length.min_length {
        schema.insert(min.to_owned(), Value::from(value));
    }
    if let Some(value) = length.max_length {
        schema.insert(max.to_owned(), Value::from(value));
    }
}

/// pydantic's `literal_schema`: `const` for one value, `enum` otherwise, plus the JSON type.
fn literal_schema(values: impl Iterator<Item = Value>, name: &str) -> Map<String, Value> {
    let mut values: Vec<Value> = values.collect();
    let mut schema = Map::new();
    if values.len() == 1 {
        schema.insert("const".to_owned(), values.remove(0));
    } else {
        schema.insert("enum".to_owned(), Value::Array(values));
    }
    schema.insert("type".to_owned(), Value::from(name));
    schema
}

/// pydantic's `get_flattened_anyof`: members that are only an `anyOf` are spliced in, duplicates
/// are dropped, and a single member stands alone.
fn flattened_any_of(schemas: Vec<Map<String, Value>>) -> Map<String, Value> {
    let mut members: Vec<Value> = Vec::new();
    for mut schema in schemas {
        let nested = match schema.get("anyOf") {
            Some(Value::Array(_)) if schema.len() == 1 => schema.remove("anyOf"),
            _ => None,
        };
        let candidates = match nested {
            Some(Value::Array(inner)) => inner,
            _ => vec![Value::Object(schema)],
        };
        for candidate in candidates {
            if !members.contains(&candidate) {
                members.push(candidate);
            }
        }
    }
    if members.len() == 1
        && let Some(Value::Object(single)) = members.pop()
    {
        return single;
    }
    let mut schema = Map::new();
    schema.insert("anyOf".to_owned(), Value::Array(members));
    schema
}

fn number_json(number: Number) -> Value {
    match number {
        Number::Int(value) => Value::from(value),
        Number::Float(value) => float_json(value),
    }
}

/// A JSON float; the table holds finite values only.
pub(crate) fn float_json(value: f64) -> Value {
    serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
}

pub(crate) fn constant_json(constant: Constant) -> Value {
    match constant {
        Constant::Null => Value::Null,
        Constant::Bool(value) => Value::Bool(value),
        Constant::Int(value) => Value::from(value),
        Constant::Float(value) => float_json(value),
        Constant::Str(value) => Value::from(value),
    }
}

#[cfg(test)]
mod tests {
    use super::title_from_name;

    #[test]
    fn titles_follow_python_str_title() {
        assert_eq!(title_from_name("time_signature"), "Time Signature");
        assert_eq!(title_from_name("bitrate_kbps"), "Bitrate Kbps");
        assert_eq!(title_from_name("dry_run"), "Dry Run");
        // A letter after a digit starts a new word in `str.title`.
        assert_eq!(title_from_name("rf64x"), "Rf64X");
        assert_eq!(title_from_name("_private_"), "Private");
    }
}
