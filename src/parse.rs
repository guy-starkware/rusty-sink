use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use super::config::Config;

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
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
pub fn parse_args(args: Vec<String>) -> Result<Config, Box<dyn Error>> {
    if args.len() < 2 {
        help();
    }
    let mut config = Config::new();
    // first we scan for the "file:..." argument, and apply the config file
    let mut seen_file = false;
    for arg in args.iter().skip(1) {
        if let Some(end) = arg.strip_prefix("file:") {
            if seen_file {
                return Err(Box::new(ParseError::new(
                    "Cannot specify more than one config file".to_string(),
                )));
            }
            config.config_file = Some(PathBuf::from(end));
            config = read_config_file(config)?;
            seen_file = true;
        }
        if arg == "help" {
            help();
        }
    }
    // then we apply the commandline arguments
    let mut seen_keys = vec![];
    for arg in args.iter().skip(1) {
        if arg.starts_with("file:") {
            continue;
        }
        let new_key = apply_key_value_pair(&mut config, arg)?;
        if !new_key.is_empty() {
            if seen_keys.contains(&new_key) {
                return Err(Box::new(ParseError::new(format!(
                    "Repeated key in argument list: {}",
                    new_key
                ))));
            }
            seen_keys.push(new_key);
        }
    }

    // check the source and target folders exist
    check_config_and_folders(&config)?;

    Ok(config)
}

/// Go over the config file and load any key-value pairs into the config struct.
fn read_config_file(mut config: Config) -> Result<Config, Box<dyn Error>> {
    let contents = fs::read_to_string(config.config_file.clone().unwrap())?;
    let mut seen_keys = vec![];
    for line in contents.lines().filter(|x| !x.trim().is_empty()) {
        let new_key = apply_key_value_pair(&mut config, line)?;

        if !new_key.is_empty() {
            if seen_keys.contains(&new_key) {
                return Err(Box::new(ParseError::new(format!(
                    "Repeated key in config file: {}",
                    new_key
                ))));
            }
            seen_keys.push(new_key);
        }
    }
    Ok(config)
}

/// Read one string composed of key:value (where value is optional) and parse it into the config struct.
/// For boolean values, not specifying the value will assume TRUE.
/// For other values, must specify the value after the colon.
fn apply_key_value_pair(config: &mut Config, line: &str) -> Result<String, Box<dyn Error>> {
    let mut parts = line.split(':');
    let output;
    if let Some(key) = parts.next() {
        output = key.trim();
        if let Some(value) = parts.next() {
            match output {
                "source" => config.source = PathBuf::from(value.trim()),
                "target" => config.target = PathBuf::from(value.trim()),
                "verbose" => config.verbose = parse_bool(value)?,
                "dry_run" => config.dry_run = parse_bool(value)?,
                "move_folders" => config.move_folders = parse_bool(value)?,
                "sync_files" => config.sync_files = parse_bool(value)?,
                "delete" => config.delete = parse_bool(value)?,
                "checksum" => config.checksum = parse_bool(value)?,
                _ => {
                    return Err(Box::new(ParseError::new(format!(
                        "Invalid key value pair: {}:{}",
                        key, value
                    ))))
                }
            }
        } else {
            // "positive approach": have option to specify just the key, and assume value is TRUE if not specified!
            match output {
                "source" => {
                    return Err(Box::new(ParseError::new(
                        "Missing value for source (use source:/path/to/source)".to_string(),
                    )))
                }
                "target" => {
                    return Err(Box::new(ParseError::new(
                        "Missing value for target (use target:/path/to/target)".to_string(),
                    )))
                }
                "verbose" => config.verbose = true,
                "dry_run" => config.dry_run = true,
                "move_folders" => config.move_folders = true,
                "sync_files" => config.sync_files = true,
                "delete" => config.delete = true,
                "checksum" => config.checksum = true,
                _ => return Err(Box::new(ParseError::new(format!("Invalid key: {}", key)))),
            }
        }
    } else {
        output = "";
    }
    Ok(output.to_string())
}

fn check_config_and_folders(config: &Config) -> Result<(), Box<dyn Error>> {
    if config.source.to_str().unwrap_or("").is_empty() {
        return Err(Box::new(ParseError::new(
            "Source folder not specified".to_string(),
        )));
    }
    if config.target.to_str().unwrap_or("").is_empty() {
        return Err(Box::new(ParseError::new(
            "Target folder not specified".to_string(),
        )));
    }
    if !config.source.is_dir() {
        return Err(Box::new(ParseError::new(format!(
            "Source folder not found: {:?}",
            config.source
        ))));
    }
    if !config.target.is_dir() {
        return Err(Box::new(ParseError::new(format!(
            "Target folder not found: {:?}",
            config.target
        ))));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::prelude::*;
    use std::sync::Once;

    static START: Once = Once::new();
    //Sure to run this once
    fn setup_tests() {
        START.call_once(|| {
            setup_folders();
        });
    }

    fn setup_folders() {
        println!("Setting up folders for tests...");
        let mut test_data_dir = std::env::current_dir().unwrap();
        println!("This is the current directory: {:?}", test_data_dir);
        test_data_dir.push(PathBuf::from("test_data"));
        if test_data_dir.is_dir() {
            let paths = test_data_dir.read_dir().unwrap();
            for path in paths {
                if let Ok(path) = path {
                    if !path.file_name().to_str().unwrap().starts_with("SOURCE")
                        && !path.file_name().to_str().unwrap().starts_with("TARGET")
                    {
                        panic!("Cannot empty the test_data dir, it contains files or folders that aren't SOURCE or TARGET");
                    }
                }
            }
            println!("The data dir contains only SOURCE and TARGET folders, we can clear it!");
            let _ = std::fs::remove_dir_all(&test_data_dir);
        } else if test_data_dir.is_file() {
            panic!("test_data is a file, not a directory!");
        }
        std::fs::create_dir(&test_data_dir).unwrap();
        std::fs::create_dir(test_data_dir.join("SOURCE")).unwrap();
        std::fs::create_dir(test_data_dir.join("TARGET")).unwrap();
    }

    #[test]
    fn test_parsing_good_arguments() -> Result<(), Box<dyn Error>> {
        setup_tests();
        let args = vec![
            "rusty-sink".to_string(),
            "source:test_data/SOURCE".to_string(),
            "target:test_data/TARGET".to_string(),
            "verbose:true".to_string(),
            "dry_run:true".to_string(),
            "move_folders:true".to_string(),
            "sync_files:true".to_string(),
            "delete:true".to_string(),
            "checksum:true".to_string(),
        ];
        let config = parse_args(args)?;
        assert_eq!(config.source, PathBuf::from("test_data/SOURCE"));
        assert_eq!(config.target, PathBuf::from("test_data/TARGET"));
        assert_eq!(config.verbose, true);
        assert_eq!(config.dry_run, true);
        assert_eq!(config.move_folders, true);
        assert_eq!(config.sync_files, true);
        assert_eq!(config.delete, true);
        assert_eq!(config.checksum, true);
        Ok(())
    }

    #[test]
    fn test_adding_whitespace() -> Result<(), Box<dyn Error>> {
        setup_tests();
        let args = vec![
            " rusty-sink ".to_string(),
            " source:test_data/SOURCE ".to_string(),
            " target : test_data/TARGET ".to_string(),
            "  verbose : true ".to_string(),
            "   dry_run  :  true  ".to_string(),
        ];
        let config = parse_args(args)?;
        assert_eq!(config.source, PathBuf::from("test_data/SOURCE"));
        assert_eq!(config.target, PathBuf::from("test_data/TARGET"));
        assert_eq!(config.verbose, true);
        assert_eq!(config.dry_run, true);
        Ok(())
    }

    #[test]
    fn test_parsing_different_booleans() -> Result<(), Box<dyn Error>> {
        setup_tests();
        let args = vec![
            "rusty-sink".to_string(),
            "source:test_data/SOURCE".to_string(),
            "target:test_data/TARGET".to_string(),
            "verbose".to_string(),          // no value, should default to true
            "dry_run:True".to_string(),     // true is true
            "move_folders:yes".to_string(), // yes is also true
            "sync_files:FALSE".to_string(), // false is false
            // deliberately skip "delete" to test default value
            "checksum:0".to_string(), // parse 0 as false
        ];

        let config = parse_args(args)?;
        assert_eq!(config.source, PathBuf::from("test_data/SOURCE"));
        assert_eq!(config.target, PathBuf::from("test_data/TARGET"));
        assert_eq!(config.verbose, true);
        assert_eq!(config.dry_run, true);
        assert_eq!(config.move_folders, true);
        assert_eq!(config.sync_files, false);
        assert_eq!(config.delete, false);
        assert_eq!(config.checksum, false);

        Ok(())
    }

    #[test]
    fn test_failure_to_parse_bad_source_target() -> Result<(), Box<dyn Error>> {
        setup_tests();
        let args = vec![
            "rusty-sink".to_string(),
            "source:test_data/X".to_string(),
            "target:test_data/TARGET".to_string(),
        ];

        if let Err(e) = parse_args(args) {
            assert_eq!(e.to_string(), "Source folder not found: \"test_data/X\"");
        } else {
            panic!("Expected an error, but got success!");
        }

        let args = vec![
            "rusty-sink".to_string(),
            "source:test_data/SOURCE".to_string(),
            "target:test_data/X".to_string(),
        ];

        if let Err(e) = parse_args(args) {
            assert_eq!(e.to_string(), "Target folder not found: \"test_data/X\"");
        } else {
            panic!("Expected an error, but got success!");
        }

        Ok(())
    }

    #[test]
    fn test_failure_to_parse_missing_source_target() -> Result<(), Box<dyn Error>> {
        setup_tests();
        let args = vec![
            "rusty-sink".to_string(),
            "target:test_data/TARGET".to_string(),
        ];

        if let Err(e) = parse_args(args) {
            assert_eq!(e.to_string(), "Source folder not specified");
        } else {
            panic!("Expected an error, but got success!");
        }

        let args = vec![
            "rusty-sink".to_string(),
            "source:test_data/SOURCE".to_string(),
        ];

        if let Err(e) = parse_args(args) {
            assert_eq!(e.to_string(), "Target folder not specified");
        } else {
            panic!("Expected an error, but got success!");
        }

        Ok(())
    }

    #[test]
    fn test_failure_to_parse_boolean_value() -> Result<(), Box<dyn Error>> {
        setup_tests();
        let args = vec![
            "rusty-sink".to_string(),
            "source:test_data/SOURCE".to_string(),
            "target:test_data/TARGET".to_string(),
            "verbose:foobar".to_string(),
        ];

        if let Err(e) = parse_args(args) {
            assert_eq!(e.to_string(), "Invalid boolean value foobar");
        } else {
            panic!("Expected an error, but got success!");
        }

        Ok(())
    }

    #[test]
    fn test_failure_to_parse_repeated_option() -> Result<(), Box<dyn Error>> {
        setup_tests();
        let args = vec![
            "rusty-sink".to_string(),
            "source:test_data/SOURCE".to_string(),
            "target:test_data/TARGET".to_string(),
            "verbose:true".to_string(),
            "verbose:true".to_string(),
        ];

        if let Err(e) = parse_args(args) {
            assert_eq!(e.to_string(), "Repeated key in argument list: verbose");
        } else {
            panic!("Expected an error, but got success!");
        }

        Ok(())
    }

    struct AutoDeleteThisFile {
        file: PathBuf,
    }

    impl Drop for AutoDeleteThisFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.file);
        }
    }

    #[test]
    fn test_failure_to_parse_repeated_config_file() -> Result<(), Box<dyn Error>> {
        setup_tests();

        let mut file = File::create("test_data/configuration_repeated.txt")?;
        let _autodelete = AutoDeleteThisFile {
            file: PathBuf::from("test_data/configuration_repeated.txt"),
        };
        file.write_all(b"source:test_data/SOURCE\ntarget:test_data/TARGET\nverbose:TRUE\n")?;

        let args = vec![
            "rusty-sink".to_string(),
            "file:test_data/configuration_repeated.txt".to_string(),
            "file:test_data/configuration_repeated.txt".to_string(),
        ];

        if let Err(e) = parse_args(args) {
            assert_eq!(e.to_string(), "Cannot specify more than one config file");
        } else {
            panic!("Expected an error, but got success!");
        }

        let mut file = File::create("test_data/configuration_repeated.txt")?;
        let _autodelete = AutoDeleteThisFile {
            file: PathBuf::from("test_data/configuration_repeated.txt"),
        };
        file.write_all(b"source:test_data/SOURCE\ntarget:test_data/TARGET\nverbose:TRUE\nsource:test_data/SOURCE")?; // added duplicate source line

        let args = vec![
            "rusty-sink".to_string(),
            "file:test_data/configuration_repeated.txt".to_string(),
        ];

        if let Err(e) = parse_args(args) {
            assert_eq!(e.to_string(), "Repeated key in config file: source");
        } else {
            panic!("Expected an error, but got success!");
        }

        Ok(())
    }

    #[test]
    fn test_read_config_file() -> Result<(), Box<dyn Error>> {
        setup_tests();
        let mut file = File::create("test_data/configuration.txt")?;
        let _autodelete = AutoDeleteThisFile {
            file: PathBuf::from("test_data/configuration.txt"),
        };
        file.write_all(
            b" source:test_data/SOURCE \n target : test_data/TARGET \n verbose : TRUE \n \n
        dry_run: False \n move_folders: 0 \n sync_files: yes \n delete: no \n ",
        )?;

        let args = vec![
            "rusty-sink".to_string(),
            "file:test_data/configuration.txt".to_string(),
        ];
        let config = parse_args(args)?;
        assert_eq!(config.source, PathBuf::from("test_data/SOURCE"));
        assert_eq!(config.target, PathBuf::from("test_data/TARGET"));
        assert_eq!(config.verbose, true);
        assert_eq!(config.dry_run, false);
        assert_eq!(config.move_folders, false);
        assert_eq!(config.sync_files, true);
        assert_eq!(config.delete, false);
        assert_eq!(config.checksum, true); // this is from the default config

        Ok(())
    }
}
