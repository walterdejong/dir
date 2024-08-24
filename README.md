dir
===

This is a `dir` like program for both Linux, Mac, Windows.

Note, `dir` is a shell builtin on Windows, and a popular alias
on Linux. You may have to adjust PATH, unalias or rename the command
entirely to call the correct binary.


Example output:

```
    Aug 24 13:32  drwxrwxr-x   <DIR>    src/
    Aug 15 17:10  drwxrwxr-x   <DIR>    target/
    Aug 24 16:42  -rw-rw-r--   17.4 kB  Cargo.lock
    Aug 24 16:42  -rw-rw-r--       188  Cargo.toml
    Aug 24 18:34  -rw-rw-r--       446  README.md
```

The UNIX permission bits are not shown on Windows.

The output can be colorized via settings in the config file `dir.json`.
An example is provided.

* Linux: `$XDG_CONFIG_HOME` or `$HOME/.config/dir/dir.json`
* Mac: `$HOME/Library/Application Support/dir/dir.json`
* Windows: `C:\Users\myname\AppData\Roaming\dir\dir.json`


Copyright (C) 2024 by Walter de Jong <walter@heiho.net>

EOB
