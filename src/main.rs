// use esh::shell_parse_arg;
// use pretty_hex::{HexConfig, PrettyHex};

// PhysicalFS

// TODO: remove and fix imports
#![allow(unused_imports)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{ArgAction, ArgMatches, Args, CommandFactory, Parser, Subcommand, ValueEnum};
use vfs_kit::{DirFS, FsBackend};

use esh::{die, shell_config, Shell, Vfs};

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
            return Err(format!("cannot open path '{}': {}", os_str, e));
        }
    };
    if !native_path.is_dir() {
        return Err(format!("not a directory: '{}'", os_str));
    }
    eprintln!("canonical path: {native_path:?}");
    Ok(native_path)
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short='p', long="path", default_value=".", value_parser = parse_vfs_root,
          help="Path to open a VFS on")]
    vfs_path: PathBuf,
}

fn create_vfs(matches: &ArgMatches) -> Option<Box<dyn Vfs>> {
    let root_path = matches.get_one::<PathBuf>("vfs_path")?;
    match DirFS::new(root_path) {
        Ok(mut fs) => {
            fs.set_auto_clean(false);
            Some(Box::new(DirFsVfs(fs)))
        }
        Err(e) => {
            eprintln!("fatal: can't open VFS at '{}': {}", root_path.display(), e);
            std::process::exit(1);
        }
    }
}

fn main() {
    let cfg = shell_config!()
        .cli_args(Arc::new(CliArgs::augment_args))
        .vfs_lookup(Arc::new(create_vfs));
    let sh = cfg.build();

    sh.run();

    // let mut first = true;
    // for arg in std::env::args().skip(1) {
    //     if !first {
    //         println!();
    //     }
    //     first = false;

    //     match shell_parse_arg(&arg) {
    //         Ok(parsed) => {
    //             eprintln!("arg: {arg}");
    //             let cfg = HexConfig {
    //                 title: false,
    //                 width: 16,
    //                 group: 8,
    //                 ..HexConfig::default()
    //             };
    //             if let Ok(s) = String::from_utf8(parsed.to_owned()) {
    //                 println!("{}", s);
    //             } else {
    //                 println!("Not a valid UTF8 string");
    //             }
    //             println!("{:?}", parsed.hex_conf(cfg));
    //         }
    //         Err(e) => eprintln!("parse error in {arg:?}: {e}"),
    //     }
    // }
}
