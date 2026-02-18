use std::path::Path;

use clap::{ArgAction, ArgMatches, Args, Command, FromArgMatches, Parser, Subcommand};
use thiserror::Error;

use std::ffi::OsString;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use tracing::{info, warn};

/// Errors returned by shell operations.
#[derive(Error, Debug)]
pub enum ShellError {
    /// An internal error that needs higher-level handling
    #[error("Internal error: {0}")]
    Internal(String),

    /// The handler did not recognize the subcommand (try the next handler)
    #[error("Command not found")]
    CommandNotFound,

    /// Catch-all for standard IO issues
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
}

/// Core trait for running the shell.
///
/// Implementations handle argument parsing, command dispatch, and VFS setup.
pub trait Shell {
    /// Parse arguments from the process environment and run the shell.
    fn run(&self) -> Result<(), ShellError>;

    /// Run the shell with the given pre-parsed argument list.
    fn run_args(&self, args: &[OsString]) -> Result<(), ShellError>;
}

type AugmentorFn = dyn Fn(Command) -> Command + Send + Sync;

/// A shared closure that augments a [`clap::Command`] with additional
/// subcommands or arguments.
pub type Augmentor = Arc<AugmentorFn>;

type HandlerFn = dyn Fn(&dyn Shell, &ArgMatches) -> Result<(), ShellError> + Send + Sync;

/// A shared closure that handles a parsed command.
///
/// Return `Ok(())` on success, [`ShellError::CommandNotFound`] to pass
/// control to the next handler, or another [`ShellError`] to abort.
pub type Handler = Arc<HandlerFn>;

/// Backend-agnostic VFS interface for the shell.
///
/// Implement this trait to plug in any filesystem backend.
pub trait Vfs: Send {
    /// Return the current working directory of this filesystem.
    fn cwd(&self) -> &Path;
}

type VfsLookupFn = dyn Fn(&ArgMatches) -> Result<Box<dyn Vfs>, ShellError> + Send + Sync;

/// A shared closure that creates a [`Vfs`] from the parsed command-line arguments.
pub type VfsLookup = Arc<VfsLookupFn>;

#[derive(Default, Clone)]
struct CommandGroup {
    args: Vec<Augmentor>,
    cmds: Vec<Augmentor>,
    hnds: Vec<Handler>,
}

struct BasicShell {
    name: String,
    pkg_name: String,
    version: String,
    cli_group: CommandGroup,
    #[allow(dead_code)] // used by future REPL mode
    shell_group: CommandGroup,
    vfs_lookup: Option<VfsLookup>,
    vfs: Mutex<Option<Box<dyn Vfs>>>,
}

/// DSL for registering subcommands, arguments, and handlers
///
/// No locks are required — all registration happens before the groups are moved into the
/// `BasicShell` struct.
///
///   - `CMDS <Type> [groups..]` — registers `<Type>::augment_subcommands`
///   - `ARGS <Type> [groups..]` — registers `<Type>::augment_args`
///   - `HNDS <fn>   [groups..]` — wraps `<fn>` in a `Handler` closure that
///     captures a `Weak<BasicShell>` (must be called inside `Arc::new_cyclic`)
///
/// # Example
///
/// ```ignore
/// add_sh!(weak => {
///     CMDS BasicSharedCommands          [ shell_group, cli_group ],
///     HNDS handle_basic_shared_command  [ shell_group, cli_group ],
///     ARGS BasicCliArgs                 [              cli_group ],
/// });
/// ```
macro_rules! add_sh {
    // Did anybody ask for a DSL here? No. But was it fun to build? YES! - @lambdanature

    // Top-level entry: $weak is a &Weak<BasicShell> from Arc::new_cyclic
    ($weak:ident => {
        $($method:ident $what:path [$($group:ident),* $(,)?] ),* $(,)?
    }) => {{
        $( add_sh!(@add $weak, $method $what [ $( $group )* ] ); )*
    }};

    // CMDS — no Weak needed
    (@add $weak:ident, CMDS $what:path [ $( $group:ident )* ] ) => {{
        type What = $what;
        let aug = Arc::new(What::augment_subcommands);
        $( $group.cmds.push(aug.clone()); )*
    }};

    // ARGS — no Weak needed
    (@add $weak:ident, ARGS $what:path [ $( $group:ident )* ] ) => {{
        type What = $what;
        let aug = Arc::new(What::augment_args);
        $( $group.args.push(aug.clone()); )*
    }};

    // HNDS — captures a Weak clone, upgrades when called
    (@add $weak:ident, HNDS $what:path [ $( $group:ident )* ] ) => {{
        let w = Weak::clone(&$weak);
        let hnd: Handler = Arc::new(move |_, m| {
            $what(&w.upgrade().expect("shell dropped while handler active"), m)
        });
        $( $group.hnds.push(hnd.clone()); )*
    }};
}

#[derive(Subcommand)]
enum BasicCliCommands {
    Shell,
}

fn handle_basic_cli_command(_sh: &BasicShell, matches: &ArgMatches) -> Result<(), ShellError> {
    match BasicCliCommands::from_arg_matches(matches) {
        Ok(BasicCliCommands::Shell) => Err(ShellError::Internal(
            "command 'shell' not implemented".into(),
        )),
        Err(_) => Err(ShellError::CommandNotFound),
    }
}

#[derive(Parser, Debug)]
struct BasicCliArgs {
    /// Suppress all output except for errors. This overrides the -v flag.
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Turn on verbose output. Supply -v multiple times to increase verbosity.
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,
}

#[derive(Subcommand)]
enum BasicShellCommands {
    Exit,
}

fn handle_basic_shell_command(_sh: &BasicShell, matches: &ArgMatches) -> Result<(), ShellError> {
    match BasicShellCommands::from_arg_matches(matches) {
        Ok(BasicShellCommands::Exit) => Ok(()),
        Err(_) => Err(ShellError::CommandNotFound),
    }
}

#[derive(Subcommand)]
enum BasicSharedCommands {
    Version,
}

fn handle_basic_shared_command(sh: &BasicShell, matches: &ArgMatches) -> Result<(), ShellError> {
    match BasicSharedCommands::from_arg_matches(matches) {
        Ok(BasicSharedCommands::Version) => {
            println!("version {} {}", sh.pkg_name, sh.version);
            Ok(())
        }
        Err(_) => Err(ShellError::CommandNotFound),
    }
}

#[derive(Subcommand)]
enum VfsSharedCommands {
    Pwd,
}

fn handle_vfs_shared_command(sh: &BasicShell, matches: &ArgMatches) -> Result<(), ShellError> {
    match VfsSharedCommands::from_arg_matches(matches) {
        Ok(VfsSharedCommands::Pwd) => {
            let vfs_guard = sh.vfs.lock().expect("vfs mutex poisoned");
            if let Some(fs) = &*vfs_guard {
                println!("{}", fs.cwd().display());
                Ok(())
            } else {
                Err(ShellError::Internal("no current cwd".into()))
            }
        }
        Err(_) => Err(ShellError::CommandNotFound),
    }
}

impl BasicShell {
    fn new(
        name: String,
        pkg_name: String,
        version: String,
        shell_group: CommandGroup,
        cli_group: CommandGroup,
        vfs_lookup: Option<VfsLookup>,
    ) -> Arc<BasicShell> {
        let has_vfs = vfs_lookup.is_some();
        let mut shell_group = shell_group;
        let mut cli_group = cli_group;

        // Build the Arc with new_cyclic so handler closures can capture a
        // Weak reference to the shell being constructed. The Weak is
        // guaranteed to upgrade successfully whenever a handler runs,
        // because the Arc owns the shell and handlers only run while it
        // is alive.
        Arc::new_cyclic(|weak: &Weak<BasicShell>| {
            add_sh!(weak => {
                CMDS BasicSharedCommands           [ shell_group, cli_group ],
                HNDS handle_basic_shared_command   [ shell_group, cli_group ],

                CMDS BasicShellCommands            [ shell_group            ],
                HNDS handle_basic_shell_command    [ shell_group            ],

                CMDS BasicCliCommands              [              cli_group ],
                ARGS BasicCliArgs                  [              cli_group ],
                HNDS handle_basic_cli_command      [              cli_group ],
            });

            if has_vfs {
                add_sh!(weak => {
                    CMDS VfsSharedCommands         [ shell_group, cli_group ],
                    HNDS handle_vfs_shared_command [ shell_group, cli_group ],
                });
            }

            BasicShell {
                name,
                pkg_name,
                version,
                shell_group,
                cli_group,
                vfs_lookup,
                vfs: Mutex::new(None),
            }
        })
    }

    fn build_cmd(&self) -> Command {
        let mut cmd = Command::new(self.name.clone())
            .subcommand_required(true)
            .arg_required_else_help(true);

        for args in &self.cli_group.args {
            cmd = (args)(cmd);
        }

        for cmds in &self.cli_group.cmds {
            cmd = (cmds)(cmd);
        }

        cmd
    }
}

static INIT_LOGGING: OnceLock<Result<(), String>> = OnceLock::new();

impl Shell for BasicShell {
    fn run(&self) -> Result<(), ShellError> {
        let mut args: Vec<OsString> = Vec::new();
        for arg in std::env::args() {
            let parsed = crate::parse::shell_parse_arg(&arg).unwrap_or_else(|e| {
                warn!("failed to parse argument {:?}: {e}, using raw value", arg);
                OsString::from(&arg)
            });
            args.push(parsed);
        }
        self.run_args(&args)
    }

    fn run_args(&self, args: &[OsString]) -> Result<(), ShellError> {
        // First, evaluate the actual command line using external argv.
        // Then we determine if we need to go into interactive mode or
        // directly execute a command from argv.
        let matches = self
            .build_cmd()
            .try_get_matches_from(args)
            .unwrap_or_else(|e| e.exit());

        let init_result = INIT_LOGGING.get_or_init(|| {
            crate::init_tracing(
                &self.name,
                matches.get_flag("quiet"),
                matches.get_count("verbose"),
            )
            .map(|(_, level_filter)| {
                info!(
                    "starting {} ({} {}), log level: {level_filter}",
                    self.name,
                    env!("CARGO_PKG_NAME"),
                    env!("CARGO_PKG_VERSION")
                );
            })
            .map_err(|e| format!("{e}"))
        });

        if let Err(e) = init_result {
            return Err(ShellError::Internal(e.clone()));
        }

        if let Some(vfs_lookup) = &self.vfs_lookup {
            let vfs = (vfs_lookup)(&matches)?;
            *self.vfs.lock().expect("vfs mutex poisoned") = Some(vfs);
        }

        for handler in &self.cli_group.hnds {
            match (handler)(self, &matches) {
                Ok(()) => return Ok(()),
                Err(ShellError::CommandNotFound) => continue,
                Err(e) => return Err(e),
            }
        }

        Err(ShellError::Internal(
            "no handler matched the command".into(),
        ))
    }
}

/// Builder for constructing a [`Shell`] instance.
///
/// Use [`shell_config!`] for a convenient starting point that automatically
/// fills in the binary name, package name, and version from Cargo metadata.
pub struct ShellConfig {
    name: String,
    pkg_name: String,
    version: String,
    cli_group: CommandGroup,
    shell_group: CommandGroup,
    vfs_lookup: Option<VfsLookup>,
}

/// Create a [`ShellConfig`] with Cargo metadata filled in automatically.
///
/// - `shell_config!()` — derives the shell name from the running binary.
/// - `shell_config!("name")` — uses the given name explicitly.
#[macro_export]
macro_rules! shell_config {
    ($name:expr) => {{ ShellConfig::new($name, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")) }};

    () => {{
        let name = esh::get_cmd_basename(env!("CARGO_BIN_NAME"));
        esh::ShellConfig::new(name, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
    }};
}

impl ShellConfig {
    /// Create a new configuration with the given name, package name, and version.
    ///
    /// Prefer [`shell_config!`] which fills these in from Cargo metadata.
    pub fn new(
        name: impl Into<String>,
        pkg_name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            pkg_name: pkg_name.into(),
            version: version.into(),
            cli_group: CommandGroup::default(),
            shell_group: CommandGroup::default(),
            vfs_lookup: None,
        }
    }

    /// Override the shell name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Register an [`Augmentor`] that adds arguments to the CLI command.
    pub fn cli_args(mut self, args: Augmentor) -> Self {
        self.cli_group.args.push(args);
        self
    }

    /// Register an [`Augmentor`] that adds subcommands to the CLI command.
    pub fn cli_cmds(mut self, cmds: Augmentor) -> Self {
        self.cli_group.cmds.push(cmds);
        self
    }

    /// Register a [`Handler`] for CLI-mode commands.
    pub fn cli_handler(mut self, handler: Handler) -> Self {
        self.cli_group.hnds.push(handler);
        self
    }

    /// Register an [`Augmentor`] that adds arguments to interactive shell commands.
    pub fn shell_args(mut self, args: Augmentor) -> Self {
        self.shell_group.args.push(args);
        self
    }

    /// Register an [`Augmentor`] that adds subcommands to the interactive shell.
    pub fn shell_cmds(mut self, cmds: Augmentor) -> Self {
        self.shell_group.cmds.push(cmds);
        self
    }

    /// Register a [`Handler`] for interactive shell commands.
    pub fn shell_handler(mut self, handler: Handler) -> Self {
        self.shell_group.hnds.push(handler);
        self
    }

    /// Set the [`VfsLookup`] closure that creates a VFS from parsed arguments.
    pub fn vfs_lookup(mut self, lookup: VfsLookup) -> Self {
        self.vfs_lookup = Some(lookup);
        self
    }

    /// Build the configured shell and return it as an `Arc<dyn Shell>`.
    pub fn build(self) -> Arc<dyn Shell + 'static> {
        BasicShell::new(
            self.name,
            self.pkg_name,
            self.version,
            self.shell_group,
            self.cli_group,
            self.vfs_lookup,
        )
    }
}
