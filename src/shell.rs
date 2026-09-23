//everything the shell remembers from one command to the next
//builtins receive it so they can read and change it

use std::collections::HashMap;

use crate::jobs::Job;

//shell variables created with "declare", by name
pub type Variables = HashMap<String, String>;

pub struct Shell {
    //completer scripts registered with "complete -C", by command name
    //"git" -> "/path/to/git/completer"
    pub completers: HashMap<String, String>,

    //commands started with "&", ordered by job number
    pub jobs: Vec<Job>,

    //every command line entered, oldest first
    pub history: Vec<String>,

    //how many history entries have already been written to the history file
    pub appended: usize,

    //variables created with "declare", used by parameter expansion
    pub variables: Variables,
}

impl Shell {
    pub fn new() -> Shell {
        Shell {
            completers: HashMap::new(),
            jobs: Vec::new(),
            history: Vec::new(),
            appended: 0,
            variables: Variables::new(),
        }
    }
}
