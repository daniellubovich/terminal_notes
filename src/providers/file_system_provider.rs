use crate::config::Config;
use crate::note_entry::NoteEntry;
use crate::NotesProvider;
use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

pub struct FileSystemNotesProvider<'a> {
    config: &'a Config,
}

impl<'a> FileSystemNotesProvider<'a> {
    pub fn new(config: &'a Config) -> FileSystemNotesProvider {
        FileSystemNotesProvider { config }
    }
}

impl<'a> NotesProvider for FileSystemNotesProvider<'a> {
    fn delete_note(&self, note: &NoteEntry) -> Result<(), String> {
        match fs::remove_file(&note.path) {
            Ok(()) => Ok(()),
            _ => Err(String::from("Could not delete note")),
        }
    }

    fn rename_note(&self, old_path: &str, new_path: &str) -> Result<String, io::Error> {
        match fs::rename(old_path, new_path) {
            Ok(_) => Ok(new_path.to_string()),
            Err(error) => Err(error),
        }
    }

    fn create_note(&self, note: NoteEntry) -> Result<NoteEntry, String> {
        match fs::File::create(&note.path) {
            Ok(_) => Ok(note),
            Err(_) => Err(String::from("Could not create file. Exiting.")),
        }
    }

    fn validate_note(&self, name: &str) -> Result<NoteEntry, String> {
        // Check for empty entry.  Re-prompt if it is.
        if name.is_empty() {
            return Err(String::from("File name empty. Try again, bro."));
        }

        let new_note_path = format!("{}{}", self.config.get_notes_directory(), name);
        let new_note_path = Path::new(&new_note_path);

        // Check for a valid extension and add one if there isn't one.
        let new_note_path = match new_note_path.extension() {
            // TODO maybe check from a list of valid extensions?
            Some(_) => new_note_path.to_path_buf(),
            None => {
                let path_with_ext = new_note_path.to_str().unwrap().to_owned()
                    + self.config.get_default_file_extension();
                Path::new(&path_with_ext).to_path_buf()
            }
        };

        // Check to confirm the file doesn't already exist. Re-prompt
        // if it does.
        if new_note_path.exists() {
            return Err(format!(
                "File {} already exists",
                new_note_path.to_str().expect("file path is present")
            ));
        }

        let new_note_entry = NoteEntry::new(
            new_note_path
                .to_str()
                .expect("Invalid file path")
                .to_owned(),
            name.to_string(),
            SystemTime::now(),
            false,
        );

        Ok(new_note_entry)
    }

    fn get_notes(&self) -> Vec<NoteEntry> {
        let files = fs::read_dir(self.config.get_notes_directory()).unwrap();
        let mut file_entries: Vec<NoteEntry> = files
            .map(|entry| {
                let file = entry.unwrap();
                let name = file.file_name().to_str().unwrap().to_owned();
                let path = file.path().to_str().unwrap().to_owned();
                let is_default = name == self.config.get_default_notes_file();
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
}
