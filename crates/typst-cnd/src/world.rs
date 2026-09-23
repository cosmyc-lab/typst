//! Minimal system world for the typst-cnd CLI.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Dict, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Features, Library, LibraryExt, World};

use crate::model::SourceInfo;
use typst_kit::datetime::Time;
use typst_kit::downloader::SystemDownloader;
use typst_kit::files::{FileLoader, FileStore, FsRoot};
use typst_kit::fonts::{self, FontStore};
use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};

/// A world for compiling `.typ` sources into CND CNDs.
pub struct CndWorld {
    main: FileId,
    library: LazyHash<Library>,
    fonts: LazyLock<FontStore, Box<dyn Fn() -> FontStore + Send + Sync>>,
    files: FileStore<CndFiles>,
    now: Time,
}

fn build_library(inputs: Dict) -> Library {
    // `Feature::CndSemantics` makes every fully-inline fragment body (a
    // `block[…]`, a grid cell, a `place`, a `box`) realize into a real,
    // locatable `ParElem`. Without it that text is laid out but never
    // located, so `Introspector::query` — the only source the CND emit
    // pipeline reads — cannot see it and the content is silently dropped.
    let features = Features::from_iter([Feature::CndSemantics]);
    // Upstream #8496 made every export format opt-in: a format's bindings
    // (`pdf.embed`, `pdf.header-cell`, …) now exist only if the format is
    // registered here. Before it, `pdf` was defined unconditionally, so
    // omitting it would reject documents the CND exporter used to accept.
    // The other formats were never reachable from this world, so they stay
    // unregistered — CND is this world's only output.
    let mut library = Library::builder([typst_pdf::FORMAT])
        .with_inputs(inputs)
        .with_features(features)
        .build();
    // DEPRECATED — see crate::cnd's module doc. Removal tracked for the next release.
    library.global.scope_mut().define("cnd", crate::cnd::module());
    library
}

impl CndWorld {
    pub fn new(input: &Path) -> Result<Self, FileError> {
        Self::new_with_inputs(input, Dict::default())
    }

    /// Like [`new`](Self::new), but also seeds `sys.inputs` with `inputs`
    /// (the CLI's `--input` and `--inputs-file`, merged before this is
    /// called). Empty by default, so [`new`](Self::new) behaves exactly as
    /// it always has.
    pub fn new_with_inputs(input: &Path, inputs: Dict) -> Result<Self, FileError> {
        let root = input
            .parent()
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(|err| FileError::from_io(err, input))?;

        let input = input.canonicalize().map_err(|err| FileError::from_io(err, input))?;

        let main = RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::virtualize(&root, &input).map_err(|_| FileError::Other(None))?,
        )
        .intern();

        Ok(Self {
            main,
            library: LazyHash::new(build_library(inputs)),
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

/// Describe the input artifact this CND is built from (spec §2.1).
///
/// The hash covers the main source file only, so it is comparable strictly
/// between two runs of *this* producer over the same source — which is
/// exactly the contract the format states for `source.hash`, and why a
/// consumer must never compare it against another producer's.
///
/// `uri` is the main file's project-relative virtual path, never an
/// absolute one: it is a producer-local identifier that is never promised
/// resolvable, and an absolute workstation path would leak the filesystem
/// tree to every downstream reader.
pub fn source_info(world: &dyn World) -> SourceInfo {
    use sha2::{Digest, Sha256};

    let main = world.main();
    let source = world.source(main).expect("main source");
    let digest = Sha256::digest(source.text().as_bytes());
    SourceInfo {
        type_: "typst".into(),
        hash: format!("sha256:{}", hex::encode(digest)),
        uri: Some(main.vpath().get_without_slash().to_string()),
    }
}

/// Current UTC timestamp in RFC 3339 format.
pub fn built_at_now() -> String {
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
