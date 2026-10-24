//! Argument validation with pydantic v2's semantics, driven by the same table as the schemas.
//!
//! The prototype validates with `Model.model_validate(arguments)`: lax mode on Python objects
//! decoded from JSON, `extra="forbid"`, and `str_strip_whitespace=True` in the `ToolInput`
//! models. This module reproduces it:
//!
//! - **Coercions.** `bool` accepts `0`/`1`, `0.0`/`1.0` and the spellings `true`/`yes`/`on`/`t`/`y`/
//!   `1` (and their negations, any case, never trimmed). `int` accepts a bool, an integral float
//!   and a numeric string. `float` accepts a bool, an integer and a numeric string. `str` accepts
//!   only strings. Literals compare like Python: `48000.0` and `true` match the integer literals
//!   `48000` and `1`; strings never match an integer literal.
//! - **Strings** are stripped (in `ToolInput` models), then checked for length (characters) and
//!   pattern. Literals and booleans are never stripped. Dict keys are stripped, but errors are
//!   reported at the original key.
//! - **Unions** use pydantic's smart mode: an exact match wins at once, otherwise the most exact
//!   successful member (`exact` > `strict` > `lax`, ties to the first). When every member fails,
//!   each member's errors are reported under its label (`int`, `str`, `constrained-str`,
//!   `list[float]`, `literal['auto','none']`).
//! - **Errors** come in pydantic's order (fields in model order, then extra keys in input order)
//!   with pydantic's messages. A list longer than its maximum reports only that.
//! - **Defaults** are filled in unvalidated, as pydantic does; factories produce fresh values.
//!
//! The result is the model's values after coercion and defaults, as a JSON object with every
//! field (`model_dump(mode="json")`), which deserializes into the typed argument structs of
//! [`super::args`].
//!
//! Two inputs cannot be carried by JSON values and are rejected instead of reproduced:
//!
//! - a float that is infinite or NaN (only reachable from strings such as `"inf"`) fails with
//!   pydantic's `finite_number` error, as pydantic itself does with `allow_inf_nan=False`;
//! - an integer outside the `i64` range fails with pydantic's `int_parsing_size` error; a JSON
//!   integer outside that range is treated as a float, as `serde_json` reads it.

use std::cmp::Ordering;

use serde_json::{Map, Value};
use sonara_core::error::DomainError;
use sonara_core::pyfmt::str_repr;

use super::coerce::{self, IntError};
use super::model::{
    Factory, FieldDefault, Kind, LengthBounds, Model, Number, NumberBounds, StrRules,
};
use super::schema::{constant_json, float_json};
use super::tools::ToolSpec;

/// One validation error in the prototype's envelope: pydantic's `loc` joined with dots (or
/// `(root)`) and its `msg`, unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    /// Where the error is, for example `notes.0.pitch.int`.
    pub field: String,
    /// pydantic's message, for example `Input should be a valid integer`.
    pub message: String,
}

/// Validates `arguments` against the tool's input model.
///
/// # Errors
///
/// [`sonara_core::error::ErrorCode::InvalidArgument`] with the message
/// `invalid arguments for <tool>` and `details.errors = [{field, message}, …]`, as the
/// prototype's registry reports a pydantic `ValidationError`.
pub fn validate(tool: &ToolSpec, arguments: &Value) -> Result<Map<String, Value>, DomainError> {
    validate_model(tool.input, arguments).map_err(|errors| invalid_arguments(tool.name, &errors))
}

/// Validates `input` against `model` and returns the normalized values.
///
/// # Errors
///
/// Every failed check, in pydantic's order.
pub fn validate_model(model: &Model, input: &Value) -> Result<Map<String, Value>, Vec<FieldError>> {
    model_fields(model, input)
        .map(|(values, _)| values)
        .map_err(|errors| {
            errors
                .into_iter()
                .map(LineError::into_field_error)
                .collect()
        })
}

/// The prototype's `INVALID_ARGUMENT` error for failed validation.
#[must_use]
pub fn invalid_arguments(tool: &str, errors: &[FieldError]) -> DomainError {
    let errors: Vec<Value> = errors
        .iter()
        .map(|error| {
            let mut entry = Map::new();
            entry.insert("field".to_owned(), Value::from(error.field.as_str()));
            entry.insert("message".to_owned(), Value::from(error.message.as_str()));
            Value::Object(entry)
        })
        .collect();
    DomainError::invalid(format!("invalid arguments for {tool}"))
        .with_detail("errors", Value::Array(errors))
}

// -- errors ----------------------------------------------------------------------------------

const MISSING: &str = "Field required";
const EXTRA_FORBIDDEN: &str = "Extra inputs are not permitted";
const DICT_TYPE: &str = "Input should be a valid dictionary";
const LIST_TYPE: &str = "Input should be a valid list";
const STRING_TYPE: &str = "Input should be a valid string";
const BOOL_TYPE: &str = "Input should be a valid boolean";
const BOOL_PARSING: &str = "Input should be a valid boolean, unable to interpret input";
const INT_TYPE: &str = "Input should be a valid integer";
const INT_PARSING: &str = "Input should be a valid integer, unable to parse string as an integer";
const INT_FROM_FLOAT: &str = "Input should be a valid integer, got a number with a fractional part";
const INT_PARSING_SIZE: &str = "Unable to parse input string as an integer, exceeded maximum size";
const FLOAT_TYPE: &str = "Input should be a valid number";
const FLOAT_PARSING: &str = "Input should be a valid number, unable to parse string as a number";
const FINITE_NUMBER: &str = "Input should be a finite number";

/// A pydantic line error under construction: the location is collected innermost first while
/// the error travels outwards.
#[derive(Debug)]
struct LineError {
    reversed_loc: Vec<String>,
    message: String,
}

impl LineError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            reversed_loc: Vec::new(),
            message: message.into(),
        }
    }

    fn into_field_error(self) -> FieldError {
        let mut loc = self.reversed_loc;
        loc.reverse();
        let field = if loc.is_empty() {
            "(root)".to_owned()
        } else {
            loc.join(".")
        };
        FieldError {
            field,
            message: self.message,
        }
    }
}

/// Prefixes the location of every error with `part`.
fn within(mut errors: Vec<LineError>, part: &str) -> Vec<LineError> {
    for error in &mut errors {
        error.reversed_loc.push(part.to_owned());
    }
    errors
}

fn int_error(error: IntError) -> LineError {
    LineError::new(match error {
        IntError::Parsing => INT_PARSING,
        IntError::Size => INT_PARSING_SIZE,
        IntError::FromFloat => INT_FROM_FLOAT,
        IntError::Finite => FINITE_NUMBER,
    })
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

// -- values ----------------------------------------------------------------------------------

/// How closely a value matched its kind (pydantic-core's `Exactness`); smart unions pick the
/// most exact member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Exactness {
    /// Converted in lax mode only (for example a string to a number).
    Lax,
    /// Converted in strict mode (an integer to a float).
    Strict,
    /// The input already had the type.
    Exact,
}

type Outcome = Result<(Value, Exactness), Vec<LineError>>;

/// A JSON number as Python's `json` module would decode it, except that integers outside the
/// `i64` range become floats (see the module documentation).
enum JsonNumber {
    Int(i64),
    Float(f64),
}

fn json_number(number: &serde_json::Number) -> JsonNumber {
    match number.as_i64() {
        Some(value) => JsonNumber::Int(value),
        None => JsonNumber::Float(number.as_f64().unwrap_or(f64::NAN)),
    }
}

fn validate_kind(kind: &Kind, input: &Value, strip: bool) -> Outcome {
    match kind {
        Kind::Bool => validate_bool(input)
            .map(|(value, exactness)| (Value::Bool(value), exactness))
            .map_err(|error| vec![error]),
        Kind::Int(bounds) => validate_int(input, bounds)
            .map(|(value, exactness)| (Value::from(value), exactness))
            .map_err(|error| vec![error]),
        Kind::Float(bounds) => validate_float(input, bounds)
            .map(|(value, exactness)| (float_json(value), exactness))
            .map_err(|error| vec![error]),
        Kind::Str(rules) => validate_str(input, rules, strip)
            .map(|value| (Value::String(value), Exactness::Exact))
            .map_err(|error| vec![error]),
        Kind::StrLiteral(values) => match input {
            Value::String(text) if values.contains(&text.as_str()) => {
                Ok((input.clone(), Exactness::Exact))
            }
            _ => Err(vec![literal_error(kind)]),
        },
        Kind::IntLiteral(values) => match int_literal_candidate(input) {
            Some(value) if values.contains(&value) => Ok((Value::from(value), Exactness::Exact)),
            _ => Err(vec![literal_error(kind)]),
        },
        Kind::List(item, length) => validate_list(item, length, input, strip),
        Kind::Dict(value) => validate_dict(value, input, strip),
        Kind::Union(members) => validate_union(members, input, strip),
        Kind::Nullable(inner) => match input {
            Value::Null => Ok((Value::Null, Exactness::Exact)),
            _ => validate_kind(inner, input, strip),
        },
        Kind::Model(model) => {
            model_fields(model, input).map(|(values, exactness)| (Value::Object(values), exactness))
        }
        Kind::Described(inner, _) => validate_kind(inner, input, strip),
    }
}

fn validate_bool(input: &Value) -> Result<(bool, Exactness), LineError> {
    match input {
        Value::Bool(value) => Ok((*value, Exactness::Exact)),
        Value::String(text) => coerce::str_as_bool(text)
            .map(|value| (value, Exactness::Lax))
            .ok_or_else(|| LineError::new(BOOL_PARSING)),
        Value::Number(number) => {
            let integer = match json_number(number) {
                JsonNumber::Int(value) => value,
                JsonNumber::Float(value) => {
                    coerce::float_as_int(value).map_err(|_| LineError::new(BOOL_TYPE))?
                }
            };
            coerce::int_as_bool(integer)
                .map(|value| (value, Exactness::Lax))
                .ok_or_else(|| LineError::new(BOOL_PARSING))
        }
        _ => Err(LineError::new(BOOL_TYPE)),
    }
}

fn validate_int(input: &Value, bounds: &NumberBounds) -> Result<(i64, Exactness), LineError> {
    let (value, exactness) = match input {
        Value::Bool(value) => (i64::from(*value), Exactness::Lax),
        Value::Number(number) => match json_number(number) {
            JsonNumber::Int(value) => (value, Exactness::Exact),
            JsonNumber::Float(value) => (
                coerce::float_as_int(value).map_err(int_error)?,
                Exactness::Lax,
            ),
        },
        Value::String(text) => (coerce::str_as_int(text).map_err(int_error)?, Exactness::Lax),
        _ => return Err(LineError::new(INT_TYPE)),
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "the table has no float bounds on integer fields; one would be compared in f64"
    )]
    let compare = |bound: Number| match bound {
        Number::Int(bound) => Some(value.cmp(&bound)),
        Number::Float(bound) => (value as f64).partial_cmp(&bound),
    };
    check_bounds(bounds, compare)?;
    Ok((value, exactness))
}

fn validate_float(input: &Value, bounds: &NumberBounds) -> Result<(f64, Exactness), LineError> {
    let (value, exactness) = match input {
        Value::Bool(value) => (f64::from(u8::from(*value)), Exactness::Lax),
        Value::Number(number) => match json_number(number) {
            #[expect(
                clippy::cast_precision_loss,
                reason = "Python converts an int to the nearest float the same way"
            )]
            JsonNumber::Int(value) => (value as f64, Exactness::Strict),
            JsonNumber::Float(value) => (value, Exactness::Exact),
        },
        Value::String(text) => (
            coerce::str_as_float(text).ok_or_else(|| LineError::new(FLOAT_PARSING))?,
            Exactness::Lax,
        ),
        _ => return Err(LineError::new(FLOAT_TYPE)),
    };
    if !value.is_finite() {
        return Err(LineError::new(FINITE_NUMBER));
    }
    check_bounds(bounds, |bound| value.partial_cmp(&bound.as_f64()))?;
    Ok((value, exactness))
}

/// A bound, the orderings that satisfy it and its wording in the error message.
type BoundCheck = (Option<Number>, fn(Ordering) -> bool, &'static str);

/// pydantic-core's constraint order (`le`, `lt`, `ge`, `gt`); the first failure is reported.
/// `compare` returns the value's ordering against a bound.
fn check_bounds(
    bounds: &NumberBounds,
    compare: impl Fn(Number) -> Option<Ordering>,
) -> Result<(), LineError> {
    let checks: [BoundCheck; 4] = [
        (bounds.le, Ordering::is_le, "less than or equal to"),
        (bounds.lt, Ordering::is_lt, "less than"),
        (bounds.ge, Ordering::is_ge, "greater than or equal to"),
        (bounds.gt, Ordering::is_gt, "greater than"),
    ];
    for (bound, holds, relation) in checks {
        if let Some(bound) = bound
            && !compare(bound).is_some_and(holds)
        {
            return Err(LineError::new(format!(
                "Input should be {relation} {bound}"
            )));
        }
    }
    Ok(())
}

fn validate_str(input: &Value, rules: &StrRules, strip: bool) -> Result<String, LineError> {
    let Value::String(text) = input else {
        return Err(LineError::new(STRING_TYPE));
    };
    let text = if strip { text.trim() } else { text.as_str() };
    let length = text.chars().count();
    if let Some(min) = rules.length.min_length
        && length < min
    {
        return Err(LineError::new(format!(
            "String should have at least {min} character{}",
            plural(min)
        )));
    }
    if let Some(max) = rules.length.max_length
        && length > max
    {
        return Err(LineError::new(format!(
            "String should have at most {max} character{}",
            plural(max)
        )));
    }
    if let Some(pattern) = rules.pattern
        && !pattern.regex().is_match(text)
    {
        return Err(LineError::new(format!(
            "String should match pattern '{}'",
            pattern.source()
        )));
    }
    Ok(text.to_owned())
}

/// The integer an input equals in Python (`True == 1`, `48000.0 == 48000`); strings never do.
fn int_literal_candidate(input: &Value) -> Option<i64> {
    match input {
        Value::Bool(value) => Some(i64::from(*value)),
        Value::Number(number) => match json_number(number) {
            JsonNumber::Int(value) => Some(value),
            JsonNumber::Float(value) => coerce::float_as_int(value).ok(),
        },
        _ => None,
    }
}

/// The reprs of a literal's values, as pydantic-core lists them.
fn literal_reprs(kind: &Kind) -> Vec<String> {
    match kind {
        Kind::StrLiteral(values) => values.iter().map(|value| str_repr(value)).collect(),
        Kind::IntLiteral(values) => values.iter().map(ToString::to_string).collect(),
        _ => Vec::new(),
    }
}

/// `literal_error`: `Input should be 'a', 'b' or 'c'`.
fn literal_error(kind: &Kind) -> LineError {
    let mut reprs = literal_reprs(kind);
    let last = reprs.pop().unwrap_or_default();
    let expected = if reprs.is_empty() {
        last
    } else {
        format!("{} or {last}", reprs.join(", "))
    };
    LineError::new(format!("Input should be {expected}"))
}

fn validate_list(item: &Kind, length: &LengthBounds, input: &Value, strip: bool) -> Outcome {
    let Value::Array(items) = input else {
        return Err(vec![LineError::new(LIST_TYPE)]);
    };
    // pydantic-core stops at the first item past the maximum and reports only the length.
    if let Some(max) = length.max_length
        && items.len() > max
    {
        return Err(vec![LineError::new(format!(
            "List should have at most {max} item{} after validation, not {}",
            plural(max),
            items.len()
        ))]);
    }
    let mut values = Vec::with_capacity(items.len());
    let mut errors = Vec::new();
    let mut exactness = Exactness::Exact;
    for (index, element) in items.iter().enumerate() {
        match validate_kind(item, element, strip) {
            Ok((value, element_exactness)) => {
                exactness = exactness.min(element_exactness);
                values.push(value);
            }
            Err(element_errors) => errors.extend(within(element_errors, &index.to_string())),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    if let Some(min) = length.min_length
        && values.len() < min
    {
        return Err(vec![LineError::new(format!(
            "List should have at least {min} item{} after validation, not {}",
            plural(min),
            values.len()
        ))]);
    }
    Ok((Value::Array(values), exactness))
}

fn validate_dict(value_kind: &Kind, input: &Value, strip: bool) -> Outcome {
    let Value::Object(entries) = input else {
        return Err(vec![LineError::new(DICT_TYPE)]);
    };
    let mut values = Map::new();
    let mut errors = Vec::new();
    let mut exactness = Exactness::Exact;
    for (key, element) in entries {
        match validate_kind(value_kind, element, strip) {
            Ok((value, element_exactness)) => {
                exactness = exactness.min(element_exactness);
                let key = if strip { key.trim() } else { key.as_str() };
                values.insert(key.to_owned(), value);
            }
            Err(element_errors) => errors.extend(within(element_errors, key)),
        }
    }
    if errors.is_empty() {
        Ok((Value::Object(values), exactness))
    } else {
        Err(errors)
    }
}

fn validate_union(members: &[Kind], input: &Value, strip: bool) -> Outcome {
    let mut best: Option<(Value, Exactness)> = None;
    let mut errors = Vec::new();
    for member in members {
        match validate_kind(member, input, strip) {
            Ok((value, Exactness::Exact)) => return Ok((value, Exactness::Exact)),
            Ok((value, exactness)) => {
                if best
                    .as_ref()
                    .is_none_or(|(_, current)| *current < exactness)
                {
                    best = Some((value, exactness));
                }
            }
            Err(member_errors) => {
                if best.is_none() {
                    errors.extend(within(member_errors, &label(member, strip)));
                }
            }
        }
    }
    best.ok_or(errors)
}

/// The name pydantic-core gives a union member's validator, used as its location tag.
fn label(kind: &Kind, strip: bool) -> String {
    match kind {
        Kind::Bool => "bool".to_owned(),
        Kind::Int(bounds) => numeric_label("int", bounds),
        Kind::Float(bounds) => numeric_label("float", bounds),
        Kind::Str(rules) => str_label(rules, strip).to_owned(),
        Kind::StrLiteral(_) | Kind::IntLiteral(_) => {
            format!("literal[{}]", literal_reprs(kind).join(","))
        }
        Kind::List(item, _) => format!("list[{}]", label(item, strip)),
        Kind::Dict(value) => format!(
            "dict[{},{}]",
            str_label(&StrRules::NONE, strip),
            label(value, strip)
        ),
        Kind::Union(members) => format!(
            "union[{}]",
            members
                .iter()
                .map(|member| label(member, strip))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Kind::Nullable(inner) => format!("nullable[{}]", label(inner, strip)),
        Kind::Model(model) => model.name.to_owned(),
        Kind::Described(inner, _) => label(inner, strip),
    }
}

fn numeric_label(name: &str, bounds: &NumberBounds) -> String {
    if *bounds == NumberBounds::NONE {
        name.to_owned()
    } else {
        format!("constrained-{name}")
    }
}

fn str_label(rules: &StrRules, strip: bool) -> &'static str {
    if strip || rules.is_constrained() {
        "constrained-str"
    } else {
        "str"
    }
}

// -- models ----------------------------------------------------------------------------------

/// Validates a model's fields: present fields in field order, then defaults, then extra keys in
/// input order.
fn model_fields(
    model: &Model,
    input: &Value,
) -> Result<(Map<String, Value>, Exactness), Vec<LineError>> {
    let Value::Object(entries) = input else {
        return Err(vec![LineError::new(format!(
            "Input should be a valid dictionary or instance of {}",
            model.name
        ))]);
    };
    let mut values = Map::new();
    let mut errors = Vec::new();
    let mut exactness = Exactness::Exact;
    for field in model.fields {
        let Some(element) = entries.get(field.name) else {
            match field.default {
                FieldDefault::Required => errors.push(LineError {
                    reversed_loc: vec![field.name.to_owned()],
                    message: MISSING.to_owned(),
                }),
                FieldDefault::Value(constant) => {
                    values.insert(field.name.to_owned(), constant_json(constant));
                }
                FieldDefault::Factory(factory) => {
                    values.insert(field.name.to_owned(), factory_value(factory));
                }
            }
            continue;
        };
        match validate_kind(&field.kind, element, model.strip_whitespace) {
            Ok((value, field_exactness)) => {
                if let Some(message) = field.after.and_then(|after| after(&value)) {
                    errors.push(LineError {
                        reversed_loc: vec![field.name.to_owned()],
                        message: format!("Value error, {message}"),
                    });
                } else {
                    exactness = exactness.min(field_exactness);
                    values.insert(field.name.to_owned(), value);
                }
            }
            Err(field_errors) => errors.extend(within(field_errors, field.name)),
        }
    }
    for key in entries.keys() {
        if model.field(key).is_none() {
            errors.push(LineError {
                reversed_loc: vec![key.clone()],
                message: EXTRA_FORBIDDEN.to_owned(),
            });
        }
    }
    if errors.is_empty() {
        Ok((values, exactness))
    } else {
        Err(errors)
    }
}

/// The value a default factory produces.
fn factory_value(factory: Factory) -> Value {
    match factory {
        Factory::EmptyList => Value::Array(Vec::new()),
        Factory::EmptyDict => Value::Object(Map::new()),
        Factory::Model(model) => Value::Object(default_instance(model)),
        Factory::ModelList(model) => Value::Array(vec![Value::Object(default_instance(model))]),
    }
}

/// `Model()`: every field at its default. A factory model has no required fields (checked by a
/// unit test), so none is skipped.
fn default_instance(model: &Model) -> Map<String, Value> {
    model
        .fields
        .iter()
        .filter_map(|field| match field.default {
            FieldDefault::Required => None,
            FieldDefault::Value(constant) => Some((field.name.to_owned(), constant_json(constant))),
            FieldDefault::Factory(factory) => Some((field.name.to_owned(), factory_value(factory))),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::inputs::{self, PATTERNS};
    use super::super::model::{Factory, FieldDefault};
    use super::super::tools::TOOLS;
    use super::validate_model;

    #[test]
    fn every_pattern_compiles() {
        for pattern in PATTERNS {
            assert!(
                regex::Regex::new(pattern.source()).is_ok(),
                "{}",
                pattern.source()
            );
        }
    }

    #[test]
    fn factory_models_have_no_required_fields() {
        for tool in &TOOLS {
            for field in tool.input.fields {
                if let FieldDefault::Factory(Factory::Model(model) | Factory::ModelList(model)) =
                    field.default
                {
                    assert!(
                        model.fields.iter().all(|f| !f.is_required()),
                        "{}",
                        model.name
                    );
                }
            }
        }
    }

    #[test]
    fn defaults_are_inserted_unvalidated() {
        let values = validate_model(&inputs::CREATE_PROJECT, &json!({})).unwrap();
        assert_eq!(values["bpm"], json!(120.0));
        assert_eq!(values["sample_rate"], json!(48000));
        assert_eq!(values["length"], json!(null));
    }

    #[test]
    fn the_coverage_examples_report_pydantics_errors() {
        let errors = validate_model(
            &inputs::ADD_MIDI_NOTES,
            &json!({"clip_id": "c", "notes": [{"pitch": [60], "start": -1, "length": 0}]}),
        )
        .unwrap_err();
        let fields: Vec<_> = errors
            .iter()
            .map(|e| (e.field.as_str(), e.message.as_str()))
            .collect();
        assert_eq!(
            fields,
            [
                ("notes.0.pitch.int", "Input should be a valid integer"),
                ("notes.0.pitch.str", "Input should be a valid string"),
                (
                    "notes.0.start",
                    "Input should be greater than or equal to 0"
                ),
                ("notes.0.length", "Input should be greater than 0"),
            ]
        );
    }
}
