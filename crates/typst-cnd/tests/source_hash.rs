//! `source.hash` covers every file the compilation read (spec §2.1: equal
//! means the input was unchanged, different means it changed), not just the
//! main source file.
//!
//! These drive the real `typst-cnd` binary over a small multi-file project
//! written to a temporary directory, then edit one input at a time.

use std::fs;
use std::path::Path;
use std::process::Command;

use typst::foundations::Dict;
use typst_cnd::world::{CndWorld, source_info};
use typst_cnd::{Cnd, CndDocument};

const MAIN: &str = r#"#set document(title: "Hash Probe", author: "Test")
= Probe

#include "chapter.typ"

#image("pic.svg", alt: "A square")

#let data = json("data.json")
Value: #data.at("value")

See @key.

#bibliography("refs.bib")
"#;

const CHAPTER: &str = "== Chapter\n\nIncluded text.\n";

const SVG_RED: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#;

const BIB: &str =
    "@book{key,\n  title = {A Title},\n  author = {Doe, Jane},\n  year = {2020},\n}\n";

const DATA: &str = r#"{"value": "one"}"#;

/// Write the probe project into `dir`.
fn write_project(dir: &Path) {
    fs::write(dir.join("main.typ"), MAIN).unwrap();
    fs::write(dir.join("chapter.typ"), CHAPTER).unwrap();
    fs::write(dir.join("pic.svg"), SVG_RED).unwrap();
    fs::write(dir.join("refs.bib"), BIB).unwrap();
    fs::write(dir.join("data.json"), DATA).unwrap();
}

/// Compile `dir/main.typ` with the standalone binary and return
/// `source.hash`.
fn binary_hash(dir: &Path) -> String {
    let output = dir.join("out.cnd");
    let result = Command::new(env!("CARGO_BIN_EXE_typst-cnd"))
        .arg("compile")
        .arg(dir.join("main.typ"))
        .arg("--output")
        .arg(&output)
        .output()
        .expect("run typst-cnd");
    assert!(
        result.status.success(),
        "typst-cnd compile failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
    );
    let cnd: Cnd = serde_json::from_str(&fs::read_to_string(&output).unwrap()).unwrap();
    let hash = cnd.source.expect("source").hash;
    assert!(hash.starts_with("sha256:"), "unexpected hash format: {hash}");
    hash
}

/// The hash after editing `file` to `contents`, next to the hash before.
fn hash_before_and_after(file: &str, contents: &str) -> (String, String) {
    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path());
    let before = binary_hash(dir.path());
    fs::write(dir.path().join(file), contents).unwrap();
    let after = binary_hash(dir.path());
    (before, after)
}

#[test]
fn unchanged_inputs_keep_the_hash() {
    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path());
    assert_eq!(binary_hash(dir.path()), binary_hash(dir.path()));
}

#[test]
fn changing_an_included_file_changes_the_hash() {
    let (before, after) =
        hash_before_and_after("chapter.typ", "== Chapter\n\nEdited text.\n");
    assert_ne!(before, after);
}

#[test]
fn changing_an_image_changes_the_hash() {
    let (before, after) =
        hash_before_and_after("pic.svg", &SVG_RED.replace("red", "blue"));
    assert_ne!(before, after);
}

#[test]
fn changing_a_bibliography_changes_the_hash() {
    let (before, after) = hash_before_and_after("refs.bib", &BIB.replace("2020", "2021"));
    assert_ne!(before, after);
}

#[test]
fn changing_a_data_file_changes_the_hash() {
    let (before, after) = hash_before_and_after("data.json", r#"{"value": "two"}"#);
    assert_ne!(before, after);
}

#[test]
fn a_file_the_compile_never_read_does_not_change_the_hash() {
    let (before, after) = hash_before_and_after("unrelated.txt", "not an input");
    assert_eq!(before, after);
}

#[test]
fn the_hash_does_not_depend_on_where_the_project_lives() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    write_project(a.path());
    write_project(b.path());
    assert_ne!(a.path(), b.path());
    assert_eq!(binary_hash(a.path()), binary_hash(b.path()));
}

/// The library path (what `typst compile --format cnd` and embedders call)
/// gives the binary's hash for the same inputs.
#[test]
fn the_library_path_matches_the_binary() {
    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path());

    let mut world =
        CndWorld::new_with_inputs(&dir.path().join("main.typ"), Dict::default()).unwrap();
    typst::compile::<CndDocument>(&world).output.expect("compile");
    let read = world.input_files();
    let library = source_info(&world, read).hash;

    assert_eq!(library, binary_hash(dir.path()));
}

/// Like [`binary_hash`], with extra command-line arguments.
fn binary_hash_with(dir: &Path, args: &[&str]) -> String {
    let output = dir.join("out.cnd");
    let result = Command::new(env!("CARGO_BIN_EXE_typst-cnd"))
        .arg("compile")
        .arg(dir.join("main.typ"))
        .arg("--output")
        .arg(&output)
        .args(args)
        .output()
        .expect("run typst-cnd");
    assert!(
        result.status.success(),
        "typst-cnd compile failed (args {args:?}):\nstderr: {}",
        String::from_utf8_lossy(&result.stderr),
    );
    let cnd: Cnd = serde_json::from_str(&fs::read_to_string(&output).unwrap()).unwrap();
    cnd.source.expect("source").hash
}

#[test]
fn different_inputs_give_different_hashes() {
    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path());
    let none = binary_hash_with(dir.path(), &[]);
    let a = binary_hash_with(dir.path(), &["--input", "k=a"]);
    let b = binary_hash_with(dir.path(), &["--input", "k=b"]);
    assert_ne!(none, a);
    assert_ne!(a, b);
    // Where the key ends and the value starts is part of the hash.
    let split1 = binary_hash_with(dir.path(), &["--input", "ab=c"]);
    let split2 = binary_hash_with(dir.path(), &["--input", "a=bc"]);
    assert_ne!(split1, split2);
}

#[test]
fn the_same_inputs_in_another_order_give_the_same_hash() {
    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path());
    let forward = binary_hash_with(dir.path(), &["--input", "a=1", "--input", "b=2"]);
    let backward = binary_hash_with(dir.path(), &["--input", "b=2", "--input", "a=1"]);
    assert_eq!(forward, backward);

    // The same inputs through `--inputs-file` hash the same, too.
    let file = dir.path().join("inputs.json");
    fs::write(&file, r#"{"b": "2", "a": "1"}"#).unwrap();
    let from_file =
        binary_hash_with(dir.path(), &["--inputs-file", file.to_str().unwrap()]);
    assert_eq!(forward, from_file);
}

#[test]
fn no_inputs_and_an_empty_inputs_file_hash_the_same() {
    let dir = tempfile::tempdir().unwrap();
    write_project(dir.path());
    let file = dir.path().join("inputs.json");
    fs::write(&file, "{}").unwrap();
    assert_eq!(
        binary_hash_with(dir.path(), &[]),
        binary_hash_with(dir.path(), &["--inputs-file", file.to_str().unwrap()]),
    );
}

/// Recomputes the hash from the framing documented on `source_info`, for a
/// one-file project, without and with inputs. This pins the framing: a
/// change to it must be deliberate, and without inputs the hash has no
/// inputs section at all.
#[test]
fn the_hash_follows_the_documented_framing() {
    use sha2::{Digest, Sha256};

    fn field(hasher: &mut Sha256, bytes: &[u8]) {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }

    let dir = tempfile::tempdir().unwrap();
    let main = "= Only file\n";
    fs::write(dir.path().join("main.typ"), main).unwrap();

    let mut files = Sha256::new();
    field(&mut files, b"typst-cnd-source-hash/1");
    field(&mut files, b"/main.typ");
    field(&mut files, b"/main.typ");
    field(&mut files, &[0]);
    field(&mut files, main.as_bytes());

    let without = format!("sha256:{}", hex::encode(files.clone().finalize()));
    assert_eq!(binary_hash_with(dir.path(), &[]), without);

    let mut with = files;
    field(&mut with, b"sys.inputs");
    for (key, value) in [("a", "1"), ("b", "2")] {
        field(&mut with, key.as_bytes());
        field(&mut with, b"str");
        field(&mut with, value.as_bytes());
    }
    let with = format!("sha256:{}", hex::encode(with.finalize()));
    assert_eq!(binary_hash_with(dir.path(), &["--input", "b=2", "--input", "a=1"]), with);
}
