//
//  dir         WJ124
//  main.rs
//

use std::fs;
use chrono::{DateTime,Local};

fn main() {
    let dir = fs::read_dir("/").expect("error reading directory");

    for dir_entry in dir {
        // dbg!(&dir_entry);
        let entry = match dir_entry {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: {}", e);
                continue;
            }
        };

        let path = entry.path();

        let metadata = match entry.metadata() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: {}: {}", path.to_string_lossy(), e);
                continue;
            }
        };

        if let Ok(mtime) = metadata.modified() {
            let dt: DateTime<Local> = mtime.into();
            let s_mtime = dt.format("%Y-%m-%d %H:%M");
            print!("{}  ", &s_mtime);
        } else {
            // mtime not supported on platform
            print!("                  ");
        }

        if metadata.is_dir() {
            print!("<DIR>  ");
        } else {
            print!("       ");
        }

        let size = metadata.len();
        print!("{:>10}  ", size);

        if let Some(name) = path.file_name() {
            print!("{}", name.to_string_lossy());

            if metadata.is_dir() {
                print!("/");
            }

            if metadata.is_symlink() {
                print!(" -> ");
                let link_dest = match fs::read_link(path) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("error: {}", e);
                        continue;
                    }
                };
                print!("{}", link_dest.to_string_lossy());
            }

        } else {
            eprintln!("error: failed to get filename from path");
            continue;
        }
        println!("");
    }
}

// EOB
