use chrono::prelude::*;

use std::collections::HashMap;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

use super::config::Config;

#[derive(Debug)]
struct Folder {
    relpath: PathBuf,
    id: String, // concatenation of the contents of the folder
    is_orphan: bool,
    is_widow: bool,
    children: Vec<Folder>,
}

impl Folder {
    fn scan(
        config: &Config,
        relpath: PathBuf,
        orphans: &mut HashMap<String, Vec<PathBuf>>,
        widows: &mut HashMap<String, Vec<PathBuf>>,
    ) -> Result<Folder, Box<dyn Error>> {
        // println!("Scanning folder: {:?}", relpath);
        let source_children = std::fs::read_dir(config.source.join(&relpath))?;
        let target_children = std::fs::read_dir(config.target.join(&relpath))?;

        let mut folder = Folder {
            relpath: relpath.clone(),
            id: "".to_string(), // this will be overwritten soon
            is_orphan: !config.source.join(&relpath).is_dir(),
            is_widow: !config.target.join(&relpath).is_dir(),
            children: Vec::new(),
        };

        // id of the folder is the contents concatenated
        if !folder.is_orphan {
            // the content of the folder in source is used as identifier
            for subpath in source_children {
                folder
                    .id
                    .push_str(&(subpath?.file_name().to_string_lossy().to_string() + ", "));
            }
        } else {
            // if this folder doesn't exist in the source, use the target content as identifier
            for subpath in target_children {
                folder
                    .id
                    .push_str(&(subpath?.file_name().to_string_lossy().to_string() + ", "));
            }
        }

        // reproduce the list of children, because it is nearly impossible to clone() this iterator for some reason!
        let source_children = std::fs::read_dir(config.source.join(&relpath))?;
        let target_children = std::fs::read_dir(config.target.join(&relpath))?;

        // merge the two lists of children, keeping only folders, and only the path relative to the current folder
        let mut children = Vec::new();
        for child in source_children {
            let child = child?
                .path()
                .strip_prefix(&config.source.join(&folder.relpath))?
                .to_owned();
            if config.source.join(&relpath).join(&child).is_dir() {
                children.push(child);
            }
        }
        let mut extra_children = Vec::new();
        for child in target_children {
            let child = child?
                .path()
                .strip_prefix(&config.target.join(&folder.relpath))?
                .to_owned();
            if config.target.join(&relpath).join(&child).is_dir() {
                extra_children.push(child);
            }
        }

        for child in extra_children {
            if !children.contains(&child) {
                children.push(child);
            }
        }
        // println!("Children: {:?}", children);
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
            for child in children {
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
        println!("{:#?}", root);

        assert_eq!(root.relpath, PathBuf::from(""));
        assert!(root.id.contains("bar"));
        assert!(root.id.contains("foo"));
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

        assert_eq!(root.children[1].relpath, PathBuf::from("foo"));
        assert!(!root.children[1].is_orphan);
        assert!(!root.children[1].is_widow);
        assert_eq!(root.children[1].children.len(), 3);
        assert_eq!(root.children[1].children[0].relpath, PathBuf::from("foo/a"));
        assert_eq!(root.children[1].children[1].relpath, PathBuf::from("foo/c"));
        assert_eq!(root.children[1].children[2].relpath, PathBuf::from("foo/b"));

        assert_eq!(root.children[2].relpath, PathBuf::from("baz"));
        assert!(!root.children[2].is_orphan);
        assert!(!root.children[2].is_widow);
        assert_eq!(root.children[2].children.len(), 0);

        resources.cleanup = false;
        Ok(())
    }
}
