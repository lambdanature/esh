//! Convenience re-exports for building an esh-based shell.
//!
//! ```rust,ignore
//! use esh::prelude::*;
//! ```
//!
//! This gives you everything needed to configure, build, and run a shell:
//! [`ShellConfig`], [`ShellError`], [`Shell`], [`Vfs`], the [`shell_config!`]
//! macro, and [`Arc`] for wrapping augmentors/handlers, For convenicence, we also re-export a few `clap` entitites that the public interface of this crate depends on.

pub use std::process::ExitCode;
pub use std::sync::Arc;

pub use crate::{
    die, shell_config, Augmentor, Handler, HandlerResult, Shell, ShellConfig, ShellError, Vfs,
    VfsLookup, HANDLER_SUCCESS,
};
pub use clap::{ArgMatches, Args, Command, CommandFactory, FromArgMatches, Parser, Subcommand};
pub use tracing::{debug, error, info, trace, warn};
