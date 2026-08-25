//! The JavaScript-facing surface of the crate.

use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::Session;

/// The JavaScript-facing editing session.
#[wasm_bindgen]
pub struct IdeSession(Session);

#[wasm_bindgen]
impl IdeSession {
    /// Creates an empty session, without any files or fonts.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(Session::new())
    }

    /// Loads a font file, adding every face it contains.
    #[wasm_bindgen(js_name = addFont)]
    pub fn add_font(&mut self, bytes: &[u8]) {
        self.0.add_font(bytes);
    }

    /// Adds or replaces a source file.
    #[wasm_bindgen(js_name = addSource)]
    pub fn add_source(&mut self, path: &str, text: &str) {
        self.0.add_source(path, text);
    }

    /// Adds or replaces a non-source file.
    #[wasm_bindgen(js_name = addAsset)]
    pub fn add_asset(&mut self, path: &str, bytes: &[u8]) {
        self.0.add_asset(path, bytes);
    }

    /// Removes a file, whether it is a source file or an asset.
    #[wasm_bindgen(js_name = removeFile)]
    pub fn remove_file(&mut self, path: &str) {
        self.0.remove_file(path);
    }

    /// Compiles the project rooted at `main_path`, refreshing the cached
    /// document that the jump and label features read from.
    ///
    /// Returns the compilation error messages as an array of strings; an empty
    /// array means the project compiled.
    pub fn compile(&mut self, main_path: &str) -> Vec<String> {
        self.0.compile_impl(main_path).err().unwrap_or_default()
    }

    /// Computes the positions in the rendered document that correspond to
    /// `cursor` (a UTF-8 byte offset) within the file at `path`.
    ///
    /// Returns `[{ page, x, y }]`, with `x`/`y` in points from the top left of
    /// the page and `page` starting at 1. The array is empty when there is
    /// nothing to jump to.
    #[wasm_bindgen(js_name = jumpFromCursor)]
    pub fn jump_from_cursor(
        &mut self,
        main_path: &str,
        path: &str,
        cursor: usize,
    ) -> JsValue {
        to_js(&self.0.jump_from_cursor_impl(main_path, path, cursor))
    }

    /// Determines where a click at `(x, y)` — in points from the top left of
    /// the 1-based `page` — leads.
    ///
    /// Returns `{ kind: "file", path, offset }`, `{ kind: "url", url }`,
    /// `{ kind: "position", page, x, y }` or `null`. The `offset` is a UTF-8
    /// byte offset.
    #[wasm_bindgen(js_name = jumpFromClick)]
    pub fn jump_from_click(
        &mut self,
        main_path: &str,
        page: usize,
        x: f64,
        y: f64,
    ) -> JsValue {
        match self.0.jump_from_click_impl(main_path, page, x, y) {
            Some(result) => to_js(&result),
            None => JsValue::NULL,
        }
    }

    /// Computes completions at `cursor` (a UTF-8 byte offset) within the
    /// file at `path`.
    ///
    /// Returns `{ from, completions }` or `null`.
    pub fn autocomplete(
        &mut self,
        main_path: &str,
        path: &str,
        cursor: usize,
        explicit: bool,
    ) -> JsValue {
        match self.0.autocomplete_impl(main_path, path, cursor, explicit) {
            Some(result) => to_js(&result),
            None => JsValue::NULL,
        }
    }

    /// Computes the tooltip at `cursor` (a UTF-8 byte offset) within the
    /// file at `path`.
    ///
    /// Returns `{ kind, value }` or `null`.
    pub fn tooltip(&mut self, main_path: &str, path: &str, cursor: usize) -> JsValue {
        match self.0.tooltip_impl(main_path, path, cursor) {
            Some(result) => to_js(&result),
            None => JsValue::NULL,
        }
    }
}

impl Default for IdeSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializes a result, falling back to `null` if that somehow fails.
fn to_js(value: &impl serde::Serialize) -> JsValue {
    serde_wasm_bindgen::to_value(value).unwrap_or(JsValue::NULL)
}
