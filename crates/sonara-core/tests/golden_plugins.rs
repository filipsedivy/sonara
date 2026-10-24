//! The built-in plugin catalog reproduces the prototype's descriptors, and parameter validation
//! resolves every preset to the prototype's values (golden `plugins/`).

use std::collections::BTreeSet;
use std::error::Error;

use serde_json::{Value, json};
use sonara_core::model::device::{ParamValue, Params};
use sonara_core::plugins::{ParamSpec, PluginDescriptor, Preset, builtin_registry};
use sonara_testkit::Golden;

type TestResult = Result<(), Box<dyn Error>>;

/// A parameter value as the recorder's `plain()` writes it: integers and floats stay apart.
fn value_json(value: &ParamValue) -> Value {
    match value {
        ParamValue::Bool(flag) => Value::from(*flag),
        ParamValue::Int(int) => Value::from(*int),
        ParamValue::Float(number) => Value::from(*number),
        ParamValue::Str(text) => Value::from(text.as_str()),
    }
}

fn params_json(params: &Params) -> Value {
    Value::Object(
        params
            .iter()
            .map(|(key, value)| (key.clone(), value_json(value)))
            .collect(),
    )
}

fn optional(value: Option<f64>) -> Value {
    value.map_or(Value::Null, Value::from)
}

/// `plain(ParamSpec)`: every dataclass field under its Python name.
fn plain_spec(spec: &ParamSpec) -> Value {
    json!({
        "automatable": spec.automatable,
        "choices": spec.choices,
        "default": value_json(&spec.default),
        "description": spec.description,
        "id": spec.id,
        "kind": spec.kind.as_str(),
        "maximum": optional(spec.maximum),
        "minimum": optional(spec.minimum),
        "name": spec.name,
        "scale": spec.scale.as_str(),
        "unit": spec.unit,
    })
}

fn plain_preset(preset: &Preset) -> Value {
    json!({
        "description": preset.description,
        "name": preset.name,
        "params": params_json(&preset.params),
    })
}

/// `plain(PluginDescriptor)`: every dataclass field except the factory and private fields.
fn plain_descriptor(d: &PluginDescriptor) -> Value {
    json!({
        "category": d.category,
        "description": d.description,
        "format": d.format.as_str(),
        "id": d.id,
        "kind": d.kind.as_str(),
        "name": d.name,
        "params": d.params.iter().map(plain_spec).collect::<Vec<_>>(),
        "presets": d.presets.iter().map(plain_preset).collect::<Vec<_>>(),
        "sidechain": d.sidechain,
        "tags": d.tags,
        "vendor": d.vendor,
        "version": d.version,
    })
}

/// Compares two JSON trees and names the path of the first difference. Objects compare as maps
/// (the recorder sorts keys); an integer never equals a float.
fn assert_same(expected: &Value, actual: &Value, path: &str) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            let expected_keys: BTreeSet<&String> = e.keys().collect();
            let actual_keys: BTreeSet<&String> = a.keys().collect();
            assert_eq!(expected_keys, actual_keys, "{path}: keys");
            for (key, value) in e {
                assert_same(value, &a[key], &format!("{path}.{key}"));
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            assert_eq!(e.len(), a.len(), "{path}: length");
            for (i, (ev, av)) in e.iter().zip(a).enumerate() {
                assert_same(ev, av, &format!("{path}[{i}]"));
            }
        }
        (Value::Number(e), Value::Number(a)) => {
            assert_eq!(
                e.is_f64(),
                a.is_f64(),
                "{path}: integer versus float ({e} versus {a})"
            );
            assert_eq!(e, a, "{path}");
        }
        _ => assert_eq!(expected, actual, "{path}"),
    }
}

/// The recorder's `_slug`: lower case, runs of other characters than `a-z0-9` become `-`.
fn slug(text: &str) -> String {
    let mut out = String::new();
    for ch in text.to_lowercase().chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_owned()
}

#[test]
fn catalog_matches_the_prototype() -> TestResult {
    let golden = Golden::open()?;
    let expected: Value = golden.json("plugins/catalog.json")?;
    let registry = builtin_registry();
    let actual: Vec<Value> = registry
        .all(None)
        .into_iter()
        .map(plain_descriptor)
        .collect();
    assert_same(&expected, &Value::Array(actual), "catalog");
    Ok(())
}

#[test]
fn presets_resolve_to_the_prototype_values() -> TestResult {
    let golden = Golden::open()?;
    let index: Value = golden.json("plugins/index.json")?;
    let registry = builtin_registry();

    // The recorder's configurations: the defaults, then every preset.
    let mut configs: Vec<(&PluginDescriptor, String, Params)> = Vec::new();
    for d in registry.all(None) {
        configs.push((d, "default".to_owned(), Params::new()));
        for preset in &d.presets {
            configs.push((
                d,
                format!("preset-{}", slug(&preset.name)),
                preset.params.clone(),
            ));
        }
    }
    let listed: Vec<Value> = configs
        .iter()
        .map(|(d, config, _)| json!({"config": config, "kind": d.kind.as_str(), "plugin": d.id}))
        .collect();
    assert_same(&index["configs"], &Value::Array(listed), "index.configs");

    for (d, config, params) in &configs {
        let path = format!("{}/{config}", d.id);
        let recorded: Value = golden.json(&format!("plugins/{path}/config.json"))?;
        assert_eq!(recorded["plugin"], json!(d.id), "{path}");
        // `resolve(validate_params(preset.params))`, with typed values as the recorder passes
        // them.
        let typed = d.resolve(&d.validate_param_values(params)?);
        assert_same(&recorded["params"], &params_json(&typed), &path);
        // The same values as JSON tool arguments.
        let Value::Object(args) = params_json(params) else {
            unreachable!("params_json returns an object")
        };
        let from_json = d.resolve(&d.validate_params(&args)?);
        assert_same(
            &recorded["params"],
            &params_json(&from_json),
            &format!("{path} (JSON)"),
        );
        // The resolved set follows the parameter order.
        assert!(
            from_json.keys().eq(d.params.iter().map(|p| &p.id)),
            "{path}: order"
        );
    }
    Ok(())
}

#[test]
fn slugs_match_the_recorder() {
    assert_eq!(slug("Master -1 dBTP"), "master-1-dbtp");
    assert_eq!(slug("Ping-Pong Quarter"), "ping-pong-quarter");
    assert_eq!(slug("Electronic 808"), "electronic-808");
}
