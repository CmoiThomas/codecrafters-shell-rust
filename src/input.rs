//reads one line from the keyboard in raw mode, so the Tab key reaches the shell
//raw mode means nothing is echoed and nothing is buffered: every character the
//user sees on screen is printed here

use std::io::{self, Write};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::completion::{candidates_for, common_prefix, split_word};
use crate::shell::Shell;

pub const PROMPT: &str = "$ ";

//read a whole line, returns None on Ctrl+D with an empty line (end of input)
pub fn read_line(shell: &Shell) -> Option<String> {
    show(PROMPT);

    let mut buffer = String::new();
    let mut last_was_tab = false;                                                //a second Tab in a row lists the matches instead of ringing the bell
    let mut recalled = shell.history.len();                                      //where the arrows are in the history, len() means "the fresh line"


    enable_raw_mode().ok()?;

    loop {
        let Some(key) = next_key() else { continue };

        match key.code {
            //"\n" alone would not bring the cursor back to column 0 in raw mode
            KeyCode::Enter => {
                show("\r\n");
                break;
            }
            //some clients send LF (Ctrl+J) instead of CR for the Enter key
            KeyCode::Char('j') if is_ctrl(&key) => {
                show("\r\n");
                break;
            }
            KeyCode::Tab => {
                handle_tab(shell, &mut buffer, &mut last_was_tab);
            }
            //the arrows walk through the history and rewrite the line
            KeyCode::Up => {
                if recalled > 0 {
                    recalled -= 1;
                    buffer = shell.history[recalled].clone();
                    redraw(&buffer);
                }
                last_was_tab = false;
            }
            KeyCode::Down => {
                if recalled < shell.history.len() {
                    recalled += 1;
                    //past the last entry, the line goes back to being empty
                    buffer = match shell.history.get(recalled) {
                        Some(entry) => entry.clone(),
                        None => String::new(),
                    };
                    redraw(&buffer);
                }
                last_was_tab = false;
            }
            KeyCode::Backspace => {
                if buffer.pop().is_some() {
                    show("\u{8} \u{8}");                                         //move left, erase with a space, move left again
                }
                last_was_tab = false;
            }
            //Ctrl+D on an empty line ends the shell
            KeyCode::Char('d') if is_ctrl(&key) && buffer.is_empty() => {
                disable_raw_mode().ok();
                return None;
            }
            //Ctrl+C drops the line and starts a new one
            KeyCode::Char('c') if is_ctrl(&key) => {
                buffer.clear();
                show(&format!("\r\n{PROMPT}"));
                last_was_tab = false;
            }
            KeyCode::Char(_) if is_ctrl(&key) => {}                              //ignore the other control combinations
            KeyCode::Char(c) => {
                buffer.push(c);
                show(&c.to_string());                                            //raw mode does not echo the typed characters
                last_was_tab = false;
            }
            _ => {}
        }
    }

    disable_raw_mode().ok();                                                     //external programs inherit the terminal, they need it back in normal mode
    Some(buffer)
}

//complete the word being typed, or tell the user nothing can be done
fn handle_tab(shell: &Shell, buffer: &mut String, last_was_tab: &mut bool) {
    let (head, word) = split_word(buffer);
    let head = head.to_string();                                                 //buffer is about to be rewritten, keep an owned copy
    let typed = word.len();

    let names = candidates_for(shell, buffer);
    let prefix = common_prefix(&names);

    //a single match is completed entirely, with a trailing space unless it is a
    //directory, which keeps its "/" so the next level can be completed right away
    if names.len() == 1 {
        let full = match names[0].ends_with('/') {
            true => names[0].clone(),
            false => format!("{} ", names[0]),
        };
        show(&full[typed..]);                                                    //only print what was added
        *buffer = format!("{head}{full}");
        *last_was_tab = false;
        return;
    }

    //several matches: complete up to what they all share, without a trailing space
    if prefix.len() > typed {
        show(&prefix[typed..]);
        *buffer = format!("{head}{prefix}");
        *last_was_tab = false;
        return;
    }

    //nothing can be added: ring the bell, and list the matches on a second Tab
    if names.is_empty() || !*last_was_tab {
        show("\x07");
        *last_was_tab = true;
    } else {
        show(&format!("\r\n{}\r\n{PROMPT}{buffer}", names.join("  ")));
        *last_was_tab = false;
    }
}

//rewrite the whole line: back to column 0, erase to the end, prompt, content
//"\x1b[K" is the escape sequence terminals understand as "clear to end of line"
fn redraw(buffer: &str) {
    show(&format!("\r\x1b[K{PROMPT}{buffer}"));
}

//wait for a key press, ignoring anything else the terminal reports
fn next_key() -> Option<KeyEvent> {
    match event::read() {
        //windows also reports key releases, only presses matter here
        Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => Some(key),
        _ => None,
    }
}

fn is_ctrl(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
}

//print without a newline, and push it out right away
//raw mode buffers nothing for us, so every character needs its own flush
fn show(text: &str) {
    print!("{text}");
    io::stdout().flush().ok();
}
