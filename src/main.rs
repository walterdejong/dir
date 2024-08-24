//
//  dir         WJ124
//  main.rs
//

pub mod entry;

use anyhow::Result;
use chrono::{DateTime, Datelike, Local};
use clap::{Arg, Command};
use entry::Entry;
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
#[cfg(unix)]
use std::fs::Permissions;
use std::{
    cmp::Ordering,
    collections::HashMap,
    ffi::OsStr,
    fs::{self, File},
    io::{self, BufReader},
    path::{Path, PathBuf},
    sync::Mutex,
};

lazy_static! {
    // hashmap: file extension -> color code
    static ref COLOR_BY_EXT: Mutex<HashMap<String, u32>> = Mutex::new(HashMap::new());

    // lookup table -> color code by filetype index
    static ref COLOR_BY_FILETYPE: Mutex<Vec<u32>> = Mutex::new(vec![0;FT_MAX]);

    // lookup table -> color code by file mode
    static ref COLOR_BY_MODE: Mutex<Vec<u32>> = Mutex::new(vec![0;FM_MAX]);
}

// filetype constant indices into COLOR_BY_FILETYPE
const FT_FILE: usize = 0;
const FT_DIR: usize = 1;
const FT_SYMLINK: usize = 2;
const FT_FIFO: usize = 3;
const FT_SOCK: usize = 4;
const FT_BLOCKDEV: usize = 5;
const FT_CHARDEV: usize = 6;
const FT_MAX: usize = 7;

// file mode constant indices into COLOR_BY_MODE
const FM_EXEC: usize = 0;
const FM_SUID: usize = 1;
const FM_SGID: usize = 2;
const FM_STICKY: usize = 3;
const FM_MAX: usize = 4;

// format time as short month name + day + hours + minutes if it is in the current year
// or less than 90 days ago
// Otherwise, format as short month name + day + year (omitting the time)
fn format_time(dt: &DateTime<Local>) -> String {
    let year = dt.year();

    static NOW: OnceCell<DateTime<Local>> = OnceCell::new();
    let now = NOW.get_or_init(|| chrono::Local::now());
    let current_year = now.year();

    if year == current_year {
        format!("{}", dt.format("%b %d %H:%M"))
    } else {
        let days_since = dt.signed_duration_since(now).num_days();
        if days_since >= -90 {
            format!("{}", dt.format("%b %d %H:%M"))
        } else {
            format!("{}", dt.format("%b %d  %Y"))
        }
    }
}

fn format_size(size: u64) -> String {
    if size < 900 {
        return format!("{}", size);
    }

    const UNITS: [char; 8] = ['k', 'M', 'G', 'T', 'P', 'E', 'Z', 'Y'];

    const MULTIPLIER: f32 = 1000.0;
    let mut f = size as f32 / MULTIPLIER;

    let mut unit = UNITS[0];
    for unit_idx in UNITS.iter() {
        unit = *unit_idx;

        if f < 900.0 {
            break;
        }

        f /= MULTIPLIER;
    }

    let s = format!("{:.1} {}B", f, unit);
    s
}

#[allow(unused)]
#[cfg(unix)]
fn format_permissions(perms: &Permissions) -> String {
    use std::os::unix::fs::PermissionsExt;

    let mode = perms.mode() as u32;

    lazy_static! {
        static ref CACHE: Mutex<HashMap<u32, String>> = Mutex::new(HashMap::new());
    }
    let mut cache = CACHE
        .lock()
        .expect("failed to lock mutex on internal cache memory");

    if let Some(mode_string) = cache.get(&mode) {
        // cache hit
        // NOTE we have to clone because can not return from local variable hashmap ...
        // (even though it's a static, yeah)
        return mode_string.clone();
    }

    // I know these are in crate nix ...
    // but nix is just not being useful to me somehow

    const S_IFMT: u32 = 0o170000;

    const S_IFSOCK: u32 = 0o140000;
    const S_IFLNK: u32 = 0o120000;
    const S_IFREG: u32 = 0o100000;
    const S_IFBLK: u32 = 0o060000;
    const S_IFDIR: u32 = 0o040000;
    const S_IFCHR: u32 = 0o020000;
    const S_IFIFO: u32 = 0o010000;

    const S_ISUID: u32 = 0o4000;
    const S_ISGID: u32 = 0o2000;
    const S_ISVTX: u32 = 0o1000;

    const S_IRWXU: u32 = 0o0700;
    const S_IRUSR: u32 = 0o0400;
    const S_IWUSR: u32 = 0o0200;
    const S_IXUSR: u32 = 0o0100;

    const S_IRWXG: u32 = 0o0070;
    const S_IRGRP: u32 = 0o0040;
    const S_IWGRP: u32 = 0o0020;
    const S_IXGRP: u32 = 0o0010;

    const S_IRWXO: u32 = 0o0007;
    const S_IROTH: u32 = 0o0004;
    const S_IWOTH: u32 = 0o0002;
    const S_IXOTH: u32 = 0o0001;

    const FILETYPE_MASK: [u32; 7] = [
        S_IFSOCK, S_IFLNK, S_IFREG, S_IFBLK, S_IFDIR, S_IFCHR, S_IFIFO,
    ];
    const FILETYPE_CHAR: [char; 7] = ['s', 'l', '-', 'b', 'd', 'c', 'p'];

    let mut s = String::with_capacity(10);

    // filetype bit
    for (idx, filetype_mask) in FILETYPE_MASK.into_iter().enumerate() {
        if mode & filetype_mask == filetype_mask {
            s.push(FILETYPE_CHAR[idx]);
            break;
        }
    }

    // rwx user (also does setuid bit)
    s.push(if mode & S_IRUSR == S_IRUSR { 'r' } else { '-' });
    s.push(if mode & S_IWUSR == S_IWUSR { 'w' } else { '-' });
    s.push(if mode & S_ISUID == S_ISUID {
        's'
    } else {
        if mode & S_IXUSR == S_IXUSR {
            'x'
        } else {
            '-'
        }
    });

    // rwx group (also does setgid bit)
    s.push(if mode & S_IRGRP == S_IRGRP { 'r' } else { '-' });
    s.push(if mode & S_IWGRP == S_IWGRP { 'w' } else { '-' });
    s.push(if mode & S_ISGID == S_ISGID {
        's'
    } else {
        if mode & S_IXGRP == S_IXGRP {
            'x'
        } else {
            '-'
        }
    });

    // rwx others (also does sticky bit)
    s.push(if mode & S_IROTH == S_IROTH { 'r' } else { '-' });
    s.push(if mode & S_IWOTH == S_IWOTH { 'w' } else { '-' });
    s.push(if mode & S_ISVTX == S_ISVTX {
        't'
    } else {
        if mode & S_IXOTH == S_IXOTH {
            'x'
        } else {
            '-'
        }
    });

    // add mode string to cache
    cache.insert(mode, s.clone());

    s
}

#[allow(unused)]
fn colorize(entry: &Entry) -> Option<String> {
    if entry.metadata.is_symlink() {
        let colormap = COLOR_BY_FILETYPE
            .lock()
            .expect("error: failed to lock interal lookup table");
        let color = colormap[FT_SYMLINK];
        return Some(format!("\x1b[{};1m", color));
    }
    if entry.metadata.is_dir() {
        let colormap = COLOR_BY_FILETYPE
            .lock()
            .expect("error: failed to lock interal lookup table");
        let color = colormap[FT_DIR];
        return Some(format!("\x1b[{};1m", color));
    }

    if entry.metadata.is_file() {
        if entry.is_exec() {
            let colormap = COLOR_BY_MODE
                .lock()
                .expect("error: failed to lock interal lookup table");
            let color = colormap[FM_EXEC];
            return Some(format!("\x1b[{};1m", color));
        }

        // by filename extension
        if let Some(color) = color_by_ext(&entry.name) {
            return Some(format!("\x1b[{};1m", color));
        }

        // TODO if unix filemode

        // normal file
        let colormap = COLOR_BY_FILETYPE
            .lock()
            .expect("error: failed to lock interal lookup table");
        let color = colormap[FT_FILE];
        if color != 0 {
            return Some(format!("\x1b[{};1m", color));
        } else {
            return None;
        }
    }

    // TODO if unix filetype ...

    None
}

// Returns color code for file extension, if the file extension is known
fn color_by_ext(filename: &OsStr) -> Option<u32> {
    let ext = get_filename_ext(filename)?.to_lowercase();
    let colormap = COLOR_BY_EXT
        .lock()
        .expect("failed to lock mutex on internal hashmap");
    let color = colormap.get(&ext)?;
    Some(*color)
}

fn get_filename_ext(filename: &OsStr) -> Option<String> {
    let lossy_name = filename.to_string_lossy();
    let parts = lossy_name.split(".").collect::<Vec<&str>>();
    if parts.len() <= 1 {
        None
    } else {
        let ext = parts.last().unwrap().to_string();
        Some(ext)
    }
}

fn format_entry(entry: &Entry) -> String {
    #[cfg(unix)]
    let perms_str = format_permissions(&entry.metadata.permissions());

    #[cfg(not(unix))]
    {
        // permissions not implemented for non-unix platform
    }

    let time_str = format_time(&entry.mtime());

    let size_str;
    if entry.metadata.is_dir() {
        size_str = format!("{:^8}", "<DIR>");
    } else {
        size_str = format_size(entry.metadata.len());
    }

    let display_name = if let Some(color_str) = colorize(&entry) {
        // format with colors
        const END_COLOR: &'static str = "\x1b[0m";
        format!(
            "{}{}{}",
            &color_str,
            entry.name.to_string_lossy(),
            END_COLOR
        )
    } else {
        entry.name.to_string_lossy().to_string()
    };

    #[cfg(unix)]
    let mut buf = format!(
        "{}  {}  {:>8}  {}",
        &time_str, &perms_str, &size_str, &display_name
    );
    #[cfg(not(unix))]
    let mut buf = format!("{}  {:>8}  {}", &time_str, &size_str, &display_name);

    if entry.metadata.is_dir() {
        buf.push('/');
    } else if entry.is_exec() {
        buf.push('*');
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

fn load_config() {
    if let Some(config_path) = dirs::config_dir() {
        let mut config_file = PathBuf::from(config_path);
        config_file.push("dir");
        config_file.push("dir.json");

        if !config_file.exists() {
            return;
        }

        let f = File::open(&config_file).expect(&format!(
            "error: failed to open {}",
            config_file.to_string_lossy()
        ));
        let reader = BufReader::new(f);
        let data: serde_json::Value = serde_json::from_reader(reader).expect(&format!(
            "error: {}: syntax error in JSON",
            config_file.to_string_lossy()
        ));

        load_config_data(&data, &config_file);
    }
}

// Returns color code
fn color_by_name(name: &str) -> Option<u32> {
    lazy_static! {
        static ref COLOR_BY_NAME: HashMap<&'static str, u32> = {
            let mut map = HashMap::new();
            map.insert("normal", 0);
            map.insert("reverse", 7);
            map.insert("black", 30);
            map.insert("red", 31);
            map.insert("green", 32);
            map.insert("yellow", 33);
            map.insert("blue", 34);
            map.insert("magenta", 35);
            map.insert("cyan", 36);
            map.insert("white", 37);
            map.insert("bg black", 40);
            map.insert("bg red", 41);
            map.insert("bg green", 42);
            map.insert("bg yellow", 43);
            map.insert("bg blue", 44);
            map.insert("bg magenta", 45);
            map.insert("bg cyan", 46);
            map.insert("bg white", 47);
            map
        };
    }

    if let Some(color) = COLOR_BY_NAME.get(name) {
        Some(*color)
    } else {
        None
    }
}

// Returns filetype index code
fn filetype_by_name(name: &str) -> Option<usize> {
    lazy_static! {
        static ref FILETYPE_BY_NAME: HashMap<&'static str, usize> = {
            let mut map = HashMap::new();
            map.insert("file", FT_FILE);
            map.insert("directory", FT_DIR);
            map.insert("symlink", FT_SYMLINK);
            map.insert("fifo", FT_FIFO);
            map.insert("sock", FT_SOCK);
            map.insert("blockdev", FT_BLOCKDEV);
            map.insert("chardev", FT_CHARDEV);
            map
        };
    }

    if let Some(filetype) = FILETYPE_BY_NAME.get(name) {
        Some(*filetype)
    } else {
        None
    }
}

// Returns filemode index code
fn filemode_by_name(name: &str) -> Option<usize> {
    lazy_static! {
        static ref FILEMODE_BY_NAME: HashMap<&'static str, usize> = {
            let mut map = HashMap::new();
            map.insert("exec", FM_EXEC);
            map.insert("suid", FM_SUID);
            map.insert("sgid", FM_SGID);
            map.insert("sticky", FM_STICKY);
            map
        };
    }

    if let Some(filemode) = FILEMODE_BY_NAME.get(name) {
        Some(*filemode)
    } else {
        None
    }
}

fn load_config_data(data: &serde_json::Value, config_file: &Path) {
    let mut errors = 0u32;

    let mut bright = false;

    if let Some(bright_value) = data.get("bright") {
        if let Some(bright_bool) = bright_value.as_bool() {
            bright = bright_bool;
        } else {
            eprintln!(
                "{}: 'bright' should be a boolean: true or false",
                config_file.to_string_lossy()
            );
            errors += 1;
        }
    }

    if let Some(extension_value) = data.get("extension") {
        errors += load_config_extension(&extension_value, config_file);
    }

    if let Some(filetype_value) = data.get("filetype") {
        errors += load_config_filetype(&filetype_value, config_file);
    }

    if let Some(mode_value) = data.get("mode") {
        errors += load_config_filemode(&mode_value, config_file);
    }

    if errors > 0 {
        std::process::exit(2);
    }
}

fn load_config_extension(extension_value: &serde_json::Value, config_file: &Path) -> u32 {
    let mut errors = 0u32;

    dbg!(&extension_value);
    if let Some(extensions) = extension_value.as_object() {
        let mut color_map = COLOR_BY_EXT
            .lock()
            .expect("error: failed to lock internal hashmap");
        for (key, value) in extensions.iter() {
            dbg!(&key);
            if let Some(svalue) = value.as_str() {
                dbg!(&svalue);
                if let Some(color) = color_by_name(&svalue) {
                    color_map.insert(key.to_string(), color);
                } else {
                    eprintln!(
                        "{}: invalid color name: '{}'",
                        &config_file.to_string_lossy(),
                        &svalue
                    );
                    errors += 1;
                }
            } else {
                eprintln!(
                    "{}: invalid color string in map 'extension'",
                    &config_file.to_string_lossy()
                );
                errors += 1;
            }
        }
        dbg!(&color_map);
    } else {
        eprintln!(
            "{}: 'extension' should be a map: {{\"ext\": \"color\"}}",
            &config_file.to_string_lossy()
        );
        errors += 1;
    }
    errors
}

fn load_config_filetype(filetype_value: &serde_json::Value, config_file: &Path) -> u32 {
    let mut errors = 0u32;

    dbg!(&filetype_value);
    if let Some(filetype) = filetype_value.as_object() {
        let mut color_map = COLOR_BY_FILETYPE
            .lock()
            .expect("error: failed to lock internal lookup table");
        for (key, value) in filetype.iter() {
            dbg!(&key);
            if let Some(ftype) = filetype_by_name(&key) {
                if let Some(svalue) = value.as_str() {
                    dbg!(&svalue);
                    if let Some(color) = color_by_name(&svalue) {
                        color_map[ftype] = color;
                    } else {
                        eprintln!(
                            "{}: invalid color name: '{}'",
                            &config_file.to_string_lossy(),
                            &svalue
                        );
                        errors += 1;
                    }
                } else {
                    eprintln!(
                        "{}: invalid color string in map 'filetype'",
                        &config_file.to_string_lossy()
                    );
                    errors += 1;
                }
            } else {
                eprintln!(
                    "{}: invalid filetype: '{}'",
                    &config_file.to_string_lossy(),
                    &key
                );
                errors += 1;
            }
        }
        dbg!(&color_map);
    } else {
        eprintln!(
            "{}: 'filetype' should be a map: {{\"ftype\": \"color\"}}",
            &config_file.to_string_lossy()
        );
        errors += 1;
    }
    errors
}

fn load_config_filemode(mode_value: &serde_json::Value, config_file: &Path) -> u32 {
    let mut errors = 0u32;

    dbg!(&mode_value);
    if let Some(mode) = mode_value.as_object() {
        let mut color_map = COLOR_BY_MODE
            .lock()
            .expect("error: failed to lock internal lookup table");
        for (key, value) in mode.iter() {
            dbg!(&key);
            if let Some(fmode) = filemode_by_name(&key) {
                if let Some(svalue) = value.as_str() {
                    dbg!(&svalue);
                    if let Some(color) = color_by_name(&svalue) {
                        color_map[fmode] = color;
                    } else {
                        eprintln!(
                            "{}: invalid color name: '{}'",
                            &config_file.to_string_lossy(),
                            &svalue
                        );
                        errors += 1;
                    }
                } else {
                    eprintln!(
                        "{}: invalid color string in map 'filetype'",
                        &config_file.to_string_lossy()
                    );
                    errors += 1;
                }
            } else {
                eprintln!(
                    "{}: invalid filetype: '{}'",
                    &config_file.to_string_lossy(),
                    &key
                );
                errors += 1;
            }
        }
        dbg!(&color_map);
    } else {
        eprintln!(
            "{}: 'mode' should be a map: {{\"fmode\": \"color\"}}",
            &config_file.to_string_lossy()
        );
        errors += 1;
    }
    errors
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
    let args = matches
        .get_many::<String>("path")
        .unwrap()
        .collect::<Vec<_>>();
    // dbg!(&args);

    load_config();

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
                Ok(_) => {}
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
                Ok(_) => {}
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
            Err(e) => return Err(e),
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
