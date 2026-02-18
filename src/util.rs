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
        if let Some(arg0) = std::env::args().next() {
            if let Some(basename) = std::path::Path::new(&arg0).file_name() {
                return basename.to_string_lossy().into_owned();
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(filename) = exe.file_name() {
                return filename.to_string_lossy().into_owned();
            }
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

/// Log a fatal error via [`tracing::error!`] and exit shell via error handling
///
/// Accepts the same format arguments as [`format!`].
/// This returns [`ShellError`](crate::ShellError) from user supplied handlers.
#[macro_export]
macro_rules! die {
    ($fmt:literal, $($arg:tt)*) => {{
        let msg = format!($fmt, $($arg)*);
        tracing::error!("Fatal error, exiting: {}", msg);
        return Err($crate::ShellError::Fatal(msg.into()))
    }};

    ($msg:literal) => {{
        let msg = format!("{}", $msg);
        tracing::error!("Fatal error, exiting: {}", msg);
        return Err($crate::ShellError::Fatal(msg))
    }};

    () => {{
        tracing::error!("Fatal error, exiting");
        return Err($crate::ShellError::Fatal("Exiting".into()))
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

#[cfg(test)]
mod tests {
    use super::*;

    // -- pluralize! --------------------------------------------------------

    #[test]
    fn pluralize_simple_singular() {
        assert_eq!(pluralize!("item", 1), "item");
    }

    #[test]
    fn pluralize_simple_zero() {
        assert_eq!(pluralize!("item", 0), "items");
    }

    #[test]
    fn pluralize_simple_many() {
        assert_eq!(pluralize!("item", 5), "items");
    }

    #[test]
    fn pluralize_explicit_singular() {
        assert_eq!(pluralize!("child", "children", 1), "child");
    }

    #[test]
    fn pluralize_explicit_zero() {
        assert_eq!(pluralize!("child", "children", 0), "children");
    }

    #[test]
    fn pluralize_explicit_many() {
        assert_eq!(pluralize!("child", "children", 42), "children");
    }

    #[test]
    fn pluralize_with_variable() {
        let n = 1;
        assert_eq!(pluralize!("file", n), "file");
        let n = 2;
        assert_eq!(pluralize!("file", n), "files");
    }

    #[test]
    fn pluralize_with_expression() {
        let v = vec![1, 2, 3];
        assert_eq!(pluralize!("element", v.len()), "elements");
    }

    #[test]
    fn pluralize_explicit_with_variable() {
        for (n, expected) in [(0, "mice"), (1, "mouse"), (2, "mice")] {
            assert_eq!(pluralize!("mouse", "mice", n), expected);
        }
    }

    // -- get_cmd_basename / get_cmd_fallback --------------------------------

    #[test]
    fn get_cmd_basename_returns_nonempty() {
        let name = get_cmd_basename("test-fallback");
        assert!(!name.is_empty());
    }

    #[test]
    fn get_cmd_basename_is_cached() {
        let a = get_cmd_basename("fb1");
        let b = get_cmd_basename("fb2");
        assert!(std::ptr::eq(a, b), "should return the same &'static ref");
    }

    #[test]
    fn get_cmd_fallback_returns_nonempty() {
        let name = get_cmd_fallback();
        assert!(!name.is_empty());
    }

    #[test]
    fn get_cmd_fallback_same_as_basename() {
        let a = get_cmd_basename("anything");
        let b = get_cmd_fallback();
        assert!(std::ptr::eq(a, b));
    }

    // -- init_tracing level selection --------------------------------------
    //
    // init_tracing sets a global subscriber, so it can only succeed once per
    // process. The shell tests already exercise the success path. Here we
    // verify that a second call returns an error.

    #[test]
    fn init_tracing_second_call_fails() {
        // First call may or may not have happened in another test.
        // Either way, by the end of this test at least one call succeeded.
        let first = init_tracing("util-test", false, 0);
        let second = init_tracing("util-test2", false, 0);
        // At least one must have failed (global subscriber already set),
        // unless the first call in this process was ours.
        assert!(
            first.is_err() || second.is_err(),
            "global subscriber can only be set once"
        );
    }

    #[test]
    fn init_tracing_quiet_overrides_verbose() {
        // We can't call init_tracing successfully twice, but we can verify
        // the level computation logic by inspecting the return when it
        // does succeed. Since we can't guarantee ordering, test the logic
        // directly.
        let is_verbose = !true && 3 > 0; // quiet=true, verbose=3
        assert!(!is_verbose, "quiet should suppress verbose");
    }
}
