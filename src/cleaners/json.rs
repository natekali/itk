use super::plain;

pub fn clean_json(s: &str, aggressive: bool) -> String {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(val) => {
            let val = json_extract_error_context(val);
            let val = json_prune_empty(val);
            if aggressive {
                let val = json_strip_metadata_keys(val);
                let val = json_truncate_long_strings(val);
                let val = json_round_floats(val);
                let val = json_dedup_array_objects(val, 1);
                let val = json_collapse_single_child_paths(val);
                serde_json::to_string_pretty(&val).unwrap_or_else(|_| s.to_string())
            } else {
                let val = json_truncate_long_strings(val);
                let val = json_dedup_array_objects(val, 2);
                let val = json_collapse_single_child_paths(val);
                serde_json::to_string_pretty(&val).unwrap_or_else(|_| s.to_string())
            }
        }
        Err(_) => plain::clean_plain(s),
    }
}

/// If root object has error-signal keys, keep only those + id fields.
fn json_extract_error_context(val: serde_json::Value) -> serde_json::Value {
    use serde_json::{Map, Value};
    const ERROR_KEYS: &[&str] = &[
        "error", "errors", "message", "code", "status",
        "detail", "details", "description", "trace", "stack",
    ];
    const ID_KEYS: &[&str] = &["id", "request_id", "trace_id", "correlation_id"];

    if let Value::Object(ref map) = val {
        let has_error = map.keys().any(|k| ERROR_KEYS.contains(&k.as_str()));
        if has_error {
            let mut extracted: Map<String, Value> = map
                .iter()
                .filter(|(k, _)| {
                    ERROR_KEYS.contains(&k.as_str()) || ID_KEYS.contains(&k.as_str())
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let omitted = map.len().saturating_sub(extracted.len());
            if omitted > 0 {
                extracted.insert(
                    "_itk_omitted".to_string(),
                    Value::String(format!("{omitted} non-error fields omitted")),
                );
            }
            return Value::Object(extracted);
        }
    }
    val
}

/// Remove null values and empty arrays/objects recursively.
fn json_prune_empty(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match val {
        Value::Object(map) => {
            let pruned: serde_json::Map<String, Value> = map
                .into_iter()
                .filter(|(_, v)| match v {
                    Value::Null => false,
                    Value::Array(a) if a.is_empty() => false,
                    Value::Object(o) if o.is_empty() => false,
                    _ => true,
                })
                .map(|(k, v)| (k, json_prune_empty(v)))
                .collect();
            Value::Object(pruned)
        }
        Value::Array(arr) => {
            Value::Array(arr.into_iter().map(json_prune_empty).collect())
        }
        other => other,
    }
}

/// If an array contains N objects all sharing the same key schema, show only
/// `show_count` examples plus a count marker.
fn json_dedup_array_objects(val: serde_json::Value, show_count: usize) -> serde_json::Value {
    use serde_json::Value;
    use std::collections::BTreeSet;

    match val {
        Value::Array(arr) if arr.len() > show_count + 1 => {
            // Check if all elements are objects with identical key sets
            let schemas: Vec<BTreeSet<String>> = arr
                .iter()
                .filter_map(|v| {
                    v.as_object()
                        .map(|o| o.keys().cloned().collect::<BTreeSet<_>>())
                })
                .collect();
            let all_objects = schemas.len() == arr.len();
            let all_same_schema = all_objects
                && !schemas.is_empty()
                && schemas.windows(2).all(|w| w[0] == w[1]);

            if all_same_schema {
                let total = arr.len();
                let shown = show_count.min(total);
                let mut result: Vec<Value> = arr
                    .into_iter()
                    .take(shown)
                    .map(|v| json_dedup_array_objects(v, show_count))
                    .collect();
                let hidden = total - shown;
                if hidden > 0 {
                    result.push(Value::String(format!(
                        "... {hidden} more objects with same structure"
                    )));
                }
                Value::Array(result)
            } else {
                // Recurse into elements but keep all
                Value::Array(
                    arr.into_iter()
                        .map(|v| json_dedup_array_objects(v, show_count))
                        .collect(),
                )
            }
        }
        Value::Array(arr) => {
            // Also compact primitive arrays > 20 (original behaviour)
            let all_primitive = arr.iter().all(|v| !v.is_array() && !v.is_object());
            if all_primitive && arr.len() > 20 {
                let len = arr.len();
                let mut result: Vec<Value> = arr.into_iter().take(3).collect();
                result.push(Value::String(format!("... [{} more items]", len - 3)));
                Value::Array(result)
            } else {
                Value::Array(
                    arr.into_iter()
                        .map(|v| json_dedup_array_objects(v, show_count))
                        .collect(),
                )
            }
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, json_dedup_array_objects(v, show_count)))
                .collect(),
        ),
        other => other,
    }
}

/// Collapse single-child object chains: {"a": {"b": {"c": 42}}} -> {"a.b.c": 42}
fn json_collapse_single_child_paths(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;

    match val {
        Value::Object(map) => {
            // Recurse first
            let map: serde_json::Map<String, Value> = map
                .into_iter()
                .map(|(k, v)| (k, json_collapse_single_child_paths(v)))
                .collect();

            // If this object has exactly one key and its value is also a single-key object,
            // merge the key paths
            if map.len() == 1 {
                let (k, v) = map.into_iter().next().unwrap();
                if let Value::Object(ref inner) = v {
                    if inner.len() == 1 {
                        let (ik, iv) = inner.clone().into_iter().next().unwrap();
                        let merged = format!("{k}.{ik}");
                        let mut result = serde_json::Map::new();
                        result.insert(merged, iv);
                        return Value::Object(result);
                    }
                }
                let mut result = serde_json::Map::new();
                result.insert(k, v);
                return Value::Object(result);
            }
            Value::Object(map)
        }
        Value::Array(arr) => {
            Value::Array(
                arr.into_iter()
                    .map(json_collapse_single_child_paths)
                    .collect(),
            )
        }
        other => other,
    }
}

/// Truncate string values > 200 chars (removes base64 blobs, long descriptions).
fn json_truncate_long_strings(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match val {
        Value::String(s) if s.len() > 200 => {
            Value::String(format!("{}...[{} chars]", &s[..100], s.len()))
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, json_truncate_long_strings(v)))
                .collect(),
        ),
        Value::Array(arr) => Value::Array(
            arr.into_iter().map(json_truncate_long_strings).collect(),
        ),
        other => other,
    }
}

/// Remove common metadata/noise keys (HAL, OData, GraphQL __typename, timestamps).
fn json_strip_metadata_keys(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    const NOISE_KEYS: &[&str] = &[
        "_links", "_embedded", "@odata.context", "@odata.type",
        "$schema", "__typename", "createdAt", "updatedAt",
        "created_at", "updated_at", "modified_at", "modifiedAt",
    ];
    match val {
        Value::Object(map) => {
            let filtered: serde_json::Map<String, Value> = map
                .into_iter()
                .filter(|(k, _)| !NOISE_KEYS.contains(&k.as_str()))
                .map(|(k, v)| (k, json_strip_metadata_keys(v)))
                .collect();
            Value::Object(filtered)
        }
        Value::Array(arr) => Value::Array(
            arr.into_iter().map(json_strip_metadata_keys).collect(),
        ),
        other => other,
    }
}

/// Round float values to 2 decimal places.
fn json_round_floats(val: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match val {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if f.fract() != 0.0 {
                    let rounded = (f * 100.0).round() / 100.0;
                    return Value::Number(
                        serde_json::Number::from_f64(rounded).unwrap_or(n)
                    );
                }
            }
            Value::Number(n)
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, json_round_floats(v)))
                .collect(),
        ),
        Value::Array(arr) => Value::Array(
            arr.into_iter().map(json_round_floats).collect(),
        ),
        other => other,
    }
}
