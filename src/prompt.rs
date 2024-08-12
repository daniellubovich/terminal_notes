use std::io::{Stdout, Write};
use termion::cursor;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::RawTerminal;

pub fn clear<W: Write>(stdout: &mut W) {
    write!(
        stdout,
        "{}{}{}",
        termion::clear::All,
        cursor::Goto(1, 1),
        cursor::Show
    )
    .unwrap();
}

pub fn prompt(
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
    prompt_string: String,
    value: &mut String,
) {
    stdout.suspend_raw_mode().unwrap();
    clear(stdout);
    write!(stdout, "{}", prompt_string).unwrap();
    let mut buffer = String::new();
    stdout.flush().unwrap();
    stdin.read_line(&mut buffer).unwrap();
    *value = buffer.trim().to_string();
    stdout.activate_raw_mode().unwrap();
}

pub fn prompt_yesno(
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
    prompt_string: String,
) -> bool {
    clear(stdout);
    write!(stdout, "{}", prompt_string).unwrap();
    stdout.flush().unwrap();

    let mut value = false;

    if let Some(c) = stdin.keys().next() {
        value = match c.unwrap() {
            Key::Char('y') => true,
            Key::Char('Y') => true,
            Key::Char('n') => false,
            Key::Char('N') => false,
            _ => false,
        };
    }

    value
}
