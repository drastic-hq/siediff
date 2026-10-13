#![forbid(unsafe_code)]
use siediff::{
    compare,
    model::{Encoding, Limits},
    parse, snapshot,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn compare_inputs(
    left: &[u8],
    right: &[u8],
    right_is_snapshot: bool,
    renumbered: bool,
) -> std::result::Result<String, JsValue> {
    let run = || -> siediff::Result<String> {
        let limits = Limits::default();
        let left = parse::parse(left, Encoding::Cp437, &limits)?;
        let right = if right_is_snapshot {
            snapshot::parse(right, &limits)?
        } else {
            parse::parse(right, Encoding::Cp437, &limits)?
        };
        let policy = compare::Policy {
            mode: if renumbered {
                compare::Mode::Renumbered
            } else {
                compare::Mode::Identity
            },
            ..Default::default()
        };
        let report = compare::compare(&left, &right, &policy, &limits)?;
        serde_json::to_string(&report)
            .map_err(|_| siediff::Error::new("SERIALIZE", "Cannot serialize comparison"))
    };
    run().map_err(|error| JsValue::from_str(&error.code))
}
