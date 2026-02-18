//! Embeddable shell framework.
//!
//! `esh` provides a configurable command-line shell that can be extended with
//! custom subcommands, argument augmentors, handlers, and a virtual filesystem.
//! Start with [`shell_config!`] to build a [`Shell`] instance.

#![warn(missing_docs)]
// Be very strict about panic-safety
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

mod parse;
mod shell;
mod util;

pub use parse::{ShellParseError, shell_parse_arg, shell_parse_line};
pub use shell::{Augmentor, Handler, Shell, ShellConfig, ShellError, Vfs, VfsLookup};
pub use util::{get_cmd_basename, get_cmd_fallback, init_tracing};
