//! Benlik Card (Draft 0.1): an application's signed self-description for AI agents.
//!
//! This crate ships the normative JSON Schema and the write rules of the specification:
//! [`writable_operations`] lists the operations an agent may perform as writes, [`blocked_operations`]
//! explains why the others may not. Full JSON Schema validation is not done here (use a JSON Schema
//! 2020-12 validator with [`SCHEMA`]); [`has_required_members`] only checks the card's top-level members.
//! The signature is not verified cryptographically: results assume the caller verified it with the
//! publisher's public key.

use serde_json::Value;

/// The Benlik Card JSON Schema (JSON Schema 2020-12).
pub const SCHEMA: &str = include_str!("../schema.json");

/// Stamps that may enable a write operation.
pub const WRITE_SAFE_STAMPS: [&str; 3] = ["certain", "data", "approved"];

/// Whether every top-level member the schema requires is present (a partial check, not schema validation).
pub fn has_required_members(card: &Value) -> bool {
    let schema: Value = serde_json::from_str(SCHEMA).expect("embedded schema is valid JSON");
    let Some(card) = card.as_object() else { return false };
    schema["required"].as_array().into_iter().flatten().filter_map(Value::as_str).all(|m| card.contains_key(m))
}

/// Rules of the specification that the JSON Schema cannot express (SPEC 9: a `data` rule names its setting with a
/// `config_table` source)
pub fn spec_errors(card: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for op in card["operations"].as_array().into_iter().flatten() {
        for (i, r) in op["rules"].as_array().into_iter().flatten().enumerate() {
            let kaynakli = r["sources"].as_array().into_iter().flatten().any(|s| s["kind"] == "config_table");
            if r["stamp"] == "data" && !kaynakli {
                out.push(format!("/operations/{}/rules/{i} 'data' rule must name its setting with a 'config_table' source (SPEC 9)", op["id"].as_str().unwrap_or("")));
            }
        }
    }
    out
}

/// Write operations that must not be performed, with the reason (SPEC 3 and 8).
pub fn blocked_operations(card: &Value) -> Vec<(String, String)> {
    let signed = card["signature"]["alg"].as_str().unwrap_or("none") != "none";
    let mut out = Vec::new();
    for op in card["operations"].as_array().into_iter().flatten() {
        if op["effect"] == "read" {
            continue;
        }
        let id = op["id"].as_str().unwrap_or("").to_string();
        let stamp = op["stamp"].as_str().unwrap_or("");
        if !signed {
            out.push((id, "card is unsigned (signature.alg 'none')".into()));
        } else if !WRITE_SAFE_STAMPS.contains(&stamp) {
            out.push((id, format!("stamp '{stamp}' does not allow writes")));
        } else if op["stale"].as_bool().unwrap_or(false) {
            out.push((id, "operation is stale".into()));
        }
    }
    out
}

/// Ids of operations an agent may perform as writes: effect is not `read`, stamp is certain / data / approved,
/// not stale, the card is signed (`signature.alg` is not `none`) and has its required members.
pub fn writable_operations(card: &Value) -> Vec<String> {
    if !has_required_members(card) || !spec_errors(card).is_empty() {
        return vec![];
    }
    let blocked: Vec<String> = blocked_operations(card).into_iter().map(|(id, _)| id).collect();
    card["operations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|op| op["effect"] != "read")
        .filter_map(|op| op["id"].as_str())
        .filter(|id| !blocked.iter().any(|b| b == id))
        .map(String::from)
        .collect()
}
