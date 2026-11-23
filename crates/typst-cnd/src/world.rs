//! Minimal system world for the typst-cnd CLI.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{
    Bytes, Datetime, Dict, Duration, Repr, SilentBindingGuard, Value,
};
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

/// Builds the standard library a CND compilation needs: `inputs` as
/// `sys.inputs`, `features` plus [`Feature::CndSemantics`], and the `pdf`
/// format's bindings. It defines no global of its own: a document tags its
/// content through the authoring package (`cnd.typ`) it imports.
pub fn cnd_library(inputs: Dict, features: impl IntoIterator<Item = Feature>) -> Library {
    // `Feature::CndSemantics` makes every fully-inline fragment body (a
    // `block[…]`, a grid cell, a `place`, a `box`) realize into a real,
    // locatable `ParElem`. Without it that text is laid out but never
    // located, so `Introspector::query` — the only source the CND emit
    // pipeline reads — cannot see it and the content is silently dropped.
    let features: Features =
        features.into_iter().chain([Feature::CndSemantics]).collect();
    // Upstream #8496 made every export format opt-in: a format's bindings
    // (`pdf.embed`, `pdf.header-cell`, …) now exist only if the format is
    // registered here. Before it, `pdf` was defined unconditionally, so
    // omitting it would reject documents the CND exporter used to accept.
    // The other formats were never reachable from this world, so they stay
    // unregistered — CND is this world's only output.
    Library::builder([typst_pdf::FORMAT])
        .with_inputs(inputs)
        .with_features(features)
        .build()
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
        Self::new_with_options(input, inputs, true)
    }

    /// Like [`new_with_inputs`](Self::new_with_inputs), but `system_fonts:
    /// false` restricts the font set to the embedded fonts, for reproducible
    /// comparisons across machines.
    pub fn new_with_options(
        input: &Path,
        inputs: Dict,
        system_fonts: bool,
    ) -> Result<Self, FileError> {
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
            library: LazyHash::new(cnd_library(inputs, [])),
            fonts: LazyLock::new(Box::new(move || {
                let mut store = FontStore::new();
                store.extend(fonts::embedded());
                if system_fonts {
                    store.extend(fonts::system());
                }
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

    /// Every file the compilations since the last [`reset`](Self::reset)
    /// read, sources and binary files alike, in no particular order. This is
    /// what [`source_info`] hashes.
    pub fn input_files(&mut self) -> Vec<FileId> {
        let (_, files) = self.files.dependencies();
        files.collect()
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
/// `read` is every file the compilation read, as the world recorded it
/// (sources and binary files alike: includes, imports, images,
/// bibliographies, data files, package files). [`CndWorld::input_files`]
/// gives it for this crate's world; another world passes the files its own
/// store served. The main file is always covered, listed or not.
///
/// # The hash
///
/// `hash` is `sha256:` followed by the hex SHA-256 of a sequence of fields,
/// each written as its byte length (a little-endian `u64`) and then its
/// bytes:
///
/// 1. the tag `typst-cnd-source-hash/1`;
/// 2. the main file's key;
/// 3. for each file, in ascending byte order of its key: the key, a status
///    of one byte, and, if the status is `0` (read), the file's bytes.
///    Status `1` marks a file the compilation asked for but could not read:
///    its absence is part of the input too;
/// 4. only if `sys.inputs` is not empty: the section name `sys.inputs`
///    (which cannot be a file key: those start with `/` or `@`), then for
///    each input, in ascending byte order of its key: the key, the value's
///    type name, and the value — a string's UTF-8 bytes, any other value's
///    `repr`. The command line only ever gives strings (type `str`).
///
/// Without inputs nothing is added, so a document compiled without inputs
/// hashes exactly as it did before inputs were covered. `sys.inputs` is
/// read back from the world's library, so it is what the document saw,
/// however the inputs were given (`--input`, `--inputs-file`, or both, in
/// any order).
///
/// A file's key is its path within its root:
/// `/dir/file.typ` for a project file, `@namespace/name:version/dir/file.typ`
/// for a package file. The two cannot collide (a project key starts with
/// `/`, a package key with `@`), and neither depends on where the project or
/// the package cache lives on disk, so the same inputs hash the same on any
/// machine. Package files are hashed by their bytes, not just their spec:
/// a local package can be edited in place under the same version.
///
/// Fonts are not part of the hash. They are served by the font store, never
/// read as files, and are the environment a document is laid out in rather
/// than an input of it, like the compiler version. The current date is not
/// covered either.
///
/// The hash is comparable strictly between two runs of *this* producer over
/// the same source — exactly the contract the format states for
/// `source.hash`, and why a consumer must never compare it against another
/// producer's.
///
/// `uri` is the main file's project-relative virtual path, never an
/// absolute one: it is a producer-local identifier that is never promised
/// resolvable, and an absolute workstation path would leak the filesystem
/// tree to every downstream reader.
pub fn source_info(
    world: &dyn World,
    read: impl IntoIterator<Item = FileId>,
) -> SourceInfo {
    use sha2::{Digest, Sha256};

    let main = world.main();
    // Keyed by path, so the order is the same on every run and every
    // machine (a `FileId`'s order depends on interning order).
    let files: BTreeMap<String, FileId> =
        read.into_iter().chain([main]).map(|id| (file_key(id), id)).collect();

    let mut hasher = Sha256::new();
    let mut field = |bytes: &[u8]| {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    };
    field(b"typst-cnd-source-hash/1");
    field(file_key(main).as_bytes());
    for (key, id) in &files {
        field(key.as_bytes());
        match world.file(*id) {
            Ok(bytes) => {
                field(&[0]);
                field(&bytes);
            }
            Err(_) => field(&[1]),
        }
    }

    let inputs = sys_inputs(world);
    if !inputs.is_empty() {
        field(b"sys.inputs");
        let sorted: BTreeMap<&str, &Value> =
            inputs.iter().map(|(key, value)| (key.as_str(), value)).collect();
        for (key, value) in sorted {
            field(key.as_bytes());
            field(value.ty().short_name().as_bytes());
            match value {
                Value::Str(text) => field(text.as_bytes()),
                other => field(other.repr().as_bytes()),
            }
        }
    }

    SourceInfo {
        type_: "typst".into(),
        hash: format!("sha256:{}", hex::encode(hasher.finalize())),
        uri: Some(main.vpath().get_without_slash().to_string()),
    }
}

/// The `sys.inputs` the world's library defines: what the document sees.
/// Empty if the library has none.
fn sys_inputs(world: &dyn World) -> Dict {
    let library = world.library();
    let guard = SilentBindingGuard::new(library.features.clone());
    let Some(Ok(Value::Module(sys))) =
        library.global.scope().get("sys").map(|binding| binding.read(&guard))
    else {
        return Dict::new();
    };
    match sys.scope().get("inputs").map(|binding| binding.read(&guard)) {
        Some(Ok(Value::Dict(inputs))) => inputs.clone(),
        _ => Dict::new(),
    }
}

/// A file's key in the source hash: its path within its root, prefixed by
/// the package spec for a package file. See [`source_info`].
fn file_key(id: FileId) -> String {
    match id.root() {
        VirtualRoot::Project => id.vpath().get_with_slash().to_string(),
        VirtualRoot::Package(spec) => format!("{spec}{}", id.vpath().get_with_slash()),
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
