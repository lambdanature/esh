## Pending Code Review

- [ ] Follow up on high priority items from REVIEW.md

## Missing Features

- [ ] Interactive REPL
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


+------------------------------+
| Delete resolved review items |
+------------------------------+

## 4. Testing

### 9.1 No input length limits (LOW)

The parser will happily process arbitrarily long input strings. For an
embeddable shell that might accept user input over a network or IPC, consider
adding configurable maximum input length.

## 10. Build & Tooling

### 10.1 No CI/CD configuration (MEDIUM)

There are no GitHub Actions workflows. The pre-commit hook provides local
safety, but CI is essential for PRs and cross-platform validation.

**Recommendation:** Add a `.github/workflows/ci.yml` that runs `cargo fmt
--check`, `cargo clippy -- -D warnings`, `cargo test`, and `cargo audit`.

## Prioritized Action Items

| Priority | Item | Section |
|----------|------|---------|

| **P1** | Add tests for `shell.rs` | 4.1 |
| **P2** | Set up CI/CD | 10.1 |
