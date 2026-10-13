//! `typst compile --format cnd` with `--cnd-pdf` and `--cnd-preview`: a PDF
//! of the whole document and a PNG of its first page, exported from the
//! layout the CND compile already built.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// Environment variables the CLI reads as defaults for its flags. A test
/// must not depend on the environment it runs in (same list as `cnd.rs`).
const CLI_ENV: &[&str] = &[
    "TYPST_CERT",
    "TYPST_ROOT",
    "SOURCE_DATE_EPOCH",
    "TYPST_FEATURES",
    "TYPST_DIAGNOSTIC_FORMAT",
    "TYPST_PACKAGE_PATH",
    "TYPST_PACKAGE_CACHE_PATH",
    "TYPST_FONT_PATHS",
    "TYPST_IGNORE_SYSTEM_FONTS",
    "TYPST_IGNORE_EMBEDDED_FONTS",
];

/// Two pages of 200pt × 100pt.
const TWO_PAGES: &str =
    "#set page(width: 200pt, height: 100pt)\n= One\n#pagebreak()\n= Two\n";

fn exec() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_typst"));
    for var in CLI_ENV {
        cmd.env_remove(var);
    }
    cmd
}

fn write(dir: &Path, rel: &str, data: &str) -> PathBuf {
    let path = dir.join(rel);
    std::fs::write(&path, data).unwrap();
    path
}

fn png_width(bytes: &[u8]) -> u32 {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    // The IHDR chunk follows the signature: length, type, then width (BE).
    u32::from_be_bytes(bytes[16..20].try_into().unwrap())
}

fn pdf_page_count(bytes: Vec<u8>) -> usize {
    let pdf = hayro_syntax::Pdf::new(Arc::new(bytes))
        .unwrap_or_else(|_| panic!("not a readable PDF"));
    pdf.pages().len()
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn renders_are_written_from_the_cnd_compile() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", TWO_PAGES);
    let pdf = dir.path().join("document.pdf");
    let png = dir.path().join("page-1.png");
    let out = exec()
        .arg("compile")
        .arg(&main)
        .args(["--format", "cnd", "--cnd-pdf"])
        .arg(&pdf)
        .arg("--cnd-preview")
        .arg(&png)
        .args(["--cnd-preview-ppi", "144"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(dir.path().join("main.cnd").exists());

    let pdf_bytes = std::fs::read(&pdf).unwrap();
    assert!(pdf_bytes.starts_with(b"%PDF-"));
    assert_eq!(pdf_page_count(pdf_bytes), 2);
    // 200pt wide at 144 ppi: 200 × 144 / 72.
    assert_eq!(png_width(&std::fs::read(&png).unwrap()), 400);
}

#[test]
fn the_preview_ppi_defaults_to_72() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", TWO_PAGES);
    let png = dir.path().join("page-1.png");
    let out = exec()
        .arg("compile")
        .arg(&main)
        .args(["--format", "cnd", "--cnd-preview"])
        .arg(&png)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(png_width(&std::fs::read(&png).unwrap()), 200);
    assert!(!dir.path().join("document.pdf").exists());
}

#[test]
fn without_the_flags_only_the_cnd_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", TWO_PAGES);
    let out = exec()
        .arg("compile")
        .arg(&main)
        .args(["--format", "cnd"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["main.cnd", "main.typ"]);
}

#[test]
fn an_unwritable_render_is_a_warning_and_keeps_the_cnd() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", TWO_PAGES);
    let missing = dir.path().join("missing");
    let out = exec()
        .arg("compile")
        .arg(&main)
        .args(["--format", "cnd", "--diagnostic-format", "short", "--cnd-pdf"])
        .arg(missing.join("document.pdf"))
        .arg("--cnd-preview")
        .arg(missing.join("page-1.png"))
        .output()
        .unwrap();
    let err = stderr(&out);
    // Printed so a caller parsing these lines can copy them exactly.
    eprintln!("{err}");
    assert!(out.status.success(), "{err}");
    assert!(dir.path().join("main.cnd").exists());
    assert!(
        err.lines()
            .any(|l| l.starts_with("warning: cnd-pdf: failed to write PDF file")),
        "{err}"
    );
    assert!(
        err.lines()
            .any(|l| l.starts_with("warning: cnd-preview: failed to write PNG file")),
        "{err}"
    );
}

#[test]
fn the_render_flags_need_the_cnd_format() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", TWO_PAGES);
    let out = exec()
        .arg("compile")
        .arg(&main)
        .args(["--format", "pdf", "--cnd-pdf"])
        .arg(dir.path().join("x.pdf"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("--cnd-pdf and --cnd-preview need --format cnd"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn the_preview_ppi_must_be_positive() {
    let dir = tempfile::tempdir().unwrap();
    let main = write(dir.path(), "main.typ", TWO_PAGES);
    let out = exec()
        .arg("compile")
        .arg(&main)
        .args(["--format", "cnd", "--cnd-preview-ppi", "0", "--cnd-preview"])
        .arg(dir.path().join("p.png"))
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("--cnd-preview-ppi must be a positive number"),
        "{}",
        stderr(&out)
    );
}
