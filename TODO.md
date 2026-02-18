## Pending Code Review

- [ ] Follow up on high priority items from REVIEW.md

## Missing Features

- [ ] Interactive REPL
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
- [ ] Replace all `.unwrap()` and `.expect()` with proper **Result/Option** handling
- [ ] Use Clippy to block future regressions, along the lines of
  - `#![cfg_attr(not(test), deny(clippy::unwrap_used))]`
  - `#![cfg_attr(not(test), deny(clippy::expect_used))]`
  - `#![cfg_attr(not(test), deny(clippy::panic))]`
- [ ] Add `#![deny(clippy::unwrap_used, clippy::expect_used)]` to `lib.rs`
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


+------------------------------+
| Delete resolved review items |
+------------------------------+


## 4. Testing

### 4.1 No tests for `shell.rs` (HIGH)

The shell module (417 lines) has zero unit tests. This is the most complex module
with macro-driven registration, command dispatch, and VFS integration.

**Recommendation:** Add tests for at minimum:
- `ShellConfig` builder (round-trip: build then inspect)
- `BasicShell::build_cmd()` (verify registered subcommands appear)
- Command dispatch (version, exit, pwd)
- Handler ordering and `NotFound` fallthrough
- Error paths (missing VFS, unknown subcommand)

### 4.2 No tests for `util.rs` (MEDIUM)

`get_cmd_basename`, `get_cmd_fallback`, `pluralize!`, and `die!` are untested.

**Recommendation:** `pluralize!` and `get_cmd_basename` (with controlled env) are
easily unit-testable. `die!` can be tested with `#[should_panic]` or by
extracting the format logic.

### 4.3 No integration tests (MEDIUM)

The `tests/` directory does not exist. The binary should be tested end-to-end
with `assert_cmd` or similar.

**Recommendation:** Add integration tests that invoke the binary and verify:
- `esh version` outputs the version
- `esh -p . pwd` outputs the CWD
- `esh -p /nonexistent pwd` produces an error
- Unknown commands print help

### 4.4 `parse.rs` tests are excellent but miss edge cases (LOW)

The 48 parser tests cover the main paths well. Consider adding:
- Deeply nested quoting: `"a'b\"c'd"e`
- Empty input to `shell_parse_arg` with escapes
- Maximum-length octal/hex/unicode sequences
- Octal overflow case (`\0777`)
- Multi-line input with continuations
- Very long input strings (performance / DoS)

---

## 5. Code Quality

## 6. API & Documentation

### 6.2 `Shell` trait is too thin to be useful as a trait (MEDIUM)

The `Shell` trait has only `run()` and `run_args()`. Consumers cannot query the
shell's name, version, registered commands, or VFS from the trait. If the goal
is to allow alternative `Shell` implementations, the trait needs more surface
area. If not, it could be a concrete type.

### 6.3 No `# Panics` section on panicking functions (LOW)

`init_tracing` can panic in 3 places but has no `# Panics` doc section.
Clippy's `missing_panics_doc` lint catches this.

---

## 7. Platform & Portability

### 7.1 Unix-only `OsStringExt` import (HIGH — for cross-platform goals)

**File:** `src/parse.rs:5`

```rust
use std::os::unix::ffi::OsStringExt;
```

This prevents compilation on Windows entirely. The TODO comment acknowledges
this. For a library published to crates.io, this should either:
- Be gated behind `#[cfg(unix)]` with a Windows-compatible alternative
- Use a cross-platform abstraction (e.g., `os_str_bytes` crate)
- Be clearly documented as unix-only in the crate metadata

---

## 8. Dependencies

### 8.1 `rustyline` filter without `rustyline` dependency (TRIVIAL)

**File:** `src/util.rs:104-108`

```rust
.add_directive(
    "rustyline=warn"
        .parse()
        .expect("Failed to parse rustyline directive"),
);
```

The `rustyline` crate is not a dependency. This is presumably forward-looking for
the REPL feature. Consider adding a comment explaining this, or gating it behind
a feature flag.

### 8.2 `tracing-log` bridge may be unnecessary (TRIVIAL)

`tracing-log` bridges the `log` crate to `tracing`, but no current dependency
uses the `log` crate directly. If `vfs-kit` or future deps use `log`, this is
valuable. Otherwise it adds ~2 extra crates to the dependency tree for no
immediate benefit.

---

## 9. Security & Hardening

### 9.1 No input length limits (LOW)

The parser will happily process arbitrarily long input strings. For an
embeddable shell that might accept user input over a network or IPC, consider
adding configurable maximum input length.

### 9.2 No `#[forbid(unsafe_code)]` (LOW)

The crate contains no `unsafe` code. Adding `#![forbid(unsafe_code)]` to
`lib.rs` would prevent accidental introduction and signal safety to users.

### 9.3 `edition = "2024"` is very new (LOW)

Rust edition 2024 was stabilized recently. While this is fine for the project,
it limits the potential user base to very recent Rust toolchains. Consider
documenting the MSRV explicitly in `Cargo.toml` (`rust-version` field) and the
README.

---

## 10. Build & Tooling

### 10.1 No CI/CD configuration (MEDIUM)

There are no GitHub Actions workflows. The pre-commit hook provides local
safety, but CI is essential for PRs and cross-platform validation.

**Recommendation:** Add a `.github/workflows/ci.yml` that runs `cargo fmt
--check`, `cargo clippy -- -D warnings`, `cargo test`, and `cargo audit`.

### 10.2 `.gitignore` has duplicate `/target` entries (TRIVIAL)

Lines 4 and 26 both ignore `target`. The first (`target` without leading slash)
is broader; the second (`/target`) is more specific. Keep only one.

---

## Prioritized Action Items

| Priority | Item | Section |
|----------|------|---------|
| **P0** | Fix silent argument dropping in `Shell::run()` | 1.1 |
| **P0** | Fix octal escape overflow | 1.2 |
| **P0** | Remove `RwLock`/`Mutex` where unnecessary | 3.1 |
| **P1** | Replace `.unwrap()` on locks with proper error handling | 2.1 |
| **P1** | Add tests for `shell.rs` | 4.1 |
| **P1** | Gate or abstract `unix::ffi::OsStringExt` | 7.1 |
| **P2** | Add integration tests | 4.3 |
| **P2** | Add tests for `util.rs` | 4.2 |
| **P2** | Set up CI/CD | 10.1 |
| **P3** | Add `#![forbid(unsafe_code)]` | 9.2 |
| **P3** | Document MSRV | 9.3 |
