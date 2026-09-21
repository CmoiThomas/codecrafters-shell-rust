//runs a command, builtin or external, with its output plugged on the right place

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::builtins::Builtin;
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

//run an external program
//each stream that has a redirection is plugged on its file, the others stay on
//the terminal
pub fn run_external(name: &str, args: &[&str], redirect: &Redirect) {
    let mut cmd = Command::new(name);
    cmd.args(args);

    //here too the file is created even if the command fails afterwards:
    //"cat missing > out.txt" leaves an empty out.txt behind
    if let Some(target) = &redirect.out {
        match open_file(target) {
            Ok(file) => { cmd.stdout(Stdio::from(file)); }
            Err(e) => { eprintln!("{}: {}", target.path, e); return; }
        }
    }

    if let Some(target) = &redirect.err {
        match open_file(target) {
            Ok(file) => { cmd.stderr(Stdio::from(file)); }
            Err(e) => { eprintln!("{}: {}", target.path, e); return; }
        }
    }

    //status() waits for the program to finish
    //Err means the launch itself failed, so the command does not exist
    if cmd.status().is_err() {
        println!("{name}: command not found");
    }
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
