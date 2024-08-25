//
//  dir     WJ124
//  entry.rs
//

use chrono::{DateTime, Local, TimeZone};
use std::ffi::OsString;
use std::fs;
use std::fs::{DirEntry, Metadata};
use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

pub const S_IFMT: u32 = 0o170000;
pub const S_IFSOCK: u32 = 0o140000;
pub const S_IFLNK: u32 = 0o120000;
pub const S_IFREG: u32 = 0o100000;
pub const S_IFBLK: u32 = 0o060000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFCHR: u32 = 0o020000;
pub const S_IFIFO: u32 = 0o010000;
pub const S_ISUID: u32 = 0o4000;
pub const S_ISGID: u32 = 0o2000;
pub const S_ISVTX: u32 = 0o1000;

#[derive(Debug)]
pub struct Entry {
    pub name: OsString,
    pub metadata: Metadata,
    pub link_dest: Option<PathBuf>,
}

impl Entry {
    pub fn from_dir_entry(d: &DirEntry) -> Result<Entry, io::Error> {
        let path = d.path();
        let some_filename = path.file_name();
        if some_filename.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid filename",
            ));
        }
        let filename = some_filename.unwrap().to_os_string();

        let metadata = d.metadata()?;
        let link_dest = if metadata.is_symlink() {
            Some(fs::read_link(path)?)
        } else {
            None
        };

        Ok(Entry {
            name: filename,
            metadata,
            link_dest,
        })
    }

    pub fn from_path(path: &Path) -> Result<Entry, io::Error> {
        let some_filename = path.file_name();
        if some_filename.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid filename",
            ));
        }
        let filename = some_filename.unwrap().to_os_string();
        let metadata = fs::metadata(path)?;
        let link_dest = if metadata.is_symlink() {
            Some(fs::read_link(path)?)
        } else {
            None
        };

        Ok(Entry {
            name: filename,
            metadata,
            link_dest,
        })
    }

    pub fn mtime(&self) -> DateTime<Local> {
        if let Ok(t) = self.metadata.modified() {
            t.into()
        } else {
            Local.timestamp_opt(0, 0).unwrap()
        }
    }

    #[cfg(unix)]
    pub fn is_hidden(&self) -> bool {
        // sucks that we have to convert this entire thing just to look at one first character
        let s = self.name.to_string_lossy();
        let first = s
            .chars()
            .next()
            .expect("panic: this should not have happened");
        first == '.'
    }

    #[cfg(windows)]
    pub fn is_hidden(&self) -> bool {
        use std::os::windows::fs::MetadataExt;
        let attribs = self.metadata.file_attributes();

        const FILE_ATTRIBUTE_HIDDEN: u32 = 2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 4;

        if attribs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0 {
            return true;
        }

        // file is not hidden, BUT if it starts with a dot then assume
        // the same behavior as for UNIX; starting with a dot means hidden
        // This is a convenience for using UNIX tooling under Windows
        let s = self.name.to_string_lossy();
        let first = s
            .chars()
            .next()
            .expect("panic: this should not have happened");
        first == '.'
    }

    #[cfg(unix)]
    pub fn is_exec(&self) -> bool {
        let perms = self.metadata.mode() & 0o111;
        self.metadata.is_file() && (perms != 0)
    }

    #[cfg(not(unix))]
    pub fn is_exec(&self) -> bool {
        let lossy_name = self.name.to_string_lossy();
        lossy_name.ends_with(".exe") || lossy_name.ends_with(".EXE")
    }

    #[cfg(unix)]
    pub fn is_suid(&self) -> bool {
        const S_ISUID: u32 = 0o4000;
        let perms = self.metadata.mode() & S_ISUID;
        perms != 0
    }

    #[cfg(not(unix))]
    pub fn is_suid(&self) -> bool {
        false
    }

    #[cfg(unix)]
    pub fn is_sgid(&self) -> bool {
        const S_ISGID: u32 = 0o2000;
        let perms = self.metadata.mode() & S_ISGID;
        perms != 0
    }

    #[cfg(not(unix))]
    pub fn is_sgid(&self) -> bool {
        false
    }

    #[cfg(unix)]
    pub fn is_sticky(&self) -> bool {
        const S_ISVTX: u32 = 0o1000;
        let perms = self.metadata.mode() & S_ISVTX;
        perms != 0
    }

    #[cfg(not(unix))]
    pub fn is_sticky(&self) -> bool {
        false
    }

    #[cfg(unix)]
    pub fn is_fifo(&self) -> bool {
        const S_ISVTX: u32 = 0o1000;
        let perms = self.metadata.mode() & S_ISVTX;
        perms != 0
    }

    #[cfg(not(unix))]
    pub fn is_fifo(&self) -> bool {
        false
    }
}

// EOB
