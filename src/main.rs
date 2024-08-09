extern crate termion;
use chrono::{DateTime, Utc};
use clap::Parser;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{stdin, stdout, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{thread, time};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{color, cursor};

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
    mode: String,
}

fn launch_editor(filename: &str, editor: &str) {
    Command::new(editor)
        .args([filename])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .output()
        .expect("Failed to execute command");
}

fn print_files<W: Write>(stdout: &mut W, files: &[std::fs::DirEntry], selected_index: usize) {
    writeln!(stdout, "{}{}", termion::clear::All, cursor::Goto(1, 1)).unwrap();
    for (i, f) in files.iter().enumerate() {
        let entry = f;
        if i == selected_index {
            writeln!(
                stdout,
                "{goto}{highlight}{file}{reset}",
                goto = cursor::Goto(1, (i + 1) as u16),
                highlight = color::Bg(color::White),
                file = entry.path().to_str().unwrap(),
                reset = color::Bg(color::Reset)
            )
            .unwrap();
        } else {
            writeln!(
                stdout,
                "{goto}{file}",
                goto = cursor::Goto(1, (i + 1) as u16),
                file = entry.path().to_str().unwrap()
            )
            .unwrap();
        }
    }

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

fn main() {
    let args = Args::parse();

    let notes_directory = "/home/daniel/.notes/";
    let quick_notes_filename = "default_notes.txt";
    let quick_notes_file_path = format!("{}{}", notes_directory, quick_notes_filename);

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

        let mut state = NavigationState {
            selected_index: 0,
            mode: "e".to_string(),
        };

        //edit_mode => navigate(notes_directory, state, stdout, stdin),
        fn navigate<W: Write>(
            notes_directory: &str,
            state: &mut NavigationState,
            stdout: &mut W,
            stdin: &std::io::Stdin,
        ) {
            let files = fs::read_dir(notes_directory).unwrap();
            let file_entries: Vec<std::fs::DirEntry> = files.map(|entry| entry.unwrap()).collect();
            print_files(stdout, &file_entries, state.selected_index);
            for c in stdin.keys() {
                match c.unwrap() {
                    Key::Char('j') => {
                        if state.selected_index < file_entries.len() - 1 {
                            state.selected_index = state.selected_index.saturating_add(1);
                        }
                    }
                    Key::Char('k') => {
                        if state.selected_index > 0 {
                            state.selected_index = state.selected_index.saturating_sub(1);
                        }
                    }
                    Key::Char('q') => {
                        write!(
                            stdout,
                            "{}{}{}",
                            termion::clear::All,
                            cursor::Goto(1, 1),
                            cursor::Show
                        )
                        .unwrap();
                        state.mode = String::from("q");
                        break;
                    }
                    Key::Char('D') => {
                        let file_to_del = file_entries[state.selected_index]
                            .path()
                            .to_str()
                            .expect("valid filename")
                            .to_owned();
                        if !file_to_del.contains("default_notes.txt") {
                            write!(
                                stdout,
                                "{}{}Deleting {}...",
                                termion::clear::All,
                                cursor::Goto(1, 1),
                                file_to_del
                            )
                            .unwrap();
                            stdout.flush().unwrap();
                            thread::sleep(time::Duration::from_secs(1));
                            fs::remove_file(file_to_del).unwrap();
                            state.selected_index = 0;
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
                        //let new_file_name = prompt("Enter a file name: ");
                        state.mode = String::from("p");
                        break;
                    }
                    Key::Char('\n') => {
                        let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
                        launch_editor(
                            file_entries[state.selected_index].path().to_str().unwrap(),
                            &editor,
                        )
                    }
                    _ => {}
                }
                let files = fs::read_dir(notes_directory).unwrap();
                let file_entries: Vec<std::fs::DirEntry> =
                    files.map(|entry| entry.unwrap()).collect();
                print_files(stdout, &file_entries, state.selected_index);
            }
        }

        loop {
            match state.mode.as_str() {
                "e" => {
                    navigate(notes_directory, &mut state, &mut stdout, &stdin);
                }
                "q" => {
                    break;
                }
                "p" => {
                    write!(
                        stdout,
                        "{}{}{}Enter a thing: ",
                        termion::clear::All,
                        cursor::Goto(1, 1),
                        cursor::Show
                    )
                    .unwrap();
                    stdout.flush().unwrap();
                    let _ = stdout.suspend_raw_mode();
                    thread::sleep(time::Duration::from_secs(5));
                    state.mode = String::from("e");
                    let _ = stdout.activate_raw_mode();
                }
                _ => {
                    writeln!(stdout, "Something happened").unwrap();
                    stdout.flush().unwrap();
                }
            }
        }
    } else {
        println!("Invalid arguments.");
    }
}
