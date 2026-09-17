#[allow(unused_imports)]
use std::io::{self, Write};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const BUILTINS: &[(&str, fn(&[&str]))] = &[
    ("exit", exit_command),
    ("echo", echo_command),
    ("type", type_command),
    ("pwd", pwd_command),
    ("cd", cd_command),
];

fn main() {
    //initialization of the built-in commands
    let commands: HashMap<&str, fn(&[&str])> = BUILTINS.iter().copied().collect();
    
    loop 
    {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input= String::new();
        io::stdin().read_line(&mut input).unwrap();

        let words: Vec<&str> = input.split_whitespace().collect();

        let Some((name, args)) = words.split_first()                               //deconstruct the input into the command ("name") and its arguments (args")
        else {
            continue; //if no command is entered, continue to the next iteration of the loop
        };

        match commands.get(*name) {
            Some(f) => f(args),
            None => run_external(name, args),
        }
    }
}

fn exit_command(args: &[&str])
{
    std::process::exit(0);
}

fn echo_command(args: &[&str]) 
{
    println!("{}",args.join(" "));
}

fn pwd_command(args: &[&str]) 
{
    println!("{}", env::current_dir().unwrap().display());
}

fn cd_command(args: &[&str]) 
{
    let raw = args.first().copied().unwrap_or("~");
    let path = raw.replacen('~', &env::var("HOME").unwrap_or_default(), 1);

    if env::set_current_dir(&path).is_err() {
    eprintln!("cd: {path}: No such file or directory");
    }
}

fn type_command(args: &[&str]) 
{
    let Some(name) = args.first() else { return };

    if BUILTINS.iter().any(|(n, _)| n == name) {
        println!("{name} is a shell builtin");                                  //check if the command is builtin
    } else if let Some(path) = find_in_path(name) {
        println!("{name} is {}", path.display());                               //if not, check if the command is in the PATH
    } else {
        println!("{name}: not found");                                          //if the command do not exists
    }  
}

//check if a file is executable 
//use in find_in_path()
//CODECRAFTER USE A UNIX OS, SO THIS PART IS ONLY TO VALIDATE THE TESTS AND GO TO THE NEXT STEP
#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

//check if the file specified by "name" exists
//use in type_command()
fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var("PATH").ok()?;
    env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| is_executable(p))
}

fn run_external(name: &str, args: &[&str]) {
    match Command::new(name).args(args).status() {
        Ok(_) => {}
        Err(_) => println!("{}: command not found", name),
    }
}