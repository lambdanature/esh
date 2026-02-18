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


### 3.3 `Shell` trait's `run_args` uses concrete `std::slice::Iter` (LOW)

**File:** `src/shell.rs:22`

```rust
fn run_args(&self, args: std::slice::Iter<OsString>) -> ExitCode;
```

This forces callers to have a `&[OsString]`. A more flexible signature would
accept `impl IntoIterator<Item = impl AsRef<OsStr>>` or at least
`&[OsString]` directly.

### 3.5 `Handler` / `Augmentor` type aliases are not public (LOW)

Users must manually construct `Arc<dyn Fn(&dyn Shell, &ArgMatches) ->
CommandResult + Send + Sync>`. Exporting these type aliases would improve
ergonomics.

### 3.6 Double wrapping in `Arc` (LOW)

**File:** `src/shell.rs:405-416`

```rust
pub fn build(self) -> Arc<dyn Shell + 'static> {
    let sh = BasicShell::new(...); // returns Arc<BasicShell>
    Arc::new(sh) as Arc<dyn Shell> // wraps Arc<BasicShell> in another Arc
}
```

`BasicShell::new()` returns `Arc<BasicShell>`. Then `build()` wraps it in another
`Arc`. Since `Shell` is implemented for `Arc<BasicShell>`, this creates an
`Arc<Arc<BasicShell>>`. The extra indirection is unnecessary.

**Recommendation:** Return the `Arc<BasicShell>` directly (cast to `Arc<dyn
Shell>` without re-wrapping), or implement `Shell` only on `BasicShell` (not
`Arc<BasicShell>`) and coerce directly.

---

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

### 5.1 Commented-out code (LOW)

- `src/shell.rs:226-233` — commented-out `execute_line` and `execute_args`
- `src/parse.rs:186` — commented-out UTF-8 validation line

**Recommendation:** Remove or move to a tracking issue. Version control
preserves history.

### 5.2 `#[allow(unused_imports)]` for tracing (LOW)

**File:** `src/shell.rs:11`, `src/util.rs:6`

```rust
#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};
```

This suppresses warnings for unused log levels. Import only the levels actually
used in each file.

### 5.3 Typo in `util.rs` (TRIVIAL)

**File:** `src/util.rs:29`

```rust
// I officialy give up
```

Should be "officially".

### 5.4 Pedantic clippy warnings (LOW)

Running `cargo clippy -- -W clippy::pedantic -W clippy::nursery` produces 21
warnings. Notable ones:
- Missing `#[must_use]` on all `ShellConfig` builder methods and `build()`
- `cast_lossless`: `h as u32` should be `u32::from(h)` in unicode parsing
- `significant_drop_tightening`: `cli_group` lock held longer than needed in
  `build_cmd()`
- `uninlined_format_args`: format strings in `main.rs` should use inline
  variables
- `items_after_statements`: `State` enum declared after `let` bindings in
  `shell_parse_line`

---

## 6. API & Documentation

### 6.1 Most public items lack doc comments (HIGH)

- `Shell` trait: no doc comment
- `ShellConfig`: no doc comment
- All `ShellConfig` builder methods: no doc comments
- `CommandResult`: no doc comment
- `init_tracing`: no doc comment
- `get_cmd_basename` / `get_cmd_fallback`: no doc comments
- `die!` / `pluralize!` macros: no doc comments

`parse.rs` is the exception — it has thorough documentation. The rest of the
public API should match that standard.

**Recommendation:** Add `#![warn(missing_docs)]` to `lib.rs` and fill in the
gaps.

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
| **P0** | Fix double `Arc` wrapping | 3.6 |
| **P1** | Replace `.unwrap()` on locks with proper error handling | 2.1 |
| **P1** | Add tests for `shell.rs` | 4.1 |
| **P1** | Add doc comments to public API | 6.1 |
| **P1** | Gate or abstract `unix::ffi::OsStringExt` | 7.1 |
| **P2** | Re-export `CommandResult`, `Handler`, `Augmentor` | 3.4, 3.5 |
| **P2** | Add integration tests | 4.3 |
| **P2** | Add tests for `util.rs` | 4.2 |
| **P2** | Set up CI/CD | 10.1 |
| **P2** | Add `#[must_use]` to builder methods | 5.5 |
| **P3** | Remove commented-out code | 5.1 |
| **P3** | Clean up unused imports | 5.2 |
| **P3** | Fix pedantic clippy warnings | 5.5 |
| **P3** | Add `#![forbid(unsafe_code)]` | 9.2 |
| **P3** | Add `#![warn(missing_docs)]` | 6.1 |
| **P3** | Document MSRV | 9.3 |
