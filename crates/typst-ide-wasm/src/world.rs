//! An in-memory [`World`] implementation suitable for browsers.

use std::collections::BTreeSet;
use std::sync::Mutex;

use ecow::EcoString;
use rustc_hash::{FxHashMap, FxHashSet};
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

/// Accepts a library image name only if it is a bare file name.
///
/// Anything with a separator, an empty name or a `.`/`..` component is
/// rejected, so a name handed in by the host can never become a path that
/// reaches beyond the directory it is looked up in.
pub(crate) fn library_name(name: &str) -> Option<EcoString> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return None;
    }
    let path = VirtualPath::new(name).ok()?;
    (path.file_name() == Some(name)).then(|| name.into())
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
    /// Image names that any project file which does not exist may resolve
    /// to, by its bare file name (the same rule as the CLI's
    /// `--fallback-dir`).
    library_names: FxHashSet<EcoString>,
    /// The bytes of library images the host has supplied so far.
    library_bytes: FxHashMap<EcoString, Bytes>,
    /// Library images a lookup needed but whose bytes were not supplied yet.
    /// `World::file` takes `&self`, hence the lock.
    missing_library: Mutex<BTreeSet<String>>,
}

impl BrowserWorld {
    /// Creates a world without any files or fonts.
    pub fn new() -> Self {
        // A plain file name can never escape the project root, so this is
        // infallible by construction.
        let main = file_id(DEFAULT_MAIN).expect("`main.typ` is a valid virtual path");
        Self {
            // No export formats are registered: this world only ever
            // serves IDE queries, never exports. Upstream #8496 made the
            // format bindings (`pdf.*`, `html.*`) conditional on
            // registration, and pulling `typst-pdf` in would bloat the wasm
            // bundle for completions the CND pipeline is the real authority
            // on.
            library: LazyHash::new(Library::builder([]).build()),
            book: LazyHash::new(FontBook::new()),
            fonts: Vec::new(),
            sources: FxHashMap::default(),
            assets: FxHashMap::default(),
            main,
            library_names: FxHashSet::default(),
            library_bytes: FxHashMap::default(),
            missing_library: Mutex::new(BTreeSet::new()),
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

    /// Replaces the set of library image names.
    ///
    /// Invalid names are ignored. Bytes and pending lookups of names that are
    /// no longer in the set are dropped. Returns whether the set changed.
    pub fn set_library_images(&mut self, names: &[String]) -> bool {
        let names: FxHashSet<EcoString> =
            names.iter().filter_map(|name| library_name(name)).collect();
        if names == self.library_names {
            return false;
        }
        self.library_names = names;
        self.library_bytes.retain(|name, _| self.library_names.contains(name));
        self.missing_library
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|name| self.library_names.contains(name.as_str()));
        true
    }

    /// Supplies the bytes of a library image.
    ///
    /// Ignored unless `name` is in the current set. Returns whether the bytes
    /// were taken.
    pub fn add_library_image(&mut self, name: &str, bytes: &[u8]) -> bool {
        let Some(name) = library_name(name) else { return false };
        if !self.library_names.contains(&name) {
            return false;
        }
        self.missing_library
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(name.as_str());
        self.library_bytes.insert(name, Bytes::new(bytes.to_vec()));
        true
    }

    /// Returns, sorted, the library images lookups needed but that have no
    /// bytes yet, and forgets them.
    pub fn take_missing_library_images(&self) -> Vec<String> {
        let mut missing = self
            .missing_library
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::mem::take(&mut *missing).into_iter().collect()
    }

    /// Serves a project file that does not exist from the library.
    ///
    /// Returns `None` when the library has nothing to say about `id`: a
    /// package file, a name outside the set, or a file the project has.
    fn library_file(&self, id: FileId) -> Option<FileResult<Bytes>> {
        if !matches!(id.root(), VirtualRoot::Project)
            || self.sources.contains_key(&id)
            || self.assets.contains_key(&id)
        {
            return None;
        }
        let name = id.vpath().file_name()?;
        if !self.library_names.contains(name) {
            return None;
        }
        Some(match self.library_bytes.get(name) {
            Some(bytes) => Ok(bytes.clone()),
            None => {
                self.missing_library
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .insert(name.to_owned());
                Err(FileError::NotFound(id.vpath().get_without_slash().into()))
            }
        })
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
        // Deliberately no library fallback here: the library holds images
        // only, even though the CLI's own `--fallback-dir` serves sources
        // too.
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

        if let Some(result) = self.library_file(id) {
            return result;
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

    fn files(&self, base: FileId, prefix: Option<&str>) -> Vec<VirtualPath> {
        // Upstream widened this in #8699: the IDE now asks for virtual paths
        // relative to the file being completed, optionally narrowed by what
        // the user has already typed.
        let dir = base.vpath().parent();
        let matches_prefix = |path: &VirtualPath| {
            prefix.is_none_or(|prefix| {
                dir.as_ref()
                    .map(|dir| path.relative_from(dir))
                    .is_none_or(|relative| relative.starts_with(prefix))
            })
        };

        let project: FxHashSet<&VirtualPath> = self
            .sources
            .keys()
            .chain(self.assets.keys())
            .map(|id| id.vpath())
            .collect();
        let mut paths: Vec<VirtualPath> = project
            .iter()
            .filter(|path| matches_prefix(path))
            .map(|&path| path.clone())
            .collect();

        // A library image resolves by its bare name from any directory, so it
        // is offered as if it sat next to the file being completed — unless
        // the project has a file there, which wins and is already listed.
        // Paths are compared as they are, never interned: file ids are never
        // freed, and interning one per name and directory asked about would
        // exhaust them over a long session. The order of the hash set does
        // not matter: the IDE sorts the paths it gets.
        if let Some(dir) = &dir {
            paths.extend(
                self.library_names
                    .iter()
                    .filter_map(|name| dir.join(name).ok())
                    .filter(|path| !project.contains(path) && matches_prefix(path)),
            );
        }

        paths
    }

    fn packages(&self) -> &[(PackageSpec, Option<EcoString>)] {
        // Packages cannot be resolved without network access.
        &[]
    }
}
