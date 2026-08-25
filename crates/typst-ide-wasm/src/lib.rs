//! Browser bindings for [`typst_ide`].
//!
//! The crate is split in two layers:
//!
//! - [`Session`], a plain-Rust facade over an in-memory [`World`] that is
//!   available on every target and therefore testable natively.
//! - `IdeSession`, a thin `wasm-bindgen` wrapper around it that is only
//!   compiled for `wasm32` targets, so native builds pull in no JS glue.
//!
//! All paths handed to a session are project-root-relative and given _without_
//! a leading slash. All cursor positions are UTF-8 byte offsets into the file
//! they refer to.
//!
//! [`World`]: typst::World

#[cfg(target_arch = "wasm32")]
mod bindings;
mod world;

#[cfg(target_arch = "wasm32")]
pub use self::bindings::IdeSession;
pub use self::world::BrowserWorld;

use typst::syntax::Side;
use typst_ide::{Completion, Tooltip};
use typst_layout::PagedDocument;

/// The result of an autocompletion request.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CompletionsResult {
    /// The byte offset from which the completions apply, i.e. the start of the
    /// text they replace.
    pub from: usize,
    /// The completion candidates.
    pub completions: Vec<Completion>,
}

/// The result of a tooltip request.
///
/// This mirrors [`typst_ide::Tooltip`], which is not serializable itself.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum TooltipResult {
    /// A string of text.
    Text(String),
    /// A string of Typst code.
    Code(String),
}

impl From<Tooltip> for TooltipResult {
    fn from(tooltip: Tooltip) -> Self {
        match tooltip {
            Tooltip::Text(text) => Self::Text(text.into()),
            Tooltip::Code(code) => Self::Code(code.into()),
        }
    }
}

/// An editing session over a set of in-memory files.
///
/// The session owns the world, so files and fonts are added once and reused
/// across requests.
pub struct Session {
    world: BrowserWorld,
}

impl Session {
    /// Creates an empty session, without any files or fonts.
    pub fn new() -> Self {
        Self { world: BrowserWorld::new() }
    }

    /// Loads a font file, adding every face it contains.
    pub fn add_font(&mut self, bytes: &[u8]) {
        self.world.add_font(bytes);
    }

    /// Adds or replaces a source file.
    pub fn add_source(&mut self, path: &str, text: &str) {
        self.world.add_source(path, text);
    }

    /// Adds or replaces a non-source file.
    pub fn add_asset(&mut self, path: &str, bytes: &[u8]) {
        self.world.add_asset(path, bytes);
    }

    /// Removes a file, whether it is a source file or an asset.
    pub fn remove_file(&mut self, path: &str) {
        self.world.remove_file(path);
    }

    /// Computes completions at `cursor` within the file at `path`, compiling
    /// in the context of `main_path`.
    ///
    /// Returns `None` if either path is unknown or if there is nothing to
    /// complete at the cursor. Set `explicit` when the user asked for
    /// completions themselves rather than just typing.
    pub fn autocomplete_impl(
        &mut self,
        main_path: &str,
        path: &str,
        cursor: usize,
        explicit: bool,
    ) -> Option<CompletionsResult> {
        let source = self.prepare(main_path, path)?;
        let cursor = sanitize_cursor(source.text(), cursor);
        let (from, completions) = typst_ide::autocomplete(
            &self.world,
            Option::<&PagedDocument>::None,
            &source,
            cursor,
            explicit,
        )?;
        Some(CompletionsResult { from, completions })
    }

    /// Computes the tooltip at `cursor` within the file at `path`, compiling
    /// in the context of `main_path`.
    ///
    /// Returns `None` if either path is unknown or if there is nothing to show
    /// at the cursor.
    pub fn tooltip_impl(
        &mut self,
        main_path: &str,
        path: &str,
        cursor: usize,
    ) -> Option<TooltipResult> {
        let source = self.prepare(main_path, path)?;
        let cursor = sanitize_cursor(source.text(), cursor);
        typst_ide::tooltip(
            &self.world,
            Option::<&PagedDocument>::None,
            &source,
            cursor,
            Side::Before,
        )
        .map(TooltipResult::from)
    }

    /// Points the world at `main_path` and retrieves the source at `path`.
    fn prepare(&mut self, main_path: &str, path: &str) -> Option<typst::syntax::Source> {
        let main = self::world::file_id(main_path)?;
        self.world.set_main(main);
        self.world.source_at(path)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// Makes a caller-supplied byte offset safe to slice `text` at.
///
/// The offset is clamped to the length of the text and then snapped back to
/// the nearest preceding UTF-8 character boundary. Both are load-bearing:
/// `typst-ide` slices the source at the cursor, so an offset that is past the
/// end or in the middle of a multi-byte character would panic — and a panic
/// would poison the whole module for the rest of the session.
fn sanitize_cursor(text: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(text.len());
    // Terminates because offset 0 is always a character boundary.
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with(text: &str) -> Session {
        let mut s = Session::new();
        for data in typst_assets::fonts() {
            s.add_font(data);
        }
        s.add_source("main.typ", text);
        s
    }

    #[test]
    fn completes_stdlib_function() {
        let mut s = session_with("#tab");
        let result = s
            .autocomplete_impl("main.typ", "main.typ", 4, true)
            .expect("completions");
        assert!(result.completions.iter().any(|c| c.label == "table"));
    }

    #[test]
    fn tooltip_on_function() {
        let mut s = session_with("#table()");
        let tip = s.tooltip_impl("main.typ", "main.typ", 3);
        assert!(tip.is_some());
    }

    #[test]
    fn completes_project_files_in_import() {
        let mut s = session_with("#import \"\"");
        s.add_source("chapter.typ", "= Ch");
        let result = s
            .autocomplete_impl("main.typ", "main.typ", 9, true)
            .expect("completions");
        assert!(result.completions.iter().any(|c| c.label.contains("chapter.typ")));
    }

    #[test]
    fn serializes_to_the_documented_shape() {
        let mut s = session_with("#tab");
        let result = s
            .autocomplete_impl("main.typ", "main.typ", 4, true)
            .expect("completions");
        let json = serde_json::to_value(&result).expect("serializable");
        assert!(json["from"].is_u64());
        let entry = json["completions"]
            .as_array()
            .expect("array")
            .iter()
            .find(|c| c["label"] == "table")
            .expect("`table` completion");
        // A unit `CompletionKind` serializes to a bare kebab-case string.
        assert_eq!(entry["kind"], "func");
        assert!(entry["apply"].as_str().is_some_and(|s| s.contains("table")));
        assert_eq!(entry["detail"], "A table of items.");

        let mut s = session_with("#table()");
        let tip = s
            .tooltip_impl("main.typ", "main.typ", 3)
            .expect("tooltip on `#table`");
        let json = serde_json::to_value(&tip).expect("serializable");
        assert!(json["kind"] == "text" || json["kind"] == "code");
        assert!(json["value"].is_string());
    }

    /// The variant `CompletionKind::Symbol(_)` is externally tagged, so it
    /// serializes to an object where every other kind is a bare string.
    /// Consumers have to handle both, hence this guard.
    #[test]
    fn symbol_completions_serialize_as_an_object() {
        let mut s = session_with("#sym.");
        let result = s
            .autocomplete_impl("main.typ", "main.typ", 5, true)
            .expect("completions");
        let symbol = result
            .completions
            .iter()
            .find(|c| matches!(c.kind, typst_ide::CompletionKind::Symbol(_)))
            .expect("at least one symbol completion");

        let json = serde_json::to_value(symbol).expect("serializable");
        assert!(json["kind"].is_object(), "got {}", json["kind"]);
        let inner = json["kind"]["symbol"].as_str().expect("`symbol` payload");
        assert!(!inner.is_empty());
        assert!(json["label"].is_string());
    }

    #[test]
    fn unknown_paths_yield_none() {
        let mut s = session_with("#tab");
        assert!(s.autocomplete_impl("main.typ", "missing.typ", 0, true).is_none());
        assert!(s.tooltip_impl("main.typ", "missing.typ", 0).is_none());
    }

    #[test]
    fn unknown_main_path_yields_none() {
        let mut s = session_with("#tab");
        // Unknown but well-formed: the file simply is not in the world.
        assert!(s.autocomplete_impl("missing.typ", "main.typ", 4, true).is_some());
        // Malformed: escapes the project root, so no `FileId` can be built.
        assert!(s.autocomplete_impl("../escape.typ", "main.typ", 4, true).is_none());
        assert!(s.tooltip_impl("../escape.typ", "main.typ", 4).is_none());
    }

    #[test]
    fn mid_character_cursors_do_not_panic() {
        // `é` occupies bytes 1..3, so offset 2 is inside it.
        let mut s = session_with("#émoji");
        let _ = s.autocomplete_impl("main.typ", "main.typ", 2, true);
        let _ = s.tooltip_impl("main.typ", "main.typ", 2);

        // Well past the end of the text.
        let _ = s.autocomplete_impl("main.typ", "main.typ", 9_999, true);
        let _ = s.tooltip_impl("main.typ", "main.typ", 9_999);
    }

    #[test]
    fn sanitize_cursor_snaps_back_to_a_boundary() {
        let text = "#émoji";
        assert_eq!(sanitize_cursor(text, 0), 0);
        assert_eq!(sanitize_cursor(text, 1), 1);
        assert_eq!(sanitize_cursor(text, 2), 1); // inside `é`
        assert_eq!(sanitize_cursor(text, 3), 3);
        assert_eq!(sanitize_cursor(text, 9_999), text.len());
        assert_eq!(sanitize_cursor("", 5), 0);
    }
}
