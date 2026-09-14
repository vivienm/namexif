//! Directory-entry identities for symlink dependency checks. Never follow the
//! final symlink: an intermediate link can itself be scheduled for renaming.

use std::{fs, io, path::Path};

#[cfg(unix)]
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    ffi::OsString,
    os::unix::fs::MetadataExt,
};

// One resolver lives only for the dependency check, before any rename occurs.
#[derive(Default)]
pub struct Resolver {
    #[cfg(unix)]
    directories: HashMap<(u64, u64), Directory>,
}

#[cfg(unix)]
struct Directory {
    names: HashSet<OsString>,
    by_inode: HashMap<(u64, u64), Vec<OsString>>,
}

#[cfg(unix)]
impl Directory {
    fn read(path: &Path) -> io::Result<Self> {
        let mut names = HashSet::new();
        let mut by_inode = HashMap::<_, Vec<_>>::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            // DirEntry::metadata does not follow the final symlink.
            let metadata = entry.metadata()?;
            let name = entry.file_name();
            names.insert(name.clone());
            by_inode
                .entry((metadata.dev(), metadata.ino()))
                .or_default()
                .push(name);
        }
        Ok(Self { names, by_inode })
    }
}

#[cfg(unix)]
#[derive(Debug, Eq, Hash, PartialEq)]
pub enum Key {
    Unique(u64, u64),
    HardLink(u64, u64, std::ffi::OsString),
}

#[cfg(unix)]
impl Resolver {
    pub fn keys(&mut self, path: &Path, metadata: &fs::Metadata) -> io::Result<Vec<Key>> {
        if metadata.nlink() == 1 {
            return Ok(vec![Key::Unique(metadata.dev(), metadata.ino())]);
        }

        // An inode alone cannot distinguish separate hard links. Resolve their
        // stored names within the parent, whose identity also handles case aliases.
        let parent = parent(path);
        let parent_metadata = fs::metadata(parent)?;
        let name = path.file_name().ok_or_else(no_filename)?;
        let parent_id = (parent_metadata.dev(), parent_metadata.ino());
        // Key the cache by directory identity so aliases share the same index.
        let directory = match self.directories.entry(parent_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Directory::read(parent)?),
        };
        let key = |name: OsString| Key::HardLink(parent_id.0, parent_id.1, name);
        if directory.names.contains(name) {
            return Ok(vec![key(name.to_os_string())]);
        }
        // If several hard links share an inode and the requested spelling is an
        // alias, portable Unix metadata cannot tell which name was resolved. Keep
        // every candidate so uncertainty cannot allow a link to be broken.
        let candidates = directory
            .by_inode
            .get(&(metadata.dev(), metadata.ino()))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "directory entry disappeared")
            })?;
        Ok(candidates.iter().cloned().map(key).collect())
    }
}

#[cfg(windows)]
pub type Key = std::path::PathBuf;

#[cfg(windows)]
impl Resolver {
    pub fn keys(&mut self, path: &Path, _metadata: &fs::Metadata) -> io::Result<Vec<Key>> {
        use std::{
            ffi::OsString,
            os::windows::ffi::{OsStrExt, OsStringExt},
        };
        use windows_sys::Win32::{
            Foundation::INVALID_HANDLE_VALUE,
            Storage::FileSystem::{FindClose, FindFirstFileW, WIN32_FIND_DATAW},
        };

        let parent = fs::canonicalize(parent(path))?;
        let name = path.file_name().ok_or_else(no_filename)?;
        // FindFirstFileW is a pattern API; never accept '*', '?' or NUL in a name.
        if name.encode_wide().any(|c| matches!(c, 0 | 42 | 63)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid directory entry",
            ));
        }
        let path = parent.join(name);
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide.push(0);
        let mut data = WIN32_FIND_DATAW::default();
        // SAFETY: wide is NUL-terminated, data is writable, and a successful search
        // fills it. Close the search handle before reading the returned name.
        unsafe {
            let handle = FindFirstFileW(wide.as_ptr(), &mut data);
            if handle == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            FindClose(handle);
        }
        let len = data
            .cFileName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(data.cFileName.len());
        // FindFirstFileW reports the link's own stored name, including for symlinks:
        // https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfilew
        Ok(vec![
            parent.join(OsString::from_wide(&data.cFileName[..len])),
        ])
    }
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn no_filename() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "path has no filename")
}
