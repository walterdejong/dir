//
//  dir         WJ124
//  main.rs
//

pub mod entry;

use chrono::{DateTime, Datelike, Local};
use clap::{Arg, ArgAction, ColorChoice, Command};
use entry::Entry;
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
#[cfg(unix)]
use std::fs::Permissions;
#[cfg(unix)]
use std::sync::Mutex;
use std::{
    cmp::Ordering,
    collections::HashMap,
    ffi::OsStr,
    fs::{self, File, Metadata},
    io::{self, BufReader},
    path::{Path, PathBuf},
};

struct Settings {
    color: bool,
    bold: bool,
    all: bool,
    classify: bool,
    long: bool,
    one: bool,
    sort_by_size: bool,
    sort_by_time: bool,
    sort_by_extension: bool,
    sort_reverse: bool,
    color_by_extension: HashMap<String, u32>,
    color_by_filetype: Vec<u32>,
    color_by_mode: Vec<u32>,
}

impl Settings {
    #[allow(dead_code)]
    fn new() -> Settings {
        Default::default()
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            color: true,
            bold: true,
            all: false,
            classify: true,
            long: true,
            one: false,
            sort_by_size: false,
            sort_by_time: false,
            sort_by_extension: false,
            sort_reverse: false,
            color_by_extension: HashMap::new(),
            // note, color zero is 'normal'
            color_by_filetype: vec![0; FT_MAX],
            color_by_mode: vec![0; FM_MAX],
        }
    }
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

#[cfg(windows)]
fn format_attributes(metadata: &Metadata) -> String {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_READONLY: u32 = 1;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 4;
    // FILE_ATTRIBUTE_ARCHIVE is pretty useless; do not show
    // the other bits are incredibly rare; do not bother

    let attribs = metadata.file_attributes();

    let mut s = String::with_capacity(3);

    s.push(if attribs & FILE_ATTRIBUTE_READONLY != 0 {
        'R'
    } else {
        ' '
    });
    s.push(if attribs & FILE_ATTRIBUTE_HIDDEN != 0 {
        'H'
    } else {
        ' '
    });
    s.push(if attribs & FILE_ATTRIBUTE_SYSTEM != 0 {
        'S'
    } else {
        ' '
    });

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

    let mut s = String::with_capacity(10);

    // filetype bit
    s.push(match mode & entry::S_IFMT {
        entry::S_IFREG => '-',
        entry::S_IFDIR => 'd',
        entry::S_IFLNK => 'l',
        entry::S_IFBLK => 'b',
        entry::S_IFCHR => 'c',
        entry::S_IFIFO => 'p',
        entry::S_IFSOCK => 's',
        _ => '-',
    });

    // I know these are in crate nix ...
    // but nix is just not being useful to me somehow

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

    // rwx user (also does setuid bit)
    s.push(if mode & S_IRUSR == S_IRUSR { 'r' } else { '-' });
    s.push(if mode & S_IWUSR == S_IWUSR { 'w' } else { '-' });
    s.push(if mode & S_IXUSR == S_IXUSR {
        if mode & entry::S_ISUID == entry::S_ISUID {
            's'
        } else {
            'x'
        }
    } else {
        if mode & entry::S_ISUID == entry::S_ISUID {
            'S'
        } else {
            '-'
        }
    });

    // rwx group (also does setgid bit)
    s.push(if mode & S_IRGRP == S_IRGRP { 'r' } else { '-' });
    s.push(if mode & S_IWGRP == S_IWGRP { 'w' } else { '-' });
    s.push(if mode & S_IXGRP == S_IXGRP {
        if mode & entry::S_ISGID == entry::S_ISGID {
            's'
        } else {
            'x'
        }
    } else {
        if mode & entry::S_ISGID == entry::S_ISGID {
            'S'
        } else {
            '-'
        }
    });

    // rwx others (also does sticky bit)
    s.push(if mode & S_IROTH == S_IROTH { 'r' } else { '-' });
    s.push(if mode & S_IWOTH == S_IWOTH { 'w' } else { '-' });
    s.push(if mode & S_IXOTH == S_IXOTH {
        if mode & entry::S_ISVTX == entry::S_ISVTX {
            't'
        } else {
            'x'
        }
    } else {
        if mode & entry::S_ISVTX == entry::S_ISVTX {
            'T'
        } else {
            '-'
        }
    });

    // add mode string to cache
    cache.insert(mode, s.clone());

    s
}

// Returns FT_xxx constant for entry filetype
#[cfg(unix)]
fn metadata_filetype(metadata: &Metadata) -> usize {
    use std::os::unix::fs::PermissionsExt;

    let mode = metadata.permissions().mode() as u32;
    match mode & entry::S_IFMT {
        entry::S_IFREG => FT_FILE,
        entry::S_IFDIR => FT_DIR,
        entry::S_IFLNK => FT_SYMLINK,
        entry::S_IFBLK => FT_BLOCKDEV,
        entry::S_IFCHR => FT_CHARDEV,
        entry::S_IFIFO => FT_FIFO,
        entry::S_IFSOCK => FT_SOCK,
        _ => FT_FILE,
    }
}

// Returns FT_xxx constant for entry filetype
#[cfg(windows)]
fn metadata_filetype(metadata: &Metadata) -> usize {
    if metadata.is_file() {
        return FT_FILE;
    }
    if metadata.is_dir() {
        return FT_DIR;
    }
    if metadata.is_symlink() {
        return FT_SYMLINK;
    }

    FT_FILE
}

fn format_color(color: u32, config_bold: bool) -> Option<String> {
    if color == 0 {
        None
    } else {
        if config_bold && color < 40 {
            Some(format!("\x1b[{};1m", color))
        } else {
            Some(format!("\x1b[{}m", color))
        }
    }
}

fn colorize(entry: &Entry, settings: &Settings) -> Option<String> {
    if !settings.color {
        return None;
    }

    let filetype = metadata_filetype(&entry.metadata);

    if filetype == FT_DIR {
        #[cfg(unix)]
        if entry.is_sticky() {
            let colormap = &settings.color_by_mode;
            let color = colormap[FM_STICKY];
            return format_color(color, settings.bold);
        }

        let colormap = &settings.color_by_filetype;
        let color = colormap[FT_DIR];
        return format_color(color, settings.bold);
    }

    if filetype == FT_FILE {
        #[cfg(unix)]
        if entry.is_suid() {
            let colormap = &settings.color_by_mode;
            let color = colormap[FM_SUID];
            return format_color(color, settings.bold);
        }

        #[cfg(unix)]
        if entry.is_sgid() {
            let colormap = &settings.color_by_mode;
            let color = colormap[FM_SGID];
            return format_color(color, settings.bold);
        }

        #[cfg(unix)]
        if entry.is_sticky() {
            let colormap = &settings.color_by_mode;
            let color = colormap[FM_STICKY];
            return format_color(color, settings.bold);
        }

        // by filename extension
        if let Some(color) = color_by_ext(&entry.name, settings) {
            return format_color(color, settings.bold);
        }

        if entry.is_exec() {
            let colormap = &settings.color_by_mode;
            let color = colormap[FM_EXEC];
            return format_color(color, settings.bold);
        }
    }

    let colormap = &settings.color_by_filetype;
    let color = colormap[filetype];
    format_color(color, settings.bold)
}

// Returns color code for file extension, if the file extension is known
fn color_by_ext(filename: &OsStr, settings: &Settings) -> Option<u32> {
    let ext = get_filename_ext(filename)?.to_lowercase();
    let colormap = &settings.color_by_extension;
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

fn format_entry(entry: &Entry, settings: &Settings) -> String {
    if settings.one {
        // show only the name
        return entry.name.to_string_lossy().to_string();
    }

    #[cfg(unix)]
    let perms_str = format_permissions(&entry.metadata.permissions());

    let time_str = format_time(&entry.mtime());

    let size_str;
    if entry.metadata.is_dir() {
        size_str = format!("{:^8}", "<DIR>");
    } else {
        size_str = format_size(entry.metadata.len());
    }

    let display_name = if let Some(color_str) = colorize(entry, settings) {
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
    #[cfg(windows)]
    let mut buf = if settings.all {
        format!(
            "{}  {}  {:>8}  {}",
            &time_str,
            &format_attributes(&entry.metadata),
            &size_str,
            &display_name
        )
    } else {
        format!("{}  {:>8}  {}", &time_str, &size_str, &display_name)
    };
    #[cfg(not(any(unix, windows)))]
    let mut buf = format!("{}  {:>8}  {}", &time_str, &size_str, &display_name);

    if let Some(token) = classify(entry, settings) {
        buf.push(token);
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

fn format_wide_entry(entry: &Entry, settings: &Settings) -> String {
    let mut buf = if let Some(color_str) = colorize(entry, settings) {
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
    if let Some(token) = classify(entry, settings) {
        buf.push(token);
    }
    buf
}

fn classify(entry: &Entry, settings: &Settings) -> Option<char> {
    if !settings.classify {
        return None;
    }

    let filetype = metadata_filetype(&entry.metadata);

    match filetype {
        FT_FILE => {
            if entry.is_exec() {
                Some('*')
            } else {
                None
            }
        }
        FT_DIR => Some(std::path::MAIN_SEPARATOR),
        FT_SYMLINK => {
            if settings.long {
                None
            } else {
                Some('@')
            }
        }
        FT_FIFO => Some('|'),
        FT_SOCK => Some('='),
        _ => None,
    }
}

fn load_config() -> Settings {
    if let Some(config_path) = dirs::config_dir() {
        let mut config_file = PathBuf::from(config_path);
        config_file.push("dir");
        config_file.push("dir.json");

        if !config_file.exists() {
            return Settings::default();
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

        return load_config_data(&data, &config_file);
    }
    Settings::default()
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

fn load_config_data(data: &serde_json::Value, config_file: &Path) -> Settings {
    let mut settings = Settings::default();

    let mut errors = 0u32;

    if let Some(color_value) = data.get("color") {
        if let Some(color_bool) = color_value.as_bool() {
            settings.color = color_bool;
        } else {
            eprintln!(
                "{}: 'color' should be a boolean: true or false",
                config_file.to_string_lossy()
            );
            errors += 1;
        }
    }
    if let Some(bold_value) = data.get("bold") {
        if let Some(bold_bool) = bold_value.as_bool() {
            settings.bold = bold_bool;
        } else {
            eprintln!(
                "{}: 'bold' should be a boolean: true or false",
                config_file.to_string_lossy()
            );
            errors += 1;
        }
    }
    if let Some(classify_value) = data.get("classify") {
        if let Some(classify_bool) = classify_value.as_bool() {
            settings.classify = classify_bool;
        } else {
            eprintln!(
                "{}: 'classify' should be a boolean: true or false",
                config_file.to_string_lossy()
            );
            errors += 1;
        }
    }

    if let Some(extension_value) = data.get("extension") {
        let n_errors;
        (settings.color_by_extension, n_errors) =
            load_config_extension(&extension_value, config_file);
        errors += n_errors;
    }

    if let Some(filetype_value) = data.get("filetype") {
        let n_errors;
        (settings.color_by_filetype, n_errors) = load_config_filetype(&filetype_value, config_file);
        errors += n_errors;
    }

    if let Some(mode_value) = data.get("mode") {
        let n_errors;
        (settings.color_by_mode, n_errors) = load_config_filemode(&mode_value, config_file);
        errors += n_errors;
    }

    if errors > 0 {
        std::process::exit(2);
    }
    settings
}

fn load_config_extension(
    extension_value: &serde_json::Value,
    config_file: &Path,
) -> (HashMap<String, u32>, u32) {
    let mut color_map = HashMap::new();
    let mut errors = 0u32;

    if let Some(extensions) = extension_value.as_object() {
        for (key, value) in extensions.iter() {
            if let Some(svalue) = value.as_str() {
                if let Some(color) = color_by_name(&svalue.to_lowercase()) {
                    color_map.insert(key.to_lowercase(), color);
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
    } else {
        eprintln!(
            "{}: 'extension' should be a map: {{\"ext\": \"color\"}}",
            &config_file.to_string_lossy()
        );
        errors += 1;
    }
    (color_map, errors)
}

fn load_config_filetype(filetype_value: &serde_json::Value, config_file: &Path) -> (Vec<u32>, u32) {
    let mut color_map = vec![0; FT_MAX];
    let mut errors = 0u32;

    if let Some(filetype) = filetype_value.as_object() {
        for (key, value) in filetype.iter() {
            if let Some(ftype) = filetype_by_name(&key.to_lowercase()) {
                if let Some(svalue) = value.as_str() {
                    if let Some(color) = color_by_name(&svalue.to_lowercase()) {
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
    } else {
        eprintln!(
            "{}: 'filetype' should be a map: {{\"ftype\": \"color\"}}",
            &config_file.to_string_lossy()
        );
        errors += 1;
    }
    (color_map, errors)
}

fn load_config_filemode(mode_value: &serde_json::Value, config_file: &Path) -> (Vec<u32>, u32) {
    let mut color_map = vec![0; FM_MAX];
    let mut errors = 0u32;

    if let Some(mode) = mode_value.as_object() {
        for (key, value) in mode.iter() {
            if let Some(fmode) = filemode_by_name(&key.to_lowercase()) {
                if let Some(svalue) = value.as_str() {
                    if let Some(color) = color_by_name(&svalue.to_lowercase()) {
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
    } else {
        eprintln!(
            "{}: 'mode' should be a map: {{\"fmode\": \"color\"}}",
            &config_file.to_string_lossy()
        );
        errors += 1;
    }
    (color_map, errors)
}

#[cfg(windows)]
fn windows_globbing(args: &[&String]) -> Vec<PathBuf> {
    let mut v = Vec::new();

    for arg in args.iter() {
        let mut glob_iter = glob::glob(*arg).expect("error in file globbing").peekable();
        if glob_iter.peek().is_none() {
            // arg is not a globbing pattern
            // but we wish to see its dir listing anyway, so keep the path
            v.push(PathBuf::from(*arg));
            continue;
        }
        // expand all globbing
        for path in glob_iter {
            match path {
                Ok(path) => v.push(path),
                Err(_) => {
                    // dbg!("I have a problem");
                    eprintln!("error in file globbing");
                    continue;
                }
            }
        }
    }
    v
}

fn main() {
    let matches = Command::new("dir")
        .color(ColorChoice::Never)
        .version(env!("CARGO_PKG_VERSION"))
        .author("Walter de Jong <walter@heiho.net>")
        .about("Show directory listing")
        .after_help("Copyright (C) 2024 Walter de Jong <walter@heiho.net>")
        .args([
            Arg::new("all")
                .short('a')
                .long("all")
                .action(ArgAction::SetTrue)
                .help("show all, including hidden"),
            Arg::new("wide")
                .short('w')
                .long("wide")
                .action(ArgAction::SetTrue)
                .help("show listing in columns without details"),
            Arg::new("one")
                .short('1')
                .long("one")
                .action(ArgAction::SetTrue)
                .help("show only names in one column without details"),
            Arg::new("no-color")
                .long("no-color")
                .action(ArgAction::SetTrue)
                .help("do not colorize output"),
            Arg::new("size")
                .short('s')
                .long("size")
                .action(ArgAction::SetTrue)
                .help("sort by file size"),
            Arg::new("time")
                .short('t')
                .long("time")
                .action(ArgAction::SetTrue)
                .help("sort by last modified time"),
            Arg::new("extension")
                .short('X')
                .long("extension")
                .visible_alias("ext")
                .action(ArgAction::SetTrue)
                .help("sort by extension"),
            Arg::new("reverse")
                .short('r')
                .long("reverse")
                .action(ArgAction::SetTrue)
                .help("sort in reverse order"),
            Arg::new("path").num_args(0..).default_value("."),
        ])
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

    let mut settings = load_config();

    if matches.get_flag("all") {
        settings.all = true;
    }
    if matches.get_flag("wide") {
        settings.long = false;
    }
    if matches.get_flag("no-color") {
        settings.color = false;
    }
    if matches.get_flag("one") {
        settings.one = true;
        // this also implies these flags;
        settings.color = false;
        settings.long = true;
        settings.classify = false;
    }
    if matches.get_flag("size") {
        settings.sort_by_size = true;
    }
    if matches.get_flag("time") {
        settings.sort_by_time = true;
    }
    if matches.get_flag("extension") {
        settings.sort_by_extension = true;
    }
    if matches.get_flag("reverse") {
        settings.sort_reverse = true;
    }
    let settings = settings; // remove `mut`

    // it's easier to work with Paths, so
    // convert Vec<&String> args to Vec<PathBuf>
    #[cfg(unix)]
    let arg_paths = args
        .iter()
        .map(|s| PathBuf::from(s))
        .collect::<Vec<PathBuf>>();
    // on Windows perform file globbing on args
    #[cfg(windows)]
    let arg_paths = windows_globbing(&args);

    // we first group the given directory arguments together and list those
    // then group the files together and list those
    let dir_paths = arg_paths
        .iter()
        .filter(|x| x.is_dir())
        .map(|x| x.clone())
        .collect::<Vec<PathBuf>>();
    let file_paths = arg_paths
        .iter()
        .filter(|x| !x.is_dir())
        .map(|x| x.clone())
        .collect::<Vec<PathBuf>>();

    let mut errors = 0;

    errors += list_directories(&dir_paths, &settings);

    // when listing dirs and files, put a newline in between
    if dir_paths.len() > 0 && file_paths.len() > 0 {
        println!("");
    }

    errors += list_files(&file_paths, &settings);

    if errors > 0 {
        std::process::exit(2);
    }
    std::process::exit(0);
}

// show directory listings
// Returns number of printed errors
fn list_directories(dir_paths: &[PathBuf], settings: &Settings) -> u32 {
    let mut errors = 0u32;

    for (idx, dir_path) in dir_paths.iter().enumerate() {
        let mut entries = match list_dir(&dir_path) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}: {}", &dir_path.to_string_lossy(), e);
                errors += 1;
                continue;
            }
        };

        sort_entries(&mut entries, settings);

        // when listing multiple directories, show the directory name on top
        if dir_paths.len() > 1 {
            let path = dir_path.as_path().to_string_lossy();
            if path.ends_with(std::path::MAIN_SEPARATOR_STR) {
                println!("{}", &path);
            } else {
                println!("{}{}", &path, std::path::MAIN_SEPARATOR);
            }
        }

        show_listing(&entries, &settings);

        // when listing multiple directories, put a newline in between
        if dir_paths.len() > 1 && idx < dir_paths.len() - 1 {
            println!("");
        }
    }
    errors
}

// show listing of files given on command-line
// Returns number of printed errors
fn list_files(file_paths: &[PathBuf], settings: &Settings) -> u32 {
    let mut errors = 0u32;

    let mut entries = Vec::new();
    for file_path in file_paths.iter() {
        let path = file_path.as_path();
        let entry = match Entry::from_path(path) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("{}: {}", &path.to_string_lossy(), e);
                errors += 1;
                continue;
            }
        };
        entries.push(entry);
    }

    sort_entries(&mut entries, settings);
    show_listing(&entries, settings);

    errors
}

// sort entries in-place
fn sort_entries(entries: &mut [Entry], settings: &Settings) {
    if settings.sort_by_size {
        if settings.sort_reverse {
            entries.sort_by_key(|x| std::cmp::Reverse(x.metadata.len()))
        } else {
            entries.sort_by_key(|x| x.metadata.len());
        }
    } else if settings.sort_by_time {
        if settings.sort_reverse {
            entries.sort_by_key(|x| std::cmp::Reverse(x.mtime()))
        } else {
            entries.sort_by_key(|x| x.mtime())
        }
    } else if settings.sort_by_extension {
        if settings.sort_reverse {
            entries.sort_by(|a, b| sorter_fn_extension(b, a));
        } else {
            entries.sort_by(sorter_fn_extension);
        }
    } else {
        // sort by name, directories first
        if settings.sort_reverse {
            entries.sort_by(|a, b| sorter_dirs_first(b, a));
        } else {
            entries.sort_by(sorter_dirs_first);
        }
    }
}

fn sorter_fn_extension(a: &Entry, b: &Entry) -> Ordering {
    if a.metadata.is_dir() || b.metadata.is_dir() {
        // do not treat dots in directory names as file extension
        return sorter_dirs_first(a, b);
    }

    if let Some(a_ext) = get_filename_ext(&a.name) {
        let a_lower_ext = a_ext.to_lowercase();
        if let Some(b_ext) = get_filename_ext(&b.name) {
            let b_lower_ext = b_ext.to_lowercase();
            let order = a_lower_ext.cmp(&b_lower_ext);
            if order == Ordering::Equal {
                return sorter_dirs_first(a, b);
            }
            return order;
        } else {
            // b_ext is None; a > b
            return Ordering::Greater;
        }
    } else {
        if let Some(_) = get_filename_ext(&b.name) {
            // a_ext is None; a < b
            return Ordering::Less;
        }
        // else both None
    }
    sorter_dirs_first(a, b)
}

fn sorter_dirs_first(a: &Entry, b: &Entry) -> Ordering {
    if a.metadata.is_dir() {
        if b.metadata.is_dir() {
            let a_lower = a.name.to_string_lossy().to_lowercase();
            let b_lower = b.name.to_string_lossy().to_lowercase();
            a_lower.cmp(&b_lower)
        } else {
            Ordering::Less
        }
    } else {
        // a is a file or something else, but not a directory
        if b.metadata.is_dir() {
            Ordering::Greater
        } else {
            let a_lower = a.name.to_string_lossy().to_lowercase();
            let b_lower = b.name.to_string_lossy().to_lowercase();
            a_lower.cmp(&b_lower)
        }
    }
}

fn show_listing(entries: &[Entry], settings: &Settings) {
    // show listing of all entries
    // if not option --long (equals --wide), show wide listing
    // if not option --all, do not show hidden files

    let entries = if !settings.all {
        entries
            .iter()
            .filter(|x| !x.is_hidden())
            .collect::<Vec<&Entry>>()
    } else {
        entries.iter().collect::<Vec<&Entry>>()
    };

    if !settings.long {
        show_wide_listing(&entries, settings);
        return;
    }

    for entry in entries {
        println!("{}", format_entry(entry, settings));
    }
}

fn show_wide_listing(entries: &[&Entry], settings: &Settings) {
    // print in columns
    // we have variable column widths

    if entries.is_empty() {
        return;
    }

    let column_widths = determine_column_widths(entries, settings);
    // dbg!(&column_widths);

    // print entries

    let mut num_lines = entries.len() / column_widths.len();
    if entries.len() % column_widths.len() != 0 {
        num_lines += 1;
    }
    let num_lines = num_lines; // remove mut

    for line in 0..num_lines {
        let mut col = 0;
        let mut i = line;

        loop {
            let entry = entries[i];

            let column_width = column_widths[col];
            col += 1;

            print!("{}", format_wide_entry(entry, settings));

            i += num_lines;
            if i >= entries.len() {
                break;
            }
            if col >= column_widths.len() || column_widths[col] == 0 {
                break;
            }

            let spacer = column_width - display_width(entry, settings);
            if spacer > 0 {
                print!("{:<spacer$}", " ");
            }
        }
        println!("");
    }
}

#[derive(Debug)]
struct ColumnInfo {
    valid: bool,
    line_length: usize,
    column_widths: Vec<usize>,
}

impl ColumnInfo {
    const SPACER: usize = 2;

    fn new() -> ColumnInfo {
        ColumnInfo {
            valid: true,
            line_length: 0,
            column_widths: Vec::new(),
        }
    }
}

// Returns width of filename on screen
fn display_width(entry: &Entry, settings: &Settings) -> usize {
    let mut width = entry.name.to_string_lossy().chars().count();
    if let Some(_) = classify(entry, settings) {
        width += 1;
    }
    width
}

// Returns minimum column width of all entries
fn determine_min_column_width(entries: &[&Entry], settings: &Settings, term_width: usize) -> usize {
    let mut min_width = term_width;

    for entry in entries.iter() {
        let w = display_width(*entry, settings);
        min_width = std::cmp::min(min_width, w + ColumnInfo::SPACER);
    }
    min_width
}

// Returns vec of column widths
fn determine_column_widths(entries: &[&Entry], settings: &Settings) -> Vec<usize> {
    /*
        The procedure used here to determine the variable column widths
        is the same as what GNU coreutils `ls` does
        which is try to fit filenames in columns, while checking the line length
        If the line length goes over the terminal width, then that's invalid;
        you can't have as many columns
        If it does fit, try fitting the next file
    */

    // determine terminal width
    let term_width = if let Some((terminal_size::Width(w), terminal_size::Height(_))) =
        terminal_size::terminal_size()
    {
        w as usize
    } else {
        // note, getting the terminal size will fail when output is redirected
        80usize
    };

    if entries.len() <= 1 {
        return vec![term_width];
    }

    // number of possible columns
    let min_width = determine_min_column_width(entries, settings, term_width);
    let num_possible = term_width / min_width;
    if num_possible <= 1 {
        return vec![term_width];
    }
    let mut column_info = Vec::<ColumnInfo>::with_capacity(num_possible);
    /*
        make a triangular data structure;
        column_info[0] has 1 column widths
        column_info[1] has 2 column widths
        column_info[2] has 3 column widths
        and so on
    */
    for u in 0..num_possible {
        column_info.push(ColumnInfo::new());
        column_info
            .last_mut()
            .expect("unexpected memory error")
            .column_widths = vec![0; u + 1];
    }

    // determine column widths by fitting entries in

    for (n, entry) in entries.iter().enumerate() {
        for i in 0..num_possible {
            if !column_info[i].valid {
                continue;
            }
            let col = n / ((entries.len() + i) / (i + 1));
            let mut width = display_width(*entry, settings);
            if col != i {
                width += ColumnInfo::SPACER;
            }
            let width = width; // remove mut

            if width >= column_info[i].column_widths[col] {
                // filename is longer than the column's width;
                // column needs adjusting
                let old_column_width = column_info[i].column_widths[col];
                column_info[i].column_widths[col] = width;
                column_info[i].line_length += width - old_column_width;
                // does it still fit onscreen?
                column_info[i].valid = column_info[i].line_length < term_width;
            }
        }
    }

    // the highest number of columns is the one that is valid
    let mut col = 0;
    for i in (0..num_possible).rev() {
        if column_info[i].valid {
            col = i;
            break;
        }
    }
    // return column widths
    // NOTE the vec of columns may be larger than the actual number of columns
    // displayed onscreen; the rightmost columns may have width zero
    column_info[col].column_widths.clone()
}

fn list_dir(path: &Path) -> Result<Vec<Entry>, io::Error> {
    let mut entries = Vec::new();

    for dir_entry in fs::read_dir(path)? {
        // an fs::DirEntry holds an open file descriptor to the directory
        // we don't want that ... so therefore I convert it to a custom Entry type
        // the Entry holds all the same attributes; name, metadata, linkdest (if it is a symbolic link)
        // but also (attempts) has an easier interface
        // Mind that the conversion may error, in which case we print the error
        // and skip this entry

        let entry = match dir_entry {
            Ok(d) => {
                match Entry::from_dir_entry(&d) {
                    Ok(x) => x,
                    Err(err) => {
                        // failed to read this single entry
                        eprintln!("{}: {}", &d.path().to_string_lossy(), err);
                        continue;
                    }
                }
            }
            Err(e) => return Err(e),
        };
        entries.push(entry);
    }
    Ok(entries)
}

// EOB
