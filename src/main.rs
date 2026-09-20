use std::collections::HashMap;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

//common signature for every builtin
//"out" and "err" are the output destinations: the terminal, or a file when a redirection is used
type Builtin = fn(&[&str], &mut dyn Write, &mut dyn Write);

//one redirection target: where to write, and whether to append or to overwrite
struct Target {
    path: String,
    append: bool,
}

//redirection targets found on the command line, None means "keep the terminal"
struct Redirect {
    out: Option<Target>,
    err: Option<Target>,
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

        let Some(input) = read_line_raw() else { break };

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
            None => run_external(name, &args, &redirect),
        }
    }
}

//open a redirection file
//">" truncates the file, ">>" keeps the existing content and writes at the end
//both create the file when it does not exist
fn open_file(t: &Target) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .append(t.append)
        .truncate(!t.append)
        .open(&t.path)
}

//open a redirection file, or return None on failure so the caller can skip the command
//Some(None) means "no redirection asked", Some(Some(file)) means "file ready"
fn open_target(target: &Option<Target>) -> Option<Option<File>> {
    match target {
        Some(t) => match open_file(t) {
            Ok(file) => Some(Some(file)),
            Err(e) => {
                eprintln!("{}: {}", t.path, e);
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

//run an external program
//each stream that has a redirection is plugged on its file, the others stay on the terminal
fn run_external(name: &str, args: &[&str], redirect: &Redirect) {
    let mut cmd = Command::new(name);
    cmd.args(args);

    if let Some(t) = &redirect.out {
        match open_file(t) {                                                     //the file is created even if the command fails afterwards ("cat missing > out.txt" leaves out.txt empty)
            Ok(file) => { cmd.stdout(Stdio::from(file)); }
            Err(e) => { eprintln!("{}: {}", t.path, e); return; }
        }
    }

    if let Some(t) = &redirect.err {
        match open_file(t) {                                                     //same here: "echo hello 2> errors.txt" leaves errors.txt empty
            Ok(file) => { cmd.stderr(Stdio::from(file)); }
            Err(e) => { eprintln!("{}: {}", t.path, e); return; }
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

//build a redirection target from the token that follows the operator
fn take_target(iter: &mut std::vec::IntoIter<String>, append: bool) -> Option<Target> {
    iter.next().map(|path| Target { path, append })                              //consume the next token so it does not end up in the arguments
}

//separate the command tokens from the redirection targets
//"echo hi >> out.txt" gives (["echo", "hi"], Redirect { out: Some(out.txt, append), err: None })
fn split_redirect(tokens: Vec<String>) -> (Vec<String>, Redirect) {
    let mut cmd = Vec::new();
    let mut redirect = Redirect { out: None, err: None };
    let mut iter = tokens.into_iter();                                           //into_iter() consumes the Vec and gives owned String, so no clone

    while let Some(t) = iter.next() {
        match t.as_str() {
            ">" | "1>" => redirect.out = take_target(&mut iter, false),
            ">>" | "1>>" => redirect.out = take_target(&mut iter, true),
            "2>" => redirect.err = take_target(&mut iter, false),
            "2>>" => redirect.err = take_target(&mut iter, true),
            _ => cmd.push(t),
        }
    }

    (cmd, redirect)
}

fn read_line_raw() -> Option<String> {
    let mut buffer = String::new();
    let mut last_was_tab = false;                                                //a second Tab in a row lists the matches instead of ringing the bell
    enable_raw_mode().ok()?;

    loop {
        let Ok(Event::Key(KeyEvent { code, modifiers, kind, .. })) = event::read() else {
            continue;
        };
        if kind != KeyEventKind::Press {                                         //windows also reports key releases, ignore them
            continue;
        }

        match code {
            KeyCode::Enter => {
                print!("\r\n");                                                  //in raw mode "\n" alone does not move the cursor back to column 0
                io::stdout().flush().ok();
                break;
            }
            //THIS PART IS ONLY TO GO THROUGH CODECRAFTER TEST, CODECRAFTER SEND LF (WHICH CORRESPOND TO "CTRL+J", SO IT ADD A "j" AT THE END OF EVERY LINE ) INSTEAD OF CR FOR THE ENTER KEY
            KeyCode::Char('j') if modifiers.contains(KeyModifiers::CONTROL) => {
                print!("\r\n");
                io::stdout().flush().ok();
                break;
            }
            //END OF CODECRAFTER ONLY PART

            KeyCode::Tab => {
                let (head, word) = split_word(&buffer);
                let head = head.to_string();                                     //buffer is about to be reassigned, keep an owned copy
                let word_len = word.len();

                let names = candidates_for(&buffer);
                let prefix = common_prefix(&names);

                if names.len() == 1 {
                    let completed = format!("{} ", names[0]);                    //single match: complete fully, with a trailing space
                    print!("{}", &completed[word_len..]);
                    buffer = format!("{head}{completed}");
                    last_was_tab = false;
                } else if prefix.len() > word_len {
                    print!("{}", &prefix[word_len..]);                           //several matches: complete up to their common prefix, no trailing space
                    buffer = format!("{head}{prefix}");
                    last_was_tab = false;
                } else if names.is_empty() || !last_was_tab {
                    print!("\x07");                                              //nothing to add: ring the bell
                    last_was_tab = true;
                } else {
                    print!("\r\n{}\r\n$ {}", names.join("  "), buffer);          //second Tab in a row: list the matches
                    last_was_tab = false;
                }

                io::stdout().flush().ok();
            }
            KeyCode::Backspace => {
                if buffer.pop().is_some() {
                    print!("\u{8} \u{8}");                                       //move left, erase with a space, move left again
                    io::stdout().flush().ok();
                }
            }
            KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                if buffer.is_empty() {
                    disable_raw_mode().ok();
                    return None;
                }
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                buffer.clear();
                print!("\r\n$ ");
                io::stdout().flush().ok();
            }
            KeyCode::Char(c) => {
                buffer.push(c);
                print!("{c}");                                                   //raw mode does not echo the typed characters
                io::stdout().flush().ok();
            }
            _ => {}
        }
    }

    disable_raw_mode().ok();
    Some(buffer)
}

//collect every builtin and every executable in the PATH starting with the prefix
fn candidates(prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = BUILTINS
        .iter()
        .map(|(n, _)| n.to_string())
        .filter(|n| n.starts_with(prefix))
        .collect();

    if let Ok(path) = env::var("PATH") {
        for dir in env::split_paths(&path) {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };         //a PATH entry may not exist, just skip it
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(prefix) && is_executable(&entry.path()) {
                    names.push(name);
                }
            }
        }
    }

    names.sort();
    names.dedup();                                                               //the same name can appear in several PATH directories
    names
}

//complete a partial command name, with a trailing space so arguments can be typed right away
//returns None when nothing matches or when several names match
fn complete(prefix: &str) -> Option<String> {
    if prefix.is_empty() {
        return None;
    }

    let names = candidates(prefix);
    if names.len() != 1 {
        return None;
    }

    Some(format!("{} ", names[0]))
}

//longest prefix shared by every candidate
//"xyz_foo", "xyz_foo_bar" and "xyz_foo_bar_baz" give "xyz_foo"
fn common_prefix(names: &[String]) -> String {
    let Some(first) = names.first() else { return String::new() };

    let mut prefix: Vec<char> = first.chars().collect();
    for n in &names[1..] {
        let common = prefix
            .iter()
            .zip(n.chars())
            .take_while(|(a, b)| **a == *b)
            .count();                                                            //how many leading characters this name still shares
        prefix.truncate(common);
    }

    prefix.into_iter().collect()
}

//split the line into what stays untouched and the word being completed
//"cat re" gives ("cat ", "re")
fn split_word(line: &str) -> (&str, &str) {
    match line.rfind(' ') {
        Some(i) => line.split_at(i + 1),
        None => ("", line),
    }
}

//files in the current directory starting with the prefix
fn file_candidates(prefix: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(".") else { return Vec::new() };

    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(prefix))
        .collect();

    names.sort();
    names
}

//first word of the line completes commands, the following ones complete filenames
fn candidates_for(line: &str) -> Vec<String> {
    let (head, word) = split_word(line);
    if head.is_empty() {
        candidates(word)
    } else {
        file_candidates(word)
    }
}