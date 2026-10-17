//! Validates every example's emitted CND against the pinned cnd-sdk.
//!
//! Needs a checkout of cnd-sdk at the SHA in `CND_SDK_REF`, named by
//! `CND_SDK_DIR`, with its environment synced (`uv sync`). Without the
//! variable the test prints a skip line and passes, so a plain
//! `cargo test` stays self-contained; CI sets it.

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::{all_example_files, cnd_for_example};
use typst_cnd::cnd_to_json;

#[test]
fn every_example_validates_against_the_pinned_sdk() {
    let Some(sdk) = std::env::var_os("CND_SDK_DIR") else {
        println!("sdk_gate: CND_SDK_DIR unset, skipped");
        return;
    };
    let sdk = PathBuf::from(sdk);
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let pinned = std::fs::read_to_string(crate_dir.join("CND_SDK_REF"))
        .expect("read CND_SDK_REF")
        .trim()
        .to_string();
    let out = Command::new("git")
        .arg("-C")
        .arg(&sdk)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("run git");
    assert!(out.status.success(), "git rev-parse failed in {}", sdk.display());
    let actual = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(actual, pinned, "CND_SDK_DIR is at {actual}, CND_SDK_REF pins {pinned}");

    let tmp = tempfile::tempdir().expect("tempdir");
    let mut count = 0;
    for path in all_example_files() {
        let name = path.file_name().and_then(|n| n.to_str()).expect("name");
        let stem = path.file_stem().and_then(|n| n.to_str()).expect("stem");
        let cnd = cnd_for_example(name);
        let json = cnd_to_json(&cnd).expect("serialize");
        std::fs::write(tmp.path().join(format!("{stem}.cnd")), json).expect("write");
        count += 1;
    }
    assert!(count > 0, "no examples found");

    let output = Command::new("uv")
        .arg("run")
        .arg("--project")
        .arg(&sdk)
        .args(["--frozen", "python", "-I"])
        .arg(crate_dir.join("tests").join("sdk_gate.py"))
        .arg(tmp.path())
        .env("CND_SDK_DIR", &sdk)
        .output()
        .expect("run uv");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("{stdout}");
    assert!(
        output.status.success(),
        "sdk gate failed ({:?})\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
        output.status.code()
    );
    println!("sdk_gate: {count} examples PASS");
}
