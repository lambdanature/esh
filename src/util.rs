// TODO: use log!() in shell-less fallbacks for die/warn/info macros
// TODO: Enrich log!() output with env!("CARGO_PKG_NAME") / env!("CARGO_PKG_VERSION") /
//       file!() / line!() / column!()

use tracing_subscriber::{
    filter::{Directive, EnvFilter, LevelFilter},
    prelude::*,
    registry::Registry,
};

use std::sync::OnceLock;

use crate::ShellError;

/// Return the basename of the running binary, cached for the process lifetime.
///
/// Tries `argv[0]` first, then [`std::env::current_exe`], and falls back to
/// the provided string if neither yields a usable name.
pub fn get_cmd_basename(fallback: impl Into<String>) -> &'static String {
    static BASENAME: OnceLock<String> = OnceLock::new();
    BASENAME.get_or_init(|| {
        if let Some(arg0) = std::env::args().next()
            && let Some(basename) = std::path::Path::new(&arg0).file_name()
        {
            return basename.to_string_lossy().into_owned();
        }
        if let Ok(exe) = std::env::current_exe()
            && let Some(filename) = exe.file_name()
        {
            return filename.to_string_lossy().into_owned();
        }
        // I officially give up
        fallback.into()
    })
}

/// Convenience wrapper around [`get_cmd_basename`] that uses
/// `CARGO_PKG_NAME` as the fallback.
#[must_use]
pub fn get_cmd_fallback() -> &'static String {
    get_cmd_basename(env!("CARGO_PKG_NAME"))
}

/// Log a fatal error via [`tracing::error!`] and exit the process with code 1.
///
/// Accepts the same format arguments as [`format!`].
/// Prefer returning [`ShellError`](crate::ShellError) from library code instead.
#[macro_export]
macro_rules! die {
    ($fmt:literal, $($arg:tt)*) => {{
        tracing::error!("Fatal error, exiting: {}", format!($fmt, $($arg)*));
        std::process::exit(1)   // No semicolon, we want to return ! (Never)

    }};

    ($msg:literal) => {{
        tracing::error!("Fatal error, exiting: {}", $msg);
        std::process::exit(1)   // No semicolon, we want to return ! (Never)
    }};

    () => {{
        tracing::error!("Fatal error, exiting");
        std::process::exit(1)   // No semicolon, we want to return ! (Never)
    }};
}

/// Simple English pluralisation helper.
///
/// - `pluralize!("item", count)` — appends "s" when `count != 1`.
/// - `pluralize!("child", "children", count)` — uses explicit singular/plural forms.
#[macro_export]
macro_rules! pluralize {
    // Case 1: Word and Count (Simple "s" suffix)
    ($word:expr, $count:expr) => {
        if $count == 1 {
            $word.to_string()
        } else {
            format!("{}s", $word)
        }
    };

    // Case 2: Singular, Plural, and Count (Explicit forms)
    ($singular:expr, $plural:expr, $count:expr) => {
        if $count == 1 {
            $singular.to_string()
        } else {
            $plural.to_string()
        }
    };
}

/// Initialise the global tracing/logging subscriber.
///
/// Sets up a compact stderr logger and installs a panic hook that logs panics.  When the
/// `tracing-log` feature is enabled, also bridges the `log` crate to `tracing` so that libraries
/// using `log::*` macros are captured.
///
/// Returns `(is_verbose, level_filter)` on success.
///
/// # Errors
///
/// Returns [`ShellError::Internal`] if a log tracer or tracing subscriber
/// is already set.
pub fn init_tracing(
    name: impl Into<String>,
    quiet: bool,
    verbose: u8,
) -> Result<(bool, LevelFilter), ShellError> {
    let is_verbose = !quiet && verbose > 0;

    let level_filter = if quiet {
        LevelFilter::ERROR
    } else {
        match verbose {
            0 => LevelFilter::WARN,
            1 => LevelFilter::INFO,
            2 => LevelFilter::DEBUG,
            _ => LevelFilter::TRACE,
        }
    };

    // Bridge log crate macros to tracing (for library code that uses log::*)
    #[cfg(feature = "tracing-log")]
    tracing_log::LogTracer::init()
        .map_err(|e| ShellError::Internal(format!("failed to set log tracer: {e}")))?;

    let registry = Registry::default();

    let log_env_name = format!("{}_LOG", name.into().to_uppercase());

    let rustyline_directive: Directive = "rustyline=warn"
        .parse()
        .map_err(|e| ShellError::Internal(format!("failed to parse rustyline directive: {e}")))?;

    let env_filter = EnvFilter::builder()
        .with_default_directive(level_filter.into())
        .with_env_var(log_env_name)
        .from_env_lossy()
        .add_directive(rustyline_directive);

    let subscriber = registry.with(env_filter).with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .compact(),
    );

    tracing::subscriber::set_global_default(subscriber).map_err(|e| {
        ShellError::Internal(format!("failed to set default tracing subscriber: {e}"))
    })?;

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing_panic::panic_hook(info);
        prev_hook(info); // daisy-chain to old panic hook
    }));

    Ok((is_verbose, level_filter))
}
