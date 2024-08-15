//
//  dir         WJ124
//  main.rs
//

pub mod entry;

use entry::Entry;
use std::{cmp::Ordering, fs};
use chrono::{DateTime, Local, Datelike};
use lazy_static::lazy_static;

lazy_static!(
    static ref NOW: DateTime<Local> = chrono::Local::now();
);

// format time as short month name + day + hours + minutes if it is in the current year
// or less than 90 days ago
// Otherwise, format as short month name + day + year (omitting the time)
fn format_time(dt: &DateTime<Local>) -> String {
    let year = dt.year();
    let current_year = NOW.year();

    if year == current_year {
        format!("{}", dt.format("%b %d %H:%M"))
    } else {
        let days_since = dt.signed_duration_since(*NOW).num_days();
        if days_since >= -90 {
            format!("{}", dt.format("%b %d %H:%M"))
        } else {
            format!("{}", dt.format("%b %d  %Y"))
        }
    }
}

fn format_entry(entry: &Entry) -> String {
    let time_str = format_time(&entry.mtime());

    let size_str;
    if entry.metadata.is_dir() {
        size_str = format!("{:^16}", "<DIR>");
    } else {
        size_str = format!("{:>16}", entry.metadata.len());
    }

    let display_name = entry.name.to_string_lossy();

    let mut buf = format!("{}  {}  {}", &time_str, &size_str, &display_name);

    if entry.metadata.is_dir() {
        buf.push('/');
    }

    if entry.metadata.is_symlink() {
        if let Some(linkdest_path) = &entry.link_dest {
            let display_linkdest = linkdest_path.to_string_lossy();
            buf.push_str(&format!(" -> {}", &display_linkdest));
        }
        // else: should not / can not happen, just ignore it
    }

    buf
}

fn main() {
    let dir = fs::read_dir("/home/walter").expect("error reading directory");

    let mut entries = Vec::new();

    for dir_entry in dir {
        // dbg!(&dir_entry);
        let entry = match dir_entry {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: {}", e);
                continue;
            }
        };

        // an fs::DirEntry holds an open file descriptor to the directory
        // we don't want that ... so therefore I convert it to a custom Entry type
        // the Entry holds all the same attributes; name, metadata, linkdest (if it is a symbolic link)
        // but also (attempts) has an easier interface
        // Mind that the conversion may error, in which case we print the error
        // and skip this entry

        let entry = match Entry::from_dir_entry(&entry) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: {}", e);
                continue;
            }
        };

        if entry.is_hidden() {
            continue;
        }

        entries.push(entry);
    }

    entries.sort_by(sort_dirs_first);

    for entry in entries.iter() {
        println!("{}", format_entry(entry));
    }
}

fn sort_dirs_first(a: &Entry, b: &Entry) -> Ordering {
    if a.metadata.is_dir() {
        if b.metadata.is_dir() {
            a.name.cmp(&b.name)
        } else {
            Ordering::Less
        }
    } else {
        // a is a file or something else, but not a directory
        if b.metadata.is_dir() {
            Ordering::Greater
        } else {
            a.name.cmp(&b.name)
        }
    }
}

// EOB
