mod config;
mod note_entry;
mod prompt;
mod providers;

use crate::config::Config;
use crate::note_entry::NoteEntry;
use crate::prompt::{clear, flash_warning, prompt, prompt_yesno};
use crate::providers::file_system_provider::FileSystemNotesProvider;
use crate::providers::provider::NotesProvider;

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use log::{debug, error, warn, LevelFilter};
use std::io::{stdin, stdout, Stdout, Write};
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
        short = 'e',
        long,
        default_value_t = false,
        exclusive = true,
        help = "Generate a default configuration toml to be used in ~/.noteconfig"
    )]
    example_config: bool,
}

#[derive(Eq, PartialEq)]
enum SortField {
    Modified,
    Size,
    Name,
}

#[derive(Eq, PartialEq)]
enum SortDir {
    Asc = 1,
    Desc = -1,
}

struct NavigationState {
    selected_index: usize,
    sort_dir: SortDir,
    sort_field: SortField,
}

impl NavigationState {
    fn new(selected_index: usize) -> Self {
        NavigationState {
            selected_index,
            sort_field: SortField::Modified,
            sort_dir: SortDir::Asc,
        }
    }

    fn selected_index(&self) -> usize {
        self.selected_index
    }

    fn set_selected_index(&mut self, new_index: usize) {
        self.selected_index = new_index;
    }

    fn sort(&mut self, sort_field: SortField) {
        let mut sort_dir = SortDir::Desc;
        if self.sort_field == sort_field {
            if self.sort_dir == SortDir::Desc {
                sort_dir = SortDir::Asc;
            } else {
                sort_dir = SortDir::Desc;
            }
        }

        self.sort_dir = sort_dir;
        self.sort_field = sort_field;
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

fn get_filename_column_width(files: &[NoteEntry]) -> usize {
    let default_indicator = "  [Default]".to_owned();
    let mut width = 0;
    for f in files.iter() {
        if f.name.len() + default_indicator.len() > width {
            width = f.name.len() + default_indicator.len();
        }
    }
    width
}

fn draw_header(files: &[NoteEntry], state: &NavigationState) -> String {
    let filename_col_width = get_filename_column_width(files);

    let mut col1 = String::from("Notes Files:");
    let mut col2 = String::from("Modified");
    let mut col3 = String::from("Size(b)");

    let sort_indicator = match state.sort_dir {
        SortDir::Desc => "↓",
        SortDir::Asc => "↑",
    };
    match state.sort_field {
        SortField::Name => col1 = format!("{} {}", col1, sort_indicator),
        SortField::Modified => col2 = format!("{} {}", col2, sort_indicator),
        SortField::Size => col3 = format!("{} {}", col3, sort_indicator),
    };

    format!(
        "{clear}{goto}{color}{col1:<filename_col_width$}\t{col2}\t{col3}{reset}",
        filename_col_width = filename_col_width,
        col1 = col1,
        col2 = col2,
        col3 = col3,
        goto = cursor::Goto(1, 1),
        clear = termion::clear::All,
        color = color::Fg(color::Yellow),
        reset = color::Fg(color::Reset)
    )
}

fn display_file_list<W: Write>(
    stdout: &mut W,
    files: &[NoteEntry],
    state: &mut NavigationState,
) -> Result<()> {
    let selected_index = state.selected_index();

    // Draw the header.
    writeln!(stdout, "{}", draw_header(files, state))?;

    let filename_col_width = get_filename_column_width(files);
    let default_indicator = "  [Default]";

    // Iterate over each file and draw it
    for (i, f) in files.iter().enumerate() {
        let entry = f;
        let date: chrono::DateTime<chrono::Local> = entry.modified.into();
        let filename = format!(
            "{file}{default_indicator}",
            file = entry.name,
            default_indicator = if entry.is_default {
                default_indicator
            } else {
                ""
            },
        );

        if i == selected_index {
            writeln!(
                stdout,
                "{goto}{highlight}{fontcolor}{filename:<filename_col_width$}\t{modified}\t{size}{reset_highlight}{reset_fontcolor}",
                goto = cursor::Goto(1, (i + 2) as u16),
                highlight = color::Bg(color::White),
                reset_highlight = color::Bg(color::Reset),
                reset_fontcolor = color::Fg(color::Reset),
                fontcolor = color::Fg(color::Black),
                filename = filename,
                filename_col_width = filename_col_width,
                size = entry.get_size(),
                modified = date.format(DATE_FORMAT),
            )?
        } else {
            writeln!(
                stdout,
                "{goto}{filename:<filename_col_width$}\t{modified}\t{size}",
                goto = cursor::Goto(1, (i + 2) as u16),
                filename = filename,
                filename_col_width = filename_col_width,
                modified = date.format(DATE_FORMAT),
                size = entry.get_size(),
            )?
        }
    }

    // Print the command prompt at the bottom of the terminal.
    let (_, h) = termion::terminal_size()?;
    write!(
        stdout,
        "{hide}{goto}New file [n]; Rename file [r]; Delete file [dd]; Sort[s]; Quit [q]",
        hide = cursor::Hide,
        goto = cursor::Goto(1, h)
    )?;
    stdout.flush()?;
    Ok(())
}

fn run<T: NotesProvider>(
    notes_provider: &T,
    state: &mut NavigationState,
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
    config: &Config,
) -> Result<()> {
    let mut note_list = notes_provider.get_notes(&state.sort_field, &state.sort_dir);
    display_file_list(stdout, &note_list, state)?;

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
            state.set_selected_index(0);
        } else if key_buffer == [Key::Char('d'), Key::Char('d')] {
            key_buffer.clear();
            let note_to_del = &note_list[state.selected_index];

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
                    if state.selected_index() > note_list.len() - 2 {
                        state.set_selected_index(state.selected_index.saturating_sub(1));
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
            Key::Char('s') => {
                // Toggle between sort modes
                let default_indicator = "  [Default]".to_owned();
                let mut width = 0;
                for f in note_list.iter() {
                    if f.name.len() + default_indicator.len() > width {
                        width = f.name.len() + default_indicator.len();
                    }
                }

                let col1 = String::from("[n] Notes Files:");
                let col2 = String::from("[m] Modified");
                let col3 = String::from("[s] Size(b)");

                writeln!(
                    stdout,
                    "{goto}{color}{col1:<width$}\t{col2}\t{col3}{reset}",
                    col1 = col1,
                    col2 = col2,
                    col3 = col3,
                    width = width,
                    goto = cursor::Goto(1, 1),
                    color = color::Fg(color::Yellow),
                    reset = color::Fg(color::Reset)
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
            Key::Char('r') => {
                let selected_note = &note_list[state.selected_index()];

                loop {
                    // Prompt in a loop, only exiting if we create a valid file.
                    let note_name = prompt(
                        stdout,
                        stdin,
                        format!("Enter a new name for '{}': ", selected_note.name),
                    )?;

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
                            let mut new_note_path = new_note_path.to_path_buf();
                            new_note_path.set_extension(config.get_default_file_extension());
                            new_note_path
                        }
                    };

                    let mut new_note = selected_note.clone();
                    new_note.path = new_note_path;

                    match notes_provider.note_exists(&new_note.path) {
                        false => {
                            // Note with new path doesn't already exist, so we're good to
                            // try to rename it.
                            notes_provider.rename_note(selected_note, &new_note.path)?;
                            state.set_selected_index(0);
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
                            state.set_selected_index(0);
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
                let file_path = note_list[state.selected_index()]
                    .path
                    .to_str()
                    .context("could not convert file path to string")?;
                let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
                launch_editor(file_path, &editor)
            }
            _ => {}
        }

        note_list = notes_provider.get_notes(&state.sort_field, &state.sort_dir);
        display_file_list(stdout, &note_list, state)?;
    }

    Ok(())
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
    // TODO move this logic into the provider. Need a validate_config func or something.
    let notes_directory = config.get_notes_directory();
    let default_notes_file_path = config.get_default_notes_path();
    if !Path::new(&notes_directory).exists() {
        bail!(format!(
            "No {} folder exists. Please create it first.",
            notes_directory
        ))
    }
    if !Path::new(&default_notes_file_path).exists() {
        bail!(format!(
            "No {} file exists. Please create it first.",
            default_notes_file_path
        ))
    }

    // Create stdout and stdin for the main application loop
    let mut stdout = stdout()
        .into_raw_mode()
        .with_context(|| "Could not open stdout. Something went very wrong")?;
    let stdin = stdin();

    // TODO let's eventually save navigation state across sessions.
    let mut state = NavigationState::new(0);

    // Main application loop
    if let Err(e) = run(&notes_provider, &mut state, &mut stdout, &stdin, &config) {
        error!("{}", e.to_string());
        return Err(anyhow!(e));
    }

    clear(&mut stdout)?;
    Ok(())
}
