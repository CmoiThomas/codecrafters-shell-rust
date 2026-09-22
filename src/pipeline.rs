//runs a pipeline: "cat file | head -n 3 | wc"
//the output of each command becomes the input of the next one

use std::io::{self, Write};
use std::process::{Child, ChildStdout, Command, Stdio};

use crate::builtins::{self, Builtin};
use crate::exec::{build_command, run_builtin};
use crate::redirect::{split_redirect, Redirect};
use crate::shell::Shell;

//where a command reads its input from
enum Source {
    Terminal,                                                                    //the first command of the pipeline
    Pipe(ChildStdout),                                                           //the output of an external command, streamed
    Text(Vec<u8>),                                                               //the output of a builtin, which ran inside the shell
}

//start every command of the pipeline, then wait for all of them
pub fn run(shell: &mut Shell, stages: Vec<Vec<String>>) {
    let count = stages.len();
    let mut children = Vec::new();
    let mut input = Source::Terminal;

    for (index, tokens) in stages.into_iter().enumerate() {
        let is_last = index + 1 == count;
        let (tokens, redirect) = split_redirect(tokens);

        let Some((name, args)) = tokens.split_first() else {
            input = Source::Terminal;                                            //an empty stage, like in "ls |": drop what came before
            continue;
        };

        let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        match builtins::find(name) {
            //a builtin runs inside the shell, so its input is simply ignored
            //assigning the result closes the pipe it was reading from
            Some(f) => input = run_builtin_stage(shell, f, &args, &redirect, is_last),
            None => match spawn_stage(name, &args, &redirect, input, is_last) {
                Some((child, next)) => {
                    children.push(child);
                    input = next;
                }
                None => input = Source::Terminal,
            },
        }
    }

    //like bash, the shell waits for every command of the pipeline
    for mut child in children {
        child.wait().ok();
    }
}

//run a builtin as one stage of the pipeline
//the last stage writes where it normally would, the others into memory
fn run_builtin_stage(shell: &mut Shell, f: Builtin, args: &[&str], redirect: &Redirect, is_last: bool) -> Source {
    if is_last {
        run_builtin(shell, f, args, redirect);
        return Source::Terminal;
    }

    let mut output = Vec::new();
    let mut stderr = io::stderr();                                               //error messages keep going to the terminal
    f(shell, args, &mut output, &mut stderr);

    Source::Text(output)
}

//start an external command as one stage of the pipeline
fn spawn_stage(name: &str, args: &[&str], redirect: &Redirect, input: Source, is_last: bool) -> Option<(Child, Source)> {
    let mut cmd = build_command(name, args, redirect)?;

    //what this command reads
    let mut text = None;
    match input {
        Source::Terminal => {}
        Source::Pipe(previous) => { cmd.stdin(Stdio::from(previous)); }
        Source::Text(bytes) => {
            cmd.stdin(Stdio::piped());                                           //the shell will write the bytes itself
            text = Some(bytes);
        }
    }

    //what this command writes: the terminal for the last one, a pipe for the others
    //a redirection on this stage wins, and build_command already set it up
    if !is_last && redirect.out.is_none() {
        cmd.stdout(Stdio::piped());
    }

    let mut child = start(cmd, name)?;

    //hand the output of a builtin over, then close the pipe so the command sees the end of its input
    if let Some(bytes) = text {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&bytes).ok();
        }
    }

    let next = match child.stdout.take() {
        Some(output) => Source::Pipe(output),
        None => Source::Terminal,
    };

    Some((child, next))
}

fn start(mut cmd: Command, name: &str) -> Option<Child> {
    match cmd.spawn() {
        Ok(child) => Some(child),
        Err(_) => {
            println!("{name}: command not found");
            None
        }
    }
}
