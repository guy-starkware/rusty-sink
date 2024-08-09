use chrono::prelude::*;

use super::config::Config;
use std::collections::HashMap;
use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};

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
        if file_to_ignore(&path) {
            continue;
        }
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

// do the entire synchronization process
pub fn run(config: &mut Config) -> Result<(), Box<dyn Error>> {
    make_lost_and_found(config)?;
    make_logfile(config)?;
    write_line(config, "Starting scan of both folders...")?;

    let (_root, orphans, widows) = scan_trees(config)?;
    write_line(
        config,
        &format!(
            "Scan complete. Found {} orphans and {} widows. ",
            orphans.len(),
            widows.len()
        ),
    )?;

    if config.move_folders {
        move_orphans(config, &orphans, &widows)?;
        write_line(config, "Done matching and moving orphans. ")?;
    }

    if config.delete {
        remove_orphans(config, &config.target.clone())?;
        write_line(config, "Done removing orphans. ")?;
    }

    if config.sync_files {
        copy_files_and_folders(config, &config.source.clone())?;
        write_line(config, "Done copying files. ")?;
    }

    Ok(())
}

// create a folder under the target folder to store any files that are deleted (or old versions of updated files)
// will have a timestamp in the folder name, and each file moved there is stored under its original relpath
fn make_lost_and_found(config: &Config) -> Result<(), Box<dyn Error>> {
    let path: PathBuf = config.lost_and_found_path();
    std::fs::create_dir_all(path)?;
    Ok(())
}

// create a logfile under the target folder, with a timestamp in the name
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

fn file_to_ignore(path: &Path) -> bool {
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    //println!("file_name to ignore is {:?}", file_name);
    file_name.starts_with("RUSTYSINK_LOST_AND_FOUND")
        || (file_name.starts_with("rustysink_") && file_name.ends_with(".log"))
}

type ReturnAll = (
    Folder,
    HashMap<String, Vec<PathBuf>>,
    HashMap<String, Vec<PathBuf>>,
);

// scan both the source and target folders, and return a tuple with the root folder, and two hashmaps with orphans and widows
fn scan_trees(config: &Config) -> Result<ReturnAll, Box<dyn Error>> {
    // assumes the source and target folders exist (so neither is widow/orphan)
    let mut orphans = HashMap::new();
    let mut widows = HashMap::new();

    let root = Folder::scan(config, PathBuf::from(""), &mut orphans, &mut widows)?;

    Ok((root, orphans, widows))
}

// move orphans to the corresponding widow folder location (all moves are inside the target folder!)
fn move_orphans(
    config: &mut Config,
    orphans: &HashMap<String, Vec<PathBuf>>,
    widows: &HashMap<String, Vec<PathBuf>>,
) -> Result<(), Box<dyn Error>> {
    for (orphan_id, orphan_paths) in orphans.iter() {
        // go over orphans
        if let Some(widow_paths) = widows.get(orphan_id) {
            // if there is a widow with the same id
            for (i, orphan_path) in orphan_paths.iter().enumerate() {
                // can have multiple orphans with the same id
                let orphan_path = config.target.join(orphan_path);
                if i < widow_paths.len() {
                    // if there are more orphans than widows, we can't match them
                    let target = config.target.join(&widow_paths[i]); // the path we want to put this orphan in
                                                                      // println!("Moving orphan: {:?} -> {:?}", orphan_path.strip_prefix(&config.target)?, target.strip_prefix(&config.target)?);

                    // check if a folder aleady exists where the move will take place, if so, move that folder to LOST AND FOUND
                    if target.exists() {
                        delete_file_or_folder(config, &target)?;
                    }

                    // move this orphan folder to the corresponding widow folder location
                    write_line(
                        config,
                        &format!(
                            "MOVE: {:?} -> {:?}",
                            orphan_path.strip_prefix(&config.target)?,
                            target.strip_prefix(&config.target)?
                        ),
                    )?;
                    if !config.dry_run {
                        std::fs::rename(orphan_path, target)?;
                    }
                }
            }
        }
    }
    Ok(())
}

// goes over the target folder recursively and moves to lost and found any folders or files not in the source
fn remove_orphans(config: &mut Config, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    for entry in std::fs::read_dir(path)? {
        let orphan_path = entry?.path();
        if file_to_ignore(&orphan_path) {
            // skip the lost and found and log file
            continue;
        }
        let source_path = config
            .source
            .join(orphan_path.strip_prefix(&config.target)?);
        if orphan_path.is_dir() && source_path.is_dir() {
            remove_orphans(config, &orphan_path)?; // recursively go into the folder tree
            continue;
        }
        // only reach this part if we didn't go into the folder tree
        if !source_path.exists() {
            // if the file or folder doesn't exist in the source, move it from target to LOST AND FOUND
            delete_file_or_folder(config, &orphan_path)?;
        }
    }
    Ok(())
}

// recursively copy files and folders from the source to the target
// for each folder that exists in the source and target, will call the sync_files function to
// check each file and copy it if necessary
fn copy_files_and_folders(config: &mut Config, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    if config.verbose {
        println!("Copying files and folders in {:?}", path);
    }
    for entry in std::fs::read_dir(path)? {
        let path = entry?.path();
        if file_to_ignore(&path) {
            // skip the lost and found and log file
            continue;
        }
        if path.is_dir() {
            let target_path = config.target.join(path.strip_prefix(&config.source)?);
            if !target_path.is_dir() {
                // if the folder doesn't exist in the target, create it
                write_line(
                    config,
                    &format!("COPY: {:?}", path.strip_prefix(&config.source)?),
                )?;
                if !config.dry_run {
                    std::fs::create_dir_all(target_path)?;
                }
            }
            copy_files_and_folders(config, &path)?; // recursively go into the folder tree
        }
    }

    // sync the files in this folder
    sync_files(config, path)?;

    Ok(())
}

// go over the files in a single folder on source, and copy the ones that are missing or outdated
fn sync_files(config: &mut Config, folder: &PathBuf) -> Result<(), Box<dyn Error>> {
    let relpath = folder.strip_prefix(&config.source)?;
    if config.verbose {
        println!("Syncing files in {:?}", relpath);
    }
    for file in std::fs::read_dir(folder)? {
        let file = file?;
        let path = file.path();
        let filename = path.file_name().unwrap().to_string_lossy().to_string();
        // this function skips folders (they would be treated recursively by the caller)
        if path.is_dir() {
            continue;
        }

        // file exists in source
        if path.is_file() {
            let target = config.target.join(relpath).join(&filename);
            if target.exists() {
                // it exists in the target as well, must check if it needs to be updated
                if check_need_update(config, &path, &target)? {
                    if config.keep_versions {
                        delete_file_or_folder(config, &target)?;
                    }
                } else {
                    // if the files are the same, can skip the copy operation below
                    continue;
                }
            } // if the file doesn't exist in the target, we should copy it

            // if we've reached here, without hitting any continue statements, we should copy the file
            write_line(config, &format!("COPY: {:?}", relpath.join(&filename)))?;
            if !config.dry_run {
                std::fs::copy(path, target)?;
            }
        }
    }

    Ok(())
}
// move the file or folder in "path" to the lost and found folder, including the path relative to the target folder

fn delete_file_or_folder(config: &mut Config, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    write_line(
        config,
        &format!("DELETE: {:?}", path.strip_prefix(&config.target)?),
    )?;
    if !config.dry_run {
        // create the path to the moved file inside lost and found
        let lost_and_found = config.lost_and_found_path();
        let relpath = path.strip_prefix(&config.target)?;
        if path.is_file() {
            if let Some(path_parent) = relpath.parent() {
                std::fs::create_dir_all(lost_and_found.join(path_parent))?;
            }
        }
        if path.is_dir() {
            std::fs::create_dir_all(lost_and_found.join(relpath))?
        }

        // do the actual move
        std::fs::rename(path, lost_and_found.join(relpath))?;
    }
    Ok(())
}

// check if a file needs to be updated, based on its size, the modified time, and (optionally) by comparing its checksum
fn check_need_update(
    config: &Config,
    source: &PathBuf,
    target: &PathBuf,
) -> Result<bool, Box<dyn Error>> {
    // first check if the files are the same size
    let source_metadata = std::fs::metadata(source)?;
    let target_metadata = std::fs::metadata(target)?;

    if source_metadata.len() != target_metadata.len() {
        return Ok(true);
    }

    // check the modified time
    if source_metadata.modified()? > target_metadata.modified()? {
        return Ok(true);
    }

    // if checksum is enabled, check the checksum
    if config.checksum {
        let source_checksum = md5::compute(std::fs::read(source)?);
        let target_checksum = md5::compute(std::fs::read(target)?);
        if source_checksum != target_checksum {
            return Ok(true);
        }
    }

    // if all the above conditions don't come true, then return false (no need to update)
    Ok(false)
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
        fn new(config: &Config, add_files: bool) -> Result<Self, Box<dyn Error>> {
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

            if add_files {
                // add some fake files in these folders!
                make_a_file(&config.source.join("foo/a"))?;
                make_a_file(&config.source.join("foo/b"))?;
                make_a_file(&config.source.join("foo/c"))?;

                // make multiple files in bar/d
                make_a_file(&config.source.join("bar/d"))?;
                make_a_file(&config.source.join("bar/d"))?;
                make_a_file(&config.source.join("bar/d"))?;

                // make a file in bar/e
                make_a_file(&config.source.join("bar/e"))?;

                // leave bar/f empty

                // make a file in bar itself
                make_a_file(&config.source.join("bar"))?;

                // make some different files in the target folder
                make_a_file(&config.target.join("foo/a"))?;
                make_a_file(&config.target.join("foo/b"))?;
                make_a_file(&config.target.join("foo/c"))?;

                // make multiple files in bar/d
                make_a_file(&config.target.join("bar/d"))?;
                make_a_file(&config.target.join("bar/d"))?;
                make_a_file(&config.target.join("bar/d"))?;

                // make a file in bar/e
                make_a_file(&config.target.join("bar/e"))?;

                // leave bar/f empty

                // make a file in bar itself
                make_a_file(&config.target.join("bar"))?;
            }

            Ok(Self {
                logfile,
                source,
                target,
                cleanup,
            })
        }
    }

    fn make_a_file(parent: &PathBuf) -> Result<(), Box<dyn Error>> {
        let text = random_string();
        let path = parent.join(format!("test_file_{}.txt", text));
        let mut file = std::fs::File::create(path)?;
        writeln!(file, "This is a test file")?;
        writeln!(file, "The content of the file is: {}", text)?;
        Ok(())
    }

    /// make a random string, use it to make a config struct, use that to make source/target folders
    /// make sure these folders (and logfile name) are saved to the resources struct
    /// which will cleanup at the end of the test
    fn setup_resources(add_files: bool) -> Result<(Config, TestFoldersAndLog), Box<dyn Error>> {
        let rand = random_string();
        let config = Config {
            source: std::path::PathBuf::from(format!("test_data/SOURCE_{}", rand)),
            target: std::path::PathBuf::from(format!("test_data/TARGET_{}", rand)),
            ..Default::default()
        };
        let resources = TestFoldersAndLog::new(&config, add_files)?;
        Ok((config, resources))
    }

    // recursively copies a folder and its contents to a target folder
    fn copy_folder(source: &PathBuf, target: &PathBuf) -> Result<(), Box<dyn Error>> {
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                std::fs::copy(&path, target.join(path.file_name().unwrap()))?;
            } else {
                std::fs::create_dir(target.join(path.file_name().unwrap()))?;
                copy_folder(&path, &target.join(path.file_name().unwrap()))?;
            }
        }
        Ok(())
    }

    /// re-scans both source and target and crashes if there are any differences
    fn assert_folder_trees_equal(source_dir: &PathBuf, target_dir: &PathBuf, check_orphans: bool) {
        if file_to_ignore(&target_dir) {
            // skip this file if it is on the ignore list
            return;
        }

        // check all files in the source directory have been successfully copied to the target directory
        for src in std::fs::read_dir(&source_dir).unwrap() {
            let src = src.unwrap();
            let src_path = src.path();
            let tgt_path = target_dir.join(src.file_name());

            assert!(
                tgt_path.exists(),
                "File {:?} is missing in target",
                src_path
            );
            if src_path.is_dir() {
                assert_folder_trees_equal(&src_path, &tgt_path, check_orphans);
            } else {
                assert!(tgt_path.is_file());
                // check the file md5 checksum is the same
                let src_md5 = md5::compute(std::fs::read(&src_path).unwrap());
                let tgt_md5 = md5::compute(std::fs::read(&tgt_path).unwrap());
                assert_eq!(src_md5, tgt_md5);
            }
        }

        // check all the files in the target directory are in the source directory (check against remaining orphans)
        if check_orphans {
            for tgt in std::fs::read_dir(&target_dir).unwrap() {
                let tgt = tgt.unwrap();
                let tgt_path = tgt.path();
                if file_to_ignore(&tgt_path) {
                    continue;
                }
                let src_path = source_dir.join(tgt.file_name());

                assert!(
                    src_path.exists(),
                    "File {:?} is missing in source",
                    tgt_path
                );
            }
        }
    }

    #[test]
    fn test_make_folder_and_logfile() -> Result<(), Box<dyn Error>> {
        let (mut config, mut resources) = setup_resources(false)?;

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

        resources.cleanup = true; // set this to true to clean up, to false to inspect the folders
        Ok(())
    }

    #[test]
    fn test_read_identical_trees() -> Result<(), Box<dyn Error>> {
        let (config, mut resources) = setup_resources(false)?;

        let (root, orphans, widows) = scan_trees(&config)?;
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

        assert!(orphans.is_empty());
        assert!(widows.is_empty());

        resources.cleanup = true; // set this to true to clean up, to false to inspect the folders
        Ok(())
    }

    #[test]
    fn test_tree_with_widow() -> Result<(), Box<dyn Error>> {
        let (config, mut resources) = setup_resources(false)?;

        // delete one folder from the source to produce an orphan
        let path = resources.target.join("foo");
        std::fs::remove_dir_all(&path)?;

        let (root, orphans, widows) = scan_trees(&config)?;
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
        assert!(root.children[2].is_widow); // this is a widow!
        assert_eq!(root.children[2].children.len(), 0);

        assert!(orphans.is_empty());
        assert_eq!(widows.len(), 1);
        assert_eq!(widows.get(&root.children[2].id).unwrap().len(), 1);
        assert_eq!(
            widows.get(&root.children[2].id).unwrap()[0],
            PathBuf::from("foo")
        );

        resources.cleanup = true; // set this to true to clean up, to false to inspect the folders
        Ok(())
    }

    #[test]
    fn test_tree_with_orphan() -> Result<(), Box<dyn Error>> {
        let (config, mut resources) = setup_resources(false)?;

        // delete one folder from the source to produce an orphan
        let path = resources.source.join("foo");
        std::fs::remove_dir_all(&path)?;

        let (root, orphans, widows) = scan_trees(&config)?;
        assert_eq!(root.relpath, PathBuf::from(""));
        assert_eq!(root.id, "bar, baz");
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

        assert!(widows.is_empty());
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans.get(&root.children[2].id).unwrap().len(), 1);
        assert_eq!(
            orphans.get(&root.children[2].id).unwrap()[0],
            PathBuf::from("foo")
        );

        resources.cleanup = true; // set this to true to clean up, to false to inspect the folders
        Ok(())
    }

    #[test]
    fn test_fix_moved_folder() -> Result<(), Box<dyn Error>> {
        let (mut config, mut resources) = setup_resources(false)?;

        // move one folder from the source to produce an orphan and a widow
        let path = resources.source.join("foo");
        std::fs::rename(&path, resources.source.join("baz").join("foo"))?;

        // to inspect the logfile later
        make_lost_and_found(&config)?;
        make_logfile(&mut config)?;

        // scan and then move the orphan folder
        let (_root, orphans, widows) = scan_trees(&config)?;
        move_orphans(&mut config, &orphans, &widows)?;

        assert_folder_trees_equal(&config.source, &config.target, true);
        resources.cleanup = true; // set this to true to clean up, to false to inspect the folders
        Ok(())
    }

    #[test]
    fn test_run_with_moved_folder() -> Result<(), Box<dyn Error>> {
        let (mut config, mut resources) = setup_resources(true)?;
        let mut orphans = vec![]; // we do not include "foo" as we will move it, not delete it
        for subfolder in ["bar", "bar/d", "bar/e"] {
            for entry in std::fs::read_dir(resources.target.join(subfolder))? {
                let path = entry.unwrap().path();
                if path.is_file() {
                    let ending = path.strip_prefix(&resources.target).unwrap();
                    orphans.push(ending.to_string_lossy().to_string());
                }
            }
        }

        // delete one folder from the source to produce an orphan
        let source_path = resources.source.join("foo");
        std::fs::rename(&source_path, resources.source.join("baz").join("foo"))?; // foo is now moved into baz
        let source_path = resources.source.join("baz").join("foo"); // foo is now moved into baz

        std::fs::remove_dir_all(resources.target.join("foo"))?; // remove foo from target
        let target_path = resources.target.join("foo"); // foo remains at the same place in target
        std::fs::create_dir_all(&target_path)?;

        // copy the files from the original source location ("foo") to the new target location ("baz/foo")
        copy_folder(&source_path, &target_path)?;

        // to inspect the logfile later
        make_lost_and_found(&config)?;
        make_logfile(&mut config)?;

        // make sure the config is set to move, copy and delete
        config.move_folders = true;
        config.sync_files = true;
        config.delete = true;

        // run the sync
        run(&mut config)?;

        // first of all, check that the file trees are the same
        assert_folder_trees_equal(&config.source, &config.target, true);

        // then check the logfile to see things happened the way they were supposed to
        let logfile = std::fs::read_to_string(config.log_file_path())?;
        assert!(logfile.contains("Found 1 orphans and 1 widows"));
        assert!(logfile.contains("MOVE: \"foo\" -> \"baz/foo\""));
        assert!(!logfile.contains("DELETE: \"foo")); // doesn't delete foo or any subfolder
        assert!(!logfile.contains("COPY: \"foo")); // doesn't copy foo or any subfolder
        assert!(!logfile.contains("COPY: \"baz/foo")); // doesn't copy baz/foo or any subfolder

        // check the deleted files ended up in the lost and found folder (in the correct subfolder!)
        let lost_and_found = config.lost_and_found_path();
        for orphan in orphans {
            // println!("{:?}", lost_and_found.join(&orphan));
            assert!(lost_and_found.join(&orphan).exists());
        }

        resources.cleanup = true; // set this to true to clean up, to false to inspect the folders
        Ok(())
    }

    #[test]
    fn test_run_with_moved_folder_without_move() -> Result<(), Box<dyn Error>> {
        let (mut config, mut resources) = setup_resources(true)?;
        let mut orphans = vec!["foo".to_string()];
        for subfolder in ["bar", "bar/d", "bar/e"] {
            for entry in std::fs::read_dir(resources.target.join(subfolder))? {
                let path = entry.unwrap().path();
                if path.is_file() {
                    let ending = path.strip_prefix(&resources.target).unwrap();
                    orphans.push(ending.to_string_lossy().to_string());
                }
            }
        }

        // delete one folder from the source to produce an orphan
        let source_path = resources.source.join("foo");
        std::fs::rename(&source_path, resources.source.join("baz").join("foo"))?; // foo is now moved into baz
        let source_path = resources.source.join("baz").join("foo"); // foo is now moved into baz

        std::fs::remove_dir_all(resources.target.join("foo"))?; // remove foo from target
        let target_path = resources.target.join("foo"); // foo remains at the same place in target
        std::fs::create_dir_all(&target_path)?;

        // copy the files from the original source location ("foo") to the new target location ("baz/foo")
        copy_folder(&source_path, &target_path)?;

        // to inspect the logfile later
        make_lost_and_found(&config)?;
        make_logfile(&mut config)?;

        // make sure the config is set to move, copy and delete
        config.move_folders = false;
        config.sync_files = true;
        config.delete = true;

        // run the sync
        run(&mut config)?;

        // first of all, check that the file trees are the same
        assert_folder_trees_equal(&config.source, &config.target, true);

        // then check the logfile to see things happened the way they were supposed to
        let logfile = std::fs::read_to_string(config.log_file_path())?;
        assert!(logfile.contains("Found 1 orphans and 1 widows"));
        assert!(!logfile.contains("MOVE: \"foo\" -> \"baz/foo\""));
        assert!(logfile.contains("DELETE: \"foo")); // does delete foo or a subfolder
        assert!(logfile.contains("COPY: \"baz/foo")); // does copy foo inside baz

        // check the deleted files ended up in the lost and found folder (in the correct subfolder!)
        let lost_and_found = config.lost_and_found_path();
        for orphan in orphans {
            // println!("{:?}", lost_and_found.join(&orphan));
            assert!(lost_and_found.join(&orphan).exists());
        }

        resources.cleanup = true; // set this to true to clean up, to false to inspect the folders
        Ok(())
    }

    // TODO: test what happens when delete=false (add orphans=false to the check function)
    #[test]
    fn test_run_without_delete() -> Result<(), Box<dyn Error>> {
        let (mut config, mut resources) = setup_resources(true)?;
        let mut orphans = vec![]; // we don't include "foo" because it is moved and doesn't get left behind as orphan
        for subfolder in ["bar", "bar/d", "bar/e"] {
            for entry in std::fs::read_dir(resources.target.join(subfolder))? {
                let path = entry.unwrap().path();
                if path.is_file() {
                    let ending = path.strip_prefix(&resources.target).unwrap();
                    orphans.push(ending.to_string_lossy().to_string());
                }
            }
        }

        // delete one folder from the source to produce an orphan
        let source_path = resources.source.join("foo");
        std::fs::rename(&source_path, resources.source.join("baz").join("foo"))?; // foo is now moved into baz
        let source_path = resources.source.join("baz").join("foo"); // foo is now moved into baz

        std::fs::remove_dir_all(resources.target.join("foo"))?; // remove foo from target
        let target_path = resources.target.join("foo"); // foo remains at the same place in target
        std::fs::create_dir_all(&target_path)?;

        // copy the files from the original source location ("foo") to the new target location ("baz/foo")
        copy_folder(&source_path, &target_path)?;

        // to inspect the logfile later
        make_lost_and_found(&config)?;
        make_logfile(&mut config)?;

        // make sure the config is set to move, copy but NOT delete
        config.move_folders = true;
        config.sync_files = true;
        config.delete = false;

        // run the sync
        run(&mut config)?;

        // first of all, check that the file trees are the same
        assert_folder_trees_equal(&config.source, &config.target, false);

        // then check the logfile to see things happened the way they were supposed to
        let logfile = std::fs::read_to_string(config.log_file_path())?;
        assert!(logfile.contains("Found 1 orphans and 1 widows"));
        assert!(logfile.contains("MOVE: \"foo\" -> \"baz/foo\""));

        // check that the orphan files remain where they were
        for orphan in orphans {
            // println!("{:?}", config.target.join(&orphan));
            assert!(config.target.join(&orphan).exists());
        }

        resources.cleanup = true; // set this to true to clean up, to false to inspect the folders
        Ok(())
    }

    // TODO: test what happens when file contents are changed but filenames are the same
    // TODO: test what happens when checksum is enabled and files are different but have the same size / modified time
}
