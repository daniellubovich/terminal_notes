mod config;
mod prompt;
use crate::config::Config;
use crate::prompt::{clear, prompt, prompt_yesno};
use chrono::{DateTime, Utc};
use clap::Parser;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Stdout;
use std::io::{stdin, stdout, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::SystemTime;
use std::{thread, time};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;
use termion::{color, cursor};
use toml::Table;

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

struct NoteEntry {
    path: String,
    name: String,
    modified: SystemTime,
    is_default: bool,
}

impl NoteEntry {
    fn new(path: String, name: String, modified: SystemTime, is_default: bool) -> Self {
        NoteEntry {
            path,
            name,
            modified,
            is_default,
        }
    }
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
) -> io::Result<()> {
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

    for (i, f) in files.iter().enumerate() {
        let entry = f;
        if i == selected_index {
            writeln!(
                stdout,
                "{goto}{highlight}{fontcolor}{file}{default_indicator}{reset_highlight}{reset_fontcolor}",
                goto = cursor::Goto(1, (i + 2) as u16),
                highlight = color::Bg(color::White),
                fontcolor = color::Fg(color::Black),
                file = entry.name,
                default_indicator = if entry.is_default { "  [Default]".to_owned() } else { String::new() },
                reset_highlight = color::Bg(color::Reset),
                reset_fontcolor = color::Fg(color::Reset)
            )?
        } else {
            writeln!(
                stdout,
                "{goto}{file}{default_indicator}",
                goto = cursor::Goto(1, (i + 2) as u16),
                default_indicator = if entry.is_default {
                    "  [Default]".to_owned()
                } else {
                    String::new()
                },
                file = entry.name
            )?
        }
    }

    // Print the command prompt at the bottom of the terminal.
    let (_, h) = termion::terminal_size()?;
    write!(stdout, "{hide}", hide = cursor::Hide).unwrap();
    write!(
        stdout,
        "{goto}New file [n]; Delete file [D]; Quit [q]",
        goto = cursor::Goto(1, h)
    )?;
    stdout.flush()?;
    Ok(())
}

fn get_notes_entries(config: &Config) -> Vec<NoteEntry> {
    let files = fs::read_dir(config.get_notes_directory()).unwrap();
    let mut file_entries: Vec<NoteEntry> = files
        .map(|entry| {
            let file = entry.unwrap();
            let name = file.file_name().to_str().unwrap().to_owned();
            let path = file.path().to_str().unwrap().to_owned();
            let is_default = name == config.get_default_notes_file();
            NoteEntry::new(
                path,
                name,
                file.metadata().unwrap().modified().unwrap(),
                is_default,
            )
        })
        .collect();
    file_entries.sort_by(|a, b| {
        let a_ts = a.modified;
        let b_ts = b.modified;
        b_ts.cmp(&a_ts)
    });
    file_entries
}

fn show_file_navigation(
    notes_directory: &str,
    state: &mut NavigationState,
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
    config: &Config,
) -> io::Result<()> {
    let mut file_entries = get_notes_entries(config);
    display_file_list(stdout, &file_entries, state.selected_index())?;

    for c in stdin.keys() {
        match c.unwrap() {
            Key::Char('j') => {
                if state.selected_index < file_entries.len() - 1 {
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
            Key::Char('q') => {
                break;
            }
            Key::Char('r') => {
                let mut file_name = String::new();

                loop {
                    // Prompt in a loop, only exiting if we create a valid file.
                    prompt(
                        stdout,
                        stdin,
                        format!(
                            "Enter a new name note file {}: ",
                            file_entries[state.selected_index()].name,
                        ),
                        &mut file_name,
                    );

                    // Check for empty entry.  Re-prompt if it is.
                    if file_name.is_empty() {
                        clear(stdout);
                        write!(stdout, "File name empty. Try again, bro.")?;
                        stdout.flush()?;
                        thread::sleep(time::Duration::from_secs(1));
                        continue;
                    }

                    let new_file_path = format!("{}{}", notes_directory, file_name);
                    let new_file_path = Path::new(&new_file_path);

                    // Check for a valid extension and add one if there isn't one.
                    let new_file_path = match new_file_path.extension() {
                        // TODO maybe check from a list of valid extensions?
                        Some(_) => new_file_path.to_path_buf(),
                        None => {
                            let path_with_ext = new_file_path.to_str().unwrap().to_owned()
                                + config.get_default_file_extension();
                            Path::new(&path_with_ext).to_path_buf()
                        }
                    };

                    // Check to confirm the file doesn't already exist. Re-prompt
                    // if it does.
                    if new_file_path.exists() {
                        clear(stdout);
                        write!(
                            stdout,
                            "File {} already exists",
                            new_file_path.to_str().expect("file path is present")
                        )?;
                        stdout.flush()?;
                        thread::sleep(time::Duration::from_secs(1));
                        continue;
                    }

                    // If we can't get a string and/or the file can't be created, time to
                    // panic.
                    new_file_path.to_str().expect("Invalid file path");
                    let old_file_path = file_entries[state.selected_index()].path.clone();
                    fs::rename(old_file_path, new_file_path).unwrap();
                    state.set_selected_index(0);
                    break;
                }
            }
            Key::Char('D') => {
                let file_to_del = &file_entries[state.selected_index].path;

                if !file_to_del.contains(config.get_default_notes_file()) {
                    if prompt_yesno(
                        stdout,
                        stdin,
                        format!("Are you sure you want to delete {}? [y/N] ", file_to_del),
                    ) {
                        fs::remove_file(file_to_del).expect("Could not delete file.");
                        if state.selected_index() > file_entries.len() - 2 {
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
            }
            Key::Char('n') => {
                let mut file_name = String::new();

                loop {
                    // Prompt in a loop, only exiting if we create a valid file.
                    prompt(
                        stdout,
                        stdin,
                        String::from("Enter a name for your new note file: "),
                        &mut file_name,
                    );

                    // Check for empty entry.  Re-prompt if it is.
                    if file_name.is_empty() {
                        clear(stdout);
                        write!(stdout, "File name empty. Try again, bro.")?;
                        stdout.flush()?;
                        thread::sleep(time::Duration::from_secs(1));
                        continue;
                    }

                    let new_file_path = format!("{}{}", notes_directory, file_name);
                    let new_file_path = Path::new(&new_file_path);

                    // Check for a valid extension and add one if there isn't one.
                    let new_file_path = match new_file_path.extension() {
                        // TODO maybe check from a list of valid extensions?
                        Some(_) => new_file_path.to_path_buf(),
                        None => {
                            let path_with_ext = new_file_path.to_str().unwrap().to_owned()
                                + config.get_default_file_extension();
                            Path::new(&path_with_ext).to_path_buf()
                        }
                    };

                    // Check to confirm the file doesn't already exist. Re-prompt
                    // if it does.
                    if new_file_path.exists() {
                        clear(stdout);
                        write!(
                            stdout,
                            "File {} already exists",
                            new_file_path.to_str().expect("file path is present")
                        )?;
                        stdout.flush()?;
                        thread::sleep(time::Duration::from_secs(1));
                        continue;
                    }

                    // If we can't get a string and/or the file can't be created, time to
                    // panic.
                    new_file_path.to_str().expect("Invalid file path");
                    fs::File::create(new_file_path).expect("Could not create file. Exiting.");
                    state.set_selected_index(0);
                    break;
                }
            }
            Key::Char('\n') => {
                let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
                launch_editor(&file_entries[state.selected_index()].path, &editor)
            }
            _ => {}
        }

        file_entries = get_notes_entries(config);
        display_file_list(stdout, &file_entries, state.selected_index())?;
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
                Ok(file) => match file.parse::<Table>() {
                    Ok(table) => table,
                    _ => {
                        panic!("Unable to parse config file. Make sure it is valid toml.");
                    }
                },
                _ => Table::new(),
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

        let mut file = OpenOptions::new()
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
        match show_file_navigation(notes_directory, &mut state, &mut stdout, &stdin, &config) {
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
