#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    print!("$ ");
    io::stdout().flush().unwrap();

    let mut input= String::new();
    io::stdin().read_line(&mut input).unwrap();

    println!("{}: command not found", input.trim()); //the read_line method includes the newline character in the input, so we use trim() to remove it before printing the error message
}
