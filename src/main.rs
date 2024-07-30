use std::env;
use std::error::Error;
use std::fmt;
use std::fs;

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

#[derive(Debug)]
struct Config {
    config_file: Option<String>,
    source: String,
    target: String,
    verbose: bool,
    dry_run: bool,
    move_folders: bool,
    sync_files: bool,
    delete: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            config_file: None,
            source: String::from(""),
            target: String::from(""),
            verbose: false,
            dry_run: false,
            move_folders: false, // TODO: change this to true when all is ready
            sync_files: false,   // TODO: change this to true when all is ready
            delete: false,       // TODO: change this to true when all is ready
        }
    }
}

impl Config {
    fn new() -> Self {
        Config::default()
    }
}

#[derive(Debug)]
struct ParseError {
    message: String,
}

impl ParseError {
    fn new(message: String) -> Self {
        ParseError { message }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ParseError {
    fn description(&self) -> &str {
        &self.message
    }
}

/// Convert a string to a boolean, using "true", "yes", "on", "1" for true,
/// and "false", "no", "off", "0" for false.
/// Cther values will return a ParseError
fn parse_bool(arg: &str) -> Result<bool, ParseError> {
    match arg.trim().to_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        _ => Err(ParseError::new(format!("Invalid boolean value {arg}"))),
    }
}

/// Ingest commandline arguments. If file:path/to/config/file is given
/// will first apply the config file, and the OVERWRITE with commandline arguments.
fn parse_args(args: Vec<String>) -> Result<Config, Box<dyn Error>> {
    if args.len() < 2 {
        help();
    }
    let mut config = Config::new();
    // first we scan for the "file:..." argument, and apply the config file
    for arg in args.iter().skip(1) {
        if let Some(end) = arg.strip_prefix("file:") {
            config.config_file = Some(end.to_string());
            config = read_config(config)?;
        }
        if arg == "help" {
            help();
        }
    }
    // then we apply the commandline arguments
    for arg in args.iter().skip(1) {
        if arg.starts_with("file:") {
            continue;
        }
        apply_key_value_pair(&mut config, arg)?;
    }

    Ok(config)
}

/// Go over the config file and load any key-value pairs into the config struct.
fn read_config(mut config: Config) -> Result<Config, Box<dyn Error>> {
    let contents = fs::read_to_string(config.config_file.clone().unwrap())?;
    for line in contents.lines() {
        apply_key_value_pair(&mut config, line)?;
    }
    Ok(config)
}

/// Read one string composed of key:value (where value is optional) and parse it into the config struct.
/// For boolean values, not specifying the value will assume TRUE.
/// For other values, must specify the value after the colon.
fn apply_key_value_pair(config: &mut Config, line: &str) -> Result<(), Box<dyn Error>> {
    let mut parts = line.split(':');
    if let Some(key) = parts.next() {
        if let Some(value) = parts.next() {
            match key.trim() {
                "source" => config.source = value.to_string(),
                "target" => config.target = value.to_string(),
                "verbose" => config.verbose = parse_bool(value)?,
                "dry_run" => config.dry_run = parse_bool(value)?,
                "move_folders" => config.move_folders = parse_bool(value)?,
                "sync_files" => config.sync_files = parse_bool(value)?,
                "delete" => config.delete = parse_bool(value)?,
                _ => {
                    return Err(Box::new(ParseError::new(format!(
                        "Invalid key value pair: {}:{}",
                        key, value
                    ))))
                }
            }
        } else {
            // "positive approach": have option to specify just the key, and assume value is TRUE if not specified!
            match key.trim() {
                "verbose" => config.verbose = true,
                "dry_run" => config.dry_run = true,
                "move_folders" => config.move_folders = true,
                "sync_files" => config.sync_files = true,
                "delete" => config.delete = true,
                _ => return Err(Box::new(ParseError::new(format!("Invalid key: {}", key)))),
            }
        }
    }
    Ok(())
}

/// This is called in cases where no variables are given, or when using the command "help".
fn help() {
    println!("Usage: rusty-sink <command>");
    println!("Commands:");
    println!(" - file:<path/to/config/file>  : Apply the config file, and overwrite with commandline arguments.");
    println!(" - source:<path/to/source>     : Specify the source folder.");
    println!(" - target:<path/to/target>     : Specify the target folder.");
    println!(" - verbose:<true|false>        : Specify verbose mode, will output the log file to stdout as well as to log file. ");
    println!(" - dry_run:<true|false>        : Specify dry-run mode, only produce log file (and optional verbose output), does not touch files. ");
    println!(" - move_folders:<true|false>   : Before syncing files, will try to find and updated moved folders with the same file list. ");
    println!(" - sync_files:<true|false>     : Will sync any outdated and changed files from source to target. ");
    println!(" - delete: <true|false>        : Will delete (move to LOST+FOUND) any files in target that are not in source. ");
    println!(" - help                        : Show this help message");
    println!();
    println!("Note that this will never change the source folder, only the target folder.");
    println!("Note that files or folders not found on source, but found on target, will be moved to LOST+FOUND, if using delete:true.");
    println!();
    println!("Default config: {:?}", Config::new());
    std::process::exit(0);
}
