//! Reading Typst `state` values for CND metadata export.

use std::collections::HashMap;

use comemo::Track;
use typst_library::diag::SourceResult;
use typst_library::engine::Engine;
use typst_library::foundations::{Repr, Value};
use typst_library::introspection::{Introspector, state_value_at};
use typst_library::introspection::Location;

use crate::cnd::metadata_state;

/// Read accumulated CND metadata at a document location.
pub fn metadata_at(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    location: Location,
) -> SourceResult<HashMap<String, serde_json::Value>> {
    let state = metadata_state();
    let value = state_value_at(&state, engine, introspector.track(), location)?;
    Ok(dict_value_to_metadata(&value))
}

fn dict_value_to_metadata(value: &Value) -> HashMap<String, serde_json::Value> {
    let Value::Dict(dict) = value else {
        return HashMap::new();
    };

    dict.iter()
        .filter_map(|(key, value)| value_to_json(value).map(|json| (key.to_string(), json)))
        .collect()
}

fn value_to_json(value: &Value) -> Option<serde_json::Value> {
    match value {
        Value::None => None,
        Value::Bool(v) => Some(serde_json::Value::Bool(*v)),
        Value::Int(v) => Some(serde_json::json!(v)),
        Value::Float(v) => Some(serde_json::json!(v)),
        Value::Decimal(v) => Some(serde_json::Value::String(v.to_string())),
        Value::Str(v) => Some(serde_json::Value::String(v.as_str().into())),
        Value::Dict(dict) => {
            let map: serde_json::Map<String, serde_json::Value> = dict
                .iter()
                .filter_map(|(k, v)| value_to_json(v).map(|j| (k.to_string(), j)))
                .collect();
            Some(serde_json::Value::Object(map))
        }
        Value::Array(array) => {
            let items: Vec<serde_json::Value> =
                array.iter().filter_map(value_to_json).collect();
            Some(serde_json::Value::Array(items))
        }
        _ => Some(serde_json::Value::String(value.repr().into())),
    }
}
