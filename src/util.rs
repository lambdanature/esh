// TODO: use log!() in shell-less fallbacks for die/warn/info macros
// TODO: Enrich log!() output with env!("CARGO_PKG_NAME") / env!("CARGO_PKG_VERSION") /
//       file!() / line!() / column!()

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
    ($sh:expr, $fmt:literal, $($arg:tt)*) => {{
        $sh.die(format_args!($fmt, $($arg)*));
    }};

    ($sh:expr, $msg:literal) => {{
        $sh.die(format_args!("{}", $msg));
    }};

    ($fmt:literal, $($arg:tt)*) => {{
        let basename = $crate::get_cmd_fallback();
        eprintln!("{}: Fatal error: {}", basename, format!($fmt, $($arg)*));
        std::process::exit(1);
    }};

    ($msg:literal) => {{
        let basename = $crate::get_cmd_fallback();
        eprintln!("{}: Fatal error: {}", basename, $msg);
        std::process::exit(1);
    }};

    () => {{
        let basename = $crate::get_cmd_fallback();
        eprintln!("{}: Fatal error, program terminated unexpectedly", basename);
        std::process::exit(1);
    }};
}

#[macro_export]
macro_rules! warn {
    ($sh:expr, $fmt:literal, $($arg:tt)*) => {{
        $sh.warn(format_args!($fmt, $($arg)*));
    }};

    ($sh:expr, $msg:literal) => {{
        $sh.warn(format_args!("{}", $msg));
    }};

    ($fmt:literal, $($arg:tt)*) => {{
        // let basename = get_cmd_basename();
        // eprintln!("{}: {}", basename, format!($fmt, $($arg)*));
        tracing::warn!("{}",  format!($fmt, $($arg)*));
    }};

    ($msg:literal) => {{
        // let basename = get_cmd_basename();
        // eprintln!("{}: {}", basename, $msg);
        tracing::warn!("{}", $msg);
    }};
}

#[macro_export]
macro_rules! info {
    ($sh:expr, $fmt:literal, $($arg:tt)*) => {{
        $sh.info(format_args!($fmt, $($arg)*));
    }};

    ($sh:expr, $msg:literal) => {{
        $sh.info(format_args!("{}", $msg));
    }};

    ($fmt:literal, $($arg:tt)*) => {{
        // let basename = $crate::get_cmd_fallback();
        // eprintln!("{}: {}", basename, format!($fmt, $($arg)*));
        tracing::info!("{}",  format!($fmt, $($arg)*));
    }};

    ($msg:literal) => {{
        // let basename = $crate::get_cmd_fallback();
        // eprintln!("{}: {}", basename, $msg);
        tracing::info!("{}", $msg);
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

pub fn init_tracing(quiet: bool, verbose: u8) -> (bool, LevelFilter) {
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

    let env_filter = EnvFilter::builder()
        .with_default_directive(level_filter.into())
        .with_env_var("FJALL_LOG")
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

    if tracing::subscriber::set_global_default(subscriber).is_err() {
        die!("INTERNAL ERROR: setting default tracing::subscriber failed");
    }

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing_panic::panic_hook(info);
        prev_hook(info); // daisy-chain to old panic hook
    }));

    (is_verbose, level_filter)
}
