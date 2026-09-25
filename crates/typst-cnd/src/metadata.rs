//! Reading Typst `state` values for CND metadata export.

use comemo::Track;
use typst_library::diag::SourceResult;
use typst_library::engine::Engine;
use typst_library::foundations::{Repr, Value};
use typst_library::introspection::Location;
use typst_library::introspection::{Introspector, state_value_at};

use crate::cnd::metadata_state;

/// Read accumulated CND metadata at a document location.
///
/// Returns a `serde_json::Map` (a `BTreeMap` under the hood, since this
/// workspace never enables `serde_json`'s `preserve_order` feature), not a
/// `HashMap`: a `HashMap`'s iteration order is randomized per process, so
/// the same document would serialize its metadata keys in a different order
/// on every run — see the doc comment on `NodeBase::state_metadata`.
pub fn metadata_at(
    engine: &mut Engine,
    introspector: &dyn Introspector,
    location: Location,
) -> SourceResult<serde_json::Map<String, serde_json::Value>> {
    let state = metadata_state();
    let value = state_value_at(&state, engine, introspector.track(), location)?;
    Ok(dict_value_to_metadata(&value))
}

fn dict_value_to_metadata(value: &Value) -> serde_json::Map<String, serde_json::Value> {
    let Value::Dict(dict) = value else {
        return serde_json::Map::new();
    };

    dict.iter()
        .filter_map(|(key, value)| {
            value_to_json(value).map(|json| (key.to_string(), json))
        })
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

#[cfg(test)]
mod tests {
    use typst_library::foundations::{Value, dict};

    use super::dict_value_to_metadata;

    /// Regression test for the parity bug found by
    /// `cnd_format_matches_the_standalone_exporter_on_every_example`
    /// (`typst-cli/tests/cnd.rs`): two compilations of the same document,
    /// in two different processes, serialized `state_metadata` with a
    /// different key order, because the field was a `HashMap` (random
    /// per-process order) rather than a `serde_json::Map` (alphabetical,
    /// deterministic across every run and every machine).
    ///
    /// The dict below inserts keys in a deliberately non-alphabetical
    /// order; with 8 keys, a `HashMap`'s random iteration order would only
    /// coincidentally sort itself alphabetically about 1 time in 8! (over
    /// 40,000), so this reliably caught the bug before the fix.
    #[test]
    fn state_metadata_keys_serialize_in_a_stable_alphabetical_order() {
        let value = Value::Dict(dict! {
            "zone" => "z",
            "wolf" => "w",
            "mango" => "m",
            "kite" => "k",
            "juno" => "j",
            "delta" => "d",
            "banana" => "b",
            "apple" => "a",
        });

        let metadata = dict_value_to_metadata(&value);
        let keys: Vec<&str> = metadata.keys().map(String::as_str).collect();

        assert_eq!(
            keys,
            vec!["apple", "banana", "delta", "juno", "kite", "mango", "wolf", "zone"],
        );
    }
}
