#[allow(unused_imports)]
use std::io::{self, Write};
use std::collections::HashMap;

fn main() {
    //initialization of the built-in commands
    let mut commands: HashMap<&str, fn(&[&str])> = HashMap::new();
    commands.insert("exit", exit_command);
    commands.insert("echo", echo_command);

    loop 
    {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input= String::new();
        io::stdin().read_line(&mut input).unwrap();

        let words: Vec<&str> = input.split_whitespace().collect();

        let Some((name, args)) = words.split_first() 
        else {
            continue; //if no command is entered, continue to the next iteration of the loop
        };

        match commands.get(*name) {
            Some(f) => f(args),
            None => println!("{}: command not found", name),
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