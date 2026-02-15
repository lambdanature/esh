// use vfs::VfsPath;
// PhysicalFS
use clap::{ArgAction, ArgMatches, Args, Command, FromArgMatches, Parser, Subcommand};

use std::process::ExitCode;
use std::sync::{Arc, RwLock};

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
    fn run_args(&self, args: std::slice::Iter<String>) -> ExitCode;
}

// Note: Internally, the agumentor being an Arc<> refcounted closure is
//       overkill, but we leave it in
//           (a) for symmetry purposes with Handler and
//           (b) since we don't know if an external caller might
//               need to register a closure.
type Augmentor = Arc<dyn Fn(Command) -> Command>;
type Handler = Arc<dyn Fn(&dyn Shell, &ArgMatches) -> CommandResult>;

#[derive(Clone)]
struct CommandGroup {
    args: Augmentor,
    cmds: Augmentor,
    handler: Handler,
}

pub fn noop_augmentor(cmd: Command) -> Command {
    cmd
}

struct BasicShell {
    name: String,
    pkg_name: String,
    version: String,
    cli_commands: RwLock<Vec<CommandGroup>>,
    shell_commands: RwLock<Vec<CommandGroup>>,
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

impl BasicShell {
    fn new(name: String, pkg_name: String, version: String) -> Arc<BasicShell> {
        let sh = Arc::new(BasicShell {
            shell_commands: RwLock::new(vec![]),
            cli_commands: RwLock::new(vec![]),
            name,
            pkg_name,
            version,
        });

        let shared_cmd_augmentor = Arc::new(BasicSharedCommands::augment_subcommands);
        let shared_cmd_sh = sh.clone();
        let shared_cmd_handler: Handler =
            Arc::new(move |_, m| handle_basic_shared_command(&shared_cmd_sh, m));

        let shell_cmd_augmentor = Arc::new(BasicShellCommands::augment_subcommands);
        let shell_cmd_sh = sh.clone();
        let shell_cmd_handler: Handler =
            Arc::new(move |_, m| handle_basic_shell_command(&shell_cmd_sh, m));

        let cli_arg_augmentor = Arc::new(BasicCliArgs::augment_args);
        let cli_cmd_augmentor = Arc::new(BasicCliCommands::augment_subcommands);
        let cli_cmd_sh = sh.clone();
        let cli_cmd_handler: Handler =
            Arc::new(move |_, m| handle_basic_cli_command(&cli_cmd_sh, m));

        // default internal shell commands and shared commands
        sh.shell_commands.write().unwrap().extend([
            CommandGroup {
                args: Arc::new(noop_augmentor),
                cmds: shared_cmd_augmentor.clone(),
                handler: shared_cmd_handler.clone(),
            },
            CommandGroup {
                args: Arc::new(noop_augmentor),
                cmds: shell_cmd_augmentor.clone(),
                handler: shell_cmd_handler.clone(),
            },
        ]);

        // default external cli commands and shared commands
        sh.cli_commands.write().unwrap().extend([
            CommandGroup {
                args: Arc::new(noop_augmentor),
                cmds: shared_cmd_augmentor.clone(),
                handler: shared_cmd_handler.clone(),
            },
            CommandGroup {
                args: cli_arg_augmentor,
                cmds: cli_cmd_augmentor.clone(),
                handler: cli_cmd_handler.clone(),
            },
        ]);

        sh
    }

    // // execute a line from within the shell
    // fn execute_line(&self, line: &str) {
    //     die!(self, "not implemented: execute_line {}", line);
    // }

    // fn execute_args(&self) {
    //     die!(self, "not implemented: execute_args");
    // }

    fn add_shell_commands(&self, cmds: CommandGroup) {
        self.shell_commands.write().unwrap().push(cmds);
    }

    fn add_cli_commands(&self, cmds: CommandGroup) {
        self.cli_commands.write().unwrap().push(cmds);
    }

    fn build_cmd(&self) -> Command {
        let mut cmd = Command::new(self.name.clone())
            .subcommand_required(true)
            .arg_required_else_help(true);

        // Run all registered cmd augmentors - this is where we register args and subcommands
        for cmd_group in self.cli_commands.read().unwrap().iter() {
            cmd = (cmd_group.args)(cmd); // register args for main command
            cmd = (cmd_group.cmds)(cmd); // register subcommands
        }
        cmd
    }
}

use std::sync::Once;
static INIT_LOGGING: Once = Once::new();

impl Shell for BasicShell {
    fn run(&self) -> ExitCode {
        let args: Vec<String> = std::env::args().collect();
        // TODO: escape parsing
        self.run_args(args.iter())
    }

    fn run_args(&self, args: std::slice::Iter<String>) -> ExitCode {
        // First, evaluate the actual command line using external argv.
        // Then we determine if we need to go into interactive mode or
        // directly execute a command from argv.
        let mut cmd = self.build_cmd();

        match cmd.clone().try_get_matches_from(args) {
            Ok(matches) => {
                INIT_LOGGING.call_once(|| {
                    let (_, level_filter) = crate::init_tracing(
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

                for cmds in self.cli_commands.read().unwrap().iter() {
                    match (cmds.handler)(self, &matches) {
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
    fn run_args(&self, args: std::slice::Iter<String>) -> ExitCode {
        (**self).run_args(args)
    }
}

pub struct ShellConfig {
    name: String,
    pkg_name: String,
    version: String,
    cli_commands: Vec<CommandGroup>,
    shell_commands: Vec<CommandGroup>,
    shared_commands: Vec<CommandGroup>,
    // root: Option<VfsPath>,
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
            cli_commands: vec![],
            shell_commands: vec![],
            shared_commands: vec![],
            // vfs_root: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn cli_commands(
        mut self,
        augmentor: impl Fn(Command) -> Command + 'static,
        handler: impl Fn(&dyn Shell, &ArgMatches) -> CommandResult + 'static,
    ) -> Self {
        self.cli_commands.push(CommandGroup {
            args: Arc::new(noop_augmentor),
            cmds: Arc::new(augmentor),
            handler: Arc::new(handler),
        });
        self
    }

    pub fn shell_commands(
        mut self,
        augmentor: impl Fn(Command) -> Command + 'static,
        handler: impl Fn(&dyn Shell, &ArgMatches) -> CommandResult + 'static,
    ) -> Self {
        self.shell_commands.push(CommandGroup {
            args: Arc::new(noop_augmentor),
            cmds: Arc::new(augmentor),
            handler: Arc::new(handler),
        });
        self
    }

    pub fn shared_commands(
        mut self,
        augmentor: impl Fn(Command) -> Command + 'static,
        handler: impl Fn(&dyn Shell, &ArgMatches) -> CommandResult + 'static,
    ) -> Self {
        self.shared_commands.push(CommandGroup {
            args: Arc::new(noop_augmentor),
            cmds: Arc::new(augmentor),
            handler: Arc::new(handler),
        });
        self
    }

    pub fn build(self) -> Box<dyn Shell + 'static> {
        let sh = BasicShell::new(self.name, self.pkg_name, self.version);

        for cmds in self.cli_commands {
            sh.add_cli_commands(cmds);
        }

        for cmds in self.shell_commands {
            sh.add_shell_commands(cmds);
        }

        for cmds in self.shared_commands {
            sh.add_cli_commands(cmds.clone());
            sh.add_shell_commands(cmds);
        }

        Box::new(sh) as Box<dyn Shell>
    }
}
