//! Minimal system world for the typst-cnd CLI.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::downloader::SystemDownloader;
use typst_kit::files::{FileLoader, FileStore, FsRoot};
use typst_kit::fonts::{self, FontStore};
use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};

/// A world for compiling `.typ` sources into CND manifests.
pub struct CndWorld {
    main: FileId,
    library: LazyHash<Library>,
    fonts: LazyLock<FontStore, Box<dyn Fn() -> FontStore + Send + Sync>>,
    files: FileStore<CndFiles>,
    now: Time,
}

fn build_library() -> Library {
    let mut library = Library::builder().build();
    library.global.scope_mut().define("cnd", crate::cnd::module());
    library
}

impl CndWorld {
    pub fn new(input: &Path) -> Result<Self, FileError> {
        let root = input
            .parent()
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(|err| FileError::from_io(err, input))?;

        let input = input
            .canonicalize()
            .map_err(|err| FileError::from_io(err, input))?;

        let main = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::virtualize(&root, &input).map_err(|_| FileError::Other(None))?,
        )
        .intern();

        Ok(Self {
            main,
            library: LazyHash::new(build_library()),
            fonts: LazyLock::new(Box::new(|| {
                let mut store = FontStore::new();
                store.extend(fonts::embedded());
                store.extend(fonts::system());
                store
            })),
            files: FileStore::new(CndFiles {
                project: FsRoot::new(root),
                packages: SystemPackages::from_parts(
                    FsPackages::system_data(),
                    FsPackages::system_cache(),
                    UniversePackages::new(SystemDownloader::new("typst-cnd")),
                ),
            }),
            now: Time::system(),
        })
    }

    pub fn reset(&mut self) {
        self.files.reset();
        self.now.reset();
    }
}

impl World for CndWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.now.today(offset)
    }
}

struct CndFiles {
    project: FsRoot,
    packages: SystemPackages,
}

impl CndFiles {
    fn root(&self, id: FileId) -> FileResult<FsRoot> {
        Ok(match id.root() {
            VirtualRoot::Project => self.project.clone(),
            VirtualRoot::Package(spec) => self.packages.obtain(spec)?,
        })
    }
}

impl FileLoader for CndFiles {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        self.root(id)?.load(id.vpath())
    }
}

/// Compute a SHA-256 content hash for the main source file.
pub fn doc_hash(world: &dyn World) -> String {
    use sha2::{Digest, Sha256};

    let main = world.main();
    let source = world.source(main).expect("main source");
    let digest = Sha256::digest(source.text().as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

/// Current UTC timestamp in RFC 3339 format.
pub fn compiled_at_now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

/// Resolve a source path for diagnostics.
pub fn source_path(_world: &dyn World, id: FileId) -> PathBuf {
    match id.root() {
        VirtualRoot::Project => PathBuf::from(id.vpath().get_without_slash()),
        VirtualRoot::Package(spec) => {
            PathBuf::from(format!("{spec}{}", id.vpath().get_with_slash()))
        }
    }
}
