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

use std::num::NonZeroUsize;

use typst::World;
use typst::introspection::PagedPosition;
use typst::layout::{Abs, Point};
use typst::syntax::{FileId, Side, Source};
use typst_ide::{Completion, Jump, Tooltip};
use typst_layout::PagedDocument;

/// How many compilations a memoized result may go unused before it is dropped.
///
/// The same value the CLI's watch loop uses.
const EVICT_MAX_AGE: usize = 10;

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

/// A position in the rendered document.
///
/// Coordinates are in **points**, measured from the top left of the page, and
/// the page number is **1-based**.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct PreviewPosition {
    /// The page the position is on, starting at 1.
    pub page: usize,
    /// The horizontal offset from the left edge of the page, in points.
    pub x: f64,
    /// The vertical offset from the top edge of the page, in points.
    pub y: f64,
}

/// Where a click in the rendered document leads.
///
/// This mirrors [`typst_ide::Jump`], which is not serializable itself.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum JumpResult {
    /// A position in a source file.
    File {
        /// The project-root-relative path, _without_ a leading slash.
        path: String,
        /// The UTF-8 byte offset into that file.
        offset: usize,
    },
    /// An external URL.
    Url {
        /// The target of the link.
        url: String,
    },
    /// Another position in the same document, e.g. an internal link.
    Position {
        /// The page the position is on, starting at 1.
        page: usize,
        /// The horizontal offset from the left edge of the page, in points.
        x: f64,
        /// The vertical offset from the top edge of the page, in points.
        y: f64,
    },
}

/// An editing session over a set of in-memory files.
///
/// The session owns the world, so files and fonts are added once and reused
/// across requests. It also caches the most recently compiled document, which
/// is what makes label completions, label tooltips and both jump directions
/// work.
pub struct Session {
    world: BrowserWorld,
    /// The most recently compiled document, if the last compilation succeeded.
    document: Option<PagedDocument>,
    /// The main file `document` was compiled from.
    document_main: Option<FileId>,
    /// Whether a file or font changed since `document` was compiled.
    dirty: bool,
}

impl Session {
    /// Creates an empty session, without any files or fonts.
    pub fn new() -> Self {
        Self {
            world: BrowserWorld::new(),
            document: None,
            document_main: None,
            dirty: true,
        }
    }

    /// Loads a font file, adding every face it contains.
    pub fn add_font(&mut self, bytes: &[u8]) {
        self.world.add_font(bytes);
        // Layout depends on the available fonts, so this invalidates the
        // cached document just like a file change does.
        self.dirty = true;
    }

    /// Adds or replaces a source file.
    pub fn add_source(&mut self, path: &str, text: &str) {
        self.world.add_source(path, text);
        self.dirty = true;
    }

    /// Adds or replaces a non-source file.
    pub fn add_asset(&mut self, path: &str, bytes: &[u8]) {
        self.world.add_asset(path, bytes);
        self.dirty = true;
    }

    /// Removes a file, whether it is a source file or an asset.
    pub fn remove_file(&mut self, path: &str) {
        self.world.remove_file(path);
        self.dirty = true;
    }

    /// Replaces the set of library image names (see
    /// [`BrowserWorld::set_library_images`]).
    pub fn set_library_images(&mut self, names: &[String]) {
        self.world.set_library_images(names);
        self.dirty = true;
    }

    /// Supplies the bytes of a library image.
    pub fn add_library_image(&mut self, name: &str, bytes: &[u8]) {
        self.world.add_library_image(name, bytes);
        self.dirty = true;
    }

    /// Returns and forgets the library images that lookups needed but whose
    /// bytes have not been supplied yet.
    pub fn take_missing_library_images(&mut self) -> Vec<String> {
        self.world.take_missing_library_images()
    }

    /// Compiles the project rooted at `main_path` and caches the result.
    ///
    /// Returns the error messages if compilation failed; the cached document
    /// is then cleared. Either way the session stops being dirty, so a failing
    /// project is not recompiled over and over by the on-demand paths.
    pub fn compile_impl(&mut self, main_path: &str) -> Result<(), Vec<String>> {
        let Some(main) = self.select_main(main_path) else {
            return Err(vec![format!("invalid main file path: {main_path}")]);
        };
        self.compile(main)
    }

    /// Computes the positions in the rendered document that correspond to
    /// `cursor` within the file at `path`.
    ///
    /// A span can be laid out more than once (e.g. in a repeated header), so
    /// this returns every match. Recompiles first if the cached document is
    /// stale: unlike a completion, a coordinate from an outdated layout is not
    /// merely incomplete, it is wrong.
    pub fn jump_from_cursor_impl(
        &mut self,
        main_path: &str,
        path: &str,
        cursor: usize,
    ) -> Vec<PreviewPosition> {
        let Some(main) = self.select_main(main_path) else {
            return Vec::new();
        };
        let Some(source) = self.world.source_at(path) else {
            return Vec::new();
        };
        // Sanitize before compiling, so the caller's offset is made safe on
        // every path through this function rather than only the one where a
        // document comes back.
        let cursor = sanitize_cursor(source.text(), cursor);

        self.ensure_document(main);
        let Some(document) = self.document.as_ref() else {
            return Vec::new();
        };

        typst_ide::jump_from_cursor(document, &source, cursor)
            .into_iter()
            .map(|position| PreviewPosition {
                page: position.page.get(),
                x: position.point.x.to_pt(),
                y: position.point.y.to_pt(),
            })
            .collect()
    }

    /// Determines where a click at `(x, y)` in points on the 1-based `page`
    /// of the rendered document leads.
    ///
    /// Returns `None` if there is nothing to jump to at that point, or if the
    /// project does not compile. Recompiles first if the cached document is
    /// stale, for the same reason as [`Self::jump_from_cursor_impl`].
    pub fn jump_from_click_impl(
        &mut self,
        main_path: &str,
        page: usize,
        x: f64,
        y: f64,
    ) -> Option<JumpResult> {
        // Validate the click before doing any work: pages are 1-based.
        let position = PagedPosition {
            page: NonZeroUsize::new(page)?,
            point: Point::new(Abs::pt(x), Abs::pt(y)),
        };

        let main = self.select_main(main_path)?;
        self.ensure_document(main);
        let document = self.document.as_ref()?;

        let jump = typst_ide::jump_from_click(&self.world, document, &position)?;
        Some(match jump {
            Jump::File(id, offset) => JumpResult::File {
                path: id.vpath().get_without_slash().to_owned(),
                offset,
            },
            Jump::Url(url) => JumpResult::Url { url: url.to_string() },
            Jump::Position(position) => JumpResult::Position {
                page: position.page.get(),
                x: position.point.x.to_pt(),
                y: position.point.y.to_pt(),
            },
        })
    }

    /// Computes completions at `cursor` within the file at `path`, compiling
    /// in the context of `main_path`.
    ///
    /// Returns `None` if either path is unknown or if there is nothing to
    /// complete at the cursor. Set `explicit` when the user asked for
    /// completions themselves rather than just typing.
    ///
    /// The cached document is passed along when there is one for the same main
    /// file, which is what makes label completions appear. It is deliberately
    /// _not_ recompiled here, even if it is stale: completions run on every
    /// keystroke, and a slightly outdated set of labels is far better than a
    /// full layout per character.
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
            self.cached_document(),
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
    /// at the cursor. Uses the cached document under the same rules as
    /// [`Self::autocomplete_impl`].
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
            self.cached_document(),
            &source,
            cursor,
            Side::Before,
        )
        .map(TooltipResult::from)
    }

    /// Points the world at `main_path`, returning its file id.
    ///
    /// Returns `None` if the path is malformed.
    fn select_main(&mut self, main_path: &str) -> Option<FileId> {
        let main = self::world::file_id(main_path)?;
        self.world.set_main(main);
        Some(main)
    }

    /// Points the world at `main_path` and retrieves the source at `path`.
    fn prepare(&mut self, main_path: &str, path: &str) -> Option<Source> {
        self.select_main(main_path)?;
        self.world.source_at(path)
    }

    /// The cached document, if it belongs to the currently selected main file.
    ///
    /// May be stale — see [`Self::autocomplete_impl`].
    fn cached_document(&self) -> Option<&PagedDocument> {
        if self.document_main != Some(self.world.main()) {
            return None;
        }
        self.document.as_ref()
    }

    /// Recompiles if the cached document does not match the current state of
    /// the world.
    ///
    /// A document is stale both when a file changed and when it was compiled
    /// from a different main file. Compilation errors are dropped here; the
    /// callers of this treat "no document" and "broken project" alike.
    fn ensure_document(&mut self, main: FileId) {
        if self.dirty || self.document_main != Some(main) {
            let _ = self.compile(main);
        }
    }

    /// Compiles the world as it stands and caches the outcome.
    fn compile(&mut self, main: FileId) -> Result<(), Vec<String>> {
        let compiled = typst::compile::<PagedDocument>(&self.world);

        // The session is long-lived and memoized layout results would
        // otherwise accumulate for every keystroke's worth of edits. This
        // matches what the CLI's watch loop does after each recompilation.
        comemo::evict(EVICT_MAX_AGE);

        self.dirty = false;
        self.document_main = Some(main);
        match compiled.output {
            Ok(document) => {
                self.document = Some(document);
                Ok(())
            }
            Err(errors) => {
                self.document = None;
                Err(errors.iter().map(|error| error.message.to_string()).collect())
            }
        }
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
    fn cursor_jump_lands_on_a_page() {
        let mut s = session_with("= Heading\n\nSome text here.");
        s.compile_impl("main.typ").expect("compiles");
        let positions = s.jump_from_cursor_impl("main.typ", "main.typ", 14);
        assert!(!positions.is_empty());
        assert_eq!(positions[0].page, 1);
    }

    #[test]
    fn click_jump_resolves_back_to_source() {
        let mut s = session_with("= Heading\n\nSome text here.");
        s.compile_impl("main.typ").expect("compiles");
        let positions = s.jump_from_cursor_impl("main.typ", "main.typ", 14);
        let p = &positions[0];
        let jump = s.jump_from_click_impl("main.typ", p.page, p.x, p.y).expect("jump");
        match jump {
            JumpResult::File { path, .. } => assert_eq!(path, "main.typ"),
            other => panic!("expected file jump, got {other:?}"),
        }
    }

    /// `read` goes through `World::file`, which must serve files that were
    /// handed to the session as sources — the assertion inside the document
    /// fails the compilation if the bytes come back wrong.
    #[test]
    fn read_serves_source_files() {
        let mut s = session_with("#assert.eq(read(\"chapter.typ\"), \"= Ch\")");
        s.add_source("chapter.typ", "= Ch");
        s.compile_impl("main.typ")
            .unwrap_or_else(|errors| panic!("compile failed: {errors:?}"));
    }

    #[test]
    fn an_edit_invalidates_the_cached_document() {
        let mut s = session_with("Some text here.");
        s.compile_impl("main.typ").expect("compiles");
        let before = s.jump_from_cursor_impl("main.typ", "main.typ", 2);
        assert!(!before.is_empty());

        // Push the paragraph down the page. Without invalidation the jump
        // would still report the old, higher position.
        s.add_source("main.typ", "#v(100pt)\nSome text here.");
        let after = s.jump_from_cursor_impl("main.typ", "main.typ", 12);
        assert!(!after.is_empty());
        assert!(
            after[0].y > before[0].y + 50.0,
            "expected the text to move down, got {} then {}",
            before[0].y,
            after[0].y
        );
    }

    /// Label completions are the reason the document is threaded into
    /// `autocomplete` at all, so they double as the proof that it is.
    #[test]
    fn label_completions_come_from_the_cached_document() {
        let mut s = session_with("= Heading <intro>\n\n@");
        let without = s.autocomplete_impl("main.typ", "main.typ", 20, true);
        assert!(
            !without.is_some_and(|r| r.completions.iter().any(|c| c.label == "intro")),
            "there is no document yet, so there are no labels to offer"
        );

        s.compile_impl("main.typ").expect("compiles");
        let with = s
            .autocomplete_impl("main.typ", "main.typ", 20, true)
            .expect("completions");
        assert!(with.completions.iter().any(|c| c.label == "intro"));
    }

    #[test]
    fn jumps_yield_nothing_on_bad_input() {
        let mut s = session_with("= Heading\n\nSome text here.");
        // Malformed main path: no file id can be built.
        assert!(s.jump_from_cursor_impl("../escape.typ", "main.typ", 14).is_empty());
        assert!(s.jump_from_click_impl("../escape.typ", 1, 10.0, 10.0).is_none());
        // Unknown file to jump from.
        assert!(s.jump_from_cursor_impl("main.typ", "missing.typ", 14).is_empty());
        // Pages are 1-based, and there is only one of them.
        assert!(s.jump_from_click_impl("main.typ", 0, 10.0, 10.0).is_none());
        assert!(s.jump_from_click_impl("main.typ", 99, 10.0, 10.0).is_none());
        // A click on the empty margin has nothing under it.
        assert!(s.jump_from_click_impl("main.typ", 1, 1.0, 1.0).is_none());
    }

    #[test]
    fn jump_results_serialize_to_the_documented_shape() {
        let mut s = session_with("= Heading\n\nSome text here.");
        s.compile_impl("main.typ").expect("compiles");

        let positions = s.jump_from_cursor_impl("main.typ", "main.typ", 14);
        let json = serde_json::to_value(positions[0]).expect("serializable");
        assert_eq!(json["page"], 1);
        assert!(json["x"].as_f64().is_some_and(|x| x > 0.0));
        assert!(json["y"].as_f64().is_some_and(|y| y > 0.0));

        let p = positions[0];
        let jump = s.jump_from_click_impl("main.typ", p.page, p.x, p.y).expect("jump");
        let json = serde_json::to_value(&jump).expect("serializable");
        assert_eq!(json["kind"], "file");
        assert_eq!(json["path"], "main.typ");
        assert!(json["offset"].is_u64());
    }

    #[test]
    fn clicking_a_link_yields_its_url() {
        let mut s = session_with("#link(\"https://typst.app\")[Go]");
        s.compile_impl("main.typ").expect("compiles");
        // Byte 28 sits inside the `Go` text node.
        let positions = s.jump_from_cursor_impl("main.typ", "main.typ", 28);
        let p = positions[0];
        let jump = s.jump_from_click_impl("main.typ", p.page, p.x, p.y).expect("jump");
        let json = serde_json::to_value(&jump).expect("serializable");
        assert_eq!(json["kind"], "url");
        assert_eq!(json["url"], "https://typst.app");
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

        // The jump path takes a cursor too, so it carries the same contract.
        // The document has to compile for the offset to reach the sanitizer,
        // hence a plain paragraph rather than the code expression above.
        let mut s = session_with("héllo wörld");
        s.compile_impl("main.typ").expect("compiles");
        // `é` occupies bytes 1..3.
        let _ = s.jump_from_cursor_impl("main.typ", "main.typ", 2);
        let _ = s.jump_from_cursor_impl("main.typ", "main.typ", 9_999);
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

    /// A 10×10 SVG: small, valid, and decodable without any image feature.
    const SVG: &[u8] =
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| name.to_string()).collect()
    }

    fn completion_labels(s: &mut Session, path: &str, cursor: usize) -> Vec<String> {
        s.autocomplete_impl("main.typ", path, cursor, true)
            .map(|r| r.completions.into_iter().map(|c| c.label.to_string()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn library_images_resolve_by_bare_name_from_any_directory() {
        let mut s = session_with("#include \"chapters/intro.typ\"\n#image(\"logo.svg\")");
        s.add_source("chapters/intro.typ", "#image(\"logo.svg\")");
        s.set_library_images(&names(&["logo.svg"]));
        s.add_library_image("logo.svg", SVG);
        s.compile_impl("main.typ")
            .unwrap_or_else(|errors| panic!("compile failed: {errors:?}"));
        assert!(s.take_missing_library_images().is_empty());
    }

    #[test]
    fn a_project_file_wins_over_the_library() {
        // The project's own `logo.svg` is not an image at all, so the
        // compilation can only succeed if the library copy were used.
        let mut s = session_with("#image(\"logo.svg\")");
        s.add_asset("logo.svg", b"not an image");
        s.set_library_images(&names(&["logo.svg"]));
        s.add_library_image("logo.svg", SVG);
        assert!(s.compile_impl("main.typ").is_err());
    }

    #[test]
    fn a_project_file_in_a_subfolder_wins_there() {
        let mut s = session_with("#include \"chapters/intro.typ\"");
        s.add_source("chapters/intro.typ", "#image(\"logo.svg\")");
        s.add_asset("chapters/logo.svg", b"not an image");
        s.set_library_images(&names(&["logo.svg"]));
        s.add_library_image("logo.svg", SVG);
        assert!(s.compile_impl("main.typ").is_err());
    }

    #[test]
    fn missing_bytes_are_reported_once_then_served() {
        let mut s = session_with("#image(\"logo.svg\")");
        s.set_library_images(&names(&["logo.svg"]));
        let errors = s.compile_impl("main.typ").expect_err("bytes not there yet");
        assert!(errors.iter().any(|e| e.contains("not found")), "{errors:?}");
        assert_eq!(s.take_missing_library_images(), names(&["logo.svg"]));
        assert!(s.take_missing_library_images().is_empty(), "drained");

        s.add_library_image("logo.svg", SVG);
        s.compile_impl("main.typ")
            .unwrap_or_else(|errors| panic!("compile failed: {errors:?}"));
    }

    #[test]
    fn names_outside_the_set_are_never_served_or_reported() {
        let mut s = session_with("#image(\"other.svg\")");
        s.set_library_images(&names(&["logo.svg"]));
        s.add_library_image("logo.svg", SVG);
        // Bytes for a name that is not in the set are ignored.
        s.add_library_image("other.svg", SVG);
        assert!(s.compile_impl("main.typ").is_err());
        assert!(s.take_missing_library_images().is_empty());
    }

    #[test]
    fn hostile_names_are_ignored() {
        let hostile = ["../x.svg", "a/b.svg", "a\\b.svg", "", ".", ".."];
        let mut s = session_with("#image(\"x.svg\")");
        s.set_library_images(&names(&hostile));
        for name in hostile {
            s.add_library_image(name, SVG);
        }
        assert!(s.compile_impl("main.typ").is_err());
        assert!(s.take_missing_library_images().is_empty());
    }

    #[test]
    fn replacing_the_set_drops_bytes_and_invalidates_the_document() {
        // A cursor jump only lands on text, so the image comes after a word
        // the cursor (offset 1) sits in.
        let mut s = session_with("Hello\n#image(\"logo.svg\")");
        s.set_library_images(&names(&["logo.svg"]));
        s.add_library_image("logo.svg", SVG);
        s.compile_impl("main.typ").expect("compiles");
        assert!(!s.jump_from_cursor_impl("main.typ", "main.typ", 1).is_empty());

        s.set_library_images(&names(&[]));
        // The jump recompiles because the set changed; the image is gone,
        // so the project no longer compiles and there is nothing to jump to.
        assert!(s.jump_from_cursor_impl("main.typ", "main.typ", 1).is_empty());
        // Re-adding the name without bytes does not resurrect the old bytes.
        s.set_library_images(&names(&["logo.svg"]));
        assert!(s.compile_impl("main.typ").is_err());
    }

    #[test]
    fn package_files_never_fall_back() {
        use typst::World;
        use typst::syntax::package::PackageSpec;
        use typst::syntax::{RootedPath, VirtualPath, VirtualRoot};

        let mut world = BrowserWorld::new();
        world.set_library_images(&names(&["logo.svg"]));
        world.add_library_image("logo.svg", SVG);
        let spec: PackageSpec = "@preview/pkg:0.1.0".parse().expect("spec");
        let id = RootedPath::new(
            VirtualRoot::Package(spec),
            VirtualPath::new("logo.svg").expect("path"),
        )
        .intern();
        assert!(world.file(id).is_err());
        assert!(world.take_missing_library_images().is_empty());
    }

    #[test]
    fn library_name_rejects_anything_but_a_bare_file_name() {
        use crate::world::library_name;
        assert_eq!(library_name("a/b.svg"), None);
        assert_eq!(library_name("a\\b.svg"), None);
        assert_eq!(library_name(".."), None);
        assert_eq!(library_name("."), None);
        assert_eq!(library_name(""), None);
        assert_eq!(library_name("logo.svg"), Some("logo.svg".into()));
    }

    #[test]
    fn library_images_complete_by_bare_name() {
        let mut s = session_with("#image(\"\")");
        s.set_library_images(&names(&["logo.svg"]));
        let labels = completion_labels(&mut s, "main.typ", 8);
        assert!(labels.iter().any(|l| l.contains("logo.svg")), "{labels:?}");
    }

    #[test]
    fn library_images_complete_by_bare_name_in_a_subfolder() {
        let mut s = session_with("#include \"chapters/intro.typ\"");
        s.add_source("chapters/intro.typ", "#image(\"\")");
        s.set_library_images(&names(&["logo.svg"]));
        let labels = completion_labels(&mut s, "chapters/intro.typ", 8);
        assert!(labels.iter().any(|l| l.trim_matches('"') == "logo.svg"), "{labels:?}");
        assert!(!labels.iter().any(|l| l.contains("../logo.svg")), "{labels:?}");
    }

    #[test]
    fn a_project_file_and_a_library_image_of_the_same_name_complete_once() {
        let mut s = session_with("#image(\"\")");
        s.add_asset("logo.svg", SVG);
        s.set_library_images(&names(&["logo.svg"]));
        let labels = completion_labels(&mut s, "main.typ", 8);
        let count = labels.iter().filter(|l| l.contains("logo.svg")).count();
        assert_eq!(count, 1, "{labels:?}");
    }

    #[test]
    fn library_completions_respect_the_typed_prefix_and_extension() {
        let mut s = session_with("#image(\"zz\")");
        s.set_library_images(&names(&["logo.svg", "notes.csv"]));
        assert!(
            !completion_labels(&mut s, "main.typ", 10)
                .iter()
                .any(|l| l.contains("logo"))
        );

        let mut s = session_with("#image(\"\")");
        s.set_library_images(&names(&["logo.svg", "notes.csv"]));
        let labels = completion_labels(&mut s, "main.typ", 8);
        assert!(!labels.iter().any(|l| l.contains("notes.csv")), "{labels:?}");
    }

    #[test]
    fn hostile_names_never_complete() {
        let mut s = session_with("#image(\"\")");
        s.set_library_images(&names(&["../x.svg", "a/b.svg", "a\\b.svg"]));
        let labels = completion_labels(&mut s, "main.typ", 8);
        assert!(
            !labels.iter().any(|l| l.contains("x.svg") || l.contains("b.svg")),
            "{labels:?}"
        );
    }
}
