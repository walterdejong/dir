dir
===

This is a `dir` like program for both Linux, Mac, Windows.

Note, `dir` is a shell builtin on Windows, and a popular alias
on Linux. You may have to adjust PATH, unalias or rename the command
entirely to call the correct binary.


Example output:

```
    Aug 31 20:12  drwxr-xr-x   <DIR>    src/
    Aug 15 19:31  drwxr-xr-x   <DIR>    target/
    Aug 25 22:10  -rw-r--r--   18.4 kB  Cargo.lock
    Aug 25 22:10  -rw-r--r--       210  Cargo.toml
    Aug 31 20:12  -rw-r--r--    1.1 kB  dir.json
    Sep 01 15:00  -rw-r--r--    1.1 kB  LICENSE
    Sep 01 15:09  -rw-r--r--       891  README.md
```

The UNIX permission bits are not shown on Windows.

The output can be colorized via settings in the config file `dir.json`.
An example is provided.

* Linux: `$XDG_CONFIG_HOME` or `$HOME/.config/dir/dir.json`
* Mac: `$HOME/Library/Application Support/dir/dir.json`
* Windows: `C:\Users\myname\AppData\Roaming\dir\dir.json`


_Copyright (C) 2024 Walter de Jong <walter@heiho.net>_
