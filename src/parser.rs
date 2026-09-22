//turns a raw input line into the list of words the shell will work with
//handles single quotes, double quotes and backslash escaping

//split the input line into tokens
pub fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut has_token = false;                                                   //a token is being built
    let mut in_single = false;
    let mut in_double = false;

    let mut chars = line.chars();                                                //advanced by hand to consume the character following a backslash
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
            if c == '"' {
                in_double = false;
            } else if c == '\\' {
                escape_in_double(chars.next(), &mut current);
            } else {
                current.push(c);
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
            }
            '"' => {
                in_double = true;
                has_token = true;
            }
            //only a whitespace outside quotes ends a token
            //that is why 'hello''world' gives a single argument
            _ if c.is_whitespace() => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            _ => {
                current.push(c);
                has_token = true;
            }
        }
    }

    if has_token {                                                               //last token, when the line does not end with a whitespace
        tokens.push(current);
    }

    tokens
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
