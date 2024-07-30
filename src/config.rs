use std::fs::File;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Config {
    pub config_file: Option<PathBuf>,
    pub source: PathBuf,
    pub target: PathBuf,
    pub verbose: bool,
    pub dry_run: bool,
    pub move_folders: bool,
    pub sync_files: bool,
    pub delete: bool,
    pub checksum: bool,
    pub start_time: String,
    pub logfile: Option<File>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            config_file: None,
            source: PathBuf::from(""),
            target: PathBuf::from(""),
            verbose: false,
            dry_run: false,
            move_folders: false, // TODO: change this to true when all is ready
            sync_files: false,   // TODO: change this to true when all is ready
            delete: false,       // TODO: change this to true when all is ready
            checksum: true,
            start_time: chrono::Local::now().format("%Y%m%dT%H%M%S").to_string(),
            logfile: None,
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Config::default()
    }

    pub fn lost_and_found_path(&self) -> PathBuf {
        let mut lost_and_found = self.target.clone();
        lost_and_found.push(format!("RUSTYSINK_LOST_AND_FOUND_{}", self.start_time));
        lost_and_found
    }

    pub fn log_file_path(&self) -> PathBuf {
        let mut logfile = self.target.clone();
        logfile.push(format!("rustysink_{}.log", self.start_time));
        logfile
    }
}
