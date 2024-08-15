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

    pub fn is_hidden(&self) -> bool {
        // sucks that we have to convert this entire thing just to look at one first character
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
}

// EOB
