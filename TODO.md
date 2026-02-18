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
- [ ] Revisit BASENAME cache and remove (only takes first fallback)

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
- [ ] CI tarpaulin test coverage checks (see Makefile), with tarpaulin.toml

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

## 3. Performance

### 3.4 [LOW] `Arc`/`Weak` overhead per handler call

Each handler invocation does a `Weak::upgrade()` (atomic increment + decrement).
This is negligible for CLI use but would appear in hot-loop profiling of REPL
mode. Not worth optimizing now.

---

### 4.5 [LOW] `Vfs` trait only requires `Send`, not `Sync`

**File:** `src/shell.rs:65`

```rust
pub trait Vfs: Send {
```

Since `Vfs` is stored inside `Mutex<Option<Box<dyn Vfs>>>`, `Send` is sufficient
(the `Mutex` provides `Sync`). This is correct, but worth documenting why `Sync`
is not required, so future contributors don't "fix" it.

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
