//moves the command history between memory and a file
//the file holds one command per line, and ends with a newline

use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};

use crate::shell::Shell;

//the variable holding the path of the file to load at startup and save at exit
const HISTFILE: &str = "HISTFILE";

//add the lines of a file to the history in memory
pub fn read_file(shell: &mut Shell, path: &str) -> io::Result<()> {
    let file = File::open(path)?;

    for line in BufReader::new(file).lines() {
        let line = line?;
        if !line.trim().is_empty() {                                             //the file ends with a newline, which reads as an empty line
            shell.history.push(line);
        }
    }

    Ok(())
}

//write the whole history to a file, replacing whatever was in it
pub fn write_file(shell: &Shell, path: &str) -> io::Result<()> {
    let mut file = File::create(path)?;                                          //creates the file, or empties it if it exists

    for entry in &shell.history {
        writeln!(file, "{entry}")?;
    }

    Ok(())
}

//add to a file only the commands entered since the last append
pub fn append_file(shell: &mut Shell, path: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;

    let start = shell.appended.min(shell.history.len());
    for entry in &shell.history[start..] {
        writeln!(file, "{entry}")?;
    }

    shell.appended = shell.history.len();                                        //next append starts from here
    Ok(())
}

//load HISTFILE when the shell starts, if that variable is set
pub fn load_at_startup(shell: &mut Shell) {
    let Ok(path) = env::var(HISTFILE) else { return };

    read_file(shell, &path).ok();                                                //a missing file is not an error, the history just starts empty
    shell.appended = shell.history.len();                                        //what came from the file is already in it
}

//add this session's commands to HISTFILE when the shell stops
pub fn save_at_exit(shell: &mut Shell) {
    let Ok(path) = env::var(HISTFILE) else { return };

    append_file(shell, &path).ok();
}
