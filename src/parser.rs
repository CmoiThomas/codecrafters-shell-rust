//turns a raw input line into the list of words the shell will work with
//handles single quotes, double quotes, backslash escaping and parameter expansion

use std::env;
use std::iter::Peekable;
use std::str::Chars;

use crate::shell::Variables;

//split the input line into tokens, expanding "$NAME" and "${NAME}" on the way
pub fn tokenize(line: &str, variables: &Variables) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut has_token = false;                                                   //a token is being built
    let mut quoted = false;                                                      //this token contains quotes, so it counts even if it ends up empty
    let mut in_single = false;
    let mut in_double = false;

    let mut chars = line.chars().peekable();                                      //peekable, to look at the character after a "$" without eating it
    while let Some(c) = chars.next() {
        if in_single {
            //everything is literal inside single quotes, except the closing quote
            if c == '\'' {
                in_single = false;
            } else {
                current.push(c);
            }
            continue;
        }

        if in_double {
            match c {
                '"' => in_double = false,
                '\\' => escape_in_double(chars.next(), &mut current),
                '$' => expand(&mut chars, variables, &mut current),              //variables are expanded inside double quotes
                _ => current.push(c),
            }
            continue;
        }

        match c {
            //outside quotes the backslash escapes any character
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
                has_token = true;
            }
            //'' alone must still produce an empty argument
            '\'' => {
                in_single = true;
                has_token = true;
                quoted = true;
            }
            '"' => {
                in_double = true;
                has_token = true;
                quoted = true;
            }
            '$' => {
                expand(&mut chars, variables, &mut current);
                has_token = true;
            }
            //only a whitespace outside quotes ends a token
            //that is why 'hello''world' gives a single argument
            _ if c.is_whitespace() => {
                if has_token {
                    push_token(&mut tokens, std::mem::take(&mut current), quoted);
                    has_token = false;
                    quoted = false;
                }
            }
            _ => {
                current.push(c);
                has_token = true;
            }
        }
    }

    if has_token {                                                               //last token, when the line does not end with a whitespace
        push_token(&mut tokens, current, quoted);
    }

    tokens
}

//a word left empty by an unset variable disappears, but "" and '' still count
//so "${missing}" passes no argument at all, while '' passes an empty one
fn push_token(tokens: &mut Vec<String>, token: String, quoted: bool) {
    if quoted || !token.is_empty() {
        tokens.push(token);
    }
}

//inside double quotes the backslash only escapes a short list of characters
//in front of anything else it stays literal, so both characters are kept
fn escape_in_double(next: Option<char>, current: &mut String) {
    match next {
        Some(c) if matches!(c, '"' | '\\' | '$' | '`') => current.push(c),
        Some(c) => {
            current.push('\\');
            current.push(c);
        }
        None => current.push('\\'),
    }
}

//read the variable name that follows a "$" and append its value
//"$NAME" ends at the first character a name cannot contain
//"${NAME}" is delimited by the braces, so literal text can follow right after
fn expand(chars: &mut Peekable<Chars>, variables: &Variables, current: &mut String) {
    let mut name = String::new();

    match chars.peek() {
        Some('{') => {
            chars.next();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                name.push(c);
            }
        }
        //a name starts with a letter or an underscore, then allows digits too
        Some(c) if c.is_ascii_alphabetic() || *c == '_' => {
            while let Some(c) = chars.peek() {
                if c.is_ascii_alphanumeric() || *c == '_' {
                    name.push(*c);
                    chars.next();
                } else {
                    break;
                }
            }
        }
        //a "$" in front of anything else is just a character
        _ => {
            current.push('$');
            return;
        }
    }

    current.push_str(&value_of(&name, variables));
}

//a shell variable first, then the environment, and nothing at all when unset
fn value_of(name: &str, variables: &Variables) -> String {
    match variables.get(name) {
        Some(value) => value.clone(),
        None => env::var(name).unwrap_or_default(),
    }
}

//a trailing "&" asks for the command to run in the background
//it is removed from the tokens, and the answer tells whether it was there
pub fn take_background_marker(tokens: &mut Vec<String>) -> bool {
    if tokens.last().is_some_and(|t| t == "&") {
        tokens.pop();
        return true;
    }
    false
}

//cut the tokens at every "|", one group per command of the pipeline
//there is always at least one group, so a line without "|" gives a single command
pub fn split_pipeline(tokens: Vec<String>) -> Vec<Vec<String>> {
    let mut stages = vec![Vec::new()];

    for token in tokens {
        if token == "|" {
            stages.push(Vec::new());
        } else if let Some(stage) = stages.last_mut() {
            stage.push(token);
        }
    }

    stages
}
