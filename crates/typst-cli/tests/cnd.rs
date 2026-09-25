//! Integration tests for `typst compile --format cnd`.

use std::path::{Path, PathBuf};
use std::process::Command;

use typst_cnd::Cnd;

fn exec() -> Command {
    Command::new(env!("CARGO_BIN_EXE_typst"))
}

fn write(dir: &Path, rel: &str, data: impl AsRef<[u8]>) -> PathBuf {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, data).unwrap();
    path
}

#[track_caller]
fn run_ok(cmd: &mut Command) -> std::process::Output {
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "command failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn read_cnd(path: &Path) -> Cnd {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn cnd_format_by_flag_and_by_extension() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", "= Hello\n\nWorld.");

    run_ok(exec().arg("compile").arg(&main).arg("--format").arg("cnd"));
    let by_flag = read_cnd(&dir.path().join("main.cnd"));
    assert!(!by_flag.nodes.is_empty());

    let out = dir.path().join("explicit.cnd");
    run_ok(exec().arg("compile").arg(&main).arg(&out));
    assert!(!read_cnd(&out).nodes.is_empty());
}

#[test]
fn cnd_to_stdout_is_only_json() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", "= Hello");
    let out =
        run_ok(exec().arg("compile").arg(&main).arg("-").arg("--format").arg("cnd"));
    let cnd: Cnd =
        serde_json::from_slice(&out.stdout).expect("stdout is exactly one CND");
    assert!(!cnd.nodes.is_empty());
}

#[test]
fn cnd_semantics_feature_is_on() {
    // Text inside a block body is only located (and so emitted) under
    // `Feature::CndSemantics`; without it the paragraph silently vanishes.
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", "#block[Inside a block.]");
    run_ok(exec().arg("compile").arg(&main).arg("--format").arg("cnd"));
    let json = std::fs::read_to_string(dir.path().join("main.cnd")).unwrap();
    assert!(json.contains("Inside a block."), "{json}");
}

#[test]
fn root_flag_resolves_absolute_imports_from_a_nested_main() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "shared.typ", "#let greeting = [Shared text.]");
    let main = write(
        dir.path(),
        "chapters/main.typ",
        "#import \"/shared.typ\": greeting\n#greeting",
    );
    let out = dir.path().join("out.cnd");
    run_ok(
        exec()
            .arg("compile")
            .arg(&main)
            .arg(&out)
            .arg("--root")
            .arg(dir.path()),
    );
    let cnd = read_cnd(&out);
    let json = serde_json::to_string(&cnd).unwrap();
    assert!(json.contains("Shared text."));
    // source.uri is the main file's path relative to --root.
    assert_eq!(
        cnd.source.as_ref().and_then(|source| source.uri.as_deref()),
        Some("chapters/main.typ")
    );
}
