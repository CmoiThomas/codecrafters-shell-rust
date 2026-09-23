mod builtins;
mod completion;
mod exec;
mod history;
mod input;
mod jobs;
mod parser;
mod pipeline;
mod redirect;
mod shell;

use std::io;

use input::read_line;
use parser::{split_pipeline, take_background_marker, tokenize};
use redirect::split_redirect;
use shell::Shell;

fn main() {
    let mut shell = Shell::new();
    history::load_at_startup(&mut shell);                                        //HISTFILE, when it is set, fills the history before the first prompt

    loop {
        jobs::report(&mut shell.jobs, &mut io::stdout(), false);                 //announce the background jobs that finished since the last prompt

        let Some(line) = read_line(&shell) else {                                //None means end of input (Ctrl+D on an empty line)
            history::save_at_exit(&mut shell);
            break;
        };

        //the line is remembered before it runs, so "history" can list itself
        let entry = line.trim();
        if !entry.is_empty() {
            shell.history.push(entry.to_string());
        }

        let mut tokens = tokenize(&line, &shell.variables);
        let background = take_background_marker(&mut tokens);                    //a trailing "&" runs the command without waiting for it
        let command_text = tokens.join(" ");                                     //kept for the jobs table, redirections included

        let mut stages = split_pipeline(tokens);                                 //one group of tokens per command of the pipeline

        if stages.len() > 1 {
            pipeline::run(&mut shell, stages);
            continue;
        }

        let Some(tokens) = stages.pop() else { continue };
        let (tokens, redirect) = split_redirect(tokens);                         //extract the redirections

        let Some((name, args)) = tokens.split_first() else { continue };         //an empty line has nothing to run

        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        match builtins::find(name) {
            //a builtin runs inside the shell itself, so it always runs in the foreground
            Some(f) => exec::run_builtin(&mut shell, f, &args, &redirect),
            None if background => exec::run_background(&mut shell, name, &args, &redirect, command_text),
            None => exec::run_external(name, &args, &redirect),
        }
    }
}
