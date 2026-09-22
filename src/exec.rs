//runs a command, builtin or external, with its output plugged on the right place

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::builtins::Builtin;
use crate::jobs;
use crate::redirect::{open_file, open_target, Redirect};
use crate::shell::Shell;

//run a builtin, writing either to the terminal or to the redirection files
pub fn run_builtin(shell: &mut Shell, f: Builtin, args: &[&str], redirect: &Redirect) {
    //the files are opened first: they must exist even when nothing is written
    let (Ok(mut out_file), Ok(mut err_file)) =
        (open_target(&redirect.out), open_target(&redirect.err))
    else {
        return;
    };

    let mut stdout = io::stdout();
    let mut stderr = io::stderr();

    let out: &mut dyn Write = match out_file.as_mut() {
        Some(file) => file,
        None => &mut stdout,
    };
    let err: &mut dyn Write = match err_file.as_mut() {
        Some(file) => file,
        None => &mut stderr,
    };

    f(shell, args, out, err);
}

//run an external program and wait for it to finish
pub fn run_external(name: &str, args: &[&str], redirect: &Redirect) {
    let Some(mut cmd) = build_command(name, args, redirect) else { return };

    //status() waits for the program to finish
    //Err means the launch itself failed, so the command does not exist
    if cmd.status().is_err() {
        println!("{name}: command not found");
    }
}

//start an external program without waiting for it, and add it to the jobs table
pub fn run_background(shell: &mut Shell, name: &str, args: &[&str], redirect: &Redirect, command: String) {
    let Some(mut cmd) = build_command(name, args, redirect) else { return };

    //stdout and stderr stay on the terminal, but the keyboard belongs to the shell
    cmd.stdin(Stdio::null());

    match cmd.spawn() {
        Ok(child) => jobs::add(&mut shell.jobs, child, command),
        Err(_) => println!("{name}: command not found"),
    }
}

//prepare a program with its arguments and redirections, without starting it
//None means a redirection file could not be opened, the error is already printed
fn build_command(name: &str, args: &[&str], redirect: &Redirect) -> Option<Command> {
    let mut cmd = Command::new(name);
    cmd.args(args);

    //the files are created even if the command fails afterwards:
    //"cat missing > out.txt" leaves an empty out.txt behind
    if let Some(target) = &redirect.out {
        match open_file(target) {
            Ok(file) => { cmd.stdout(Stdio::from(file)); }
            Err(e) => { eprintln!("{}: {}", target.path, e); return None; }
        }
    }

    if let Some(target) = &redirect.err {
        match open_file(target) {
            Ok(file) => { cmd.stderr(Stdio::from(file)); }
            Err(e) => { eprintln!("{}: {}", target.path, e); return None; }
        }
    }

    Some(cmd)
}

//first directory of the PATH holding an executable with this name
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var("PATH").ok()?;

    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }

    None
}

//CODECRAFTER USE A UNIX OS, SO THIS PART IS ONLY TO VALIDATE THE TESTS AND GO TO THE NEXT STEP
#[cfg(unix)]
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match path.metadata() {
        //0o111 holds the three execute bits, one of them is enough
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

#[cfg(windows)]
pub fn is_executable(path: &Path) -> bool {
    path.is_file()
}
