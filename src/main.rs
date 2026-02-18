// Be very strict about safety
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
#![forbid(unsafe_code)]

/// Note that in this example, the `-p` flag is trusted input —
/// do not expose it to untrusted users for security sandboxing.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{ArgMatches, Args, Parser};
use vfs_kit::{DirFS, FsBackend};

use esh::{shell_config, ShellError, Vfs};
use tracing::info;

struct DirFsVfs(DirFS);

impl Vfs for DirFsVfs {
    fn cwd(&self) -> &Path {
        self.0.cwd()
    }
}

fn parse_vfs_root(os_str: &str) -> Result<PathBuf, String> {
    let native_path = match PathBuf::from(os_str).canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return Err(format!("cannot open path '{os_str}': {e}"));
        }
    };
    if !native_path.is_dir() {
        return Err(format!("not a directory: '{os_str}'"));
    }
    Ok(native_path)
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short='p', long="path", default_value=".", value_parser = parse_vfs_root,
          help="Path to open a VFS on")]
    vfs_path: PathBuf,
}

fn create_vfs(matches: &ArgMatches) -> Result<Box<dyn Vfs>, ShellError> {
    let root_path = matches
        .get_one::<PathBuf>("vfs_path")
        .ok_or_else(|| ShellError::Internal("missing vfs_path argument".into()))?;
    let mut fs = DirFS::new(root_path).map_err(|e| {
        ShellError::Internal(format!("can't open VFS at '{}': {e}", root_path.display()))
    })?;
    info!("Created DirFS with root {root_path:?}");
    fs.set_auto_clean(false);
    Ok(Box::new(DirFsVfs(fs)))
}

fn main() -> Result<(), ShellError> {
    let cfg = shell_config!()
        .cli_args(Arc::new(CliArgs::augment_args))
        .vfs_lookup(Arc::new(create_vfs));
    let sh = cfg.build();

    sh.run()
}
