use std::{
    cmp::Ordering,
    fmt, fs,
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

use serde::Serialize;

const DEFAULT_PREVIEW_BYTES: usize = 256 * 1024;
const MAX_PREVIEW_BYTES: usize = 1024 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 2_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileEntry {
    pub name: String,
    pub relative_path: String,
    pub kind: ProjectEntryKind,
    pub size_bytes: Option<u64>,
    pub modified_at_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDirectory {
    pub relative_path: String,
    pub entries: Vec<ProjectFileEntry>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFilePreview {
    pub relative_path: String,
    pub content: Option<String>,
    pub size_bytes: u64,
    pub truncated: bool,
    pub binary: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectFileError {
    MissingRoot,
    InvalidRelativePath,
    OutsideProjectRoot,
    NotDirectory,
    NotFile,
    Io(String),
}

impl fmt::Display for ProjectFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRoot => formatter.write_str("the project root does not exist"),
            Self::InvalidRelativePath => {
                formatter.write_str("the requested path is not a safe relative path")
            }
            Self::OutsideProjectRoot => {
                formatter.write_str("the requested path resolves outside the project root")
            }
            Self::NotDirectory => formatter.write_str("the requested path is not a directory"),
            Self::NotFile => formatter.write_str("the requested path is not a regular file"),
            Self::Io(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ProjectFileError {}

pub fn list_project_directory(
    root: impl AsRef<Path>,
    relative_path: Option<&str>,
) -> Result<ProjectDirectory, ProjectFileError> {
    let root = canonical_root(root.as_ref())?;
    let relative_path = validate_relative(relative_path.unwrap_or_default())?;
    let directory = resolve_inside(&root, &relative_path)?;
    if !directory.is_dir() {
        return Err(ProjectFileError::NotDirectory);
    }

    let mut entries = fs::read_dir(&directory)
        .map_err(io_error)?
        .take(MAX_DIRECTORY_ENTRIES + 1)
        .map(|entry| {
            entry
                .map_err(io_error)
                .and_then(|entry| map_entry(&root, entry))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let truncated = entries.len() > MAX_DIRECTORY_ENTRIES;
    entries.truncate(MAX_DIRECTORY_ENTRIES);
    entries.sort_by(|left, right| match (left.kind, right.kind) {
        (ProjectEntryKind::Directory, ProjectEntryKind::Directory)
        | (ProjectEntryKind::File, ProjectEntryKind::File)
        | (ProjectEntryKind::Symlink, ProjectEntryKind::Symlink)
        | (ProjectEntryKind::Other, ProjectEntryKind::Other) => {
            left.name.to_lowercase().cmp(&right.name.to_lowercase())
        }
        (ProjectEntryKind::Directory, _) => Ordering::Less,
        (_, ProjectEntryKind::Directory) => Ordering::Greater,
        (ProjectEntryKind::File, _) => Ordering::Less,
        (_, ProjectEntryKind::File) => Ordering::Greater,
        (ProjectEntryKind::Symlink, _) => Ordering::Less,
        (_, ProjectEntryKind::Symlink) => Ordering::Greater,
    });

    Ok(ProjectDirectory {
        relative_path: portable_relative(&relative_path),
        entries,
        truncated,
    })
}

pub fn preview_project_file(
    root: impl AsRef<Path>,
    relative_path: &str,
    requested_max_bytes: Option<usize>,
) -> Result<ProjectFilePreview, ProjectFileError> {
    let root = canonical_root(root.as_ref())?;
    let relative_path = validate_relative(relative_path)?;
    let file = open_file_inside(&root, &relative_path)?;
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() {
        return Err(ProjectFileError::NotFile);
    }
    let max_bytes = requested_max_bytes
        .unwrap_or(DEFAULT_PREVIEW_BYTES)
        .clamp(1, MAX_PREVIEW_BYTES);
    let file_size_for_capacity = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    let mut bytes = Vec::with_capacity(max_bytes.min(file_size_for_capacity));
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    let truncated = bytes.len() > max_bytes;
    bytes.truncate(max_bytes);

    let content = if bytes.contains(&0) {
        None
    } else {
        String::from_utf8(bytes).ok()
    };
    let binary = content.is_none();

    Ok(ProjectFilePreview {
        relative_path: portable_relative(&relative_path),
        content,
        size_bytes: metadata.len(),
        truncated,
        binary,
    })
}

fn canonical_root(root: &Path) -> Result<PathBuf, ProjectFileError> {
    root.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ProjectFileError::MissingRoot
        } else {
            io_error(error)
        }
    })
}

fn validate_relative(relative_path: &str) -> Result<PathBuf, ProjectFileError> {
    let path = Path::new(relative_path);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ProjectFileError::InvalidRelativePath);
    }
    Ok(path.to_path_buf())
}

fn resolve_inside(root: &Path, relative_path: &Path) -> Result<PathBuf, ProjectFileError> {
    let resolved = root.join(relative_path).canonicalize().map_err(io_error)?;
    if !resolved.starts_with(root) {
        return Err(ProjectFileError::OutsideProjectRoot);
    }
    Ok(resolved)
}

#[cfg(unix)]
fn open_file_inside(root: &Path, relative_path: &Path) -> Result<File, ProjectFileError> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
    };

    fn open_at(
        directory: &File,
        name: &std::ffi::OsStr,
        directory_only: bool,
    ) -> std::io::Result<File> {
        let name = CString::new(name.as_bytes()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "project paths cannot contain NUL bytes",
            )
        })?;
        let mut flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
        if directory_only {
            flags |= libc::O_DIRECTORY;
        }
        // SAFETY: `directory` is a live file descriptor, `name` is a
        // NUL-terminated component, and ownership of a successful descriptor
        // is transferred immediately into `File`.
        let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if descriptor < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            // SAFETY: `openat` returned a new owned descriptor.
            Ok(unsafe { File::from_raw_fd(descriptor) })
        }
    }

    let root = File::open(root).map_err(io_error)?;
    let components = relative_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            Component::CurDir => None,
            _ => None,
        })
        .collect::<Vec<_>>();
    let Some((file_name, directories)) = components.split_last() else {
        return Err(ProjectFileError::NotFile);
    };
    let mut directory = root;
    for component in directories {
        directory = open_at(&directory, component, true).map_err(open_error)?;
    }
    open_at(&directory, file_name, false).map_err(open_error)
}

#[cfg(not(unix))]
fn open_file_inside(root: &Path, relative_path: &Path) -> Result<File, ProjectFileError> {
    let path = resolve_inside(root, relative_path)?;
    File::open(path).map_err(io_error)
}

#[cfg(unix)]
fn open_error(error: std::io::Error) -> ProjectFileError {
    if error.raw_os_error() == Some(libc::ELOOP) {
        ProjectFileError::OutsideProjectRoot
    } else {
        io_error(error)
    }
}

fn map_entry(root: &Path, entry: fs::DirEntry) -> Result<ProjectFileEntry, ProjectFileError> {
    let file_type = entry.file_type().map_err(io_error)?;
    let kind = if file_type.is_symlink() {
        ProjectEntryKind::Symlink
    } else if file_type.is_dir() {
        ProjectEntryKind::Directory
    } else if file_type.is_file() {
        ProjectEntryKind::File
    } else {
        ProjectEntryKind::Other
    };
    let metadata = entry.metadata().ok();
    let relative_path = entry
        .path()
        .strip_prefix(root)
        .map_err(|_| ProjectFileError::OutsideProjectRoot)?
        .to_path_buf();
    Ok(ProjectFileEntry {
        name: entry.file_name().to_string_lossy().into_owned(),
        relative_path: portable_relative(&relative_path),
        kind,
        size_bytes: metadata
            .as_ref()
            .filter(|_| kind == ProjectEntryKind::File)
            .map(fs::Metadata::len),
        modified_at_unix_ms: metadata
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64),
    })
}

fn portable_relative(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy()),
            Component::CurDir => None,
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn io_error(error: std::io::Error) -> ProjectFileError {
    ProjectFileError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("utu-project-files-{}-{nonce}", std::process::id()));
            fs::create_dir_all(path.join("src")).expect("fixture directories");
            fs::write(path.join("README.md"), "hello from Utu").expect("text fixture");
            fs::write(path.join("src/lib.rs"), "pub fn run() {}\n").expect("source fixture");
            fs::write(path.join("blob.bin"), [0, 159, 146, 150]).expect("binary fixture");
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn directories_are_sorted_before_files() {
        let fixture = Fixture::new();
        let listing = list_project_directory(&fixture.0, None).expect("listing");
        assert_eq!(listing.entries[0].name, "src");
        assert_eq!(listing.entries[0].kind, ProjectEntryKind::Directory);
        assert!(!listing.truncated);
    }

    #[test]
    fn text_preview_is_bounded_and_binary_is_not_decoded() {
        let fixture = Fixture::new();
        let text = preview_project_file(&fixture.0, "README.md", Some(5)).expect("preview");
        assert_eq!(text.content.as_deref(), Some("hello"));
        assert!(text.truncated);
        assert!(!text.binary);

        let binary = preview_project_file(&fixture.0, "blob.bin", None).expect("binary");
        assert!(binary.content.is_none());
        assert!(binary.binary);
    }

    #[test]
    fn parent_traversal_is_rejected() {
        let fixture = Fixture::new();
        assert_eq!(
            preview_project_file(&fixture.0, "../secret", None),
            Err(ProjectFileError::InvalidRelativePath)
        );
    }

    #[test]
    fn absolute_paths_are_rejected() {
        let fixture = Fixture::new();
        assert_eq!(
            preview_project_file(&fixture.0, "/etc/passwd", None),
            Err(ProjectFileError::InvalidRelativePath)
        );
    }

    #[cfg(unix)]
    #[test]
    fn preview_refuses_a_symlink_even_when_it_points_inside_the_project() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        symlink(fixture.0.join("README.md"), fixture.0.join("readme-link")).expect("symlink");
        assert_eq!(
            preview_project_file(&fixture.0, "readme-link", None),
            Err(ProjectFileError::OutsideProjectRoot)
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        symlink(std::env::temp_dir(), fixture.0.join("escape")).expect("symlink");
        assert_eq!(
            list_project_directory(&fixture.0, Some("escape")),
            Err(ProjectFileError::OutsideProjectRoot)
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_files_outside_the_root_are_rejected() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        let outside = std::env::temp_dir().join(format!(
            "utu-outside-file-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::write(&outside, "secret").expect("outside fixture");
        symlink(&outside, fixture.0.join("outside.txt")).expect("symlink");
        assert_eq!(
            preview_project_file(&fixture.0, "outside.txt", None),
            Err(ProjectFileError::OutsideProjectRoot)
        );
        let _ = fs::remove_file(outside);
    }
}
