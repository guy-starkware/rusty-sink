use chrono::prelude::*;

use super::config::Config;
use std::collections::HashMap;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug)]
struct Folder {
    relpath: PathBuf,
    id: String, // concatenation of the contents of the folder
    is_orphan: bool,
    is_widow: bool,
    children: Vec<Folder>,
}

/// gets a path to a folder, and returns a vector of strings with the names of the files or folders
/// can choose to get either folders or files, or both
/// returns the vector ordered alphabetically, mixing folders and files
fn collect_names(
    path: &PathBuf,
    folders: bool,
    files: bool,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut filenames = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if (folders && path.is_dir()) || (files && path.is_file()) {
            if let Some(path) = path.file_name() {
                let new_str = path.to_string_lossy().to_string();
                filenames.push(new_str);
            }
        }
    }
    filenames.sort();
    Ok(filenames)
}

impl Folder {
    fn scan(
        config: &Config,
        relpath: PathBuf,
        orphans: &mut HashMap<String, Vec<PathBuf>>,
        widows: &mut HashMap<String, Vec<PathBuf>>,
    ) -> Result<Folder, Box<dyn Error>> {
        // println!("Scanning folder: {:?}", relpath);

        let mut folder = Folder {
            relpath: relpath.clone(),
            id: "".to_string(), // this will be overwritten a little bit later in this function
            is_orphan: !config.source.join(&relpath).is_dir(),
            is_widow: !config.target.join(&relpath).is_dir(),
            children: Vec::new(),
        };

        // id of the folder is the contents concatenated
        if !folder.is_orphan {
            // the content of the folder in source is used as identifier
            let source_children = collect_names(&config.source.join(&relpath), true, true)?;
            folder.id = source_children.join(", ");
        } else {
            // if this folder doesn't exist in the source, use the target content as identifier
            let target_children = collect_names(&config.target.join(&relpath), true, true)?;
            folder.id = target_children.join(", ");
        }

        if folder.is_orphan {
            orphans
                .entry(folder.id.clone())
                .or_default()
                .push(folder.relpath.clone());
        } else if folder.is_widow {
            widows
                .entry(folder.id.clone())
                .or_default()
                .push(folder.relpath.clone());
        } else {
            // only in case where this folder exists in both source and target, can we scan its children
            let source_children = collect_names(&config.source.join(&relpath), true, false)?;
            // println!("Source children: {:?}", source_children);
            let target_children = collect_names(&config.target.join(&relpath), true, false)?;
            // println!("Target children: {:?}", target_children);

            // merge the two lists of children
            let mut children = Vec::new();
            for child in source_children {
                children.push(child.to_string());
            }

            let mut extra_children = Vec::new();
            for child in target_children {
                extra_children.push(child.to_string());
            }

            for child in extra_children {
                if !children.contains(&child) {
                    children.push(child);
                }
            }

            // println!("Children: {:?}", children);
            children.sort(); // make sure folders are in alphabetical order
            for child in children {
                // add the children, but also recursively scan each one
                folder.children.push(Folder::scan(
                    config,
                    folder.relpath.join(&child),
                    orphans,
                    widows,
                )?);
            }
        }

        Ok(folder)
    }
}

pub fn run(config: &mut Config) -> Result<(), Box<dyn Error>> {
    make_lost_and_found(config)?;
    make_logfile(config)?;
    write_line(config, "Starting scan of both folders...")?;
    scan_trees(config)?;
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

fn scan_trees(config: &Config) -> Result<Folder, Box<dyn Error>> {
    // assumes the source and target folders exist (so neither is widow/orphan)
    let root = Folder::scan(
        config,
        PathBuf::from(""),
        &mut HashMap::new(),
        &mut HashMap::new(),
    )?;

    Ok(root)
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

            // make some folders under both the source and the target
            let folders = vec!["foo", "bar", "baz"];
            let subfolders = vec!["a", "b", "c"];
            let subfolders2 = vec!["d", "e", "f"];

            // top level are foo, bar, baz
            for folder in folders.iter() {
                let subsource = config.source.join(folder);
                let subtarget = config.target.join(folder);
                std::fs::create_dir(&subsource)?;
                std::fs::create_dir(&subtarget)?;
            }

            // inside foo, put a,b,c
            let subsource = config.source.join("foo");
            let subtarget = config.target.join("foo");
            for subfolder in subfolders.iter() {
                std::fs::create_dir(&subsource.join(subfolder))?;
                std::fs::create_dir(&subtarget.join(subfolder))?;
            }

            // inside bar, put d,e,f
            let subsource = config.source.join("bar");
            let subtarget = config.target.join("bar");
            for subfolder in subfolders2.iter() {
                std::fs::create_dir(&subsource.join(subfolder))?;
                std::fs::create_dir(&subtarget.join(subfolder))?;
            }
            Ok(Self {
                logfile,
                source,
                target,
                cleanup,
            })
        }
    }

    /// make a random string, use it to make a config struct, use that to make source/target folders
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
        let lost_and_found = resources
            .target
            .join(format!("RUSTYSINK_LOST_AND_FOUND_{}", config.start_time));

        assert!(lost_and_found.exists());
        make_logfile(&mut config)?;

        assert!(config.logfile.is_some());
        assert!(config.log_file_path().exists());

        resources.cleanup = false;
        Ok(())
    }

    #[test]
    fn test_read_identical_trees() -> Result<(), Box<dyn Error>> {
        let (config, mut resources) = setup_resources()?;

        let root = scan_trees(&config)?;
        // println!("{:#?}", root);

        assert_eq!(root.relpath, PathBuf::from(""));
        assert_eq!(root.id, "bar, baz, foo");
        assert!(!root.is_orphan);
        assert!(!root.is_widow);
        assert_eq!(root.children.len(), 3);

        assert_eq!(root.children[0].relpath, PathBuf::from("bar"));
        assert!(!root.children[0].is_orphan);
        assert!(!root.children[0].is_widow);
        assert_eq!(root.children[0].children.len(), 3);

        assert_eq!(root.children[0].children[0].relpath, PathBuf::from("bar/d"));
        assert_eq!(root.children[0].children[1].relpath, PathBuf::from("bar/e"));
        assert_eq!(root.children[0].children[2].relpath, PathBuf::from("bar/f"));

        assert_eq!(root.children[1].relpath, PathBuf::from("baz"));
        assert!(!root.children[1].is_orphan);
        assert!(!root.children[1].is_widow);
        assert_eq!(root.children[1].children.len(), 0);

        assert_eq!(root.children[2].relpath, PathBuf::from("foo"));
        assert!(!root.children[2].is_orphan);
        assert!(!root.children[2].is_widow);
        assert_eq!(root.children[2].children.len(), 3);
        assert_eq!(root.children[2].children[0].relpath, PathBuf::from("foo/a"));
        assert_eq!(root.children[2].children[1].relpath, PathBuf::from("foo/b"));
        assert_eq!(root.children[2].children[2].relpath, PathBuf::from("foo/c"));

        resources.cleanup = false;
        Ok(())
    }

    #[test]
    fn test_tree_with_orphan() -> Result<(), Box<dyn Error>> {
        let (config, mut resources) = setup_resources()?;

        // delete one folder from the source to produce an orphan
        let path = resources.source.join("foo");
        std::fs::remove_dir_all(&path)?;

        let root = scan_trees(&config)?;
        assert_eq!(root.relpath, PathBuf::from(""));
        assert!(root.id.contains("bar"));
        assert!(root.id.contains("baz"));
        assert!(!root.is_orphan);
        assert!(!root.is_widow);
        assert_eq!(root.children.len(), 3);

        assert_eq!(root.children[0].relpath, PathBuf::from("bar"));
        assert!(!root.children[0].is_orphan);
        assert!(!root.children[0].is_widow);
        assert_eq!(root.children[0].children.len(), 3);

        assert_eq!(root.children[0].children[0].relpath, PathBuf::from("bar/d"));
        assert_eq!(root.children[0].children[1].relpath, PathBuf::from("bar/e"));
        assert_eq!(root.children[0].children[2].relpath, PathBuf::from("bar/f"));

        assert_eq!(root.children[1].relpath, PathBuf::from("baz"));
        assert!(!root.children[1].is_orphan);
        assert!(!root.children[1].is_widow);
        assert_eq!(root.children[1].children.len(), 0);

        assert_eq!(root.children[2].relpath, PathBuf::from("foo"));
        assert!(root.children[2].is_orphan); // this is an orphan!
        assert!(!root.children[2].is_widow);
        assert_eq!(root.children[2].children.len(), 0);

        resources.cleanup = false;
        Ok(())
    }
}
