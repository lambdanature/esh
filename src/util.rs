// TODO: use log!() in shell-less fallbacks for die/warn/info macros
// TODO: Enrich log!() output with env!("CARGO_PKG_NAME") / env!("CARGO_PKG_VERSION") /
//       file!() / line!() / column!()

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    prelude::*,
    registry::Registry,
};

use std::sync::OnceLock;

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
        // I officialy give up
        fallback.into()
    })
}

pub fn get_cmd_fallback() -> &'static String {
    get_cmd_basename(env!("CARGO_PKG_NAME"))
}

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

#[macro_export]
// There are multiple relevant crates, but this should suffice
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

pub fn init_tracing(name: impl Into<String>, quiet: bool, verbose: u8) -> (bool, LevelFilter) {
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
    tracing_log::LogTracer::init().expect("Failed to set log tracer");

    let registry = Registry::default();

    let log_env_name = format!("{}_LOG", name.into().to_uppercase());

    let env_filter = EnvFilter::builder()
        .with_default_directive(level_filter.into())
        .with_env_var(log_env_name)
        .from_env_lossy()
        .add_directive(
            "rustyline=warn"
                .parse()
                .expect("Failed to parse rustyline directive"),
        );

    let subscriber = registry.with(env_filter).with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .compact(),
    );

    tracing::subscriber::set_global_default(subscriber)
        .expect("INTERNAL ERROR: setting default tracing::subscriber failed");

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing_panic::panic_hook(info);
        prev_hook(info); // daisy-chain to old panic hook
    }));

    (is_verbose, level_filter)
}
