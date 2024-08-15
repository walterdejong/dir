//
//  dir         WJ124
//  main.rs
//

pub mod entry;

use chrono::{DateTime, Datelike, Local};
use clap::{Arg, Command};
use entry::Entry;
use lazy_static::lazy_static;
use std::{
    cmp::Ordering, fs, io, path::Path
};

lazy_static! {
    static ref NOW: DateTime<Local> = chrono::Local::now();
}

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
    let matches = Command::new("dir")
        .version("0.1.0")
        .author("Walter de Jong <walter@heiho.net>")
        .about("Show directory listing")
        .args([Arg::new("path").num_args(0..).default_value(".")])
        .get_matches();
    // dbg!(&matches);

    // NOTE I would really like to use OsStr here, but clap won't let me
    // do a .get_many()::<OsStr> nor OsString
    // (Yet it is said that clap supports OsStr arguments...? I dunno)
    let args = matches.get_many::<String>("path").unwrap().collect::<Vec<_>>();
    // dbg!(&args);

    let mut exit_code = 0;

    let mut file_printed = false;

    let mut it = args.iter().peekable();
    while let Some(arg) = it.next() {
        let path = Path::new(arg);

        if path.is_dir() {
            if args.len() > 1 {
                if file_printed {
                    println!("");
                }
                if arg.ends_with(std::path::MAIN_SEPARATOR_STR) {
                    println!("{}", arg);
                } else {
                    println!("{}/", arg);
                }
            }
            match list_dir(&path) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("error: {}: {}", &arg, e);
                    exit_code = 2;
                }
            }

            if it.peek().is_some() {
                println!("");
            }

            file_printed = false;
        } else {
            match list_file(&path) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("error: {}: {}", &arg, e);
                    exit_code = 2;
                }
            }
            file_printed = true;
        }
    }

    std::process::exit(exit_code);
}

fn list_file(path: &Path) -> Result<(), io::Error> {
    let entry = Entry::from_path(path)?;
    println!("{}", format_entry(&entry));
    Ok(())
}

fn list_dir(path: &Path) -> Result<(), io::Error> {
    let mut entries = Vec::new();

    for dir_entry in fs::read_dir(path)? {
        // an fs::DirEntry holds an open file descriptor to the directory
        // we don't want that ... so therefore I convert it to a custom Entry type
        // the Entry holds all the same attributes; name, metadata, linkdest (if it is a symbolic link)
        // but also (attempts) has an easier interface
        // Mind that the conversion may error, in which case we print the error
        // and skip this entry

        let entry = match dir_entry {
            Ok(d) => Entry::from_dir_entry(&d)?,
            Err(e) => return Err(e)
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

    Ok(())
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
