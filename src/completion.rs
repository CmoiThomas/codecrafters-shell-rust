//finds what a partially typed word could be completed into
//the first word of a line is a command; the following ones are handed to the
//completer script registered for that command, or treated as filenames

use std::env;
use std::fs;
use std::process::{Command, Stdio};

use crate::builtins::BUILTINS;
use crate::exec::is_executable;
use crate::shell::Shell;

//split the line into what stays untouched and the word being completed
//"cat re" gives ("cat ", "re")
pub fn split_word(line: &str) -> (&str, &str) {
    match line.rfind(' ') {
        Some(i) => line.split_at(i + 1),
        None => ("", line),
    }
}

//every name the last word of the line could be completed into, sorted
pub fn candidates_for(shell: &Shell, line: &str) -> Vec<String> {
    let (head, word) = split_word(line);

    if head.is_empty() {
        return command_candidates(word);
    }

    let command = head.split_whitespace().next().unwrap_or("");

    match shell.completers.get(command) {
        Some(script) => completer_candidates(script, command, line, head, word),
        None => file_candidates(word),
    }
}

//builtins and executables of the PATH starting with the prefix
fn command_candidates(prefix: &str) -> Vec<String> {
    if prefix.is_empty() {
        return Vec::new();
    }

    let mut names = Vec::new();

    for (name, _) in BUILTINS {
        if name.starts_with(prefix) {
            names.push(name.to_string());
        }
    }

    if let Ok(path) = env::var("PATH") {
        for dir in env::split_paths(&path) {
            let Ok(entries) = fs::read_dir(&dir) else { continue };              //a PATH entry may not exist, just skip it

            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(prefix) && is_executable(&entry.path()) {
                    names.push(name);
                }
            }
        }
    }

    names.sort();
    names.dedup();                                                               //the same name can sit in several PATH directories
    names
}

//files and directories starting with the prefix
//"path/to/f" looks inside "path/to" and keeps that part in the results, so the
//candidates always start with what the user typed
fn file_candidates(word: &str) -> Vec<String> {
    let (dir, prefix) = match word.rfind('/') {
        Some(i) => word.split_at(i + 1),                                         //"path/to/" and "f"
        None => ("", word),
    };

    let search = if dir.is_empty() { "." } else { dir };
    let Ok(entries) = fs::read_dir(search) else { return Vec::new() };           //the directory may not exist

    let mut names = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(prefix) {
            continue;
        }
        //hidden files only show up when the user asked for them, like in bash
        if name.starts_with('.') && !prefix.starts_with('.') {
            continue;
        }

        //a directory keeps a trailing "/" so the caller knows not to add a space
        let suffix = if entry.path().is_dir() { "/" } else { "" };
        names.push(format!("{dir}{name}{suffix}"));
    }

    names.sort();
    names
}

//ask a completer script registered with "complete -C" for candidates
//like bash, the script gets the context in three arguments:
//  argv[1] the command, argv[2] the word being completed, argv[3] the word before it
//and in two environment variables: COMP_LINE (the whole line) and COMP_POINT
//(the cursor position in bytes, always the end of the line here)
//every line the script prints is a candidate
fn completer_candidates(script: &str, command: &str, line: &str, head: &str, word: &str) -> Vec<String> {
    let previous = head.split_whitespace().last().unwrap_or("");

    let output = Command::new(script)
        .args([command, word, previous])
        .env("COMP_LINE", line)
        .env("COMP_POINT", line.len().to_string())
        .stdin(Stdio::null())                                                    //the terminal is in raw mode, keep the script away from it
        .stderr(Stdio::null())
        .output();                                                               //output() waits for the script to finish, so nothing is read half-written

    let Ok(output) = output else { return Vec::new() };                          //a script that cannot run gives no candidates

    let mut names = Vec::new();

    for candidate in String::from_utf8_lossy(&output.stdout).lines() {
        //keep only what matches the word typed so far, in case the script does not filter
        if !candidate.is_empty() && candidate.starts_with(word) {
            names.push(candidate.to_string());
        }
    }

    names.sort();
    names.dedup();
    names
}

//longest prefix shared by every candidate
//"xyz_foo", "xyz_foo_bar" and "xyz_foo_bar_baz" give "xyz_foo"
pub fn common_prefix(names: &[String]) -> String {
    let Some(first) = names.first() else { return String::new() };

    //start with a whole name, every other one can only make it shorter
    let mut prefix: Vec<char> = first.chars().collect();

    for name in &names[1..] {
        let mut shared = 0;
        for (a, b) in prefix.iter().zip(name.chars()) {
            if *a != b {
                break;
            }
            shared += 1;
        }
        prefix.truncate(shared);
    }

    prefix.into_iter().collect()
}
