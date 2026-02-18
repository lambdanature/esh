//! Embeddable shell framework.
//!
//! `esh` provides a configurable command-line shell that can be extended with
//! custom subcommands, argument augmentors, handlers, and a virtual filesystem.
//! Start with [`shell_config!`] to build a [`Shell`] instance.
//!
//! # Feature flags
//!
//! | Flag | Default | Description |
//! |------|---------|-------------|
//! | `tracing-log` | off | Bridges the [`log`](https://docs.rs/log) crate to [`tracing`] so libraries that use `log::*` macros are captured by the tracing subscriber. |

#![warn(missing_docs)]
// Be very strict about safety
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
#![forbid(unsafe_code)]

mod parse;
mod shell;
mod util;

pub mod prelude;

pub use parse::{
    shell_parse_arg, shell_parse_arg_bytes, shell_parse_line, shell_parse_line_bytes,
    ShellParseError,
};
pub use shell::{
    Augmentor, Handler, HandlerResult, Shell, ShellConfig, ShellError, Vfs, VfsLookup,
    HANDLER_SUCCESS,
};
pub use util::{get_cmd_basename, get_cmd_fallback, init_tracing, make_env_ident};
