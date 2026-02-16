## Development Tooling
- Install Rust release, see https://rust-lang.org/tools/install/
- Read the `Makefile` (for your safety)
- Run `make tools` to install dependencies
- Read the `.git-pre-commit-template`, then run `make init-git` to activate

## Release Checklist
- [ ] `cargo fix`
- [ ] `cargo clippy`, if sensible then `cargo clippy --fix --lib -p esh`
- [ ] Change version in Cargo.toml to release version (remove `-dev`)
- [ ] Tag and release on github.com
- [ ] `cargo publish --dry-run`, then `cargo publish`
- [ ] Change version in Cargo.toml to next -dev version
