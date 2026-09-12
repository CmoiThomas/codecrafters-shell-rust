#[allow(unused_imports)]
use std::io::{self, Write};
use std::collections::HashMap;

fn main() {
    //initialization of the built-in commands
    let mut commands: HashMap<&str, fn(&[&str])> = HashMap::new();
    commands.insert("exit", exit_command);

    loop 
    {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input= String::new();
        io::stdin().read_line(&mut input).unwrap();

        if let Some(f) = commands.get(input.trim()) {     //the read_line method includes the newline character in the input, so we use trim() to remove it
            f(&[]);
        } else {
            println!("{}: command not found", input.trim());            //the read_line method includes the newline character in the input, so we use trim() to remove it before printing the error message
        }
    }
}

fn exit_command(_args: &[&str]) 
{
    std::process::exit(0);
}