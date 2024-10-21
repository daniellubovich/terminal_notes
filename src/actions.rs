use crate::config::Config;
use crate::note_entry::NoteEntry;
use crate::prompt::{flash_warning, prompt, prompt_yesno};
use crate::providers::provider::NotesProvider;

use anyhow::{Context, Result};
use log::debug;
use std::io::{Stdout, Write};
use std::path::Path;
use std::rc::Rc;
use std::time::SystemTime;
use std::{thread, time};
use termion::cursor;
use termion::raw::RawTerminal;

pub fn delete_note<T: NotesProvider>(
    note_to_del: &Rc<NoteEntry>,
    notes_provider: &T,
    config: &Config,
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
) -> Result<bool> {
    let path_str = note_to_del
        .path
        .to_str()
        .context("could not convert file path to string")?;
    if path_str.is_empty() {
        flash_warning(
            stdout,
            format!("empty path found for note {}", note_to_del.name),
        )?;
    } else if path_str.contains(config.get_default_notes_file()) {
        flash_warning(
            stdout,
            format!(
                "{}{}Cannot delete your default notes file.",
                termion::clear::All,
                cursor::Goto(1, 1),
            ),
        )?;
    } else {
        let affirmative = prompt_yesno(
            stdout,
            stdin,
            format!("Are you sure you want to delete {}? [y/N] ", path_str),
        )?;

        if affirmative {
            notes_provider
                .delete_note(note_to_del)
                .context("could not delete note")?;
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn create_note<T: NotesProvider>(
    notes_provider: &T,
    config: &Config,
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
) -> Result<()> {
    loop {
        // Prompt in a loop, only exiting if we create a valid file.
        let note_name = prompt(
            stdout,
            stdin,
            String::from("Enter a name for your new note file: "),
        )?;

        let new_note_path = format!("{}{}", config.get_notes_directory(), note_name);
        let new_note_path = Path::new(&new_note_path);
        let new_note_path = match new_note_path.extension() {
            Some(_) => new_note_path.to_path_buf(),
            None => {
                // Add an extension if there isn't one.
                let mut new_note_path = new_note_path.to_path_buf();
                new_note_path.set_extension(config.get_default_file_extension());
                new_note_path
            }
        };

        let note = NoteEntry::new(new_note_path, note_name, SystemTime::now(), false, 0);

        if note.name.is_empty() {
            debug!("note name is empty. exiting prompt.");
            return Ok(());
        }

        match notes_provider.note_exists(&note.path) {
            false => {
                notes_provider.create_note(note)?;
                return Ok(());
            }
            true => {
                // Check for empty entry.  Re-prompt if it is.
                let new_note_path = note
                    .path
                    .to_str()
                    .context("could not convert file path to string")?;
                flash_warning(stdout, format!("note {} already exists", new_note_path))?;
            }
        }
    }
}

pub fn rename_note<T: NotesProvider>(
    selected_note: &Rc<NoteEntry>,
    notes_provider: &T,
    config: &Config,
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
) -> Result<()> {
    loop {
        // Prompt in a loop, only exiting if we create a valid file.
        let note_name = prompt(
            stdout,
            stdin,
            format!("Enter a new name for '{}': ", selected_note.name),
        )?;

        // Check for empty entry.  Re-prompt if it is.
        if note_name.is_empty() {
            debug!("note name is empty. exiting prompt.");
            write!(
                stdout,
                "{}",
                String::from("Note name empty. Please enter a valid name.")
            )?;
            thread::sleep(time::Duration::from_secs(1));
            continue;
        }

        let new_note_path = format!("{}{}", config.get_notes_directory(), note_name);
        let new_note_path = Path::new(&new_note_path);
        let new_note_path = match new_note_path.extension() {
            Some(_) => new_note_path.to_path_buf(),
            None => {
                // Add an extension if there isn't one.
                let mut new_note_path = new_note_path.to_path_buf();
                new_note_path.set_extension(config.get_default_file_extension());
                new_note_path
            }
        };

        let mut new_note = (**selected_note).clone();
        new_note.path = new_note_path;

        match notes_provider.note_exists(&new_note.path) {
            false => {
                // Note with new path doesn't already exist, so we're good to
                // try to rename it.
                notes_provider.rename_note(selected_note, &new_note.path)?;
                return Ok(());
            }
            _ => {
                // If it failed to validate for some reason, write out the error and
                // re-prompt.
                let new_note_path_str = new_note
                    .path
                    .to_str()
                    .context("could not convert file path to string")?;
                flash_warning(
                    stdout,
                    format!(
                        "Note {} already exists. Please enter a unique file name.",
                        new_note_path_str
                    ),
                )?;
            }
        }
    }
}
