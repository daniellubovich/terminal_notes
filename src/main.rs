mod actions;
mod config;
mod navigation_state;
mod note_entry;
mod prompt;
mod providers;
mod render;

use crate::actions::{create_note, delete_note, rename_note};
use crate::config::Config;
use crate::navigation_state::{NavigationState, SortDir, SortField};
use crate::prompt::clear;
use crate::providers::file_system_provider::FileSystemNotesProvider;
use crate::providers::provider::NotesProvider;
use crate::render::{table, Column, Columnar, Field};

use anyhow::{bail, Context, Result};
use clap::Parser;
use log::{error, warn, LevelFilter};
use std::io::{stdin, stdout, Stdout, Write};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::str::FromStr;
use std::time::{Duration, Instant};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;

enum Action {
    Quit,
    Noop,
    OpenEditor,
    Rename,
    Delete,
    New,
    NavDown,
    NavUp,
    NavTop,
    NavBottom,
    Sort,
}

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

    // TODO add log file rotation
    let logfile = std::env::var("LOG_FILE").unwrap_or("/tmp/terminal_notes.log".to_string());
    let loglevel = std::env::var("LOG_LEVEL").unwrap_or("INFO".to_string());
    let loglevel = match LevelFilter::from_str(&loglevel) {
        Ok(loglevel) => loglevel,
        Err(_) => {
            bail!("Could not parse LOG_LEVEL from env var : {}", loglevel);
        }
    };

    simple_logging::log_to_file(&logfile, loglevel)
        .context(format!("error opening logfile {}", logfile))?;

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
    run(&notes_provider, state, &mut stdout, &stdin, &config).inspect_err(|e| {
        error!("{}", e.to_string());
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
    let footer = "New file [n]; Rename file [r]; Delete file [dd]; Sort[s]; Quit [q]";

    let mut note_list = notes_provider.get_notes(state.get_sort_field(), state.get_sort_dir());
    state.set_list_size(note_list.len() as u16);

    let mut rows: Vec<Rc<dyn Columnar>> = note_list
        .iter()
        .map(|file| file.clone() as Rc<dyn Columnar>)
        .collect();
    write!(stdout, "{}", table::draw(&rows, &columns, footer, &state))?;
    stdout.flush()?;

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

        match handle_key(event, &mut key_buffer, &mut last_keypress_time) {
            Action::Quit => break,
            Action::NavDown => {
                state.increment_selected_index(1);
            }
            Action::NavUp => {
                state.decrement_selected_index(1);
            }
            Action::NavTop => {
                state.set_selected_index(note_list.len() - 1);
            }
            Action::NavBottom => {
                state.set_selected_index(0);
            }
            Action::Rename => {
                let selected_note = &note_list[state.get_selected_index()];
                rename_note(selected_note, notes_provider, config, stdout, stdin)?;

                // TODO update this to find the index of the new note, taking into account the
                // current sort state
                state.set_selected_index(0);
            }
            Action::New => {
                create_note(notes_provider, config, stdout, stdin)?;
            }
            Action::Delete => {
                let note_to_del = &note_list[state.get_selected_index()];
                match delete_note(note_to_del, notes_provider, config, stdout, stdin) {
                    Ok(true) => {
                        // Note was deleted
                        if state.get_selected_index() > note_list.len() - 2 {
                            state.set_selected_index(state.get_selected_index().saturating_sub(1));
                        }
                    }
                    Ok(false) => {
                        // Note was not deleted
                    }
                    Err(error) => Err(error).context("error deleting note")?,
                };
            }
            Action::OpenEditor => {
                // TODO this doesn't work if we eventually convert to not using the FS provider
                let file_path = note_list[state.get_selected_index()]
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
            Action::Sort => {
                // Toggle between sort modes

                // TODO This is pretty janky right now. I think the columns could be passed a navigation
                // state and render their own [key] indicator.
                let sorted_columns = vec![
                    Column {
                        field: Field::Name,
                        name: "[n] Name".to_string(),
                        sort_field: SortField::Name,
                    },
                    Column {
                        field: Field::Size,
                        name: "[s] Size".to_string(),
                        sort_field: SortField::Size,
                    },
                    Column {
                        field: Field::Modified,
                        name: "[m] Modified".to_string(),
                        sort_field: SortField::Modified,
                    },
                ];

                write!(
                    stdout,
                    "{}",
                    table::draw(&rows, &sorted_columns, footer, &state)
                )?;
                stdout.flush()?;

                for k_event in stdin.keys() {
                    let key = k_event.context("could not read input")?;
                    match key {
                        Key::Char('s') => {
                            state.sort(SortField::Size);
                            break;
                        }
                        Key::Char('n') => {
                            state.sort(SortField::Name);
                            break;
                        }
                        Key::Char('m') => {
                            state.sort(SortField::Modified);
                            break;
                        }
                        _ => continue,
                    };
                }
            }
            Action::Noop => {}
        }

        note_list = notes_provider.get_notes(state.get_sort_field(), state.get_sort_dir());
        rows = note_list
            .iter()
            .map(|file| file.clone() as Rc<dyn Columnar>)
            .collect();
        state.set_list_size(note_list.len() as u16);
        write!(
            stdout,
            "{table}",
            table = table::draw(&rows, &columns, footer, &state)
        )?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_key(
    key_event: Key,
    key_buffer: &mut Vec<Key>,
    last_keypress_time: &mut Instant,
) -> Action {
    // Handle chords
    key_buffer.push(key_event);
    if *key_buffer == [Key::Char('g'), Key::Char('g')] {
        key_buffer.clear();
        *last_keypress_time = Instant::now();
        return Action::NavBottom;
    } else if *key_buffer == [Key::Char('d'), Key::Char('d')] {
        key_buffer.clear();
        *last_keypress_time = Instant::now();
        return Action::Delete;
    } else if key_buffer.len() == 2
        || Instant::now().duration_since(*last_keypress_time) > Duration::from_millis(300)
    {
        key_buffer.clear();
        key_buffer.push(key_event);
        *last_keypress_time = Instant::now();
    }

    // If no chord is in progress, just process the single key.
    match key_event {
        Key::Char('j') => Action::NavDown,
        Key::Char('k') => Action::NavUp,
        Key::Char('G') => Action::NavTop,
        Key::Char('q') => Action::Quit,
        Key::Char('s') => Action::Sort,
        Key::Char('r') => Action::Rename,
        Key::Char('n') => Action::New,
        Key::Char('\n') => Action::OpenEditor,
        _ => Action::Noop,
    }
}
