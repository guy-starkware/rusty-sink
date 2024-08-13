use std::env;

pub mod parse;
use parse::parse_args;

pub mod config;
pub mod sync;

fn main() {
    println!("This is rusty-sink...");

    let args: Vec<String> = env::args().collect();

    let result = parse_args(args);
    match result {
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
        Ok(mut config) => {
            let output = sync::run(&mut config);
            if let Err(output) = output {
                eprintln!("{}", output);
                std::process::exit(1);
            }
        }
    }
}
