//everything the shell remembers from one command to the next
//builtins receive it so they can read and change it

use std::collections::HashMap;

pub struct Shell {
    //completer scripts registered with "complete -C", by command name
    //"git" -> "/path/to/git/completer"
    pub completers: HashMap<String, String>,
}

impl Shell {
    pub fn new() -> Shell {
        Shell {
            completers: HashMap::new(),
        }
    }
}
