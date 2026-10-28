//! Benlik Card (Draft 0.1.1): an application's signed self-description for AI agents.
//!
//! This crate ships the normative JSON Schema and the rules of the specification. [`validate`] checks a card
//! against the schema (a strict interpreter for exactly the keywords the Benlik schema uses; anything else fails
//! closed) and against the rules the schema cannot express, and reports which operations an agent may perform as
//! writes. The Rust, npm and PyPI validators give the same decision on the shared test vectors.
//!
//! Signatures are verified cryptographically (ML-DSA-65, FIPS 204) when the publisher's key document
//! (`benlik-keys.json`) is given in [`Options::public_keys`]: the card with the context string `benlik-card`, the
//! evidence bundle with `benlik-evidence`. A key that merely comes with the card proves integrity, not identity: the
//! signature counts as [`SignatureStatus::Verified`] only when the signing key is among [`Options::trusted_keys`]
//! (keys the caller pinned); otherwise it is [`SignatureStatus::UntrustedKey`]. When keys are given, operations are
//! writable only with a verified signature. Without keys the result says [`SignatureStatus::NotVerified`] and
//! `writable` assumes the caller verified the signature itself.

use ml_dsa::{EncodedSignature, EncodedVerifyingKey, MlDsa65, Signature, VerifyingKey};
use serde_json::{Map, Value};

/// The Benlik Card JSON Schema (JSON Schema 2020-12).
pub const SCHEMA: &str = include_str!("../schema.json");

/// Stamps that may enable a write operation. Since Draft 0.1.1 only `certain`: `data` and `approved` count as
/// observed, and an observation never enables a write on its own.
pub const WRITE_SAFE_STAMPS: [&str; 1] = ["certain"];

/// FIPS 204 context string of a card signature.
pub const CARD_CONTEXT: &[u8] = b"benlik-card";
/// FIPS 204 context string of an evidence bundle signature (a bundle signature is never a card signature).
pub const EVIDENCE_CONTEXT: &[u8] = b"benlik-evidence";

/// What is known about a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStatus {
    /// `signature.alg` is `none` (or the document carries no signature): no operation is writable.
    Unsigned,
    /// Signed, but not checked: no key document was given, or the algorithm is not supported here (Ed25519).
    NotVerified,
    /// The signature does not verify with the key document: the document was changed, the key or the context is wrong.
    Invalid,
    /// The signature verifies, but the signing key is not among the trusted (pinned) keys: integrity, not identity.
    UntrustedKey,
    /// The signature verifies with a trusted key.
    Verified,
}

impl SignatureStatus {
    /// The status as the npm and PyPI validators spell it.
    pub fn as_str(self) -> &'static str {
        match self {
            SignatureStatus::Unsigned => "unsigned",
            SignatureStatus::NotVerified => "not_verified",
            SignatureStatus::Invalid => "invalid",
            SignatureStatus::UntrustedKey => "untrusted_key",
            SignatureStatus::Verified => "verified",
        }
    }
}

/// Inputs beyond the card itself.
#[derive(Debug, Clone, Default)]
pub struct Options<'a> {
    /// Current time as Unix seconds (default: the system clock). Used for `x-expires_at`.
    pub now: Option<i64>,
    /// Current SHA-256 (hex) of the project's files, by relative path. When given, staleness is computed from `x-basis`.
    pub current_digests: Option<&'a Map<String, Value>>,
    /// The evidence bundle (`benlik-evidence.json`). When given, every record must match its digest in `x-evidence`.
    pub evidence_bundle: Option<&'a Value>,
    /// The publisher's key document (`{"keys": [{"key_id", "alg", "public_key"}]}`, `benlik-keys.json`). When given,
    /// the card and bundle signatures are verified and writes need a verified signature.
    pub public_keys: Option<&'a Value>,
    /// Keys the caller trusts for this application (same document shape), matched by public key bytes, never by `key_id`.
    pub trusted_keys: Option<&'a Value>,
}

/// Result of [`validate`].
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// The card conforms to the schema and to the rules the schema cannot express.
    pub valid: bool,
    pub errors: Vec<String>,
    /// Ids of operations an agent may perform as writes, in card order.
    pub writable: Vec<String>,
    /// Write operations that must not be performed, with the reason.
    pub blocked: Vec<(String, String)>,
    /// Ids of stale operations (`stale: true`, or computed from `x-basis` and [`Options::current_digests`]).
    pub stale: Vec<String>,
    /// Evidence ids of the card that the given bundle does not contain (not an error: bundles may be partial).
    pub not_presented: Vec<String>,
    pub signature: SignatureStatus,
    /// Why the signature is not verified, when it is not.
    pub signature_detail: Option<String>,
    /// Status of the evidence bundle's signature (None: no bundle given).
    pub evidence_signature: Option<SignatureStatus>,
}

// ---- Signatures (ML-DSA-65, FIPS 204) ----

/// Strict standard base64 (with padding), the same rule in the three validators.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if !b.len().is_multiple_of(4) {
        return None;
    }
    let pad = b.iter().rev().take_while(|&&c| c == b'=').count();
    if pad > 2 || b[..b.len() - pad].contains(&b'=') {
        return None;
    }
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 0,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(b.len() / 4 * 3);
    for chunk in b.chunks(4) {
        let mut n = 0u32;
        for &c in chunk {
            n = (n << 6) | value(c)?;
        }
        out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8]);
    }
    out.truncate(out.len() - pad);
    Some(out)
}

/// ML-DSA-65 public key of an entry in a key document, if well-formed.
fn key_bytes(entry: &Value) -> Option<Vec<u8>> {
    (entry["alg"] == "ML-DSA-65").then_some(())?;
    base64_decode(entry["public_key"].as_str()?)
}

/// Verifies `doc["signature"]` over the RFC 8785 form of `doc` without `signature`, with the FIPS 204 context string.
pub fn verify_signature(doc: &Value, public_keys: &Value, trusted_keys: Option<&Value>, context: &[u8]) -> (SignatureStatus, Option<String>) {
    let s = &doc["signature"];
    let invalid = |why: &str| (SignatureStatus::Invalid, Some(why.to_string()));
    match s["alg"].as_str() {
        None | Some("none") => return (SignatureStatus::Unsigned, None),
        Some("ML-DSA-65") => {}
        Some(other) => return (SignatureStatus::NotVerified, Some(format!("algorithm '{other}' is not verified by this validator"))),
    }
    let key_id = s["key_id"].as_str().unwrap_or("");
    let Some(entry) = public_keys["keys"].as_array().and_then(|k| k.iter().find(|e| e["key_id"].as_str() == Some(key_id))) else {
        return invalid("signing key is not in the key document");
    };
    let Some(pk) = key_bytes(entry) else { return invalid("key document entry is not an ML-DSA-65 key") };
    let Ok(enc_vk) = EncodedVerifyingKey::<MlDsa65>::try_from(pk.as_slice()) else { return invalid("public key has the wrong length") };
    let vk = VerifyingKey::<MlDsa65>::decode(&enc_vk);
    let Some(sig_bytes) = s["value"].as_str().and_then(base64_decode) else { return invalid("signature value is not base64") };
    let Ok(enc_sig) = EncodedSignature::<MlDsa65>::try_from(sig_bytes.as_slice()) else { return invalid("signature has the wrong length") };
    let Some(sig) = Signature::<MlDsa65>::decode(&enc_sig) else { return invalid("signature cannot be decoded") };
    let mut unsigned = doc.clone();
    if let Some(m) = unsigned.as_object_mut() {
        m.remove("signature");
    }
    if !vk.verify_with_context(canonical(&unsigned).as_bytes(), context, &sig) {
        return invalid("signature does not verify (changed after signing, wrong key or wrong context)");
    }
    let trusted = trusted_keys.and_then(|t| t["keys"].as_array()).is_some_and(|t| t.iter().any(|e| key_bytes(e).as_deref() == Some(pk.as_slice())));
    if trusted {
        (SignatureStatus::Verified, None)
    } else {
        (SignatureStatus::UntrustedKey, Some(format!("key '{key_id}' verifies the signature but is not a trusted key")))
    }
}

/// Whether every top-level member the schema requires is present (a partial check; use [`validate`]).
pub fn has_required_members(card: &Value) -> bool {
    let schema: Value = serde_json::from_str(SCHEMA).expect("embedded schema is valid JSON");
    let Some(card) = card.as_object() else { return false };
    schema["required"].as_array().into_iter().flatten().filter_map(Value::as_str).all(|m| card.contains_key(m))
}

// ---- JSON Schema (the subset the Benlik schema uses) ----

/// Schema errors of a card (empty: the card conforms to the JSON Schema).
pub fn schema_errors(card: &Value) -> Vec<String> {
    let schema: Value = serde_json::from_str(SCHEMA).expect("embedded schema is valid JSON");
    let mut out = Vec::new();
    check(&schema, &schema, card, "", &mut out);
    out
}

/// Patterns are matched exactly as ECMA-262 would; a pattern this crate does not know fails closed (None).
fn pattern_matches(pattern: &str, s: &str) -> Option<bool> {
    let hex64 = |h: &str| h.len() == 64 && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    Some(match pattern {
        "^0\\.1(\\.\\d+)?$" => s == "0.1" || s.strip_prefix("0.1.").is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit())),
        "^[a-z0-9_.-]+$" => !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b)),
        "^x-" => s.starts_with("x-"),
        "^sha256:[0-9a-f]{64}$" => s.strip_prefix("sha256:").is_some_and(hex64),
        "^[0-9a-f]{64}$" => hex64(s),
        _ => return None,
    })
}

fn type_matches(t: &str, v: &Value) -> bool {
    match t {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "boolean" => v.is_boolean(),
        "null" => v.is_null(),
        "number" => v.is_number(),
        "integer" => v.is_i64() || v.is_u64() || v.as_f64().is_some_and(|f| f.fract() == 0.0),
        _ => false,
    }
}

/// JSON equality as JSON Schema defines it (1 and 1.0 are equal)
fn json_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| json_equal(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, p)| y.get(k).is_some_and(|q| json_equal(p, q))),
        _ => a == b,
    }
}

fn check(schema: &Value, root: &Value, inst: &Value, path: &str, out: &mut Vec<String>) {
    let at = if path.is_empty() { "/" } else { path };
    let s = match schema {
        Value::Bool(true) => return,
        Value::Bool(false) => return out.push(format!("{at} is not allowed")),
        Value::Object(s) => s,
        _ => return out.push(format!("{at} schema is not an object")),
    };
    let props = s.get("properties").and_then(Value::as_object);
    let patterns = s.get("patternProperties").and_then(Value::as_object);
    for (kw, val) in s {
        match kw.as_str() {
            "$schema" | "$id" | "title" | "description" | "$defs" => {}
            "$ref" => match val.as_str().and_then(|r| r.strip_prefix("#/$defs/")).and_then(|d| root["$defs"].get(d)) {
                Some(target) => check(target, root, inst, path, out),
                None => out.push(format!("{at} unresolvable $ref {val}")),
            },
            "type" => {
                let types: Vec<&str> = match val {
                    Value::String(t) => vec![t.as_str()],
                    Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
                    _ => vec![],
                };
                if !types.iter().any(|t| type_matches(t, inst)) {
                    out.push(format!("{at} must be {}", types.join(" or ")));
                }
            }
            "enum" => {
                if !val.as_array().is_some_and(|a| a.iter().any(|x| json_equal(x, inst))) {
                    out.push(format!("{at} must be one of {val}"));
                }
            }
            "const" => {
                if !json_equal(val, inst) {
                    out.push(format!("{at} must be {val}"));
                }
            }
            "required" => {
                if let Some(o) = inst.as_object() {
                    for m in val.as_array().into_iter().flatten().filter_map(Value::as_str) {
                        if !o.contains_key(m) {
                            out.push(format!("{at} must have required property '{m}'"));
                        }
                    }
                }
            }
            "properties" => {
                if let (Some(o), Some(p)) = (inst.as_object(), val.as_object()) {
                    for (k, sub) in p {
                        if let Some(x) = o.get(k) {
                            check(sub, root, x, &format!("{path}/{k}"), out);
                        }
                    }
                }
            }
            "patternProperties" => {
                if let (Some(o), Some(p)) = (inst.as_object(), val.as_object()) {
                    for (pat, sub) in p {
                        for (k, x) in o {
                            match pattern_matches(pat, k) {
                                Some(true) => check(sub, root, x, &format!("{path}/{k}"), out),
                                Some(false) => {}
                                None => out.push(format!("{at} unsupported pattern {pat}")),
                            }
                        }
                    }
                }
            }
            "additionalProperties" => {
                if let Some(o) = inst.as_object() {
                    for (k, x) in o {
                        let named = props.is_some_and(|p| p.contains_key(k));
                        let patterned = patterns.is_some_and(|p| p.keys().any(|pat| pattern_matches(pat, k) == Some(true)));
                        if !named && !patterned {
                            if val == &Value::Bool(false) {
                                out.push(format!("{at} must not have additional property '{k}'"));
                            } else {
                                check(val, root, x, &format!("{path}/{k}"), out);
                            }
                        }
                    }
                }
            }
            "items" => {
                for (i, x) in inst.as_array().into_iter().flatten().enumerate() {
                    check(val, root, x, &format!("{path}/{i}"), out);
                }
            }
            "pattern" => {
                if let (Some(x), Some(p)) = (inst.as_str(), val.as_str()) {
                    match pattern_matches(p, x) {
                        Some(true) => {}
                        Some(false) => out.push(format!("{at} must match pattern {p}")),
                        None => out.push(format!("{at} unsupported pattern {p}")),
                    }
                }
            }
            "minLength" => {
                if let (Some(x), Some(n)) = (inst.as_str(), val.as_u64()) {
                    if (x.chars().count() as u64) < n {
                        out.push(format!("{at} must not have fewer than {n} characters"));
                    }
                }
            }
            "minimum" => {
                if let (Some(x), Some(n)) = (inst.as_f64(), val.as_f64()) {
                    if x < n {
                        out.push(format!("{at} must be >= {n}"));
                    }
                }
            }
            "format" => {
                if let Some(x) = inst.as_str() {
                    match val.as_str() {
                        Some("date-time") if parse_time(x).is_some() => {}
                        Some("date-time") => out.push(format!("{at} must be a date-time")),
                        _ => out.push(format!("{at} unsupported format {val}")),
                    }
                }
            }
            // A keyword this interpreter does not implement must not be silently ignored
            _ => out.push(format!("{at} unsupported schema keyword '{kw}'")),
        }
    }
}

// ---- Time, digests, canonical JSON ----

/// RFC 3339 date-time to Unix seconds (same rules as the npm and PyPI validators); None if malformed.
pub fn parse_time(m: &str) -> Option<i64> {
    let b = m.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't' | b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |a: usize, z: usize| m.get(a..z).filter(|p| p.bytes().all(|c| c.is_ascii_digit())).and_then(|p| p.parse::<i64>().ok());
    let (y, mo, d, h, mi, s) = (num(0, 4)?, num(5, 7)?, num(8, 10)?, num(11, 13)?, num(14, 16)?, num(17, 19)?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if !(1..=12).contains(&mo) || h > 23 || mi > 59 || s > 60 || d < 1 || d > days[(mo - 1) as usize] {
        return None;
    }
    let mut i = 19;
    if b[i] == b'.' {
        i += 1;
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    let offset = match m.get(i..)? {
        "Z" | "z" => 0,
        o if o.len() == 6 && (o.starts_with('+') || o.starts_with('-')) && &o[3..4] == ":" => {
            let (oh, om) = (num(i + 1, i + 3)?, num(i + 4, i + 6)?);
            if oh > 23 || om > 59 {
                return None;
            }
            (oh * 3600 + om * 60) * if o.starts_with('+') { 1 } else { -1 }
        }
        _ => return None,
    };
    // days_from_civil
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400 + h * 3600 + mi * 60 + s.min(59) - offset)
}

/// SHA-256 (FIPS 180-4), dependency-free.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, c) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for (a, b) in h.iter_mut().zip(v) {
            *a = a.wrapping_add(b);
        }
    }
    let mut out = [0u8; 32];
    for (i, x) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// RFC 8785 (JCS) canonical JSON: keys in UTF-16 code unit order, no whitespace, numbers as ECMAScript prints them.
pub fn canonical(v: &Value) -> String {
    let mut s = String::new();
    write_canonical(v, &mut s);
    s
}

fn write_canonical(v: &Value, s: &mut String) {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            s.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&serde_json::to_string(k).unwrap());
                s.push(':');
                write_canonical(&m[k], s);
            }
            s.push('}');
        }
        Value::Array(a) => {
            s.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                write_canonical(x, s);
            }
            s.push(']');
        }
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            // Integers above 2^53 print like fractional numbers in JavaScript
            (Some(i), _, _) if i.unsigned_abs() > 1 << 53 => s.push_str(&es_number(i as f64)),
            (_, Some(u), _) if u > 1 << 53 => s.push_str(&es_number(u as f64)),
            (Some(i), _, _) => s.push_str(&i.to_string()),
            (_, Some(u), _) => s.push_str(&u.to_string()),
            (_, _, Some(f)) => s.push_str(&es_number(f)),
            _ => s.push_str(&n.to_string()),
        },
        _ => s.push_str(&serde_json::to_string(v).unwrap()),
    }
}

/// ECMAScript number formatting (RFC 8785 section 3.2.2.3)
fn es_number(f: f64) -> String {
    if f == 0.0 || !f.is_finite() {
        return "0".into();
    }
    let sign = if f < 0.0 { "-" } else { "" };
    // `{:e}` gives the shortest round-trip digits: "1.5e300", "1e-7"
    let e = format!("{:e}", f.abs());
    let (mantissa, exp) = e.split_once('e').unwrap();
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let (k, n) = (digits.len() as i32, exp.parse::<i32>().unwrap() + 1);
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let x = n - 1;
        let rest = if k > 1 { format!(".{}", &digits[1..]) } else { String::new() };
        format!("{}{rest}e{}{}", &digits[..1], if x >= 0 { "+" } else { "-" }, x.abs())
    };
    format!("{sign}{body}")
}

/// SHA-256 (hex) of the JCS form of an evidence record.
pub fn record_digest(record: &Value) -> String {
    hex(&sha256(canonical(record).as_bytes()))
}

/// RFC 6962 Merkle tree hash over hex digests, in order (empty list: SHA-256 of the empty string).
pub fn merkle_root(digests: &[&str]) -> String {
    fn mth(items: &[Vec<u8>]) -> [u8; 32] {
        match items.len() {
            0 => sha256(b""),
            1 => sha256(&[&[0u8][..], &items[0]].concat()),
            n => {
                let mut k = 1;
                while k * 2 < n {
                    k *= 2;
                }
                sha256(&[&[1u8][..], &mth(&items[..k]), &mth(&items[k..])].concat())
            }
        }
    }
    let leaves: Vec<Vec<u8>> = digests
        .iter()
        .map(|d| {
            if d.len() % 2 == 0 && d.bytes().all(|b| b.is_ascii_hexdigit()) {
                (0..d.len() / 2).map(|i| u8::from_str_radix(&d[2 * i..2 * i + 2], 16).unwrap()).collect()
            } else {
                vec![]
            }
        })
        .collect();
    hex(&mth(&leaves))
}

// ---- Rules the schema cannot express ----

/// Source files an operation relies on: its own, its name's, its inputs' and its rules' sources.
pub fn operation_basis(op: &Value) -> Vec<String> {
    let mut groups: Vec<&Value> = vec![&op["sources"], &op["name"]["sources"]];
    groups.extend(op["inputs"].as_object().into_iter().flatten().map(|(_, g)| &g["sources"]));
    groups.extend(op["rules"].as_array().into_iter().flatten().map(|r| &r["sources"]));
    let mut files: Vec<String> = groups.into_iter().filter_map(Value::as_array).flatten().filter_map(|s| s["file"].as_str()).filter(|f| !f.is_empty()).map(String::from).collect();
    files.sort();
    files.dedup();
    files
}

/// (errors, not presented) of the Draft 0.1.1 evidence members and of the bundle, if one is given.
fn evidence(card: &Value, bundle: Option<&Value>) -> (Vec<String>, Vec<String>) {
    let mut out = Vec::new();
    let summaries = card.get("x-evidence").and_then(Value::as_array);
    let ids: Vec<&str> = summaries.into_iter().flatten().filter_map(|e| e["id"].as_str()).collect();
    let mut dup: Vec<&str> = ids.iter().copied().filter(|i| ids.iter().filter(|j| *j == i).count() > 1).collect();
    dup.sort();
    dup.dedup();
    out.extend(dup.into_iter().map(|i| format!("/x-evidence duplicate evidence id '{i}'")));
    for op in card["operations"].as_array().into_iter().flatten() {
        for r in op["x-dna"]["evidence_refs"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if !ids.contains(&r) {
                out.push(format!("/operations/{}/x-dna evidence_refs '{r}' is not in x-evidence", op["id"].as_str().unwrap_or("")));
            }
        }
    }
    let root = card.get("x-evidence_root");
    if summaries.is_some() && root.is_none() {
        out.push("/x-evidence_root is missing".into());
    }
    if let Some(root) = root {
        let digests: Vec<&str> = summaries.into_iter().flatten().map(|e| e["sha256"].as_str().unwrap_or("")).collect();
        if root.as_str() != Some(merkle_root(&digests).as_str()) {
            out.push("/x-evidence_root does not match the Merkle root of x-evidence".into());
        }
    }
    let Some(bundle) = bundle else { return (out, vec![]) };
    let records = match bundle["records"].as_array() {
        Some(r) => {
            if bundle.get("card_revision") != card.get("x-revision") {
                out.push("evidence bundle belongs to another card revision".into());
            }
            r.as_slice()
        }
        None => {
            out.push("evidence bundle has no 'records' array".into());
            &[]
        }
    };
    let digests: Vec<(&str, &str)> = summaries.into_iter().flatten().filter_map(|e| Some((e["id"].as_str()?, e["sha256"].as_str().unwrap_or("")))).collect();
    let mut seen: Vec<&str> = Vec::new();
    for r in records {
        let id = r["id"].as_str();
        let label = id.map_or_else(|| "None".to_string(), String::from);
        match id.and_then(|i| digests.iter().find(|(d, _)| *d == i)) {
            None => out.push(format!("evidence record '{label}' is not in x-evidence")),
            Some((_, d)) if *d != record_digest(r) => out.push(format!("evidence record '{label}' digest mismatch")),
            Some((i, _)) if seen.contains(i) => out.push(format!("evidence record '{label}' appears twice")),
            Some((i, _)) => seen.push(i),
        }
    }
    let not_presented = digests.iter().map(|(i, _)| *i).filter(|i| !seen.contains(i)).map(String::from).collect();
    (out, not_presented)
}

/// Rules of the specification that the JSON Schema cannot express: SPEC 9 (a `data` rule names its setting with a
/// `config_table` source) and the Draft 0.1.1 rules (unique evidence ids, existing `x-dna.evidence_refs`, Merkle root
/// of `x-evidence`, `x-expires_at` after `x-issued_at`).
pub fn spec_errors(card: &Value) -> Vec<String> {
    spec_errors_with(card, None)
}

/// [`spec_errors`] plus the evidence bundle check (every record matches its digest in `x-evidence`).
pub fn spec_errors_with(card: &Value, evidence_bundle: Option<&Value>) -> Vec<String> {
    let mut out = Vec::new();
    for op in card["operations"].as_array().into_iter().flatten() {
        for (i, r) in op["rules"].as_array().into_iter().flatten().enumerate() {
            let named = r["sources"].as_array().into_iter().flatten().any(|s| s["kind"] == "config_table");
            if r["stamp"] == "data" && !named {
                out.push(format!("/operations/{}/rules/{i} 'data' rule must name its setting with a 'config_table' source (SPEC 9)", op["id"].as_str().unwrap_or("")));
            }
        }
    }
    let (issued, expires) = (card["x-issued_at"].as_str().and_then(parse_time), card["x-expires_at"].as_str().and_then(parse_time));
    if let (Some(i), Some(e)) = (issued, expires) {
        if e <= i {
            out.push("/x-expires_at must be after x-issued_at".into());
        }
    }
    out.extend(evidence(card, evidence_bundle).0);
    out
}

/// Ids of stale operations: `stale: true`, or (with current digests) a source file whose digest differs from `x-basis`.
pub fn stale_operations(card: &Value, current_digests: Option<&Map<String, Value>>) -> Vec<String> {
    let basis = card.get("x-basis").and_then(Value::as_object);
    let mut out = Vec::new();
    for op in card["operations"].as_array().into_iter().flatten() {
        let computed = match (current_digests, basis) {
            (Some(now), Some(b)) => operation_basis(op).iter().any(|f| now.get(f).is_none() || now.get(f) != b.get(f)),
            _ => false,
        };
        if op["stale"] == Value::Bool(true) || computed {
            out.push(op["id"].as_str().unwrap_or("").to_string());
        }
    }
    out
}

fn system_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Validates a card: schema, specification rules and write rules (SPEC 3 and 8). An operation is writable only if
/// its effect is not `read`, its stamp is `certain`, it is not stale, and the card is valid, signed
/// (`signature.alg` is not `none`) and not expired (`x-expires_at`).
pub fn validate(card: &Value, options: &Options) -> Report {
    let alg = card["signature"]["alg"].as_str();
    let mut r = Report {
        valid: false,
        errors: schema_errors(card),
        writable: vec![],
        blocked: vec![],
        stale: vec![],
        not_presented: vec![],
        signature: if alg.is_none_or(|a| a == "none") { SignatureStatus::Unsigned } else { SignatureStatus::NotVerified },
        signature_detail: None,
        evidence_signature: None,
    };
    if let Some(keys) = options.public_keys {
        (r.signature, r.signature_detail) = verify_signature(card, keys, options.trusted_keys, CARD_CONTEXT);
    }
    if let Some(bundle) = options.evidence_bundle {
        r.evidence_signature = Some(match (bundle["signature"].is_object(), options.public_keys) {
            (false, _) => SignatureStatus::Unsigned,
            (true, None) => SignatureStatus::NotVerified,
            (true, Some(keys)) => verify_signature(bundle, keys, options.trusted_keys, EVIDENCE_CONTEXT).0,
        });
    }
    if r.errors.is_empty() {
        r.errors = spec_errors_with(card, options.evidence_bundle);
        if r.evidence_signature == Some(SignatureStatus::Invalid) {
            r.errors.push("evidence bundle signature does not verify (context benlik-evidence)".into());
        }
    }
    if !r.errors.is_empty() {
        return r;
    }
    r.valid = true;
    let signed = alg != Some("none");
    let now = options.now.unwrap_or_else(system_now);
    let expired = card["x-expires_at"].as_str().and_then(parse_time).is_some_and(|e| now >= e);
    r.stale = stale_operations(card, options.current_digests);
    for op in card["operations"].as_array().into_iter().flatten() {
        if op["effect"] == "read" {
            continue;
        }
        let id = op["id"].as_str().unwrap_or("").to_string();
        let stamp = op["stamp"].as_str().unwrap_or("");
        let reason = if !signed {
            Some("card is unsigned (signature.alg 'none')".to_string())
        } else if options.public_keys.is_some() && r.signature != SignatureStatus::Verified {
            Some(format!("signature is {}: {}", r.signature.as_str(), r.signature_detail.clone().unwrap_or_default()))
        } else if expired {
            Some("card has expired (x-expires_at)".to_string())
        } else if !WRITE_SAFE_STAMPS.contains(&stamp) {
            Some(format!("stamp '{stamp}' does not allow writes"))
        } else if r.stale.contains(&id) {
            Some("operation is stale".to_string())
        } else {
            None
        };
        match reason {
            Some(why) => r.blocked.push((id, why)),
            None => r.writable.push(id),
        }
    }
    r.not_presented = evidence(card, options.evidence_bundle).1;
    r
}

/// Write operations that must not be performed, with the reason (see [`validate`]; empty for an invalid card).
pub fn blocked_operations(card: &Value) -> Vec<(String, String)> {
    validate(card, &Options::default()).blocked
}

/// Ids of operations an agent may perform as writes (see [`validate`]).
pub fn writable_operations(card: &Value) -> Vec<String> {
    validate(card, &Options::default()).writable
}
