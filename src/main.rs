mod config;
mod navigation_state;
mod note_entry;
mod prompt;
mod providers;
mod render;

use crate::config::Config;
use crate::navigation_state::{NavigationState, SortDir, SortField};
use crate::note_entry::NoteEntry;
use crate::prompt::{clear, flash_warning, prompt, prompt_yesno};
use crate::providers::file_system_provider::FileSystemNotesProvider;
use crate::providers::provider::NotesProvider;
use crate::render::{Column, Columnar, Field, TableDisplay};

use anyhow::{Context, Result};
use clap::Parser;
use log::{debug, error, warn, LevelFilter};
use std::io::{stdin, stdout, Stdout, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};
use std::{thread, time};
use termion::cursor;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(
        short = 'e',
        long,
        default_value_t = false,
        exclusive = true,
        help = "Generate a default configuration toml to be used in ~/.noteconfig"
    )]
    example_config: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    simple_logging::log_to_file("output.log", LevelFilter::Info)
        .context("opening logfile output.log")?;

    if args.example_config {
        println!("{}", &Config::generate());
        return Ok(());
    }

    // Load the config file
    let mut config_file_path =
        home::home_dir().context("could not find home directory for some reason")?;
    config_file_path.push(".noteconfig");
    let config_file = std::fs::read_to_string(config_file_path).context("reading config file")?;
    let config_toml = config_file
        .parse::<toml::Table>()
        .context("parsing config file into toml")?;
    let config = Config::new(config_toml);

    // Eventually, we'll add other providers. SQLite hopefully.
    let notes_provider = FileSystemNotesProvider::new(&config);

    // Check the notes dir and default file exist
    notes_provider.validate_default_note_exists()?;

    // Create stdout and stdin for the main application loop
    let mut stdout = stdout()
        .into_raw_mode()
        .context("Could not open stdout. Something went very wrong")?;
    let stdin = stdin();

    // TODO let's eventually save navigation state across sessions.
    let state = NavigationState::new(0);

    // Main application loop
    run(&notes_provider, state, &mut stdout, &stdin, &config).map_err(|e| {
        error!("{}", e.to_string());
        e
    })?;

    clear(&mut stdout)?;
    Ok(())
}

fn run<T: NotesProvider>(
    notes_provider: &T,
    mut state: NavigationState,
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
    config: &Config,
) -> Result<()> {
    let mut note_list = notes_provider.get_notes(state.get_sort_field(), state.get_sort_dir());
    let mut rows: Vec<Rc<dyn Columnar>> = note_list
        .iter()
        .map(|file| file.clone() as Rc<dyn Columnar>)
        .collect();

    let columns = vec![
        Column {
            field: Field::Name,
            name: "Name".to_string(),
            sort_field: SortField::Name,
        },
        Column {
            field: Field::Size,
            name: "Size".to_string(),
            sort_field: SortField::Size,
        },
        Column {
            field: Field::Modified,
            name: "Modified".to_string(),
            sort_field: SortField::Modified,
        },
    ];

    let mut table = TableDisplay {
        rows,
        columns,
        state: &mut state,
    };

    render_list(stdout, &table)?;

    let mut key_buffer: Vec<Key> = vec![];
    let mut last_keypress_time = Instant::now();

    for event_opt in stdin.keys() {
        let event = match event_opt {
            Ok(event) => event,
            Err(error) => {
                warn!(
                    "error occured when processing keystroke. Retrying. {}",
                    error
                );
                continue;
            }
        };

        key_buffer.push(event);

        if key_buffer == [Key::Char('g'), Key::Char('g')] {
            key_buffer.clear();
            table.state.set_selected_index(0);
        } else if key_buffer == [Key::Char('d'), Key::Char('d')] {
            key_buffer.clear();
            let note_to_del = &note_list[table.state.get_selected_index()];

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
                    if table.state.get_selected_index() > note_list.len() - 2 {
                        table
                            .state
                            .set_selected_index(table.state.get_selected_index().saturating_sub(1));
                    }
                }
            }
        } else if key_buffer.len() == 2
            || Instant::now().duration_since(last_keypress_time) > Duration::from_millis(300)
        {
            key_buffer.clear();
            key_buffer.push(event);
        }

        last_keypress_time = Instant::now();

        match event {
            Key::Char('j') => {
                if table.state.get_selected_index() < note_list.len() - 1 {
                    let new_index = table.state.get_selected_index().saturating_add(1);
                    table.state.set_selected_index(new_index);
                }
            }
            Key::Char('k') => {
                if table.state.get_selected_index() > 0 {
                    let new_index = table.state.get_selected_index().saturating_sub(1);
                    table.state.set_selected_index(new_index);
                }
            }
            Key::Char('G') => {
                table.state.set_selected_index(note_list.len() - 1);
            }
            Key::Char('q') => {
                break;
            }
            Key::Char('s') => {
                // Toggle between sort modes

                // This is pretty janky right now. I think the columns could be passed a navigation
                // state and render their own [key] indicator.
                table.columns[0].set_name(String::from("[n] Name"));
                table.columns[1].set_name(String::from("[s] Size"));
                table.columns[2].set_name(String::from("[m] Modified"));

                write!(stdout, "{}", table.draw())?;
                stdout.flush()?;

                for k_event in stdin.keys() {
                    let key = k_event.context("could not read input")?;
                    match key {
                        Key::Char('s') => {
                            table.state.sort(SortField::Size);
                            break;
                        }
                        Key::Char('n') => {
                            table.state.sort(SortField::Name);
                            break;
                        }
                        Key::Char('m') => {
                            table.state.sort(SortField::Modified);
                            break;
                        }
                        _ => continue,
                    };
                }

                table.columns[0].set_name(String::from("Name"));
                table.columns[1].set_name(String::from("Size"));
                table.columns[2].set_name(String::from("Modified"));
            }
            Key::Char('r') => {
                let selected_note = &note_list[table.state.get_selected_index()];

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
                            table.state.set_selected_index(0);
                            break;
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
                            continue;
                        }
                    }
                }
            }
            Key::Char('n') => {
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

                    let note =
                        NoteEntry::new(new_note_path, note_name, SystemTime::now(), false, 0);

                    if note.name.is_empty() {
                        debug!("note name is empty. exiting prompt.");
                        break;
                    }

                    match notes_provider.note_exists(&note.path) {
                        false => {
                            notes_provider.create_note(note)?;
                            table.state.set_selected_index(0);
                            break;
                        }
                        true => {
                            // Check for empty entry.  Re-prompt if it is.
                            let new_note_path = note
                                .path
                                .to_str()
                                .context("could not convert file path to string")?;
                            flash_warning(
                                stdout,
                                format!("note {} already exists", new_note_path),
                            )?;
                            continue;
                        }
                    }
                }
            }
            Key::Char('\n') => {
                let file_path = note_list[table.state.get_selected_index()]
                    .path
                    .to_str()
                    .context("could not convert file path to string")?;

                let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

                Command::new(editor)
                    .args([file_path])
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .output()
                    .context("Failed to launch editor.")?;
            }
            _ => {}
        }

        note_list =
            notes_provider.get_notes(table.state.get_sort_field(), table.state.get_sort_dir());
        rows = note_list
            .iter()
            .map(|file| file.clone() as Rc<dyn Columnar>)
            .collect();
        table.rows = rows;
        render_list(stdout, &table)?;
    }

    Ok(())
}

fn render_list<W: Write>(stdout: &mut W, table: &TableDisplay) -> Result<()> {
    // Print the command prompt at the bottom of the terminal.
    let (_, h) = termion::terminal_size()?;
    writeln!(stdout, "{table}", table = table.draw())?;
    write!(
        stdout,
        "{hide}{goto}New file [n]; Rename file [r]; Delete file [dd]; Sort[s]; Quit [q]",
        hide = cursor::Hide,
        goto = cursor::Goto(1, h)
    )?;
    stdout.flush()?;

    Ok(())
}
