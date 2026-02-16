## Development Tooling
- Install Rust release, see https://rust-lang.org/tools/install/
- Read Makefile and then run `make tools` to install dependencies

## Release Checklist
- [ ] `cargo fix`
- [ ] `cargo clippy`, if sensible then `cargo clippy --fix --lib -p esh`
- [ ] Change version in Cargo.toml to release version (remove `-dev`)
- [ ] Tag and release on github.com
- [ ] `cargo publish --dry-run`, then `cargo publish`
- [ ] Change version in Cargo.toml to next -dev version
