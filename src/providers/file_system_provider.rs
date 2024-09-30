use crate::config::Config;
use crate::note_entry::NoteEntry;
use crate::NotesProvider;
use std::fs;
use std::io;
use std::path::Path;

pub struct FileSystemNotesProvider<'a> {
    config: &'a Config,
}

impl<'a> FileSystemNotesProvider<'a> {
    pub fn new(config: &'a Config) -> FileSystemNotesProvider {
        FileSystemNotesProvider { config }
    }
}

impl<'a> NotesProvider for FileSystemNotesProvider<'a> {
    fn note_exists(&self, path: &Path) -> bool {
        // This might be more complicated in other providers. E.g. a sqlite database might get a
        // path and deconstruct it into a name or ID to check for existence in the DB.
        path.exists()
    }

    fn delete_note(&self, note: &NoteEntry) -> Result<(), String> {
        match fs::remove_file(&note.path) {
            Ok(()) => Ok(()),
            _ => Err(String::from("Could not delete note")),
        }
    }

    fn rename_note(&self, note: &NoteEntry, new_path: &Path) -> Result<bool, io::Error> {
        match fs::rename(&note.path, new_path) {
            Ok(_) => Ok(true),
            Err(error) => Err(error),
        }
    }

    fn create_note(&self, note: NoteEntry) -> Result<NoteEntry, String> {
        match fs::File::create(&note.path) {
            Ok(_) => Ok(note),
            Err(_) => Err(String::from("Could not create file. Exiting.")),
        }
    }

    fn get_notes(&self) -> Vec<NoteEntry> {
        let files = fs::read_dir(self.config.get_notes_directory()).unwrap();
        let mut file_entries: Vec<NoteEntry> = files
            .map(|entry| {
                let file = entry.unwrap();
                let name = file.file_name().to_str().unwrap().to_owned();
                let path = file.path();
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
