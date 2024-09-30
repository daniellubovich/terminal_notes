mod config;
mod note_entry;
mod prompt;
mod providers;

use crate::config::Config;
use crate::note_entry::NoteEntry;
use crate::prompt::{clear, prompt, prompt_yesno};
use crate::providers::file_system_provider::FileSystemNotesProvider;
use crate::providers::provider::NotesProvider;

use chrono::{DateTime, Utc};
use clap::Parser;
use std::io::{stdin, stdout, Result as IOResult, Stdout, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};
use std::{thread, time};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;
use termion::{color, cursor};

const DATE_FORMAT: &str = "%b %m %I:%M";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(
        short = 'g',
        long,
        default_value_t = false,
        exclusive = true,
        help = "Generate a default configuration toml to be used in ~/.noteconfig"
    )]
    generate_config: bool,

    #[arg(last = true)]
    quick_note: Vec<String>,
}

struct NavigationState {
    selected_index: usize,
}

impl NavigationState {
    fn new(selected_index: usize) -> Self {
        NavigationState { selected_index }
    }

    fn selected_index(&self) -> usize {
        self.selected_index
    }

    fn set_selected_index(&mut self, new_index: usize) {
        self.selected_index = new_index;
    }
}

fn launch_editor(filename: &str, editor: &str) {
    Command::new(editor)
        .args([filename])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .output()
        .expect("Failed to launch editor.");
}

fn display_file_list<W: Write>(
    stdout: &mut W,
    files: &[NoteEntry],
    selected_index: usize,
) -> IOResult<()> {
    // Clear terminal and prepare to list out files
    // This function just bubbles up IO errors to let the implementer handle whether to panic.
    writeln!(
        stdout,
        "{clear}{goto}{color}Notes Files:{reset}",
        clear = termion::clear::All,
        goto = cursor::Goto(1, 1),
        color = color::Fg(color::Yellow),
        reset = color::Fg(color::Reset)
    )?;

    let default_indicator = "  [Default]".to_owned();
    let mut width = 0;
    for f in files.iter() {
        if f.name.len() + default_indicator.len() > width {
            width = f.name.len() + default_indicator.len();
        }
    }

    for (i, f) in files.iter().enumerate() {
        let entry = f;
        let date: chrono::DateTime<chrono::Local> = entry.modified.into();
        let formatted_filename = format!(
            "{file}{default_indicator}",
            file = entry.name,
            default_indicator = if entry.is_default {
                default_indicator.clone()
            } else {
                String::new()
            },
        );

        if i == selected_index {
            writeln!(
                stdout,
                "{goto}{highlight}{fontcolor}{formatted_filename:<width$}\t{modified}\t{size}{reset_highlight}{reset_fontcolor}",
                goto = cursor::Goto(1, (i + 2) as u16),
                highlight = color::Bg(color::White),
                fontcolor = color::Fg(color::Black),
                width = width,
                size = entry.size,
                modified = date.format(DATE_FORMAT),
                reset_highlight = color::Bg(color::Reset),
                reset_fontcolor = color::Fg(color::Reset)
            )?
        } else {
            writeln!(
                stdout,
                "{goto}{formatted_filename:<width$}\t{modified}\t{size}",
                goto = cursor::Goto(1, (i + 2) as u16),
                width = width,
                size = entry.size,
                modified = date.format(DATE_FORMAT),
            )?
        }
    }

    // Print the command prompt at the bottom of the terminal.
    let (_, h) = termion::terminal_size()?;
    write!(stdout, "{hide}", hide = cursor::Hide).unwrap();
    write!(
        stdout,
        "{goto}New file [n]; Rename file [r]; Delete file [D]; Quit [q]",
        goto = cursor::Goto(1, h)
    )?;
    stdout.flush()?;
    Ok(())
}

fn show_notes<T: NotesProvider>(
    notes_provider: &T,
    state: &mut NavigationState,
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
    config: &Config,
) -> IOResult<()> {
    let mut note_list = notes_provider.get_notes();
    display_file_list(stdout, &note_list, state.selected_index())?;

    let mut key_buffer: Vec<Key> = vec![];
    let mut last_keypress_time = Instant::now();

    for c in stdin.keys() {
        let event = c.unwrap();

        key_buffer.push(event);

        if key_buffer == [Key::Char('g'), Key::Char('g')] {
            state.set_selected_index(0);
            key_buffer.clear();
        } else if key_buffer == [Key::Char('d'), Key::Char('d')] {
            let note_to_del = &note_list[state.selected_index];
            if !note_to_del
                .path
                .to_str()
                .unwrap()
                .contains(config.get_default_notes_file())
            {
                if prompt_yesno(
                    stdout,
                    stdin,
                    format!(
                        "Are you sure you want to delete {}? [y/N] ",
                        note_to_del.path.to_str().unwrap()
                    ),
                ) {
                    notes_provider.delete_note(note_to_del).unwrap();
                    if state.selected_index() > note_list.len() - 2 {
                        state.set_selected_index(state.selected_index.saturating_sub(1));
                    }
                }
            } else {
                write!(
                    stdout,
                    "{}{}Cannot delete your default notes file.",
                    termion::clear::All,
                    cursor::Goto(1, 1),
                )?;
                stdout.flush()?;
                thread::sleep(time::Duration::from_secs(1));
            }
        } else if key_buffer.len() == 2
            || Instant::now().duration_since(last_keypress_time) > Duration::from_millis(500)
        {
            key_buffer.clear();
            key_buffer.push(event);
        }

        last_keypress_time = Instant::now();

        match event {
            Key::Char('j') => {
                if state.selected_index < note_list.len() - 1 {
                    let new_index = state.selected_index.saturating_add(1);
                    state.set_selected_index(new_index);
                }
            }
            Key::Char('k') => {
                if state.selected_index > 0 {
                    let new_index = state.selected_index.saturating_sub(1);
                    state.set_selected_index(new_index);
                }
            }
            Key::Char('G') => {
                state.set_selected_index(note_list.len() - 1);
            }
            Key::Char('q') => {
                break;
            }
            Key::Char('D') => {
                let note_to_del = &note_list[state.selected_index];
                if !note_to_del
                    .path
                    .to_str()
                    .unwrap()
                    .contains(config.get_default_notes_file())
                {
                    notes_provider.delete_note(note_to_del).unwrap();
                    if state.selected_index() > note_list.len() - 2 {
                        state.set_selected_index(state.selected_index.saturating_sub(1));
                    }
                } else {
                    write!(
                        stdout,
                        "{}{}Cannot delete your default notes file.",
                        termion::clear::All,
                        cursor::Goto(1, 1),
                    )?;
                    stdout.flush()?;
                    thread::sleep(time::Duration::from_secs(1));
                }
            }
            Key::Char('r') => {
                let selected_note = &note_list[state.selected_index()];

                loop {
                    let mut note_name = String::new();

                    // Prompt in a loop, only exiting if we create a valid file.
                    prompt(
                        stdout,
                        stdin,
                        format!("Enter a new name for '{}': ", selected_note.name),
                        &mut note_name,
                    );

                    // Check for empty entry.  Re-prompt if it is.
                    if note_name.is_empty() {
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
                            let path_with_ext = new_note_path.to_str().unwrap().to_owned()
                                + config.get_default_file_extension();
                            Path::new(&path_with_ext).to_path_buf()
                        }
                    };

                    let mut new_note = selected_note.clone();
                    new_note.path = new_note_path;

                    match notes_provider.note_exists(&new_note.path) {
                        false => {
                            // Validation was successful, rename the note.
                            notes_provider
                                .rename_note(selected_note, &new_note.path)
                                .unwrap();
                            state.set_selected_index(0);
                            break;
                        }
                        _ => {
                            // If it failed to validate for some reason, write out the error and
                            // re-prompt.
                            clear(stdout);
                            write!(
                                stdout,
                                "note {} already exists",
                                new_note.path.to_str().unwrap()
                            )?;
                            stdout.flush()?;
                            thread::sleep(time::Duration::from_secs(1));
                            continue;
                        }
                    }
                }
            }
            Key::Char('n') => {
                loop {
                    let mut note_name = String::new();

                    // Prompt in a loop, only exiting if we create a valid file.
                    prompt(
                        stdout,
                        stdin,
                        String::from("Enter a name for your new note file: "),
                        &mut note_name,
                    );

                    let new_note_path = format!("{}{}", config.get_notes_directory(), note_name);
                    let new_note_path = Path::new(&new_note_path);
                    let new_note_path = match new_note_path.extension() {
                        Some(_) => new_note_path.to_path_buf(),
                        None => {
                            // Add an extension if there isn't one.
                            let path_with_ext = new_note_path.to_str().unwrap().to_owned()
                                + config.get_default_file_extension();
                            Path::new(&path_with_ext).to_path_buf()
                        }
                    };

                    let note =
                        NoteEntry::new(new_note_path, note_name, SystemTime::now(), false, 0);

                    match notes_provider.note_exists(&note.path) {
                        false => {
                            notes_provider.create_note(note).unwrap();
                            state.set_selected_index(0);
                            break;
                        }
                        true => {
                            // Check for empty entry.  Re-prompt if it is.
                            clear(stdout);
                            write!(
                                stdout,
                                "note {} already exists",
                                note.path.to_str().unwrap()
                            )?;
                            stdout.flush()?;
                            thread::sleep(time::Duration::from_secs(1));
                            continue;
                        }
                    }
                }
            }
            Key::Char('\n') => {
                let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
                launch_editor(
                    note_list[state.selected_index()].path.to_str().unwrap(),
                    &editor,
                )
            }
            _ => {}
        }

        note_list = notes_provider.get_notes();
        display_file_list(stdout, &note_list, state.selected_index())?;
    }

    Ok(())
}

fn main() {
    let args = Args::parse();

    if args.generate_config {
        println!("{}", &Config::generate());
        return;
    }

    // Get the config file
    let mut config_file = home::home_dir().unwrap_or_default();
    config_file.push(".noteconfig");

    let config = match config_file.to_str() {
        Some(file) => {
            let config = match std::fs::read_to_string(file) {
                Ok(file) => match file.parse::<toml::Table>() {
                    Ok(table) => table,
                    _ => {
                        panic!("Unable to parse config file. Make sure it is valid toml.");
                    }
                },
                _ => toml::Table::new(),
            };

            Config::new(config)
        }
        None => {
            panic!("Unable to find home directory. Something is very wrong :(")
        }
    };

    // Check the notes dir and default file exist
    let notes_directory = config.get_notes_directory();
    let quick_notes_file_path = config.get_default_notes_path();
    if !Path::new(&notes_directory).exists() {
        println!(
            "No {} folder exists. Please create it first.",
            notes_directory
        );
        return;
    }
    if !Path::new(&quick_notes_file_path).exists() {
        println!(
            "No {} file exists. Please create it first.",
            quick_notes_file_path
        );
        return;
    }

    if !args.quick_note.is_empty() {
        let current_utc: DateTime<Utc> = Utc::now();
        let date_time: String = current_utc.format("%Y-%m-%d:%H-%M-%S").to_string();
        let quick_note = date_time + "\n" + &args.quick_note.join(" ") + "\n";

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(quick_notes_file_path)
            .expect("it opens quick notes file");

        file.write_all(quick_note.as_bytes())
            .expect("it wrote quick note to file");
    } else {
        let mut stdout = match stdout().into_raw_mode() {
            Ok(w) => w,
            _ => {
                panic!("Could not open stdout. Something went very wrong")
            }
        };

        let stdin = stdin();
        let mut state = NavigationState::new(0);

        // Main application loop
        // Show file navigation screen
        let notes_provider = FileSystemNotesProvider::new(&config);
        match show_notes(&notes_provider, &mut state, &mut stdout, &stdin, &config) {
            Ok(_) => {
                // If we didn't fail to run, just continue implicitly.
                clear(&mut stdout);
            }
            Err(e) => {
                panic!("{}", e);
            }
        }
    }
}
