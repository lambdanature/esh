// use esh::shell_parse_arg;
// use pretty_hex::{HexConfig, PrettyHex};

#![allow(unused_imports)]

use clap::{ArgAction, CommandFactory, Parser, Subcommand, ValueEnum};

use esh::{shell_config, Shell};

fn main() {
    let cfg = shell_config!();
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
