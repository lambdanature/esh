# Releases

## 0.1.0 - 2026-02-16

Initial release of the **esh** (Embeddable Shell) library and reference binary.

### Library

- **Shell framework** (`ShellConfig` builder pattern) for assembling command-driven CLI applications with pluggable arguments, subcommands, and handlers via `clap` derive integration.
- **`Shell` trait** with `run()` and `run_args()` for executing from `env::args()` or a supplied iterator.
- **`Vfs` trait** -- a backend-agnostic virtual filesystem interface. The library has no dependency on any concrete VFS crate; consumers implement `Vfs` for their chosen backend.
- **Built-in commands**: `version`, `exit`, `pwd` (when a VFS is configured).
- **POSIX-like shell parser** (`shell_parse_line`, `shell_parse_arg`):
  - Single-quoted literals, double-quoted strings with escape processing.
  - Backslash escapes: `\n`, `\t`, `\r`, `\a`, `\b`, `\e`, `\f`, `\v`, `\\`, `\'`, `\"`, `\$`, `` \` ``, `\ `.
  - Hex bytes (`\xHH`), Unicode scalars (`\u{H..H}`), octal (`\0ooo`).
  - Line continuations (`\` + newline) and `#` comments at word boundaries.
- **Structured logging** via `tracing` with configurable verbosity (`-q`, `-v`/`-vv`/`-vvv`) and per-crate env-filter support (`<NAME>_LOG` environment variable).
- **Utility macros**: `die!` (fatal exit with tracing), `pluralize!`, `shell_config!`.

### Reference Binary (`esh`)

- Demonstrates library usage with `vfs-kit`'s `DirFS` as the VFS backend.
- `-p`/`--path` flag to open a VFS rooted at a host directory (validates and canonicalizes the path).
- Thin `DirFsVfs` adapter implementing the library's `Vfs` trait, keeping the backend choice out of the library.

### Known Limitations

- Interactive shell mode is not yet implemented (CLI subcommand dispatch only).
- The `Vfs` trait currently exposes only `cwd()`; additional filesystem operations will be added in future releases.
- Shell parser is Unix-only (`OsStringExt` for raw byte handling); Windows support is planned.
