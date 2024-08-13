# rusty-sink


This is a programs used for synchronizing a main folder (the "source") 
to a secondary backup (the "target"). 

There is an optional pre-copy move phase where it looks for folders 
that have been moved/renamed and tries to find their new path and name
before making copies and deleting the orphan folders. 

Files in the target are checked for size and modified date, 
and optionally for matching checksums (this can get very slow). 

A "lost and found" folder is added to the target where all deleted
files are moved to. 
A log file shows all the actions taken by the code for each run. 

![A rusty sink](./unit_tests_pass.png)

## Source vs. target directories

The source directory is a folder (that may contain other folders) 
where we expect the files to be newer. 
This is usually the updated version you are using in day-to-day. 
Files in the source directory are never changed by this program. 

The target directory is a folder that acts as a backup to the source. 
This is where the program will make changes, overwrite files, move folders, 
create log files and lost and found folders. 
It will (by default, but optionally) delete files in the target directory
that do not exist in the source. 
The target is assumed to be a backup that can sometimes be deleted and re-copied
if all else fails. 

## Command line arguments

To run the code, the source and target must be specified. 
Parameters are specified as `keyword:value` with a separating colon, 
without spaces! 

There is also an option to give a path to a config file. 
The config file or the command line must specify the source and target. 

For command line arguments that accept a boolean value (e.g., "verbose") 
it is possible to write just the name of the option without a colon and a value. 
This will be interpreted as setting it to true. 
Example: "verbose" is the same as "verbose:true". 

### Config file

The config file must contain options on separate lines, in the format key:value. 
The same parameters can be passed in using the config file and the command line. 
Note that the command line arguments will override the config file! 
In the config file we must specify values for any parameters we want to set, 
even those that accept booleans (e.g., we cannot use "verbose" but must write "verbose:true"). 

### A list of the commands available

Please note that the same information can be gotten by adding the command line option "help". 
In that case the program will not run, only print to the screen.

- `file:path/to/confing/file` the path to a config file to load before parsing any other arguments (command line only!).
- `source:path/to/source/directory` the relative/absolute path to the source directory. Must be specified (in file or command line).
- `target:path/to/target/folder` the relative/absolute path to the target directory. Must be specified (in file or command line).
- `verbose:(bool)` print all actions to stdout. Default is false. 
- `dry_run:(bool)` Only make a log file (and optional print to stdout) without changing other files in the target folder. Default is false. 
- `move_folders:(bool)` Try to match folders that have been moved or renamed in the target directory. After those are moved/renamed, a regular sync will verify the content is up to date. Default is true. 
- `sync_files:(bool)` copy files that are not up-to-date from the source directory to the target directory. Default is true. 
- `delete:(bool)` delete (move to lost and found) any files or folder found in the target directory that do not exist in the source directory directory. Default is true.
- `keep_versions:(bool)` any file that is found to be not up-to-date is overwritten by newer versions during the copy files phase. 
If this parameter is true, will first move the out-of-date file to lost and found before copying. Default is true. 
- `checksum:(bool)` if true, will compare the checksum (using md5) of each source and target file to see if it needs updating. Will skip files that have an old modifed date or size change. All other files will be checksummed. This is very slow for large directories, so use only when file contents are suspected of being changed or when modified dates are unreliable. Default is false. 

### Lost and found 

Any files that are deleted from the target directory are instead moved into a folder 
named `RUSTYSINK_LOST_AND_FOUND_XXXXXXXXXXXX` where the `XXXXXXXXXXXX` represents the date and time when the program was called. 
This includes files that were out-of-date and overwritten by newer files (if `keep_versions:true`). 

### Log file

A log file is created in the target directory, called `rustysing_XXXXXXXXXXXX.log`, 
where again `XXXXXXXXXXXX` is the date and time when the program started. 
The log file contains the configuration parameters and a timestamped list of
operations that the program did to the target directory. 
Using this and the lost and found folder it is possible to reverse the actions
of this program by undoing the list of actions in reverse order. 
(this option may be added at some point)

### Moved and renamed folders

To save some copy time, there is an option called `move_folders` (which is true by default)
that tries to match folders that have been moved or renamed in the source directory. 
It does this by matching the contents of each folder (the list of file and folder names, not the contents of any files). 
If it finds a folder in the target directory which is missing in the source, 
but that there is also a new folder in the source, and they have the same contents (file/folder name list) then there is a match. The folder could have a different paths and/or a different folder name. 
All matches are moved to the new path (including renaming of the folder itself) 
inside the target directory. 

Note that after the moving of folder, assuming `sync_files` is true, 
the individual files and subfolders inside each of these moved folders are checked normally. 
So any updates to individual files inside the folder are still carried out. 
In fact, if the content is entirely different, it will most likely just be 
fixed by the sync phase (unless the file modified dates and sizes all match). 

This option is mostly useful if there are large folders that were moved inside
the source directory.

### Using `checksum`

This option (which is off by default) is very slow for large files. 
The reason is that it requires reading both source and target files, 
which has similar costs to copying the file (unless over networks). 
If `checksum:true`, the checksum is calculated for __all files__, 
except the ones that have a new modified date or changed files. 
For backup systems, this will likely include all unchanged files, 
so it is not practical. 

For large data backup directories this is completely impractical. 
For small folders where minor changes may happen in file contents 
(possibly without changing the file size!) it may be a good idea 
to set `checksum:true`. 


