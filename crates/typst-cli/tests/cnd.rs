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

/// Every example of the CND suite compiles to the same CND through
/// `typst compile --format cnd` as through the typst-cnd library entry point
/// the standalone binary uses. System fonts are off on both sides: the two
/// entry points discover them in a different order, which is a font-setup
/// difference, not an exporter one.
#[test]
fn cnd_format_matches_the_standalone_exporter_on_every_example() {
    let examples =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../typst-cnd/examples");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&examples)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "typ"))
        .collect();
    files.sort();
    assert!(files.len() >= 9, "expected the example suite, found {}", files.len());

    let out_dir = tempfile::tempdir().unwrap();
    for path in files {
        let world = typst_cnd::world::CndWorld::new_with_options(
            &path,
            typst::foundations::Dict::default(),
            false,
        )
        .unwrap();
        let document = typst::compile::<typst_cnd::CndDocument>(&world)
            .output
            .unwrap_or_else(|errors| panic!("{}: {errors:?}", path.display()));
        let expected = typst_cnd::cnd_to_json(&typst_cnd::cnd_from_document(
            &document,
            typst_cnd::world::source_info(&world),
            "1970-01-01T00:00:00Z".into(),
        ))
        .unwrap();

        let out = out_dir.path().join("out.cnd");
        run_ok(
            exec()
                .arg("compile")
                .arg(&path)
                .arg(&out)
                .arg("--ignore-system-fonts")
                .arg("--creation-timestamp")
                .arg("0"),
        );
        let actual = std::fs::read_to_string(&out).unwrap();
        assert_eq!(
            normalize_uuids(&actual),
            normalize_uuids(&expected),
            "{} differs",
            path.display()
        );
    }
}

/// Node ids are random v4 UUIDs, minted per compilation. Replaces each
/// distinct UUID by its order of first appearance, so two compilations of
/// the same document compare equal exactly when their structure, text and
/// cross-references (which reuse the ids) are equal.
fn normalize_uuids(json: &str) -> String {
    fn is_uuid(s: &[u8]) -> bool {
        s.len() == 36
            && s.iter().enumerate().all(|(i, &b)| match i {
                8 | 13 | 18 | 23 => b == b'-',
                _ => b.is_ascii_hexdigit(),
            })
    }
    let bytes = json.as_bytes();
    let mut seen: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::default();
    let mut out = String::with_capacity(json.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 36 <= bytes.len() && is_uuid(&bytes[i..i + 36]) {
            let next = seen.len();
            let n = *seen.entry(&json[i..i + 36]).or_insert(next);
            out.push_str(&format!("uuid-{n}"));
            i += 36;
        } else {
            let ch = json[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

#[test]
fn normalize_uuids_keeps_references_aligned() {
    let a = r#"{"id":"11111111-1111-4111-8111-111111111111","ref":"11111111-1111-4111-8111-111111111111","b":"22222222-2222-4222-8222-222222222222"}"#;
    let b = r#"{"id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","ref":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","b":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"}"#;
    let c = r#"{"id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","ref":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","b":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"}"#;
    assert_eq!(normalize_uuids(a), normalize_uuids(b));
    assert_ne!(normalize_uuids(a), normalize_uuids(c));
}

#[test]
fn inputs_file_feeds_sys_inputs_and_input_flag_wins() {
    let dir = tempfile::tempdir().unwrap();
    let main =
        write(dir.path(), "main.typ", "#sys.inputs.at(\"a\") #sys.inputs.at(\"b\")");
    let inputs = write(dir.path(), "inputs.json", r#"{"a": "from-file", "b": "file-b"}"#);
    let out = dir.path().join("out.cnd");
    run_ok(
        exec()
            .arg("compile")
            .arg(&main)
            .arg(&out)
            .arg("--inputs-file")
            .arg(&inputs)
            .arg("--input")
            .arg("b=from-flag"),
    );
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(json.contains("from-file"), "{json}");
    assert!(json.contains("from-flag") && !json.contains("file-b"), "{json}");
}

#[test]
fn inputs_file_works_for_every_format() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", "#sys.inputs.at(\"a\")");
    let inputs = write(dir.path(), "inputs.json", r#"{"a": "x"}"#);
    run_ok(exec().arg("compile").arg(&main).arg("--inputs-file").arg(&inputs));
    assert!(dir.path().join("main.pdf").exists());
}

#[test]
fn bad_inputs_file_fails_with_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", "x");
    let inputs = write(dir.path(), "inputs.json", r#"{"a": 1}"#);
    let out = exec()
        .arg("compile")
        .arg(&main)
        .arg("--inputs-file")
        .arg(&inputs)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("inputs.json") && stderr.contains("must be a string"),
        "{stderr}"
    );
}

/// A minimal image for `image(..)`.
const SVG_1X1: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#;

fn compile_with_fallback(
    project: &Path,
    main: &Path,
    fallback: &Path,
) -> std::process::Output {
    exec()
        .arg("compile")
        .arg(main)
        .arg(project.join("out.pdf"))
        .arg("--root")
        .arg(project)
        .arg("--fallback-dir")
        .arg(fallback)
        .arg("--deps")
        .arg(project.join("deps.json"))
        .arg("--deps-format")
        .arg("json")
        .output()
        .unwrap()
}

/// Compiles `main` to CND next to it, with a fallback directory.
fn compile_cnd_with_fallback(main: &Path, fallback: &Path) -> std::process::Output {
    exec()
        .arg("compile")
        .arg(main)
        .arg(main.with_file_name("out.cnd"))
        .arg("--fallback-dir")
        .arg(fallback)
        .output()
        .unwrap()
}

fn deps_inputs(project: &Path) -> Vec<String> {
    let deps: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(project.join("deps.json")).unwrap(),
    )
    .unwrap();
    deps["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn missing_project_file_is_found_by_name_in_the_fallback_dir() {
    let project = tempfile::tempdir().unwrap();
    let fallback = tempfile::tempdir().unwrap();
    write(fallback.path(), "logo.svg", SVG_1X1);
    // From a nested file, with a directory prefix that does not exist, and
    // as an absolute project path.
    write(
        project.path(),
        "chapters/intro.typ",
        "#image(\"logo.svg\")\n#image(\"assets/logo.svg\")\n#image(\"/logo.svg\")",
    );
    let main = write(project.path(), "main.typ", "#include \"chapters/intro.typ\"");
    let out = compile_with_fallback(project.path(), &main, fallback.path());
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let fallback_logo = fallback.path().join("logo.svg").canonicalize().unwrap();
    let inputs = deps_inputs(project.path());
    assert!(
        inputs
            .iter()
            .any(|p| Path::new(p).canonicalize().ok().as_ref() == Some(&fallback_logo)),
        "deps must name the fallback file: {inputs:?}",
    );
    // The missing project paths are not reported as inputs.
    assert!(
        !inputs
            .iter()
            .any(|p| p.contains("assets") || p.ends_with("chapters/logo.svg")),
        "{inputs:?}"
    );
}

#[test]
fn project_file_wins_over_the_fallback() {
    let project = tempfile::tempdir().unwrap();
    let fallback = tempfile::tempdir().unwrap();
    write(fallback.path(), "data.txt", "from-fallback");
    write(project.path(), "data.txt", "from-project");
    let main = write(project.path(), "main.typ", "#read(\"data.txt\")");
    let out = compile_cnd_with_fallback(&main, fallback.path());
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let json = std::fs::read_to_string(project.path().join("out.cnd")).unwrap();
    assert!(json.contains("from-project") && !json.contains("from-fallback"), "{json}");
}

#[test]
fn fallback_serves_source_files_too() {
    let project = tempfile::tempdir().unwrap();
    let fallback = tempfile::tempdir().unwrap();
    write(fallback.path(), "shared.typ", "#let greeting = \"from-shared\"");
    let main = write(
        project.path(),
        "main.typ",
        "#import \"lib/shared.typ\": greeting\n#greeting",
    );
    let out = compile_cnd_with_fallback(&main, fallback.path());
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let json = std::fs::read_to_string(project.path().join("out.cnd")).unwrap();
    assert!(json.contains("from-shared"), "{json}");
}

#[test]
fn escaping_paths_and_directories_never_fall_back() {
    let outer = tempfile::tempdir().unwrap();
    let project = outer.path().join("project");
    let fallback = tempfile::tempdir().unwrap();
    write(fallback.path(), "secret.txt", "leaked");
    write(fallback.path(), "folder", "leaked: a file named like the project's folder");
    write(outer.path(), "secret.txt", "outside");
    std::fs::create_dir_all(project.join("folder")).unwrap();

    for source in [
        "#read(\"../secret.txt\")",
        "#read(\"/x/../../secret.txt\")",
        "#read(\"folder\")",
        "#read(\"folder/\")",
        "#read(\"/\")",
    ] {
        let main = write(&project, "main.typ", source);
        let out = compile_cnd_with_fallback(&main, fallback.path());
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{source} must fail: {stdout} {stderr}");
        assert!(!stderr.contains("leaked"), "{source}: {stderr}");
        assert!(!project.join("out.cnd").exists(), "{source}");
    }
}

#[test]
fn directories_in_the_fallback_dir_are_not_served() {
    let project = tempfile::tempdir().unwrap();
    let fallback = tempfile::tempdir().unwrap();
    std::fs::create_dir(fallback.path().join("data")).unwrap();
    let main = write(project.path(), "main.typ", "#read(\"data\")");
    let out = compile_cnd_with_fallback(&main, fallback.path());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(stderr.contains("file not found"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn symlinks_leaving_the_fallback_dir_are_not_followed() {
    let outer = tempfile::tempdir().unwrap();
    let fallback = outer.path().join("fallback");
    std::fs::create_dir(&fallback).unwrap();
    let secret = write(outer.path(), "secret.txt", "leaked");
    std::os::unix::fs::symlink(&secret, fallback.join("data.txt")).unwrap();
    std::os::unix::fs::symlink(outer.path(), fallback.join("up")).unwrap();
    let project = tempfile::tempdir().unwrap();

    for source in ["#read(\"data.txt\")", "#read(\"up\")"] {
        let main = write(project.path(), "main.typ", source);
        let out = compile_cnd_with_fallback(&main, &fallback);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{source} must fail");
        assert!(stderr.contains("file not found"), "{source}: {stderr}");
        assert!(!stderr.contains("leaked"), "{source}: {stderr}");
    }
}

#[cfg(unix)]
#[test]
fn symlinks_inside_the_fallback_dir_are_followed() {
    let fallback = tempfile::tempdir().unwrap();
    write(fallback.path(), "real.txt", "from-fallback");
    std::os::unix::fs::symlink(
        fallback.path().join("real.txt"),
        fallback.path().join("data.txt"),
    )
    .unwrap();
    let project = tempfile::tempdir().unwrap();
    let main = write(project.path(), "main.typ", "#read(\"data.txt\")");
    let out = compile_cnd_with_fallback(&main, fallback.path());
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let json = std::fs::read_to_string(project.path().join("out.cnd")).unwrap();
    assert!(json.contains("from-fallback"), "{json}");
}

#[test]
fn package_files_never_fall_back() {
    let packages = tempfile::tempdir().unwrap();
    let pkg = packages.path().join("local/pkg/0.1.0");
    write(
        &pkg,
        "typst.toml",
        "[package]\nname = \"pkg\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\n",
    );
    write(&pkg, "lib.typ", "#let value = read(\"data.txt\")");
    let fallback = tempfile::tempdir().unwrap();
    write(fallback.path(), "data.txt", "leaked");
    let project = tempfile::tempdir().unwrap();
    let main =
        write(project.path(), "main.typ", "#import \"@local/pkg:0.1.0\": value\n#value");
    let compile = || {
        exec()
            .arg("compile")
            .arg(&main)
            .arg(project.path().join("out.cnd"))
            .arg("--fallback-dir")
            .arg(fallback.path())
            .arg("--package-path")
            .arg(packages.path())
            .output()
            .unwrap()
    };

    let out = compile();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "a package file must not fall back");
    assert!(stderr.contains("file not found"), "{stderr}");

    // The same package compiles once its own file exists.
    write(&pkg, "data.txt", "from-package");
    let out = compile();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn missing_everywhere_keeps_the_usual_error() {
    let project = tempfile::tempdir().unwrap();
    let fallback = tempfile::tempdir().unwrap();
    let main = write(project.path(), "main.typ", "#image(\"nowhere.png\")");
    let out = compile_with_fallback(project.path(), &main, fallback.path());
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("file not found"));
}

#[test]
fn nonexistent_fallback_dir_is_reported() {
    let project = tempfile::tempdir().unwrap();
    let main = write(project.path(), "main.typ", "x");
    let out = exec()
        .arg("compile")
        .arg(&main)
        .arg("--fallback-dir")
        .arg(project.path().join("nope"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("fallback directory not found")
    );
}

#[test]
fn fallback_dir_must_be_a_directory() {
    let project = tempfile::tempdir().unwrap();
    let main = write(project.path(), "main.typ", "x");
    let file = write(project.path(), "file.txt", "x");
    let out = exec()
        .arg("compile")
        .arg(&main)
        .arg("--fallback-dir")
        .arg(&file)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(stderr.contains("fallback directory is not a directory"), "{stderr}");
}
