use std::{path::PathBuf, time::SystemTime};

#[derive(Clone)]
pub struct NoteEntry {
    pub path: PathBuf,
    pub name: String,
    pub modified: SystemTime,
    pub is_default: bool,
    pub size: u64,
}

impl NoteEntry {
    pub fn new(
        path: PathBuf,
        name: String,
        modified: SystemTime,
        is_default: bool,
        size: u64,
    ) -> Self {
        NoteEntry {
            path,
            name,
            modified,
            is_default,
            size,
        }
    }

    pub fn get_size(&self) -> &u64 {
        &self.size
    }
}
