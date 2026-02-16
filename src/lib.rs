mod parse;
mod shell;
mod util;

// TODO: real alias support

pub use parse::{ShellParseError, shell_parse_arg, shell_parse_line};
pub use shell::{Shell, ShellConfig, Vfs};
pub use util::{get_cmd_basename, get_cmd_fallback, init_tracing};
