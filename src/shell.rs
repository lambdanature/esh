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
    ///
    /// # Errors
    ///
    /// Returns [`ShellError`] if argument parsing, tracing initialisation,
    /// VFS setup, or command dispatch fails.
    fn run(&self) -> Result<(), ShellError>;

    /// Run the shell with the given pre-parsed argument list.
    ///
    /// # Errors
    ///
    /// Returns [`ShellError`] if tracing initialisation, VFS setup, or
    /// command dispatch fails.
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
            let vfs_guard = sh
                .vfs
                .lock()
                .map_err(|e| ShellError::Internal(format!("vfs mutex poisoned: {e}")))?;
            (*vfs_guard).as_ref().map_or_else(
                || Err(ShellError::Internal("no current cwd".into())),
                |fs| {
                    println!("{}", fs.cwd().display());
                    Ok(())
                },
            )
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
    ) -> Arc<Self> {
        let has_vfs = vfs_lookup.is_some();
        let mut shell_group = shell_group;
        let mut cli_group = cli_group;

        // Build the Arc with new_cyclic so handler closures can capture a
        // Weak reference to the shell being constructed. The Weak is
        // guaranteed to upgrade successfully whenever a handler runs,
        // because the Arc owns the shell and handlers only run while it
        // is alive.
        Arc::new_cyclic(|weak: &Weak<Self>| {
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

            Self {
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
            *self
                .vfs
                .lock()
                .map_err(|e| ShellError::Internal(format!("vfs mutex poisoned: {e}")))? = Some(vfs);
        }

        for handler in &self.cli_group.hnds {
            match (handler)(self, &matches) {
                Ok(()) => return Ok(()),
                Err(ShellError::CommandNotFound) => {}
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
#[must_use]
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
    ($name:expr) => {{
        ShellConfig::new($name, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
    }};

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
    #[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn config(name: &str) -> ShellConfig {
        ShellConfig::new(name, "test-pkg", "0.0.1")
    }

    fn os(s: &str) -> OsString {
        OsString::from(s)
    }

    // -- ShellError --------------------------------------------------------

    #[test]
    fn shell_error_internal_display() {
        let e = ShellError::Internal("boom".into());
        assert_eq!(e.to_string(), "Internal error: boom");
    }

    #[test]
    fn shell_error_command_not_found_display() {
        let e = ShellError::CommandNotFound;
        assert_eq!(e.to_string(), "Command not found");
    }

    #[test]
    fn shell_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let e: ShellError = io_err.into();
        assert!(e.to_string().contains("gone"));
    }

    // -- ShellConfig builder -----------------------------------------------

    #[test]
    fn config_sets_name() {
        let sh = config("mysh").build();
        // Verify it built without panic — the name is internal, so just
        // confirm the returned Arc is usable as a trait object.
        let _: &dyn Shell = &*sh;
    }

    #[test]
    fn config_name_override() {
        let sh = config("original").name("override").build();
        let _: &dyn Shell = &*sh;
    }

    #[test]
    fn config_builder_chaining() {
        let noop_aug: Augmentor = Arc::new(|cmd| cmd);
        let noop_hnd: Handler = Arc::new(|_, _| Ok(()));

        let sh = config("chain")
            .cli_args(noop_aug.clone())
            .cli_cmds(noop_aug.clone())
            .cli_handler(noop_hnd.clone())
            .shell_args(noop_aug.clone())
            .shell_cmds(noop_aug.clone())
            .shell_handler(noop_hnd.clone())
            .build();
        let _: &dyn Shell = &*sh;
    }

    #[test]
    fn config_with_vfs_lookup() {
        struct TestFs;
        impl Vfs for TestFs {
            fn cwd(&self) -> &Path {
                Path::new("/tmp")
            }
        }

        let lookup: VfsLookup = Arc::new(|_| Ok(Box::new(TestFs)));
        let sh = config("vfssh").vfs_lookup(lookup).build();
        let _: &dyn Shell = &*sh;
    }

    // -- Built-in commands -------------------------------------------------

    #[test]
    fn builtin_version_succeeds() {
        let sh = config("test-version").build();
        let result = sh.run_args(&[os("test-version"), os("version")]);
        assert!(result.is_ok());
    }

    #[test]
    fn builtin_shell_returns_not_implemented() {
        let sh = config("test-shell").build();
        let result = sh.run_args(&[os("test-shell"), os("shell")]);
        match result {
            Err(ShellError::Internal(msg)) => {
                assert!(msg.contains("not implemented"), "unexpected: {msg}");
            }
            other => panic!("expected Internal error, got: {other:?}"),
        }
    }

    #[test]
    fn builtin_pwd_with_vfs_succeeds() {
        struct TestFs(PathBuf);
        impl Vfs for TestFs {
            fn cwd(&self) -> &Path {
                &self.0
            }
        }

        let lookup: VfsLookup = Arc::new(|_| Ok(Box::new(TestFs(PathBuf::from("/test/dir")))));
        let sh = config("test-pwd").vfs_lookup(lookup).build();
        let result = sh.run_args(&[os("test-pwd"), os("pwd")]);
        assert!(result.is_ok());
    }

    // -- Custom augmentors and handlers ------------------------------------

    #[derive(Subcommand)]
    enum CustomCmds {
        Greet,
    }

    #[test]
    fn custom_handler_is_invoked() {
        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        let cmds: Augmentor = Arc::new(CustomCmds::augment_subcommands);
        let handler: Handler = Arc::new(|_, m| {
            match CustomCmds::from_arg_matches(m) {
                Ok(CustomCmds::Greet) => {
                    CALL_COUNT.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
                Err(_) => Err(ShellError::CommandNotFound),
            }
        });

        let sh = config("custom")
            .cli_cmds(cmds)
            .cli_handler(handler)
            .build();
        let result = sh.run_args(&[os("custom"), os("greet")]);
        assert!(result.is_ok());
        assert!(CALL_COUNT.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn handler_chain_falls_through_command_not_found() {
        static SECOND_CALLED: AtomicUsize = AtomicUsize::new(0);

        let first_handler: Handler = Arc::new(|_, _| Err(ShellError::CommandNotFound));
        let second_handler: Handler = Arc::new(|_, m| {
            match BasicSharedCommands::from_arg_matches(m) {
                Ok(BasicSharedCommands::Version) => {
                    SECOND_CALLED.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
                Err(_) => Err(ShellError::CommandNotFound),
            }
        });

        let sh = config("chain")
            .cli_handler(first_handler)
            .cli_handler(second_handler)
            .build();

        let result = sh.run_args(&[os("chain"), os("version")]);
        assert!(result.is_ok());
        assert!(SECOND_CALLED.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn handler_chain_stops_on_non_command_not_found_error() {
        static SECOND_CALLED: AtomicUsize = AtomicUsize::new(0);

        let failing_handler: Handler =
            Arc::new(|_, _| Err(ShellError::Internal("fatal".into())));
        let second_handler: Handler = Arc::new(|_, _| {
            SECOND_CALLED.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let sh = config("chain-err")
            .cli_handler(failing_handler)
            .cli_handler(second_handler)
            .build();

        let result = sh.run_args(&[os("chain-err"), os("version")]);
        match result {
            Err(ShellError::Internal(msg)) => assert_eq!(msg, "fatal"),
            other => panic!("expected Internal error, got: {other:?}"),
        }
        assert_eq!(SECOND_CALLED.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn handler_chain_first_match_wins() {
        static FIRST_CALLED: AtomicUsize = AtomicUsize::new(0);
        static SECOND_CALLED: AtomicUsize = AtomicUsize::new(0);

        let first_handler: Handler = Arc::new(|_, _| {
            FIRST_CALLED.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        let second_handler: Handler = Arc::new(|_, _| {
            SECOND_CALLED.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let sh = config("first-wins")
            .cli_handler(first_handler)
            .cli_handler(second_handler)
            .build();

        let before_first = FIRST_CALLED.load(Ordering::SeqCst);
        let before_second = SECOND_CALLED.load(Ordering::SeqCst);

        let result = sh.run_args(&[os("first-wins"), os("version")]);
        assert!(result.is_ok());
        assert_eq!(FIRST_CALLED.load(Ordering::SeqCst), before_first + 1);
        assert_eq!(SECOND_CALLED.load(Ordering::SeqCst), before_second);
    }

    #[derive(Subcommand)]
    enum OrphanCmd {
        Orphan,
    }

    #[test]
    fn no_handler_match_returns_error() {
        let cmds: Augmentor = Arc::new(OrphanCmd::augment_subcommands);
        let never_handler: Handler = Arc::new(|_, _| Err(ShellError::CommandNotFound));

        let sh = config("nomatch")
            .cli_cmds(cmds)
            .cli_handler(never_handler)
            .build();

        let result = sh.run_args(&[os("nomatch"), os("orphan")]);
        match result {
            Err(ShellError::Internal(msg)) => {
                assert!(msg.contains("no handler matched"), "unexpected: {msg}");
            }
            other => panic!("expected Internal error, got: {other:?}"),
        }
    }

    // -- Custom augmentor adds arguments -----------------------------------

    #[derive(Parser, Debug)]
    struct ExtraArgs {
        #[arg(long, global = true)]
        dry_run: bool,
    }

    #[test]
    fn custom_args_augmentor_adds_flags() {
        static DRY_RUN_SEEN: AtomicUsize = AtomicUsize::new(0);

        let args_aug: Augmentor = Arc::new(ExtraArgs::augment_args);
        let handler: Handler = Arc::new(|_, m| {
            if m.get_flag("dry_run") {
                DRY_RUN_SEEN.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        });

        let sh = config("augargs")
            .cli_args(args_aug)
            .cli_handler(handler)
            .build();

        let result = sh.run_args(&[
            os("augargs"),
            os("--dry-run"),
            os("version"),
        ]);
        assert!(result.is_ok());
        assert!(DRY_RUN_SEEN.load(Ordering::SeqCst) >= 1);
    }

    // -- VFS integration ---------------------------------------------------

    #[test]
    fn vfs_lookup_error_propagates() {
        let lookup: VfsLookup =
            Arc::new(|_| Err(ShellError::Internal("vfs init failed".into())));
        let sh = config("vfsfail").vfs_lookup(lookup).build();
        let result = sh.run_args(&[os("vfsfail"), os("version")]);
        match result {
            Err(ShellError::Internal(msg)) => {
                assert!(msg.contains("vfs init failed"), "unexpected: {msg}");
            }
            other => panic!("expected Internal error, got: {other:?}"),
        }
    }

    #[test]
    fn vfs_cwd_is_accessible_from_handler() {
        static CWD_MATCHED: AtomicUsize = AtomicUsize::new(0);

        struct TestFs;
        impl Vfs for TestFs {
            fn cwd(&self) -> &Path {
                Path::new("/my/cwd")
            }
        }

        let lookup: VfsLookup = Arc::new(|_| Ok(Box::new(TestFs)));
        let sh = config("vfscwd").vfs_lookup(lookup).build();

        let result = sh.run_args(&[os("vfscwd"), os("pwd")]);
        assert!(result.is_ok());

        // pwd prints to stdout — since we got Ok, the vfs was accessed
        // successfully. Also verify via a custom handler that reads it.
        let lookup2: VfsLookup = Arc::new(|_| Ok(Box::new(TestFs)));
        let handler: Handler = Arc::new(|_, _| {
            CWD_MATCHED.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        let sh2 = config("vfscwd2")
            .vfs_lookup(lookup2)
            .cli_handler(handler)
            .build();
        let result2 = sh2.run_args(&[os("vfscwd2"), os("version")]);
        assert!(result2.is_ok());
        assert!(CWD_MATCHED.load(Ordering::SeqCst) >= 1);
    }

    // -- Verbose / quiet flags ---------------------------------------------

    #[test]
    fn verbose_flag_accepted() {
        let sh = config("test-verbose").build();
        let result = sh.run_args(&[os("test-verbose"), os("-v"), os("version")]);
        assert!(result.is_ok());
    }

    #[test]
    fn quiet_flag_accepted() {
        let sh = config("test-quiet").build();
        let result = sh.run_args(&[os("test-quiet"), os("-q"), os("version")]);
        assert!(result.is_ok());
    }

    #[test]
    fn multiple_verbose_flags_accepted() {
        let sh = config("test-vvv").build();
        let result = sh.run_args(&[
            os("test-vvv"),
            os("-vvv"),
            os("version"),
        ]);
        assert!(result.is_ok());
    }

    // -- Edge cases --------------------------------------------------------

    #[test]
    fn build_returns_arc_dyn_shell() {
        let sh: Arc<dyn Shell> = config("dyn").build();
        // Confirm it can be cloned and shared
        let sh2 = Arc::clone(&sh);
        drop(sh2);
    }

    #[test]
    fn multiple_shells_coexist() {
        let sh1 = config("shell-a").build();
        let sh2 = config("shell-b").build();
        let r1 = sh1.run_args(&[os("shell-a"), os("version")]);
        let r2 = sh2.run_args(&[os("shell-b"), os("version")]);
        assert!(r1.is_ok());
        assert!(r2.is_ok());
    }
}
