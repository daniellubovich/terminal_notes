use crate::note_entry::NoteEntry;
use std::io;

pub trait NotesProvider {
    fn get_notes(&self) -> Vec<NoteEntry>;
    fn validate_note(&self, name: &str) -> Result<NoteEntry, String>;
    fn create_note(&self, note: NoteEntry) -> Result<NoteEntry, String>;
    fn rename_note(&self, old_path: &str, new_path: &str) -> Result<String, io::Error>;
    fn delete_note(&self, note: &NoteEntry) -> Result<(), String>;
}
