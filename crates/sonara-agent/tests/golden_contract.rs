//! Golden tool contract (CTR-01): the generated tool
//! contract and MCP tool listing equal `golden/contracts/tools.json` and
//! `golden/contracts/mcp-tools-list.json` as JSON values, plus exactly the additions of API 1.1
//! (`contract::versions`). The recording stays API 1.0 (GLD-01): the tests take the recorded
//! additions out of what the build publishes, compare the rest with the recording, and check that
//! every recorded addition is published. The recorder sorted the keys, so key order is
//! irrelevant; array order (tools, `required`, `enum`, `anyOf`) is compared.

use serde_json::{Value, json};
use sonara_agent::contract::versions::{missing_additions, recorded_view};
use sonara_agent::contract::{API_VERSION, RECORDED_API_VERSION, tool_contract};
use sonara_agent::mcp;
use sonara_testkit::Golden;

/// Every difference between `expected` and `actual`, as JSON-pointer-like paths.
fn differences(expected: &Value, actual: &Value, path: &str, out: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for (key, value) in e {
                match a.get(key) {
                    Some(other) => differences(value, other, &format!("{path}/{key}"), out),
                    None => out.push(format!("{path}/{key}: missing")),
                }
            }
            for key in a.keys().filter(|key| !e.contains_key(*key)) {
                out.push(format!("{path}/{key}: unexpected {}", a[key]));
            }
        }
        (Value::Array(e), Value::Array(a)) if e.len() == a.len() => {
            for (index, (value, other)) in e.iter().zip(a).enumerate() {
                differences(value, other, &format!("{path}/{index}"), out);
            }
        }
        _ if expected == actual => {}
        _ => out.push(format!("{path}: expected {expected}, got {actual}")),
    }
}

fn assert_same(expected: &Value, actual: &Value) {
    let mut out = Vec::new();
    differences(expected, actual, "", &mut out);
    assert!(
        out.is_empty(),
        "{} differences:\n{}",
        out.len(),
        out.join("\n")
    );
}

#[test]
fn tool_contract_equals_the_golden_contract_plus_the_recorded_additions() {
    let golden: Value = Golden::open()
        .and_then(|golden| golden.json("contracts/tools.json"))
        .unwrap();
    assert_eq!(golden["api_version"], RECORDED_API_VERSION);
    assert_eq!(golden["tools"].as_array().map(Vec::len), Some(57));
    let contract = tool_contract();
    // The one place that pins the number itself; everything else follows the constant.
    assert_eq!(API_VERSION, "1.1");
    assert_eq!(contract["api_version"], API_VERSION);
    assert_same(&golden, &recorded_view(&contract));
    assert_eq!(missing_additions(contract["tools"].as_array().unwrap()), []);
}

#[test]
fn mcp_listing_equals_the_golden_listing_plus_the_recorded_additions() {
    let mut golden: Value = Golden::open()
        .and_then(|golden| golden.json("contracts/mcp-tools-list.json"))
        .unwrap();
    // GLD-01 deviation: the recorder pinned the prototype's own version (0.1.0).
    // `server_info.version` reports the current application version (VER-01), so it moves with
    // every release; pin it to the build's version instead of the recording's.
    golden["server_info"]["version"] = json!(env!("CARGO_PKG_VERSION"));
    // The recorder's document: the `initialize` result (with the protocol version the recorder
    // asked for) and the `tools/list` result.
    let listing = json!({
        "protocol_version": "2025-06-18",
        "server_info": mcp::server_info(),
        "capabilities": mcp::capabilities(),
        "tools": mcp::tools(),
    });
    assert_same(&golden, &recorded_view(&listing));
    assert_eq!(missing_additions(&mcp::tools()), []);
}
