mod builtins;
mod completion;
mod exec;
mod input;
mod parser;
mod redirect;
mod shell;

use input::read_line;
use parser::tokenize;
use redirect::split_redirect;
use shell::Shell;

fn main() {
    let mut shell = Shell::new();

    loop {
        let Some(line) = read_line(&shell) else { break };                       //None means end of input (Ctrl+D on an empty line)

        let (tokens, redirect) = split_redirect(tokenize(&line));                //split the line into tokens, then extract the redirections

        let Some((name, args)) = tokens.split_first() else { continue };         //an empty line has nothing to run

        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        match builtins::find(name) {
            Some(f) => exec::run_builtin(&mut shell, f, &args, &redirect),
            None => exec::run_external(name, &args, &redirect),
        }
    }
}
