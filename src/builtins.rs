//the commands the shell runs itself, without launching a program

use std::env;
use std::io::Write;

use crate::exec::find_in_path;
use crate::history;
use crate::jobs;
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
    ("jobs", jobs_command),
    ("history", history_command),
    ("declare", declare_command),
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

fn exit_command(shell: &mut Shell, args: &[&str], _out: &mut dyn Write, _err: &mut dyn Write) {
    history::save_at_exit(shell);                                                //the session is over, HISTFILE gets what happened in it

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

//list the background jobs, and forget the ones that finished
fn jobs_command(shell: &mut Shell, _args: &[&str], out: &mut dyn Write, _err: &mut dyn Write) {
    jobs::report(&mut shell.jobs, out, true);
}

//manage the command history
//  history           list everything, "history <n>" only the last n entries
//  history -r <file> add the lines of a file to the history
//  history -w <file> write the whole history to a file, replacing its contents
//  history -a <file> add to a file the commands entered since the last append
fn history_command(shell: &mut Shell, args: &[&str], out: &mut dyn Write, err: &mut dyn Write) {
    match args {
        ["-r", path] => {
            if history::read_file(shell, path).is_err() {
                writeln!(err, "history: {path}: cannot read file").ok();
            }
        }
        ["-w", path] => {
            if history::write_file(shell, path).is_err() {
                writeln!(err, "history: {path}: cannot write file").ok();
            }
        }
        ["-a", path] => {
            if history::append_file(shell, path).is_err() {
                writeln!(err, "history: {path}: cannot write file").ok();
            }
        }
        _ => list_history(shell, args, out),
    }
}

//show the history, or only its last entries when a count is given
fn list_history(shell: &Shell, args: &[&str], out: &mut dyn Write) {
    //the entry number shown is its position in the list, starting at 1
    let first = match args.first() {
        Some(arg) => match arg.parse::<usize>() {
            Ok(count) => shell.history.len().saturating_sub(count),              //saturating_sub avoids going below zero when n is too big
            Err(_) => 0,
        },
        None => 0,
    };

    for (index, entry) in shell.history.iter().enumerate() {
        if index >= first {
            writeln!(out, "{:>5}  {}", index + 1, entry).ok();
        }
    }
}

//create and inspect shell variables
//  declare NAME=VALUE   record a variable
//  declare -p NAME      print a description of it
fn declare_command(shell: &mut Shell, args: &[&str], out: &mut dyn Write, err: &mut dyn Write) {
    match args {
        ["-p"] => {
            //no name given: describe every variable, in a stable order
            let mut names: Vec<&String> = shell.variables.keys().collect();
            names.sort();
            for name in names {
                describe(shell, name, out, err);
            }
        }
        ["-p", names @ ..] => {
            for name in names {
                describe(shell, name, out, err);
            }
        }
        _ => {
            for argument in args {
                assign(shell, argument, err);
            }
        }
    }
}

//print the line describing one variable, or say it does not exist
fn describe(shell: &Shell, name: &str, out: &mut dyn Write, err: &mut dyn Write) {
    match shell.variables.get(name) {
        Some(value) => { writeln!(out, "declare -- {name}=\"{value}\"").ok(); }
        None => { writeln!(err, "declare: {name}: not found").ok(); }
    }
}

//record one "NAME=VALUE" argument, refusing names that are not identifiers
fn assign(shell: &mut Shell, argument: &str, err: &mut dyn Write) {
    //without "=" there is only a name, and nothing to store
    let (name, value) = match argument.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (argument, None),
    };

    if !is_identifier(name) {
        //the error quotes the argument as it was written, backtick included
        writeln!(err, "declare: `{argument}\': not a valid identifier").ok();
        return;
    }

    if let Some(value) = value {
        shell.variables.insert(name.to_string(), value.to_string());
    }
}

//a name starts with a letter or an underscore, the rest may also hold digits
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();

    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }

    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
    }

    true
}
