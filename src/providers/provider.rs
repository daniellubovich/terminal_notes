use crate::note_entry::NoteEntry;
use std::{io, path::Path};

pub trait NotesProvider {
    fn get_notes(&self) -> Vec<NoteEntry>;
    fn note_exists(&self, path: &Path) -> bool;
    fn create_note(&self, note: NoteEntry) -> Result<NoteEntry, String>;
    fn rename_note(&self, note: &NoteEntry, new_path: &Path) -> Result<bool, io::Error>;
    fn delete_note(&self, note: &NoteEntry) -> Result<(), String>;
}
