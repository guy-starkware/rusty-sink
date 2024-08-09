use std::fs::File;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Config {
    pub config_file: Option<PathBuf>, // use this to pass arguments from a file (commandline arguments will override this!)
    pub source: PathBuf,              // path to the source folder (this folder is never touched)
    pub target: PathBuf, // path to the target folder (this folder is the one that will be modified)
    pub verbose: bool,   // print each action to the console
    pub dry_run: bool,   // do not actually move or copy files, just print what would be done
    pub move_folders: bool, // try to match orphan and widow folders and move them on the target before copying any data
    pub sync_files: bool,   // copy missing or outdated files and folders from source to target
    pub delete: bool, // any folders or files that are not in the source (after moving) will be moved to LOST AND FOUND
    pub keep_versions: bool, // if a file in target exists but is outdated, will keep the old version in LOST AND FOUND
    pub checksum: bool, // compare files that have a different modified data, using checksums, before deciding to copy a new version
    pub start_time: String, // timestamp automatically generated when the program starts
    pub logfile: Option<File>, // logfile pointer generated when the program starts
}

impl Default for Config {
    fn default() -> Self {
        Config {
            config_file: None,
            source: PathBuf::from(""),
            target: PathBuf::from(""),
            verbose: false,
            dry_run: false,
            move_folders: true,
            sync_files: true,
            delete: true,
            keep_versions: true,
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
