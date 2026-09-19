use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

//common signature for every builtin
//"out" and "err" are the output destinations: the terminal, or a file when a redirection is used
type Builtin = fn(&[&str], &mut dyn Write, &mut dyn Write);

//redirection targets found on the command line, None means "keep the terminal"
struct Redirect {
    out: Option<String>,
    err: Option<String>,
}

const BUILTINS: &[(&str, Builtin)] = &[
    ("exit", exit_command),
    ("echo", echo_command),
    ("type", type_command),
    ("pwd", pwd_command),
    ("cd", cd_command),
];

fn main() {
    //initialization of the built-in commands
    let commands: HashMap<&str, Builtin> = BUILTINS.iter().copied().collect();

    loop 
    {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input= String::new();
        if io::stdin().read_line(&mut input).unwrap() == 0 {                     //read_line returns the number of bytes read, 0 means end of file (Ctrl+D)
            break;
        }

        let (cmd_tokens, redirect) = split_redirect(tokenize(&input));           //split the line into tokens, then extract the redirections if there are any

        let Some((name, args)) = cmd_tokens.split_first()                        //deconstruct the input into the command ("name") and its arguments ("args")
        else {
            continue;
        };

        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();         //builtins take &str, convert the &String without copying
        match commands.get(name.as_str()) {
            Some(f) => {
                let Some(mut out_file) = open_target(&redirect.out) else { continue };
                let Some(mut err_file) = open_target(&redirect.err) else { continue };

                let mut stdout = io::stdout();
                let mut stderr = io::stderr();

                let out: &mut dyn Write = match out_file.as_mut() {              //pick the file if there is one, the terminal otherwise
                    Some(file) => file,
                    None => &mut stdout,
                };
                let err: &mut dyn Write = match err_file.as_mut() {
                    Some(file) => file,
                    None => &mut stderr,
                };

                f(&args, out, err);
            }
            None => run_external(                                                //as_deref() turns Option<String> into Option<&str>
                name,
                &args,
                redirect.out.as_deref(),
                redirect.err.as_deref(),
            ),
        }
    }
}

//open a redirection file, or return None on failure so the caller can skip the command
//Some(None) means "no redirection asked", Some(Some(file)) means "file ready"
fn open_target(path: &Option<String>) -> Option<Option<File>> {
    match path {
        Some(p) => match File::create(p) {                                       //File::create makes the file, or truncates it if it already exists
            Ok(file) => Some(Some(file)),
            Err(e) => {
                eprintln!("{}: {}", p, e);
                None
            }
        },
        None => Some(None),
    }
}

fn exit_command(args: &[&str], _out: &mut dyn Write, _err: &mut dyn Write)
{
    let code = args
        .first()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);                                                           //"exit" alone means "exit 0", an invalid argument means 0 too
    std::process::exit(code);
}

fn echo_command(args: &[&str], out: &mut dyn Write, _err: &mut dyn Write) 
{
    writeln!(out, "{}", args.join(" ")).ok();
}

fn pwd_command(_args: &[&str], out: &mut dyn Write, _err: &mut dyn Write) 
{
    writeln!(out, "{}", env::current_dir().unwrap().display()).ok();
}

//NEED TO REWORK THIS PART, THE ~ MANAGEMENT CAN BE IMPROVED, BUT IT WORKS FOR NOW
fn cd_command(args: &[&str], _out: &mut dyn Write, err: &mut dyn Write) 
{
    let raw = args.first().copied().unwrap_or("~");
    let path = raw.replacen('~', &env::var("HOME").unwrap_or_default(), 1);

    if env::set_current_dir(&path).is_err() {
    writeln!(err, "cd: {path}: No such file or directory").ok();                 //an error message goes to stderr, so "2>" can catch it
    }
}

fn type_command(args: &[&str], out: &mut dyn Write, _err: &mut dyn Write) 
{
    let Some(name) = args.first() else { return };

    if BUILTINS.iter().any(|(n, _)| n == name) {
        writeln!(out, "{name} is a shell builtin").ok();                         //check if the command is builtin
    } else if let Some(path) = find_in_path(name) {
        writeln!(out, "{name} is {}", path.display()).ok();                      //if not, check if the command is in the PATH
    } else {
        writeln!(out, "{name}: not found").ok();                                 //if the command do not exists
    }  
}

//check if a file is executable 
//use in find_in_path()
//CODECRAFTER USES A UNIX OS, SO THIS PART IS ONLY TO VALIDATE THE TESTS AND GO TO THE NEXT STEP
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

//run an external program
//each stream that has a redirection is plugged on its file, the others stay on the terminal
fn run_external(name: &str, args: &[&str], out: Option<&str>, err: Option<&str>) {
    let mut cmd = Command::new(name);
    cmd.args(args);

    if let Some(path) = out {
        match File::create(path) {                                               //the file is created even if the command fails afterwards ("cat missing > out.txt" leaves out.txt empty)
            Ok(file) => { cmd.stdout(Stdio::from(file)); }
            Err(e) => { eprintln!("{}: {}", path, e); return; }
        }
    }

    if let Some(path) = err {
        match File::create(path) {                                               //same here: "echo hello 2> errors.txt" leaves errors.txt empty
            Ok(file) => { cmd.stderr(Stdio::from(file)); }
            Err(e) => { eprintln!("{}: {}", path, e); return; }
        }
    }

    match cmd.status() {                                                         //status() waits for the program to finish, Err means the launch itself failed
        Ok(_) => {}
        Err(_) => println!("{}: command not found", name),
    }
}

//split the input line into tokens, handling single quotes, double quotes and backslash escaping
fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut has_token = false;                                                   //a token is being built
    let mut in_single = false;
    let mut in_double = false;

    let mut chars = line.chars();                                                //the iterator is advanced by hand to consume the character following a backslash
    while let Some(c) = chars.next() {
        if in_single {
            if c == '\'' {                                                       //everything is literal inside single quotes, except the closing quote
                in_single = false;
            } else {
                current.push(c);
            }
        } else if in_double {
            if c == '"' {
                in_double = false;
            } else if c == '\\' {
                match chars.next() {                                             //inside double quotes the backslash only escapes a short list of characters, elsewhere it stays literal
                    Some(next @ ('"' | '\\' | '$' | '`')) => current.push(next),
                    Some(next) => {
                        current.push('\\');
                        current.push(next);
                    }
                    None => current.push('\\'),
                }
            } else {
                current.push(c);
            }
        } else if c == '\\' {
            if let Some(next) = chars.next() {                                   //outside quotes the backslash escapes any character
                current.push(next);
            }
            has_token = true;
        } else if c == '\'' {
            in_single = true;
            has_token = true;                                                    //'' alone must produce an empty argument
        } else if c == '"' {
            in_double = true;
            has_token = true;
        } else if c.is_whitespace() {
            if has_token {                                                       //only a whitespace outside quotes ends a token, that is why 'hello''world' gives a single argument
                tokens.push(std::mem::take(&mut current));
                has_token = false;
            }
        } else {
            current.push(c);
            has_token = true;
        }
    }

    if has_token {                                                               //last token, when the line does not end with a whitespace
        tokens.push(current);
    }

    tokens
}

//separate the command tokens from the redirection targets
//"cat missing 2> err.txt" gives (["cat", "missing"], Redirect { out: None, err: Some("err.txt") })
fn split_redirect(tokens: Vec<String>) -> (Vec<String>, Redirect) {
    let mut cmd = Vec::new();
    let mut redirect = Redirect { out: None, err: None };
    let mut iter = tokens.into_iter();                                           //into_iter() consumes the Vec and gives owned String, so no clone

    while let Some(t) = iter.next() {
        match t.as_str() {
            ">" | "1>" => redirect.out = iter.next(),                            //the next token is the target, consume it so it does not end up in the arguments
            "2>" => redirect.err = iter.next(),
            _ => cmd.push(t),
        }
    }

    (cmd, redirect)
}
