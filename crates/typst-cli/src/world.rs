use std::any::Any;
use std::collections::BTreeSet;
use std::error;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use ecow::{EcoString, eco_format};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration, Repr};
use typst::syntax::{
    FileId, PathError, RootedPath, Source, VirtualPath, VirtualRoot, VirtualizeError,
};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::diagnostics::DiagnosticWorld;
use typst_kit::files::{FileLoader, FileStore, FsRoot};
use typst_kit::fonts::{FontPath, FontStore};
use typst_kit::packages::SystemPackages;

use crate::args::{Feature, Input, OutputFormat, ProcessArgs, WorldArgs};

/// A world that provides access to the operating system.
pub struct SystemWorld {
    /// The working directory.
    workdir: Option<PathBuf>,
    /// Typst's standard library.
    library: LazyHash<Library>,
    /// Metadata about discovered fonts and lazily loaded fonts.
    fonts: LazyLock<FontStore, Box<dyn Fn() -> FontStore + Send + Sync>>,
    /// Maps file ids to source files and buffers.
    files: FileStore<SystemFiles>,
    /// The current datetime if requested. This is stored here to ensure it is
    /// always the same within one compilation.
    /// Reset between compilations if not [`Time::Fixed`].
    now: Time,
    /// Indices of the fonts loaded since the last reset, for `--deps`.
    accessed_fonts: Mutex<BTreeSet<usize>>,
}

impl SystemWorld {
    /// Creates a new system world with the default (non-CND) standard
    /// library, as used by every format but CND and by `query` and `eval`.
    pub fn new(
        input: Option<&Input>,
        world_args: &'static WorldArgs,
        process_args: &ProcessArgs,
    ) -> Result<Self, WorldCreationError> {
        Self::new_with_format(input, world_args, process_args, None)
    }

    /// Creates a new system world whose standard library suits `format`.
    pub fn new_with_format(
        input: Option<&Input>,
        world_args: &'static WorldArgs,
        process_args: &ProcessArgs,
        format: Option<OutputFormat>,
    ) -> Result<Self, WorldCreationError> {
        // Set up the thread pool.
        if let Some(jobs) = process_args.jobs {
            rayon::ThreadPoolBuilder::new()
                .num_threads(jobs)
                .use_current_thread()
                .build_global()
                .ok();
        }

        let library = {
            let inputs = crate::inputs::sys_inputs(world_args)
                .map_err(WorldCreationError::Inputs)?;

            let features: Vec<typst::Feature> =
                process_args.features.iter().copied().map(Into::into).collect();

            if format == Some(OutputFormat::Cnd) {
                typst_cnd::world::cnd_library(inputs, features)
            } else {
                Library::builder([
                    typst_html::FORMAT,
                    typst_pdf::FORMAT,
                    typst_svg::FORMAT,
                    typst_render::FORMAT,
                    typst_bundle::FORMAT,
                ])
                .with_inputs(inputs)
                .with_features(features.into_iter().collect())
                .build()
            }
        };

        let now = match world_args.creation_timestamp {
            Some(time) => Time::fixed_timestamp(time)
                .map_err(|_| WorldCreationError::InvalidTimestamp)?,
            None => Time::system(),
        };

        Ok(Self {
            workdir: std::env::current_dir().ok(),
            library: LazyHash::new(library),
            fonts: LazyLock::new(Box::new(|| {
                crate::fonts::discover_fonts(&world_args.font)
            })),
            files: FileStore::new(SystemFiles::new(input, world_args)?),
            now,
            accessed_fonts: Mutex::new(BTreeSet::new()),
        })
    }

    /// The project root relative to which absolute paths are resolved.
    pub fn root(&self) -> &Path {
        self.files.loader().project.path()
    }

    /// The current working directory.
    pub fn workdir(&self) -> &Path {
        self.workdir.as_deref().unwrap_or(Path::new("."))
    }

    /// Return all paths the last compilation depended on.
    pub fn dependencies(&mut self) -> impl Iterator<Item = PathBuf> + '_ {
        let (loader, deps) = self.files.dependencies();
        deps.filter_map(|id| loader.resolve(id).ok())
    }

    /// Reset the compilation state in preparation of a new compilation.
    pub fn reset(&mut self) {
        self.files.reset();
        self.now.reset();
        self.accessed_fonts.get_mut().unwrap().clear();
    }

    /// Forcibly scan fonts instead of doing it lazily upon the first access.
    ///
    /// Does nothing if the fonts were already scanned.
    pub fn scan_fonts(&mut self) {
        LazyLock::force(&self.fonts);
    }

    /// Font files loaded by the last compilation, canonicalized (when
    /// possible), sorted and deduplicated.
    /// Fonts without a file (embedded ones) are left out. This is every font
    /// the layout loaded, which can include fonts tried for glyph fallback.
    pub fn font_dependencies(&self) -> Vec<PathBuf> {
        let accessed = self.accessed_fonts.lock().unwrap();
        let paths: BTreeSet<PathBuf> = accessed
            .iter()
            .filter_map(|&index| self.fonts.source(index))
            .filter_map(|source| (source as &dyn Any).downcast_ref::<FontPath>())
            // Canonical, like file dependencies, so two spellings of one
            // file (a symlink, a relative path) are reported once.
            .map(|font| font.path.canonicalize().unwrap_or_else(|_| font.path.clone()))
            .collect();
        paths.into_iter().collect()
    }
}

impl World for SystemWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.files.loader().main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.accessed_fonts.lock().unwrap().insert(index);
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.now.today(offset)
    }
}

impl DiagnosticWorld for SystemWorld {
    fn name(&self, id: FileId) -> String {
        let vpath = id.vpath();
        match id.root() {
            VirtualRoot::Project => {
                // Try to express the path relative to the working directory.
                vpath
                    .realize(self.root())
                    .ok()
                    .and_then(|rooted| pathdiff::diff_paths(rooted, self.workdir()))
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_else(|| vpath.get_without_slash().into())
            }
            VirtualRoot::Package(package) => {
                format!("{package}{}", vpath.get_with_slash())
            }
        }
    }
}

/// Static `FileId` allocated for stdin. This is to ensure that stdin can live
/// in the project root without colliding with any real on-disk file.
static STDIN_ID: LazyLock<FileId> = LazyLock::new(|| {
    FileId::unique(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("<stdin>").unwrap(),
    ))
});

/// Static `FileId` allocated for empty/no input at all. This is to ensure that
/// we can create a [`SystemWorld`] based on no main file or stdin at all.
static EMPTY_ID: LazyLock<FileId> = LazyLock::new(|| {
    FileId::unique(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("<empty>").unwrap(),
    ))
});

/// Provides project files from a configured directory and package files from
/// standard locations.
struct SystemFiles {
    main: FileId,
    project: FsRoot,
    /// The canonical directory where missing project files are looked up by
    /// file name.
    fallback: Option<PathBuf>,
    packages: SystemPackages,
}

impl SystemFiles {
    /// Creates a new loader given the configuration.
    pub fn new(
        input: Option<&Input>,
        world_args: &'static WorldArgs,
    ) -> Result<Self, WorldCreationError> {
        // Resolve the system-global input path.
        let input_path = match input {
            Some(Input::Path(path)) => {
                Some(path.canonicalize().map_err(|err| match err.kind() {
                    io::ErrorKind::NotFound => {
                        WorldCreationError::InputNotFound(path.clone())
                    }
                    _ => WorldCreationError::Io(err),
                })?)
            }
            _ => None,
        };

        // Resolve the system-global root directory.
        let root = {
            let path = world_args
                .root
                .as_deref()
                .or_else(|| input_path.as_deref()?.parent())
                .unwrap_or(Path::new("."));
            path.canonicalize().map_err(|err| match err.kind() {
                io::ErrorKind::NotFound => {
                    WorldCreationError::RootNotFound(path.to_path_buf())
                }
                _ => WorldCreationError::Io(err),
            })?
        };

        let main = if let Some(path) = &input_path {
            // Resolve the virtual path of the main file within the project root.
            RootedPath::new(VirtualRoot::Project, VirtualPath::virtualize(&root, path)?)
                .intern()
        } else if matches!(input, Some(Input::Stdin)) {
            // Return the special id of STDIN.
            *STDIN_ID
        } else {
            // Return the special id of EMPTY/no input at all otherwise.
            *EMPTY_ID
        };

        let fallback =
            world_args.fallback_dir.as_deref().map(fallback_dir).transpose()?;

        Ok(Self {
            main,
            project: FsRoot::new(root),
            fallback,
            packages: crate::packages::system(&world_args.package),
        })
    }

    /// Resolves the file system path for the given `id`, including a file
    /// served from the fallback directory.
    pub fn resolve(&self, id: FileId) -> FileResult<PathBuf> {
        let path = self.root(id)?.resolve(id.vpath())?;
        Ok(self.fallback_file(id, &path).unwrap_or(path))
    }

    /// The fallback file that serves `id`, whose project path is `path`.
    ///
    /// Only a project file that does not exist falls back: a file that exists
    /// wins, a directory stays an error, and an escaping path already failed
    /// when its id was created. The file is looked up by its name alone and
    /// is served only if it is a regular file whose real path lies inside the
    /// fallback directory, so a symlink cannot lead out of it.
    fn fallback_file(&self, id: FileId, path: &Path) -> Option<PathBuf> {
        let dir = self.fallback.as_deref()?;
        if !matches!(id.root(), VirtualRoot::Project)
            || id == *EMPTY_ID
            || id == *STDIN_ID
        {
            return None;
        }
        // Any outcome but "not found" (an existing file or directory, a
        // permission error, ...) keeps the project's own result.
        match fs::metadata(path) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            _ => return None,
        }
        let name = id.vpath().file_name()?;
        let mut components = Path::new(name).components();
        if !matches!(
            (components.next(), components.next()),
            (Some(Component::Normal(_)), None)
        ) {
            return None;
        }
        let candidate = dir.join(name);
        if candidate.parent() != Some(dir) {
            return None;
        }
        let real = candidate.canonicalize().ok()?;
        (real.starts_with(dir) && real.is_file()).then_some(real)
    }

    /// Resolves the root in which the given file ID resides.
    fn root(&self, id: FileId) -> FileResult<FsRoot> {
        Ok(match id.root() {
            VirtualRoot::Project => self.project.clone(),
            VirtualRoot::Package(spec) => self.packages.obtain(spec)?,
        })
    }
}

impl FileLoader for SystemFiles {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        if id == *EMPTY_ID {
            Ok(Bytes::new([]))
        } else if id == *STDIN_ID {
            read_from_stdin().map(Bytes::new)
        } else {
            let root = self.root(id)?;
            let path = root.resolve(id.vpath())?;
            if let Some(real) = self.fallback_file(id, &path) {
                return fs::read(&real)
                    .map(Bytes::new)
                    .map_err(|err| FileError::from_io(err, &real));
            }
            root.load(id.vpath())
        }
    }
}

/// Canonicalizes the `--fallback-dir` and checks that it is a directory.
fn fallback_dir(dir: &Path) -> Result<PathBuf, WorldCreationError> {
    let canonical = dir.canonicalize().map_err(|err| match err.kind() {
        io::ErrorKind::NotFound => {
            WorldCreationError::FallbackDirNotFound(dir.to_path_buf())
        }
        _ => WorldCreationError::Io(err),
    })?;
    if !canonical.is_dir() {
        return Err(WorldCreationError::FallbackDirNotADirectory(dir.to_path_buf()));
    }
    Ok(canonical)
}

/// Read from stdin.
fn read_from_stdin() -> FileResult<Vec<u8>> {
    let mut buf = Vec::new();
    let result = io::stdin().read_to_end(&mut buf);
    match result {
        Ok(_) => (),
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => (),
        Err(err) => return Err(FileError::from_io(err, Path::new("<stdin>"))),
    }
    Ok(buf)
}

/// An error that occurs during world construction.
#[derive(Debug)]
pub enum WorldCreationError {
    /// The input file does not appear to exist.
    InputNotFound(PathBuf),
    /// The input file path was malformed.
    InputMalformed(VirtualizeError),
    /// The root directory does not appear to exist.
    RootNotFound(PathBuf),
    /// The fallback directory does not appear to exist.
    FallbackDirNotFound(PathBuf),
    /// The fallback directory is not a directory.
    FallbackDirNotADirectory(PathBuf),
    /// The requested creation timestamp was invalid.
    InvalidTimestamp,
    /// The `--inputs-file` could not be used.
    Inputs(String),
    /// Another type of I/O error.
    Io(io::Error),
}

impl fmt::Display for WorldCreationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldCreationError::InputMalformed(err) => match err {
                VirtualizeError::Path(PathError::Escapes) => {
                    write!(f, "source file must be contained in project root")
                }
                VirtualizeError::Path(PathError::Backslash) => {
                    write!(f, "source path must not contain a backslash")
                }
                VirtualizeError::Invalid(s) => {
                    write!(f, "source path contains invalid sequence `{}`", s.repr())
                }
                VirtualizeError::Utf8 => write!(f, "source path must be valid UTF-8"),
            },
            WorldCreationError::InputNotFound(path) => {
                write!(f, "input file not found (searched at {})", path.display())
            }
            WorldCreationError::RootNotFound(path) => {
                write!(f, "root directory not found (searched at {})", path.display())
            }
            WorldCreationError::FallbackDirNotFound(path) => {
                write!(f, "fallback directory not found (searched at {})", path.display())
            }
            WorldCreationError::FallbackDirNotADirectory(path) => {
                write!(f, "fallback directory is not a directory ({})", path.display())
            }
            WorldCreationError::InvalidTimestamp => {
                write!(f, "creation timestamp out of range")
            }
            WorldCreationError::Inputs(message) => write!(f, "{message}"),
            WorldCreationError::Io(err) => write!(f, "{err}"),
        }
    }
}

impl error::Error for WorldCreationError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<VirtualizeError> for WorldCreationError {
    fn from(err: VirtualizeError) -> Self {
        Self::InputMalformed(err)
    }
}

impl From<WorldCreationError> for EcoString {
    fn from(err: WorldCreationError) -> Self {
        eco_format!("{err}")
    }
}

impl From<Feature> for typst::Feature {
    fn from(feature: Feature) -> Self {
        match feature {
            Feature::Html => typst::Feature::Html,
            Feature::Bundle => typst::Feature::Bundle,
            Feature::A11yExtras => typst::Feature::A11yExtras,
        }
    }
}

#[cfg(test)]
mod tests {
    use typst::syntax::package::PackageSpec;
    use typst_kit::downloader::SystemDownloader;
    use typst_kit::packages::{FsPackages, UniversePackages};

    use super::*;

    /// A loader with the project in `project`, `fallback`, and local packages
    /// in `packages`.
    fn files(project: &Path, fallback: &Path, packages: &Path) -> SystemFiles {
        SystemFiles {
            main: *EMPTY_ID,
            project: FsRoot::new(project.canonicalize().unwrap()),
            fallback: Some(fallback.canonicalize().unwrap()),
            packages: SystemPackages::from_parts(
                Some(FsPackages::new(packages.to_path_buf())),
                None,
                // Not `crate::download::downloader()`: it parses the process
                // arguments, which exits a test binary.
                UniversePackages::new(SystemDownloader::new("test")),
            ),
        }
    }

    fn id(root: VirtualRoot, path: &str) -> FileId {
        RootedPath::new(root, VirtualPath::new(path).unwrap()).intern()
    }

    #[test]
    fn project_files_fall_back_and_resolve_to_the_fallback_file() {
        let fallback = tempfile::tempdir().unwrap();
        fs::write(fallback.path().join("lib.typ"), "x").unwrap();
        let project = tempfile::tempdir().unwrap();
        let files = files(project.path(), fallback.path(), fallback.path());
        let id = id(VirtualRoot::Project, "nested/lib.typ");
        assert_eq!(files.load(id).unwrap().as_slice(), b"x");
        assert_eq!(
            files.resolve(id).unwrap(),
            fallback.path().join("lib.typ").canonicalize().unwrap()
        );
    }

    #[test]
    fn packages_never_fall_back() {
        let fallback = tempfile::tempdir().unwrap();
        fs::write(fallback.path().join("lib.typ"), "x").unwrap();
        let packages = tempfile::tempdir().unwrap();
        // The package exists, but the requested file does not.
        fs::create_dir_all(packages.path().join("local/pkg/0.1.0")).unwrap();
        let project = tempfile::tempdir().unwrap();
        let files = files(project.path(), fallback.path(), packages.path());
        let spec: PackageSpec = "@local/pkg:0.1.0".parse().unwrap();
        let id = id(VirtualRoot::Package(spec), "lib.typ");
        assert!(matches!(files.load(id), Err(FileError::NotFound(_))));
        assert!(!files.resolve(id).unwrap().starts_with(fallback.path()));
    }

    #[test]
    fn special_ids_never_fall_back() {
        let fallback = tempfile::tempdir().unwrap();
        fs::write(fallback.path().join("<stdin>"), "x").unwrap();
        fs::write(fallback.path().join("<empty>"), "x").unwrap();
        let project = tempfile::tempdir().unwrap();
        let files = files(project.path(), fallback.path(), fallback.path());
        assert!(files.load(*EMPTY_ID).unwrap().is_empty());
        let fallback = fallback.path().canonicalize().unwrap();
        assert!(!files.resolve(*EMPTY_ID).unwrap().starts_with(&fallback));
        assert!(!files.resolve(*STDIN_ID).unwrap().starts_with(&fallback));
    }
}
