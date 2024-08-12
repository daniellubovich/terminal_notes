mod prompt;
use crate::prompt::{clear, prompt, prompt_yesno};
use chrono::{DateTime, Utc};
use clap::Parser;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Stdout;
use std::io::{stdin, stdout, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{thread, time};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;
use termion::{color, cursor};

enum AppState {
    NavigatingFiles,
    Quitting,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short = 'e', long, default_value_t = true)]
    edit: bool,

    #[arg(last = true)]
    quick_note: Vec<String>,
}

struct NavigationState {
    selected_index: usize,
    mode: AppState,
}

impl NavigationState {
    fn new(selected_index: usize, mode: AppState) -> Self {
        NavigationState {
            selected_index,
            mode,
        }
    }

    fn selected_index(&self) -> usize {
        self.selected_index
    }

    fn set_selected_index(&mut self, new_index: usize) {
        self.selected_index = new_index;
    }

    fn mode(&self) -> &AppState {
        &self.mode
    }

    fn set_mode(&mut self, mode: AppState) {
        self.mode = mode;
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

fn display_file_list<W: Write>(stdout: &mut W, files: &[std::fs::DirEntry], selected_index: usize) {
    // Clear terminal and prepare to list out files
    writeln!(
        stdout,
        "{clear}{goto}{color}Notes Files:{reset}",
        clear = termion::clear::All,
        goto = cursor::Goto(1, 1),
        color = color::Fg(color::Yellow),
        reset = color::Fg(color::Reset)
    )
    .unwrap();

    for (i, f) in files.iter().enumerate() {
        let entry = f;
        if i == selected_index {
            writeln!(
                stdout,
                "{goto}{highlight}{fontcolor}{file}{reset_highlight}{reset_fontcolor}",
                goto = cursor::Goto(1, (i + 2) as u16),
                highlight = color::Bg(color::White),
                fontcolor = color::Fg(color::Black),
                file = entry.path().to_str().unwrap(),
                reset_highlight = color::Bg(color::Reset),
                reset_fontcolor = color::Fg(color::Reset)
            )
            .expect("Error writing to stdout");
        } else {
            writeln!(
                stdout,
                "{goto}{file}",
                goto = cursor::Goto(1, (i + 2) as u16),
                file = entry.path().to_str().unwrap()
            )
            .expect("Error writing to stdout");
        }
    }

    // Print the command prompt at the bottom of the terminal.
    let (_, h) = termion::terminal_size().unwrap();
    write!(stdout, "{hide}", hide = cursor::Hide).unwrap();
    write!(
        stdout,
        "{goto}New file [n]; Delete file [D]",
        goto = cursor::Goto(1, h)
    )
    .unwrap();
    stdout.flush().unwrap();
}

fn show_file_navigation(
    notes_directory: &str,
    state: &mut NavigationState,
    stdout: &mut RawTerminal<Stdout>,
    stdin: &std::io::Stdin,
) {
    let mut files = fs::read_dir(notes_directory).unwrap();
    let mut file_entries: Vec<std::fs::DirEntry> = files.map(|entry| entry.unwrap()).collect();
    display_file_list(stdout, &file_entries, state.selected_index());

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
                state.set_mode(AppState::Quitting);
                break;
            }
            Key::Char('D') => {
                let file_to_del = file_entries[state.selected_index]
                    .path()
                    .to_str()
                    .expect("valid filename")
                    .to_owned();

                if !file_to_del.contains("default_notes.txt") {
                    if prompt_yesno(
                        stdout,
                        stdin,
                        format!("Are you sure you want to delete {}? [y/N] ", file_to_del),
                    ) {
                        fs::remove_file(file_to_del).expect("Could not delete file.");
                        state.set_selected_index(0);
                    }
                } else {
                    write!(
                        stdout,
                        "{}{}Cannot delete your default notes file.",
                        termion::clear::All,
                        cursor::Goto(1, 1),
                    )
                    .unwrap();
                    stdout.flush().unwrap();
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
                        write!(stdout, "File name empty. Try again, bro.").unwrap();
                        stdout.flush().expect("Could not write output");
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
                            let path_with_ext = new_file_path.to_str().unwrap().to_owned() + ".txt";
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
                        )
                        .unwrap();
                        stdout.flush().expect("Could not write output");
                        thread::sleep(time::Duration::from_secs(1));
                        continue;
                    }

                    // If we can't get a string and/or the file can't be created, time to
                    // panic.
                    new_file_path.to_str().expect("Invalid file path");
                    fs::File::create(new_file_path).expect("Could not create file. Exiting.");
                    state.set_mode(AppState::NavigatingFiles);
                    break;
                }
            }
            Key::Char('\n') => {
                let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
                launch_editor(
                    file_entries[state.selected_index()]
                        .path()
                        .to_str()
                        .unwrap(),
                    &editor,
                )
            }
            _ => {}
        }

        files = fs::read_dir(notes_directory).unwrap();
        file_entries = files.map(|entry| entry.unwrap()).collect();
        display_file_list(stdout, &file_entries, state.selected_index());
    }
}

fn main() {
    let args = Args::parse();

    let mut home_dir = home::home_dir().unwrap();
    home_dir.push(".notes/");

    let notes_directory = home_dir.to_str().unwrap();
    let quick_notes_filename = "default_notes.txt";
    let quick_notes_file_path = format!("{}{}", notes_directory, quick_notes_filename);
    println!("{}", notes_directory);

    if !Path::new(notes_directory).exists() {
        println!("No ~/.notes/ folder exists. Please create it first.");
        return;
    }
    if !Path::new(&quick_notes_file_path).exists() {
        println!("No ~/.notes/default_notes.txt file exists. Please create it first.");
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
    } else if args.edit {
        let mut stdout = stdout().into_raw_mode().unwrap();
        let stdin = stdin();

        let mut state = NavigationState::new(0, AppState::NavigatingFiles);

        // Main application loop
        loop {
            match state.mode() {
                AppState::NavigatingFiles => {
                    // Show file navigation screen
                    show_file_navigation(notes_directory, &mut state, &mut stdout, &stdin);
                }
                AppState::Quitting => {
                    // Exit the program
                    clear(&mut stdout);
                    break;
                }
            }
        }
    } else {
        println!("Invalid arguments.");
    }
}
