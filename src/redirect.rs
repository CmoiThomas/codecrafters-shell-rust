//finds the redirection operators in a token list and opens the files they point to

use std::fs::{File, OpenOptions};
use std::io;

//one redirection target: where to write, and whether to append or to overwrite
pub struct Target {
    pub path: String,
    pub append: bool,
}

//redirection targets found on the command line, None means "keep the terminal"
pub struct Redirect {
    pub out: Option<Target>,
    pub err: Option<Target>,
}

//separate the command tokens from the redirection targets
//"echo hi >> out.txt" gives (["echo", "hi"], out = out.txt in append mode)
pub fn split_redirect(tokens: Vec<String>) -> (Vec<String>, Redirect) {
    let mut cmd = Vec::new();
    let mut redirect = Redirect { out: None, err: None };
    let mut iter = tokens.into_iter();                                           //into_iter() gives owned String, so no clone

    while let Some(token) = iter.next() {
        //the token right after an operator is its target, consume it so it does
        //not end up in the command arguments
        match token.as_str() {
            ">" | "1>" => redirect.out = take_target(iter.next(), false),
            ">>" | "1>>" => redirect.out = take_target(iter.next(), true),
            "2>" => redirect.err = take_target(iter.next(), false),
            "2>>" => redirect.err = take_target(iter.next(), true),
            _ => cmd.push(token),
        }
    }

    (cmd, redirect)
}

fn take_target(path: Option<String>, append: bool) -> Option<Target> {
    path.map(|path| Target { path, append })
}

//open a redirection file
//">" truncates it, ">>" writes at the end, both create it when it does not exist
pub fn open_file(target: &Target) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .append(target.append)
        .truncate(!target.append)
        .open(&target.path)
}

//open the file a target points to, if there is one
//Ok(None) means no redirection was asked, Err means the caller should give up
pub fn open_target(target: &Option<Target>) -> Result<Option<File>, ()> {
    let Some(target) = target else { return Ok(None) };

    match open_file(target) {
        Ok(file) => Ok(Some(file)),
        Err(e) => {
            eprintln!("{}: {}", target.path, e);
            Err(())
        }
    }
}
