use clap_sh::parse_double_quoted;
use pretty_hex::{HexConfig, PrettyHex};

fn main() {
    let mut first = true;
    for arg in std::env::args().skip(1) {
        if !first {
            println!();
        }
        first = false;

        match parse_double_quoted(&arg) {
            Ok(parsed) => {
                eprintln!("arg: {arg}");
                let cfg = HexConfig {
                    title: false,
                    width: 16,
                    group: 8,
                    ..HexConfig::default()
                };
                println!("{:?}", parsed.hex_conf(cfg));
            }
            Err(e) => eprintln!("parse error in {arg:?}: {e}"),
        }
    }
}
