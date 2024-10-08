use anyhow::{Context, Result};
use std::io::{Stdout, Write};
use std::{thread, time};
use termion::cursor;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::RawTerminal;

pub fn clear<W: Write>(stdout: &mut W) -> Result<()> {
    write!(
        stdout,
        "{}{}{}",
        termion::clear::All,
        cursor::Goto(1, 1),
        cursor::Show
    )?;

    Ok(())
}

pub fn prompt(
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
    prompt_string: String,
) -> Result<String> {
    stdout.suspend_raw_mode()?;
    clear(stdout)?;
    write!(stdout, "{}", prompt_string)?;
    let mut buffer = String::new();
    stdout.flush()?;
    stdin.read_line(&mut buffer)?;
    let answer = buffer.trim().to_string();
    stdout.activate_raw_mode()?;

    Ok(answer)
}

pub fn prompt_yesno(
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
    prompt_string: String,
) -> Result<bool> {
    clear(stdout)?;
    write!(stdout, "{}", prompt_string)?;
    stdout.flush()?;

    for event in stdin.keys() {
        let key = event.with_context(|| "Error evaluating keystroke event")?;
        let value = match key {
            Key::Char('y') => true,
            Key::Char('Y') => true,
            Key::Char('n') => false,
            Key::Char('N') => false,
            _ => continue,
        };

        return Ok(value);
    }

    Ok(false)
}

// Flash a warning for 1s. Useful in the case of a invalid prompt entry.
pub fn flash_warning<W: Write>(stdout: &mut W, warning_text: String) -> Result<()> {
    clear(stdout)?;
    write!(stdout, "{}", warning_text)?;
    stdout.flush()?;
    thread::sleep(time::Duration::from_secs(1));
    Ok(())
}
