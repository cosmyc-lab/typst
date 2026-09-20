//! Integration tests for `typst-cnd compile`'s `sys.inputs` plumbing:
//! `--input key=value` (existing Typst convention) and the fork's
//! file-based alternative, `--inputs-file`.
//!
//! These drive the real `typst-cnd` binary as a subprocess so that a CLI
//! argument regression (e.g. `--inputs-file` not being registered) shows up
//! the same way it would for a user, not just as a library-level check.

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::paragraph_texts_in_order;
use typst_cnd::Cnd;

/// A tiny document that surfaces `sys.inputs.at("k", ...)` two ways: the raw
/// string, and parsed back as JSON — the shape increment B actually needs,
/// since the registry blob is itself serialized JSON.
const PROBE_SOURCE: &str = r#"
#set document(title: "Inputs Probe", author: "Test")
= Probe

[RAW] #sys.inputs.at("k", default: "MISSING")

[PARSED] #json(bytes(sys.inputs.at("k", default: "{}"))).at("a", default: "none")
"#;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_typst-cnd")
}

/// Compile `PROBE_SOURCE` with the given extra CLI args and return the
/// paragraph texts of the resulting document, in reading order.
fn compile_probe(dir: &Path, extra_args: &[&str]) -> Vec<String> {
    let source = dir.join("main.typ");
    fs::write(&source, PROBE_SOURCE).expect("write source");
    let output = dir.join("out.cnd");

    let result = Command::new(bin())
        .arg("compile")
        .arg(&source)
        .arg("--output")
        .arg(&output)
        .args(extra_args)
        .output()
        .expect("run typst-cnd");
    assert!(
        result.status.success(),
        "typst-cnd compile failed (args {extra_args:?}):\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
    );

    let json = fs::read_to_string(&output).expect("read .cnd output");
    let cnd: Cnd = serde_json::from_str(&json).expect("parse .cnd output as JSON");
    paragraph_texts_in_order(&cnd.nodes)
}

/// Run `typst-cnd` and return its stderr plus exit status, without asserting
/// success — for cases that are expected to fail.
fn run_expect_failure(dir: &Path, extra_args: &[&str]) -> String {
    let source = dir.join("main.typ");
    fs::write(&source, PROBE_SOURCE).expect("write source");
    let output = dir.join("out.cnd");

    let result = Command::new(bin())
        .arg("compile")
        .arg(&source)
        .arg("--output")
        .arg(&output)
        .args(extra_args)
        .output()
        .expect("run typst-cnd");
    assert!(
        !result.status.success(),
        "expected typst-cnd to fail with args {extra_args:?}, but it succeeded"
    );
    String::from_utf8_lossy(&result.stderr).into_owned()
}

/// Constraint 2 (additive): compiling without `--inputs-file` (and without
/// `--input`) must behave exactly as it always has — `sys.inputs.at(...,
/// default: ...)` takes the default branch.
#[test]
fn without_flag_uses_defaults() {
    let dir = tempfile::tempdir().expect("tempdir");
    let texts = compile_probe(dir.path(), &[]);
    assert_eq!(texts, vec!["[RAW] MISSING".to_string(), "[PARSED] none".to_string()]);
}

/// The core contract: `--inputs-file` reaches `sys.inputs`, including the
/// JSON-inside-JSON shape increment B relies on.
#[test]
fn inputs_file_reaches_sys_inputs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let inputs_path = dir.path().join("inputs.json");
    fs::write(&inputs_path, r#"{"k": "{\"a\": 1}"}"#).expect("write inputs file");

    let texts =
        compile_probe(dir.path(), &["--inputs-file", inputs_path.to_str().unwrap()]);
    assert_eq!(texts, vec!["[RAW] {\"a\": 1}".to_string(), "[PARSED] 1".to_string()]);
}

/// Precedence: `--input` overrides the same key from `--inputs-file` (this
/// crate's choice — see the flag's help text).
#[test]
fn input_flag_overrides_inputs_file_on_collision() {
    let dir = tempfile::tempdir().expect("tempdir");
    let inputs_path = dir.path().join("inputs.json");
    fs::write(&inputs_path, r#"{"k": "{\"a\": 1}"}"#).expect("write inputs file");

    let texts = compile_probe(
        dir.path(),
        &["--inputs-file", inputs_path.to_str().unwrap(), "--input", "k={\"a\": 2}"],
    );
    assert_eq!(texts, vec!["[RAW] {\"a\": 2}".to_string(), "[PARSED] 2".to_string()]);
}

/// A non-object top-level JSON value is rejected with a message naming the
/// offending file.
#[test]
fn inputs_file_rejects_non_object_top_level() {
    let dir = tempfile::tempdir().expect("tempdir");
    let inputs_path = dir.path().join("inputs.json");
    fs::write(&inputs_path, "[1, 2, 3]").expect("write inputs file");

    let stderr =
        run_expect_failure(dir.path(), &["--inputs-file", inputs_path.to_str().unwrap()]);
    assert!(
        stderr.contains(inputs_path.to_str().unwrap()),
        "error should name the file path, got: {stderr}"
    );
    assert!(
        stderr.to_lowercase().contains("object"),
        "error should say an object was expected, got: {stderr}"
    );
}

/// A non-string value under a key is rejected, naming both the file and the
/// key.
#[test]
fn inputs_file_rejects_non_string_value() {
    let dir = tempfile::tempdir().expect("tempdir");
    let inputs_path = dir.path().join("inputs.json");
    fs::write(&inputs_path, r#"{"k": 42}"#).expect("write inputs file");

    let stderr =
        run_expect_failure(dir.path(), &["--inputs-file", inputs_path.to_str().unwrap()]);
    assert!(
        stderr.contains(inputs_path.to_str().unwrap()),
        "error should name the file path, got: {stderr}"
    );
    assert!(stderr.contains('k'), "error should name the offending key, got: {stderr}");
}
