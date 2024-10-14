use crate::{note_entry::NoteEntry, SortDir, SortField};
use anyhow::Result;
use std::{path::Path, rc::Rc};

pub trait NotesProvider {
    fn validate_default_note_exists(&self) -> Result<()>;
    fn get_notes(&self, sort_field: &SortField, sort_dir: &SortDir) -> Vec<Rc<NoteEntry>>;
    fn note_exists(&self, path: &Path) -> bool;
    fn create_note(&self, note: NoteEntry) -> Result<NoteEntry>;
    fn rename_note(&self, note: &NoteEntry, new_path: &Path) -> Result<bool>;
    fn delete_note(&self, note: &NoteEntry) -> Result<()>;
}
