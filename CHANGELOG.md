# Releases

## 0.1.1 - 2026-02-18

### Safety & Security

- **Removed hidden `expect()` panic** in the `add_sh!` macro handler closures; now returns `ShellError::Internal` instead of panicking.
- **Replaced `process::exit()` in argument parsing** with proper `ExitCode` returns, so the library never silently terminates the host process on clap errors.
- **Refactored `die!` macro** to return `Err(ShellError::Fatal(...))` instead of calling `process::exit(1)`, making it safe to use in library code.
- **Added `#![forbid(unsafe_code)]`** to the reference binary.
- **Sanitized environment variable names** via new `make_env_ident()` function — shell names with hyphens, spaces, or symbols are now cleaned to valid `[A-Z0-9_]` identifiers.
- **Moved clippy lints to `Cargo.toml`** (`[lints.clippy]`) with strict deny rules including `pedantic`, `nursery`, `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, and more.

### API Changes

- **`Shell::run()` and `Shell::run_args()` now return `Result<ExitCode, ShellError>`** instead of `Result<(), ShellError>`, giving callers control over process exit codes.
- **New `ShellError::Fatal` variant** for errors raised by the `die!` macro.
- **New `ShellError::Clap` variant** for argument parsing errors (already printed to stderr by clap).
- **New `ShellConfig::no_init_tracing()`** method to suppress automatic tracing subscriber setup, useful when creating additional shells in a process that already has a subscriber.
- **Exported `make_env_ident()`** as a public API for sanitizing strings into valid environment variable names.
- **Handler return type** changed to `Result<ExitCode, ShellError>` with a `HANDLER_SUCCESS` constant for convenience.

### Performance

- **ASCII fast path in `push_char`** — the shell parser now pushes ASCII characters directly as a single byte, avoiding `encode_utf8` overhead for the common case.
- **Added `#[inline]` hints** to hot parser helpers (`push_char`, `hex_digit`, `parse_backslash_escape`) for better cross-crate inlining.

### Testing

- **125+ unit tests, 15 integration tests, 4 doc tests** (up from ~30 total in 0.1.0).
- Added **concurrent access tests** for VFS mutex and handler dispatch using scoped threads.
- Added **non-UTF-8 `OsString` tests** on Unix for the `shell_parse_arg` conversion path.
- Added **`die!` macro tests** covering all three invocation forms.
- Added **`shell_config!` macro tests** and `no_init_tracing` tests.
- Added **`make_env_ident` tests** covering edge cases (leading digits, symbols, unicode, empty input).
- Added **code coverage enforcement** via `cargo-tarpaulin` (>80% threshold).

### CI & Tooling

- **GitHub Actions CI** with `rustfmt`, `clippy`, `cargo nextest`, and `cargo audit`.
- **Cross-platform testing** on Ubuntu, macOS, and Windows.
- **Switched to `cargo-nextest`** for faster, more reliable test execution.
- **Pre-commit hook** runs formatting, clippy, and security audit checks.
- Fixed `.rustfmt.toml` edition to match `Cargo.toml` (2021).

### Bug Fixes

- Fixed octal escape overflow in the shell parser (`\0400` no longer silently wraps).
- Fixed silent argument dropping in `Shell::run()`.
- Replaced `OsStringExt` with cross-platform `os_str_bytes` crate.
- Tracing init failure is now non-fatal (warning printed to stderr instead of aborting).

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
