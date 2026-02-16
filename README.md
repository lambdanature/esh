# esh - Embeddable Shell

_Note: This is pre-release / alpha software, use at your own
discretion. Notably, at the time of writing, the actual REPL mode is missing, as
well as sufficient testing, panic-safety, and library documention. See
[TODO.md](TODO.md) for next steps, and [CHANGELOG.md](CHANGELOG.md) for latest
news._

A Rust library for building interactive, command-driven CLI
applications. **esh** provides a shell framework that handles argument parsing,
command dispatch, VFS integration, and structured logging -- so you can focus on
defining commands rather than wiring up boilerplate.

The crate ships both a **library** (`esh`) and a reference **binary** (`esh`)
that demonstrates how to assemble a working shell from the library's building
blocks.

### Library

The library is backend-agnostic. It has **no dependency** on any concrete VFS
crate -- it defines a `Vfs` trait that consumers implement.

**Modules:**

- **`shell`** -- The core framework. `ShellConfig` is a builder that registers
  CLI arguments, subcommands, command handlers, and an optional VFS
  lookup. Calling `.build()` produces an `Arc<dyn Shell>` that can be
  `.run()`'d. Built-in commands include `version`, `exit`, and (when a VFS is
  configured) `pwd`.
- **`parse`** -- A POSIX-like shell parser. `shell_parse_line()` splits a string
  into words honoring single quotes, double quotes, backslash escapes, `#`
  comments, and line continuations. `shell_parse_arg()` processes escape
  sequences in a single argument. Supported escapes include `\n`, `\t`, `\xHH`
  (hex bytes), `\u{H..H}` (Unicode scalars), and `\0ooo` (octal).
- **`util`** -- Logging initialization via `tracing` + `tracing-subscriber` with
  `ENV_FILTER` support, the `die!` macro for fatal exits, and `pluralize!` for
  simple English pluralization.

**Key traits:**

- `Shell` -- Run a shell from `env::args()` or a supplied arg iterator.
- `Vfs` -- Backend-agnostic virtual filesystem interface (`fn cwd(&self) ->
  &Path`). Implement this for any FS backend you like.

### Binary

The reference binary demonstrates how to wire up the library:

1. Defines a `-p`/`--path` CLI argument that validates and canonicalizes a directory path.
2. Implements `Vfs` for `vfs-kit`'s `DirFS` via a thin `DirFsVfs` newtype adapter.
3. Registers a VFS lookup function that creates the `DirFS` from the parsed path.
4. Builds and runs the shell.

## Usage

```bash
# Run a command against a directory
esh -p /some/directory pwd

# Print version
esh version

# Verbose logging (-v, -vv, -vvv for increasing detail)
esh -v -p . pwd

# Quiet mode (errors only)
esh -q -p . pwd
```

## Shell Parser

The parser in `parse.rs` follows POSIX shell quoting conventions:

| Syntax | Behavior |
|---|---|
| `word` | Plain word, split on whitespace |
| `'...'` | Single-quoted literal (no escape processing) |
| `"..."` | Double-quoted (backslash escapes active) |
| `\ ` | Escaped space (joins words) |
| `\n`, `\t`, ... | Standard C escapes |
| `\xHH` | Hex byte (1-2 digits) |
| `\u{H..H}` | Unicode scalar (1-6 hex digits) |
| `\0ooo` | Octal byte (up to 3 digits) |
| `# comment` | Line comment (only at word boundary) |
| `\` + newline | Line continuation |

```rust
use esh::{shell_parse_line, shell_parse_arg};

let words = shell_parse_line(r#"echo "hello world" foo\ bar"#)?;
// => ["echo", "hello world", "foo bar"]

let arg = shell_parse_arg(r"\x48\x65\x6c\x6c\x6f")?;
// => "Hello"
```

## Extending with Custom Commands

Use the `ShellConfig` builder to register your own arguments, subcommands, and handlers:

```rust
use std::sync::Arc;
use esh::{shell_config, Shell};

let cfg = shell_config!()
    .cli_args(Arc::new(MyArgs::augment_args))
    .cli_cmds(Arc::new(MyCommands::augment_subcommands))
    .cli_handler(Arc::new(my_handler));

let sh = cfg.build();
sh.run();
```

## Plugging in a VFS Backend

The library's `Vfs` trait is intentionally minimal so you can bring any filesystem backend:

```rust
use std::path::Path;
use esh::Vfs;

struct MyVfs { /* ... */ }

impl Vfs for MyVfs {
    fn cwd(&self) -> &Path {
        // return current working directory within the virtual FS
        todo!()
    }
}
```

Register it via `.vfs_lookup()` on `ShellConfig`:

```rust
fn create_my_vfs(matches: &ArgMatches) -> Option<Box<dyn Vfs>> {
    let path = matches.get_one::<PathBuf>("my_path")?;
    Some(Box::new(MyVfs::new(path)))
}

let cfg = shell_config!()
    .vfs_lookup(Arc::new(create_my_vfs));
```

When a VFS is configured, the shell automatically enables the vfs-aware command,
e.g. `pwd`.

## Building

```bash
cargo build
cargo test
cargo run -- -p . version
```

## Authors

This library is written by [Michael Wildpaner](https://github.com/lambdanature).

## Acknowledgments

This project is made possible thanks to the incredible work of the Rust
community and the maintainers of the following crates:

* **[clap](https://github.com/clap-rs/clap)** – For providing the gold standard
  in command-line argument parsing.
* **[tracing](https://github.com/tokio-rs/tracing)** – For structured, scoped,
  and async-aware diagnostics.
* **[vfs-kit](https://github.com/vfs-kit/vfs-kit)** – For the elegant virtual
  file system abstractions so I don't have to.

I'm grateful to the authors and contributors of these libraries for their
dedication to the Rust ecosystem.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
 * MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
