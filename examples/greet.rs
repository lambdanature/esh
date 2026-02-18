use esh::prelude::*;

#[derive(Args)]
struct HelloArgs {
    #[arg(default_value = "world")]
    name: String,
}

#[derive(Args)]
struct ByeArgs {
    #[arg(default_value = "blackbird")]
    name: String,

    #[arg(short = 'b', long = "bye", action = clap::ArgAction::Count, default_value_t = 0)]
    count: u8,
}

#[derive(Subcommand)]
enum HelloCommands {
    Hello(HelloArgs),
    Bye(ByeArgs),
}

fn handle(_sh: &dyn Shell, matches: &ArgMatches) -> HandlerResult {
    match HelloCommands::from_arg_matches(matches) {
        Ok(HelloCommands::Hello(args)) => {
            println!("Hello, {}!", args.name);
            HANDLER_SUCCESS
        }
        Ok(HelloCommands::Bye(args)) => {
            let bye = "bye, ".repeat(args.count as usize + 1);
            println!("Bye, {}{}!", bye, args.name);
            HANDLER_SUCCESS
        }
        Err(_) => Err(ShellError::CommandNotFound),
    }
}

fn main() -> Result<ExitCode, ShellError> {
    shell_config!("hello")
        .cli_cmds(Arc::new(HelloCommands::augment_subcommands))
        .cli_handler(Arc::new(handle))
        .build()
        .run()
}
