## Pending Code Review

- [ ] Follow up on high priority items from REVIEW.md

## Missing Features

- [ ] Interactive REPL
  - [ ] Input line length limits to protect from OOM
  - [ ] Preallocate: Vec::with_capacity(input.len())
  - [ ] Ensure that we don't build_cmd() on every line
- [ ] real alias support (think ll='ls -l' etc.)
- [ ] Parsing / Escape cleanliness
- [ ] Additional VFS features and corresponding commands
- [ ] Test with real-life applications (beyond esh)
- [ ] Dynamic Protobuf support - use prost-reflect to dump
      arbitrary binary data fields to ascii protobuf or JSON
- [ ] Windows mode for vfs access - backslash as path separator
- [ ] Windows port

## General Rust Library Hardening Checklist

### Testing Stack
- [ ] Implement **Doc Tests** for all public API examples
- [ ] Set up **Unit Tests** in `mod tests` for private logic
- [ ] Create **Integration Tests** in `tests/` directory
- [ ] Add **Property-Based Testing** using `proptest` or `quickcheck`
- [ ] Configure **Fuzzing** with `cargo-fuzz` for data parsers
- [ ] Run **Loom** for concurrency/atomic validation
- [ ] Validate `unsafe` blocks with **Miri** (`cargo miri test`)
- [ ] Use **madsim** for testing distributed systems based on async/tokio

### Hardening & Security
- [X] Replace all `.unwrap()` and `.expect()` with proper **Result/Option** handling
- [X] Use Clippy to block future regressions, along the lines of
  - `#![cfg_attr(not(test), deny(clippy::unwrap_used))]`
  - `#![cfg_attr(not(test), deny(clippy::expect_used))]`
  - `#![cfg_attr(not(test), deny(clippy::panic))]`
- [ ] Define and document **MSRV** (Minimum Supported Rust Version)
- [ ] Set up **cargo-deny** and **cargo-audit** for dependency lifecycle
- [ ] Add `#[forbid(unsafe_code)]` if applicable to the crate logic
- [ ] Investigate `no_panic` crate for key functions

### API & Developer Experience (DX)
- [ ] Create a `prelude` module for easy "one-line" imports
- [ ] Refactor errors using the `thiserror` crate for structured debugging
- [ ] Implement **Feature Flags** for heavy optional dependencies
- [ ] Add `#[warn(missing_docs)]` to ensure 100% documentation coverage

### Advanced Verification & Supply Chain
- [ ] Set up **cargo-public-api** to track and prevent accidental breaking changes
- [ ] Implement **cargo-semver-checks** in CI to enforce versioning rules
- [ ] Experiment with **Kani Rust Verifier** for formal proofs of critical logic
- [ ] Audit dependencies using **cargo-vet** to ensure supply chain trust
- [ ] Monitor binary footprint with **cargo-bloat**
- [ ] Use **DHAT** (**cargo-dhat**) or **Valgrind** to profile and minimize heap allocations
- [ ] Build an SBOM using **cargo-sbom**

### Publishing Prep
- [ ] Finalize `README.md` with a copy-pasteable "Quick Start" snippet
- [ ] Configure **CI/CD** (GitHub Actions) for `fmt`, `clippy`, and cross-platform tests
  - [ ] Clippy configuration: `cargo clippy -- -D clippy::unwrap_used -D clippy::expect_used`
- [ ] Fill out categories, description, and keywords in `Cargo.toml`

### Further References
- [ ] https://anssi-fr.github.io/rust-guide/, especially the [checklist][anssi-checklist]
- [ ] https://yevh.github.io/rust-security-handbook/

[anssi-checklist]: https://anssi-fr.github.io/rust-guide/checklist.html




# Code Review — esh (Embeddable Shell)

**Date:** 2026-02-18
**Scope:** Full repository, focusing on safety, security, and performance
**Revision:** `27f241e` (HEAD of `main`, 4 commits ahead of origin)

---

## Executive Summary

`esh` is a well-structured, early-stage Rust library for building command-driven
CLI applications. The codebase demonstrates strong Rust discipline: `unsafe` code
is forbidden, `unwrap`/`expect`/`panic` are denied in non-test code via lint
gates, error propagation uses `thiserror` and `Result`, and CI enforces `clippy`,
`fmt`, and `cargo audit`. The POSIX-like shell parser is correct and well-tested.

The main areas for improvement are: (1) a hidden panic path in the `add_sh!`
macro that bypasses the `expect_used` lint, (2) escape-processing of process
arguments in `Shell::run()` which is semantically wrong, (3) absence of input
length limits ahead of the planned REPL, and (4) global static state that makes
multi-shell scenarios subtly incorrect.

---

## Table of Contents

1. [Safety](#1-safety)
2. [Security](#2-security)
3. [Performance](#3-performance)
4. [Correctness & Design](#4-correctness--design)
5. [Testing](#5-testing)
6. [CI & Tooling](#6-ci--tooling)
7. [Prioritised Action Items](#7-prioritised-action-items)

---

## 1. Safety

### 2.3 [MEDIUM] Environment variable injection via shell name

**File:** `src/util.rs:129`

```rust
let log_env_name = format!("{}_LOG", name.into().to_uppercase());
```

The shell name is used to construct an environment variable name. If the name
contains characters that are unusual in env var names (spaces, `=`, NUL), this
could create a malformed or misleading env var lookup. In practice the name comes
from `CARGO_BIN_NAME` or a hardcoded string, but since `ShellConfig::new()`
accepts arbitrary `impl Into<String>`, a library consumer could pass a
problematic name.

**Recommendation:** Sanitize the name (allow only `[A-Z0-9_]`) or validate it in
`ShellConfig::new()`.


## 3. Performance

### 3.3 [LOW] `push_char` UTF-8 encoding per character

**File:** `src/parse.rs:257–261`

Each character is encoded into a 4-byte stack buffer, then copied. For the common
ASCII case, a direct `output.push(c as u8)` fast path would avoid the
`encode_utf8` call. Again, profile first.

### 3.4 [LOW] `Arc`/`Weak` overhead per handler call

Each handler invocation does a `Weak::upgrade()` (atomic increment + decrement).
This is negligible for CLI use but would appear in hot-loop profiling of REPL
mode. Not worth optimizing now.

---

## 4. Correctness & Design

### 4.2 [MEDIUM] Global `INIT_LOGGING` makes multi-shell use silently incorrect

**File:** `src/shell.rs:289, 313–328`

`INIT_LOGGING` is a process-global `OnceLock`. If two `BasicShell` instances are
created with different verbosity settings, the first one's settings win and the
second silently uses them. The `get_or_init` call will return the first result,
and `init_tracing` will fail on the second call (global subscriber already set).

This is documented by the test `init_tracing_second_call_fails`, so it's a known
limitation. However, for library consumers who embed multiple shells (e.g. for
different subsystems), this is a subtle footgun.

**Recommendation:** Document this limitation in the `ShellConfig` API docs.
Consider accepting a pre-configured tracing subscriber instead of always
initializing one internally.

### 4.3 [MEDIUM] Global `BASENAME` cache in `get_cmd_basename`

**File:** `src/util.rs:20–35`

Same pattern as 4.2 — the `OnceLock` means the first call's result is cached
forever, even if subsequent callers pass different fallback values. The tests
verify this (`get_cmd_basename_is_cached`), but it could surprise library
consumers.

**Recommendation:** Document clearly that only the first call's fallback is used,
or remove the cache and let callers cache if needed.

### 4.5 [LOW] `Vfs` trait only requires `Send`, not `Sync`

**File:** `src/shell.rs:65`

```rust
pub trait Vfs: Send {
```

Since `Vfs` is stored inside `Mutex<Option<Box<dyn Vfs>>>`, `Send` is sufficient
(the `Mutex` provides `Sync`). This is correct, but worth documenting why `Sync`
is not required, so future contributors don't "fix" it.

### 4.6 [LOW] `handle_basic_cli_command` for `Shell` subcommand returns error

**File:** `src/shell.rs:151–158`

The `Shell` subcommand is registered but immediately returns
`ShellError::Internal("command 'shell' not implemented")`. This is a placeholder
for the future REPL. It's fine as-is, but the error message should probably be
user-facing ("shell mode is not yet implemented") rather than sounding like an
internal bug.

---

## 5. Testing

### 5.2 [MEDIUM] Missing test coverage

- **`shell_parse_arg` with non-UTF-8 bytes on Unix** — `\xFF` produces a raw
  byte that is valid in Unix `OsString` but not UTF-8. The `_bytes` variant is
  tested, but the `OsString` conversion path (`from_io_vec`) is only implicitly
  tested.
- **Concurrent handler execution** — no tests exercise the `Mutex<Option<Box<dyn
  Vfs>>>` under contention. Since the current architecture is single-threaded
  this is low priority, but the `Arc<dyn Shell>` API implies shared ownership.
- **`die!` macro** — not tested (understandably, since it calls
  `process::exit`). Consider testing with a subprocess.
- **`shell_config!` macro** — not directly tested (only tested through the
  `config()` helper which calls `ShellConfig::new`).

### 5.3 [LOW] Static `AtomicUsize` counters in tests may leak across test runs

**File:** `src/shell.rs` (multiple tests)

Tests use `static AtomicUsize` counters (e.g. `CALL_COUNT`, `SECOND_CALLED`) to
verify handler invocations. Since Rust tests run in the same process, these
counters accumulate across tests. The tests correctly account for this by
checking relative values (`>= 1`) or capturing "before" snapshots, but this
pattern is fragile.

**Recommendation:** Consider using thread-local or test-scoped counters, or
`Arc<AtomicUsize>` passed through the handler closure.

---

### 6.3 [LOW] No Windows CI

The test matrix covers `ubuntu-latest` and `macos-latest` but not Windows.
`TODO.md` notes Windows as a future goal. When Windows support is added, add
`windows-latest` to the CI matrix.

---

## 7. Prioritised Action Items

### High Priority

| # | Section | Issue | Effort |
|---|---------|-------|--------|
| 1 | 4.1 | `run()` escape-processes OS args — will mangle paths with backslashes | Small |
| 2 | 1.1 | `expect()` in `add_sh!` macro — hidden panic point that evades lints | Small |
| 3 | 2.1 | No input length limits — OOM risk in future REPL mode | Small |

### Medium Priority

| # | Section | Issue | Effort |
|---|---------|-------|--------|
| 4 | 1.2 | `clap::Error::exit()` bypasses destructors — bad for a library crate | Small |
| 5 | 4.4 | Edition mismatch `Cargo.toml` (2021) vs `.rustfmt.toml` (2024) | Trivial |
| 6 | 6.1 | Clippy CI fails on `expect_used` in integration tests | Small |
| 7 | 1.3 | `die!` macro skips destructors — document or offer alternative | Small |
| 8 | 3.1 | `build_cmd()` rebuilt per invocation — matters for future REPL | Medium |
| 9 | 4.2 | Global `INIT_LOGGING` — document or make configurable | Small |
| 10 | 2.2 | No VFS sandboxing — document trust model | Small |
| 11 | 2.3 | Shell name used in env var name without sanitization | Small |

### Low Priority

| # | Section | Issue | Effort |
|---|---------|-------|--------|
| 12 | 4.3 | Global `BASENAME` cache — document first-call-wins semantics | Trivial |
| 13 | 1.5 | Add `#![forbid(unsafe_code)]` to the binary | Trivial |
| 14 | 4.5 | Document why `Vfs: Send` (not `Sync`) is sufficient | Trivial |
| 15 | 4.6 | Improve "not implemented" error message for `shell` subcommand | Trivial |
| 16 | 6.2 | Fix `useless_conversion` warning in `tests/cli.rs` | Trivial |
| 17 | 3.2 | Parser `Vec` capacity hints | Trivial |
| 18 | 3.3 | ASCII fast path in `push_char` | Trivial |
| 19 | 5.3 | Static atomic counters in tests are fragile | Small |

---

*Review conducted against commit `27f241e`. All findings are based on static
analysis, clippy output, and manual code reading.*
