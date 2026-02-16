use std::path::Path;

use clap::{ArgAction, ArgMatches, Args, Command, FromArgMatches, Parser, Subcommand};

use std::ffi::OsString;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, RwLock};

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

#[allow(unused_imports)]
use crate::die;

pub enum CommandResult {
    Ok,
    Error,
    NotFound,
    ExitShell(u8),
}

pub trait Shell {
    fn run(&self) -> ExitCode;
    fn run_args(&self, args: std::slice::Iter<OsString>) -> ExitCode;
}

type AugmentorFn = dyn Fn(Command) -> Command + Send + Sync;
type Augmentor = Arc<AugmentorFn>;

type HandlerFn = dyn Fn(&dyn Shell, &ArgMatches) -> CommandResult + Send + Sync;
type Handler = Arc<HandlerFn>;

/// Backend-agnostic VFS interface for the shell.
/// Implement this trait to plug in any filesystem backend.
pub trait Vfs: Send {
    fn cwd(&self) -> &Path;
}

type VfsLookupFn = dyn Fn(&ArgMatches) -> Option<Box<dyn Vfs>> + Send + Sync;
type VfsLookup = Arc<VfsLookupFn>;

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
    cli_group: RwLock<CommandGroup>,
    shell_group: RwLock<CommandGroup>,
    vfs_lookup: Option<VfsLookup>,
    vfs: Mutex<Option<Box<dyn Vfs>>>,
}

#[derive(Subcommand)]
enum BasicCliCommands {
    Shell,
}

fn handle_basic_cli_command(_sh: &BasicShell, matches: &ArgMatches) -> CommandResult {
    match BasicCliCommands::from_arg_matches(matches) {
        Ok(BasicCliCommands::Shell) => {
            die!("command 'shell' not implemented");
        }
        Err(_) => CommandResult::NotFound,
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

fn handle_basic_shell_command(_sh: &BasicShell, matches: &ArgMatches) -> CommandResult {
    match BasicShellCommands::from_arg_matches(matches) {
        Ok(BasicShellCommands::Exit) => CommandResult::ExitShell(0),
        Err(_) => CommandResult::NotFound,
    }
}

#[derive(Subcommand)]
enum BasicSharedCommands {
    Version,
}

fn handle_basic_shared_command(sh: &BasicShell, matches: &ArgMatches) -> CommandResult {
    match BasicSharedCommands::from_arg_matches(matches) {
        Ok(BasicSharedCommands::Version) => {
            println!("version {} {}", sh.pkg_name, sh.version);
            CommandResult::Ok
        }
        Err(_) => CommandResult::NotFound,
    }
}

#[derive(Subcommand)]
enum VfsSharedCommands {
    Pwd,
}

fn handle_vfs_shared_command(sh: &BasicShell, matches: &ArgMatches) -> CommandResult {
    match VfsSharedCommands::from_arg_matches(matches) {
        Ok(VfsSharedCommands::Pwd) => {
            let vfs_guard = sh.vfs.lock().unwrap();
            if let Some(fs) = &*vfs_guard {
                println!("{}", fs.cwd().display());
                CommandResult::Ok
            } else {
                eprintln!("Error: no current cwd");
                CommandResult::Error
            }
        }
        Err(_) => CommandResult::NotFound,
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
        let sh = Arc::new(BasicShell {
            name,
            pkg_name,
            version,
            shell_group: RwLock::new(shell_group),
            cli_group: RwLock::new(cli_group),
            vfs_lookup,
            vfs: Mutex::new(None),
        });

        {
            let sh_cap = sh.clone();
            let cmds = Arc::new(BasicSharedCommands::augment_subcommands);
            let handler: Handler = Arc::new(move |_, m| handle_basic_shared_command(&sh_cap, m));
            sh.shell_group.write().unwrap().cmds.push(cmds.clone());
            sh.shell_group.write().unwrap().hnds.push(handler.clone());
            sh.cli_group.write().unwrap().cmds.push(cmds);
            sh.cli_group.write().unwrap().hnds.push(handler);
        }

        {
            let sh_cap = sh.clone();
            let cmds = Arc::new(BasicShellCommands::augment_subcommands);
            let handler: Handler = Arc::new(move |_, m| handle_basic_shell_command(&sh_cap, m));
            sh.shell_group.write().unwrap().cmds.push(cmds);
            sh.shell_group.write().unwrap().hnds.push(handler);
        }

        {
            let sh_cap = sh.clone();
            let args = Arc::new(BasicCliArgs::augment_args);
            let cmds = Arc::new(BasicCliCommands::augment_subcommands);
            let handler: Handler = Arc::new(move |_, m| handle_basic_cli_command(&sh_cap, m));
            sh.cli_group.write().unwrap().args.push(args);
            sh.cli_group.write().unwrap().cmds.push(cmds);
            sh.cli_group.write().unwrap().hnds.push(handler);
        }

        if let Some(_vfs_lookup) = &sh.vfs_lookup {
            let sh_cap = sh.clone();
            let cmds = Arc::new(VfsSharedCommands::augment_subcommands);
            let handler: Handler = Arc::new(move |_, m| handle_vfs_shared_command(&sh_cap, m));
            sh.cli_group.write().unwrap().cmds.push(cmds);
            sh.cli_group.write().unwrap().hnds.push(handler);
        }

        sh
    }

    // // execute a line from within the shell
    // fn execute_line(&self, line: &str) {
    //     die!(self, "not implemented: execute_line {}", line);
    // }

    // fn execute_args(&self) {
    //     die!(self, "not implemented: execute_args");
    // }

    fn build_cmd(&self) -> Command {
        let mut cmd = Command::new(self.name.clone())
            .subcommand_required(true)
            .arg_required_else_help(true);

        let cli_group = self.cli_group.read().unwrap();

        // Run all registered arg augmentors - this is where we register args
        for args in &cli_group.args {
            cmd = (args)(cmd);
        }

        // Run all registered cmd augmentors - this is where we register subcommands
        for cmds in &cli_group.cmds {
            cmd = (cmds)(cmd);
        }

        cmd
    }
}

use std::sync::Once;
static INIT_LOGGING: Once = Once::new();

impl Shell for BasicShell {
    fn run(&self) -> ExitCode {
        let mut args: Vec<OsString> = Vec::new();
        for arg in std::env::args() {
            if let Ok(parsed_arg) = crate::parse::shell_parse_arg(&arg) {
                args.push(parsed_arg);
            }
        }
        self.run_args(args.iter())
    }

    fn run_args(&self, args: std::slice::Iter<OsString>) -> ExitCode {
        // First, evaluate the actual command line using external argv.
        // Then we determine if we need to go into interactive mode or
        // directly execute a command from argv.
        let mut cmd = self.build_cmd();

        match cmd.clone().try_get_matches_from(args) {
            Ok(matches) => {
                INIT_LOGGING.call_once(|| {
                    let (_, level_filter) = crate::init_tracing(
                        &self.name,
                        matches.get_flag("quiet"),
                        matches.get_count("verbose"),
                    );
                    info!(
                        "starting {} ({} {}), log level: {level_filter}",
                        cmd.get_name(),
                        env!("CARGO_PKG_NAME"),
                        env!("CARGO_PKG_VERSION")
                    );
                });

                if let Some(vfs_lookup) = &self.vfs_lookup {
                    if let Some(vfs) = (vfs_lookup)(&matches) {
                        *self.vfs.lock().unwrap() = Some(vfs);
                    } else {
                        die!("Internal error: Can't retrieve vfs");
                    }
                }

                for handler in &self.cli_group.read().unwrap().hnds {
                    match (handler)(self, &matches) {
                        CommandResult::Ok => return ExitCode::SUCCESS,
                        CommandResult::Error => return ExitCode::FAILURE,
                        CommandResult::ExitShell(code) => return ExitCode::from(code),
                        CommandResult::NotFound => continue,
                    }
                }
            }
            Err(_) => {
                // Fall-through to print help
            }
        }
        // Fall-through, we haven't found suitable command handler
        if cmd.print_help().is_err() {
            die!("internal error, failed to print help");
        }
        ExitCode::FAILURE
    }
}

impl Shell for Arc<BasicShell> {
    fn run(&self) -> ExitCode {
        (**self).run()
    }
    fn run_args(&self, args: std::slice::Iter<OsString>) -> ExitCode {
        (**self).run_args(args)
    }
}

pub struct ShellConfig {
    name: String,
    pkg_name: String,
    version: String,
    cli_group: CommandGroup,
    shell_group: CommandGroup,
    vfs_lookup: Option<VfsLookup>,
}

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

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn cli_args(mut self, args: Augmentor) -> Self {
        self.cli_group.args.push(args);
        self
    }

    pub fn cli_cmds(mut self, cmds: Augmentor) -> Self {
        self.cli_group.cmds.push(cmds);
        self
    }

    pub fn cli_handler(mut self, handler: Handler) -> Self {
        self.cli_group.hnds.push(handler);
        self
    }

    pub fn shell_args(mut self, args: Augmentor) -> Self {
        self.shell_group.args.push(args);
        self
    }

    pub fn shell_cmds(mut self, cmds: Augmentor) -> Self {
        self.shell_group.cmds.push(cmds);
        self
    }

    pub fn shell_handler(mut self, handler: Handler) -> Self {
        self.shell_group.hnds.push(handler);
        self
    }

    pub fn vfs_lookup(mut self, lookup: VfsLookup) -> Self {
        self.vfs_lookup = Some(lookup);
        self
    }

    pub fn build(self) -> Arc<dyn Shell + 'static> {
        let sh = BasicShell::new(
            self.name,
            self.pkg_name,
            self.version,
            self.shell_group,
            self.cli_group,
            self.vfs_lookup,
        );

        Arc::new(sh) as Arc<dyn Shell>
    }
}
