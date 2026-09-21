//the commands the shell runs itself, without launching a program

use std::env;
use std::io::Write;

use crate::exec::find_in_path;
use crate::shell::Shell;

//"out" and "err" are the output destinations: the terminal, or a file when a
//redirection is used
pub type Builtin = fn(&mut Shell, &[&str], &mut dyn Write, &mut dyn Write);

pub const BUILTINS: &[(&str, Builtin)] = &[
    ("exit", exit_command),
    ("echo", echo_command),
    ("type", type_command),
    ("pwd", pwd_command),
    ("cd", cd_command),
    ("complete", complete_command),
];

//look a builtin up by name
pub fn find(name: &str) -> Option<Builtin> {
    for (builtin_name, f) in BUILTINS {
        if *builtin_name == name {
            return Some(*f);
        }
    }
    None
}

pub fn is_builtin(name: &str) -> bool {
    find(name).is_some()
}

fn exit_command(_shell: &mut Shell, args: &[&str], _out: &mut dyn Write, _err: &mut dyn Write) {
    //"exit" alone means "exit 0", and so does an argument that is not a number
    let code = match args.first() {
        Some(arg) => arg.parse::<i32>().unwrap_or(0),
        None => 0,
    };
    std::process::exit(code);
}

fn echo_command(_shell: &mut Shell, args: &[&str], out: &mut dyn Write, _err: &mut dyn Write) {
    writeln!(out, "{}", args.join(" ")).ok();
}

fn pwd_command(_shell: &mut Shell, _args: &[&str], out: &mut dyn Write, err: &mut dyn Write) {
    match env::current_dir() {
        Ok(dir) => { writeln!(out, "{}", dir.display()).ok(); }
        Err(e) => { writeln!(err, "pwd: {e}").ok(); }
    }
}

//NEED TO REWORK THIS PART, THE ~ MANAGEMENT CAN BE IMPROVED, BUT IT WORKS FOR NOW
fn cd_command(_shell: &mut Shell, args: &[&str], _out: &mut dyn Write, err: &mut dyn Write) {
    let raw = args.first().copied().unwrap_or("~");
    let path = raw.replacen('~', &env::var("HOME").unwrap_or_default(), 1);

    if env::set_current_dir(&path).is_err() {
        //an error message goes to stderr, so "2>" can catch it
        writeln!(err, "cd: {path}: No such file or directory").ok();
    }
}

fn type_command(_shell: &mut Shell, args: &[&str], out: &mut dyn Write, _err: &mut dyn Write) {
    let Some(name) = args.first() else { return };

    if is_builtin(name) {
        writeln!(out, "{name} is a shell builtin").ok();
    } else if let Some(path) = find_in_path(name) {
        writeln!(out, "{name} is {}", path.display()).ok();
    } else {
        writeln!(out, "{name}: not found").ok();
    }
}

//manage the completer scripts used by Tab
//  complete -C <script> <command>   register a script for a command
//  complete -p <command>            print what is registered
//  complete -r <command>            forget it
fn complete_command(shell: &mut Shell, args: &[&str], out: &mut dyn Write, err: &mut dyn Write) {
    match args {
        ["-C", script, commands @ ..] => {
            for command in commands {
                shell.completers.insert(command.to_string(), script.to_string());
            }
        }
        ["-r", commands @ ..] => {
            for command in commands {
                shell.completers.remove(*command);                               //removing something that is not there is not an error
            }
        }
        ["-p", commands @ ..] => {
            for command in commands {
                match shell.completers.get(*command) {
                    //the output is rebuilt from what was stored, so it is always
                    //in the same clean format whatever spacing the user typed
                    Some(script) => { writeln!(out, "complete -C '{script}' {command}").ok(); }
                    None => { writeln!(err, "complete: {command}: no completion specification").ok(); }
                }
            }
        }
        _ => {
            writeln!(err, "complete: usage: complete [-C script | -p | -r] command...").ok();
        }
    }
}
