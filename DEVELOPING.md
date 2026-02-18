## Development Tooling
- Install Rust release, see https://rust-lang.org/tools/install/
- Read the `Makefile` (for your safety)
- Run `make tools` to install dependencies
- Read the `.git-pre-commit-template`, then run `make init-git` to activate

### Setting up a Windows Build Host
- Install newest PowerShell: `winget install --id Microsoft.PowerShell --source winget`
- Install GNU Make (Windows: `winget install ezwinports.make`)
- Install Clang (Windows: `winget install LLVM.LLVM`,
  don't forget to add "C:\Program Files\LLVM\bin" to the path)

## Release Checklist
- [ ] `cargo upgrade --verbose` and manually upgrade any semver incompatible dependencies
- [ ] `cargo fix --allow-staged`
- [ ] `cargo clippy`, if sensible then `cargo clippy --fix --lib -p esh`
- [ ] `make coverage`, and bring test coverage up to level
- [ ] `make test`
- [ ] `make precommit`
- [ ] Change version in Cargo.toml to release version (remove `-dev`)
- [ ] Tag and release on github.com
- [ ] `cargo publish --dry-run`, then `cargo publish`
- [ ] Change version in Cargo.toml to next -dev version
