//! An in-memory [`World`] implementation suitable for browsers.

use ecow::EcoString;
use rustc_hash::FxHashMap;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_ide::IdeWorld;

/// The file that is used as the main file until one is selected explicitly.
const DEFAULT_MAIN: &str = "main.typ";

/// Interns a project-root-relative path (given _without_ a leading slash) into
/// a [`FileId`].
///
/// Returns `None` if the path is malformed (e.g. if it escapes the project
/// root). Callers are expected to treat that as "unknown file" rather than to
/// panic, because the paths come from the host application at runtime.
pub fn file_id(path: &str) -> Option<FileId> {
    let vpath = VirtualPath::new(path).ok()?;
    Some(RootedPath::new(VirtualRoot::Project, vpath).intern())
}

/// A [`World`] whose entire file system and font set live in memory.
///
/// Everything is owned by the world itself: there is no file system access, no
/// package downloading and no ambient clock, which makes it usable from a
/// browser (`wasm32-unknown-unknown`) as well as from native tests.
pub struct BrowserWorld {
    /// The standard library.
    library: LazyHash<Library>,
    /// Metadata about the loaded fonts.
    book: LazyHash<FontBook>,
    /// The loaded fonts themselves.
    fonts: Vec<Font>,
    /// All known source files, including the main file.
    sources: FxHashMap<FileId, Source>,
    /// All known non-source files.
    assets: FxHashMap<FileId, Bytes>,
    /// The file that is currently treated as the main file.
    main: FileId,
}

impl BrowserWorld {
    /// Creates a world without any files or fonts.
    pub fn new() -> Self {
        // A plain file name can never escape the project root, so this is
        // infallible by construction.
        let main = file_id(DEFAULT_MAIN).expect("`main.typ` is a valid virtual path");
        Self {
            library: LazyHash::new(Library::builder().build()),
            book: LazyHash::new(FontBook::new()),
            fonts: Vec::new(),
            sources: FxHashMap::default(),
            assets: FxHashMap::default(),
            main,
        }
    }

    /// Loads a font file, adding every face it contains.
    ///
    /// Font collections (`.ttc`/`.otc`) contribute multiple faces.
    pub fn add_font(&mut self, bytes: &[u8]) {
        let data = Bytes::new(bytes.to_vec());
        self.fonts.extend(Font::iter(data));
        self.book = LazyHash::new(FontBook::from_fonts(&self.fonts));
    }

    /// Adds or replaces a source file.
    ///
    /// Does nothing if `path` is malformed.
    pub fn add_source(&mut self, path: &str, text: &str) {
        let Some(id) = file_id(path) else { return };
        self.sources.insert(id, Source::new(id, text.into()));
    }

    /// Adds or replaces a non-source file.
    ///
    /// Does nothing if `path` is malformed.
    pub fn add_asset(&mut self, path: &str, bytes: &[u8]) {
        let Some(id) = file_id(path) else { return };
        self.assets.insert(id, Bytes::new(bytes.to_vec()));
    }

    /// Removes a file, whether it is a source file or an asset.
    ///
    /// Does nothing if `path` is malformed or unknown.
    pub fn remove_file(&mut self, path: &str) {
        let Some(id) = file_id(path) else { return };
        self.sources.remove(&id);
        self.assets.remove(&id);
    }

    /// Selects the file that is treated as the main file.
    pub fn set_main(&mut self, id: FileId) {
        self.main = id;
    }

    /// Retrieves a source file by path, if it is known.
    pub fn source_at(&self, path: &str) -> Option<Source> {
        self.sources.get(&file_id(path)?).cloned()
    }
}

impl Default for BrowserWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl World for BrowserWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        match self.sources.get(&id) {
            Some(source) => Ok(source.clone()),
            None => Err(FileError::NotFound(id.vpath().get_without_slash().into())),
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Some(bytes) = self.assets.get(&id) {
            return Ok(bytes.clone());
        }

        // Source files are byte-addressable too. Without this fallback,
        // `read("chapter.typ")` — or any other `file()`-path access to a file
        // that was handed to the session as a source rather than as an asset —
        // would fail even though the world holds its text.
        if let Some(source) = self.sources.get(&id) {
            return Ok(Bytes::from_string(source.text().to_owned()));
        }

        Err(FileError::NotFound(id.vpath().get_without_slash().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        // There is no trustworthy ambient clock here, and returning `None`
        // keeps compilation deterministic.
        None
    }
}

impl IdeWorld for BrowserWorld {
    fn upcast(&self) -> &dyn World {
        self
    }

    fn files(&self) -> Vec<FileId> {
        self.sources.keys().chain(self.assets.keys()).copied().collect()
    }

    fn packages(&self) -> &[(PackageSpec, Option<EcoString>)] {
        // Packages cannot be resolved without network access.
        &[]
    }
}
