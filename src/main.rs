use std::env;

pub mod parse;
use parse::parse_args;

pub mod config;
pub mod sync;

fn main() {
    println!("This is rusty-sink...");

    let args: Vec<String> = env::args().collect();

    let result = parse_args(args);
    if let Err(result) = result {
        eprintln!("{}", result);
        std::process::exit(1);
    } else {
        println!("{:?}", result.unwrap());
    }
}
