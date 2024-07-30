use chrono::prelude::*;

use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

use super::config::Config;

pub fn run(config: &mut Config) -> Result<(), Box<dyn Error>> {
    make_lost_and_found(config)?;
    make_logfile(config)?;
    write_line(config, "Starting scan of both folders...")?;
    scan_trees()?;
    Ok(())
}

fn make_lost_and_found(config: &Config) -> Result<(), Box<dyn Error>> {
    let path: PathBuf = config.lost_and_found_path();
    std::fs::create_dir_all(path)?;
    Ok(())
}

fn make_logfile(config: &mut Config) -> Result<(), Box<dyn Error>> {
    let path = config.log_file_path();
    let mut file = std::fs::File::create(path)?;
    writeln!(
        file,
        "Rustysink log file, run started at: {}",
        config.start_time
    )?;
    writeln!(file, "Configuration: {:?}", config)?;
    config.logfile = Some(file); // make sure to save the open file into the config!
    Ok(())
}

fn write_line(config: &mut Config, line: &str) -> Result<(), Box<dyn Error>> {
    let date_as_string = Utc::now().to_string();
    let text = format!("{}: {}", date_as_string, line);
    if let Some(file) = config.logfile.as_mut() {
        writeln!(file, "{}", text)?;
    }

    if config.verbose {
        println!("{}", text);
    }
    Ok(())
}

fn scan_trees() -> Result<(), Box<dyn Error>> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{distributions::Alphanumeric, Rng};

    fn random_string() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .map(char::from)
            .collect()
    }

    /// This struct will drop at the end of each test, and it will
    /// take out the source/target folders and the logfile.
    struct TestFoldersAndLog {
        logfile: std::path::PathBuf,
        source: std::path::PathBuf,
        target: std::path::PathBuf,
        cleanup: bool,
    }

    impl Drop for TestFoldersAndLog {
        fn drop(&mut self) {
            if self.cleanup {
                let _ = std::fs::remove_dir_all(&self.source);
                let _ = std::fs::remove_dir_all(&self.target);
                let _ = std::fs::remove_file(&self.logfile);
            }
        }
    }

    impl TestFoldersAndLog {
        fn new(config: &Config) -> Result<Self, Box<dyn Error>> {
            let source = config.source.clone();
            let target = config.target.clone();
            let logfile = std::path::PathBuf::from("");
            let cleanup = false;

            std::fs::create_dir_all(&source)?;
            std::fs::create_dir_all(&target)?;

            Ok(Self {
                logfile,
                source,
                target,
                cleanup,
            })
        }
    }

    /// make a random string, use it to make a config file, use that to make source/target folders
    /// make sure these folders (and logfile name) are saved to the resources struct
    /// which will cleanup at the end of the test
    fn setup_resources() -> Result<(Config, TestFoldersAndLog), Box<dyn Error>> {
        let rand = random_string();
        let config = Config {
            source: std::path::PathBuf::from(format!("test_data/SOURCE_{}", rand)),
            target: std::path::PathBuf::from(format!("test_data/TARGET_{}", rand)),
            ..Default::default()
        };
        let resources = TestFoldersAndLog::new(&config)?;
        Ok((config, resources))
    }

    #[test]
    fn test_make_folder_and_logfile() -> Result<(), Box<dyn Error>> {
        let (mut config, mut resources) = setup_resources()?;

        // make a lost and found folder inside the target folder
        make_lost_and_found(&config)?;

        assert!(resources.source.exists());
        assert!(resources.target.exists());
        assert!(resources
            .target
            .join(format!("RUSTYSINK_LOST_AND_FOUND_{}", config.start_time))
            .exists());

        make_logfile(&mut config)?;

        assert!(config.logfile.is_some());
        assert!(config.log_file_path().exists());

        resources.cleanup = false;
        Ok(())
    }
}
