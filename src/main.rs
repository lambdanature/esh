// use esh::shell_parse_arg;
// use pretty_hex::{HexConfig, PrettyHex};

// PhysicalFS

// TODO: remove and fix imports
#![allow(unused_imports)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use vfs::PhysicalFS;
use vfs::VfsPath;

use clap::{ArgAction, ArgMatches, Args, CommandFactory, Parser, Subcommand, ValueEnum};

use esh::{die, shell_config, Shell};

fn parse_to_vfs_or_die(os_str: &str) -> Result<VfsPath, String> {
    let native_path = match PathBuf::from(os_str).canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("fatal: cannot open path '{}': {}", os_str, e);
            std::process::exit(1)
        }
    };
    if !native_path.is_dir() {
        eprintln!("fatal: not a directory: '{}'", os_str);
        std::process::exit(1)
    }
    eprintln!("canonical path: {native_path:?}");
    if let Ok(root_slash_dot) = VfsPath::new(PhysicalFS::new(native_path)).join(".") {
        Ok(root_slash_dot)
    } else {
        eprintln!("fatal: internal error, can't open '.' below '{}'", os_str);
        std::process::exit(1)
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short='p', long="path", default_value=".", value_parser = parse_to_vfs_or_die,
          help="Path to open a VFS on")]
    vfs_path: VfsPath,
}

fn get_vfs_path(matches: &ArgMatches) -> Option<VfsPath> {
    matches.get_one::<VfsPath>("vfs_path").cloned()
}

fn main() {
    let cfg = shell_config!()
        .cli_args(Arc::new(CliArgs::augment_args))
        .vfs_path_lookup(Arc::new(get_vfs_path));
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
