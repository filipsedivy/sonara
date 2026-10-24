//! The error codes of the agent contract (CTR-01, TST-05): `golden/contracts/error-codes.json`
//! is the prototype's `ErrorCode` enum, recorded by `scripts/golden/area_contracts.py` in
//! declaration order. Every tool error carries one of these strings, so the Rust enum must hold
//! exactly the same codes, spelled the same and in the same order.

use std::collections::BTreeSet;

use sonara_core::error::ErrorCode;
use sonara_testkit::Golden;

#[test]
fn the_codes_equal_the_recorded_contract() {
    let golden = Golden::open().expect("golden data is available (set SONARA_GOLDEN)");
    let recorded: Vec<String> = golden
        .json("contracts/error-codes.json")
        .expect("the recorded error codes");
    let actual: Vec<&str> = ErrorCode::ALL.iter().map(|code| code.as_str()).collect();
    assert_eq!(actual, recorded, "the codes, in the prototype's order");
    // `ALL` lists every variant once: its strings are distinct and `Display` is `as_str`.
    let distinct: BTreeSet<&str> = actual.iter().copied().collect();
    assert_eq!(distinct.len(), actual.len(), "no code twice");
    for code in ErrorCode::ALL {
        assert_eq!(code.to_string(), code.as_str());
    }
}
