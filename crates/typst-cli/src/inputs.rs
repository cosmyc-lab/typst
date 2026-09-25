//! `sys.inputs` from `--input` pairs and an optional `--inputs-file`.

use std::path::Path;

use typst::foundations::{Dict, IntoValue};

use crate::args::WorldArgs;

/// Merges `--inputs-file` and `--input` into one dictionary. On a key
/// collision `--input` wins, because it is applied last.
pub fn sys_inputs(args: &WorldArgs) -> Result<Dict, String> {
    let mut pairs = Vec::new();
    if let Some(path) = &args.inputs_file {
        pairs.extend(load_inputs_file(path)?);
    }
    pairs.extend(args.inputs.iter().cloned());
    Ok(pairs
        .into_iter()
        .map(|(k, v)| (k.as_str().into(), v.as_str().into_value()))
        .collect())
}

/// Loads a JSON object whose values are all strings. Values are taken
/// verbatim (unlike `--input`, which trims): a value is often a serialized
/// document in which whitespace can matter.
pub fn load_inputs_file(path: &Path) -> Result<Vec<(String, String)>, String> {
    let shown = path.display();
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read inputs file {shown}: {err}"))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse inputs file {shown} as JSON: {err}"))?;
    let object = value.as_object().ok_or_else(|| {
        format!(
            "inputs file {shown} must contain a JSON object at its top level, found {}",
            json_kind(&value)
        )
    })?;
    let mut pairs = Vec::with_capacity(object.len());
    for (key, val) in object {
        if key.trim().is_empty() {
            return Err(format!("inputs file {shown}: keys must not be empty"));
        }
        let Some(s) = val.as_str() else {
            return Err(format!(
                "inputs file {shown}: value for key {key:?} must be a string, found {}",
                json_kind(val)
            ));
        };
        pairs.push((key.clone(), s.to_string()));
    }
    Ok(pairs)
}

fn json_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(json: &str) -> tempfile::NamedTempFile {
        let f = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(f.path(), json).unwrap();
        f
    }

    #[test]
    fn reads_string_values() {
        let f = file(r#"{"a": "1", "b": "{\"x\": 2}"}"#);
        let pairs = load_inputs_file(f.path()).unwrap();
        assert_eq!(
            pairs,
            vec![("a".into(), "1".into()), ("b".into(), "{\"x\": 2}".into())]
        );
    }

    #[test]
    fn keeps_whitespace_in_values() {
        let f = file(r#"{"a": "  padded  "}"#);
        assert_eq!(load_inputs_file(f.path()).unwrap()[0].1, "  padded  ");
    }

    #[test]
    fn rejects_non_objects_non_strings_and_empty_keys() {
        assert!(
            load_inputs_file(file("[]").path())
                .unwrap_err()
                .contains("JSON object")
        );
        assert!(
            load_inputs_file(file(r#"{"a": 1}"#).path())
                .unwrap_err()
                .contains("must be a string")
        );
        assert!(
            load_inputs_file(file(r#"{" ": "x"}"#).path())
                .unwrap_err()
                .contains("must not be empty")
        );
        assert!(load_inputs_file(file("{").path()).unwrap_err().contains("as JSON"));
    }

    #[test]
    fn missing_file_is_an_error() {
        let err = load_inputs_file(Path::new("/definitely/not/here.json")).unwrap_err();
        assert!(err.contains("failed to read inputs file"), "{err}");
    }
}
