use std::{path::PathBuf, time::SystemTime};

#[derive(Clone)]
pub struct NoteEntry {
    pub path: PathBuf,
    pub name: String,
    pub modified: SystemTime,
    pub is_default: bool,
}

impl NoteEntry {
    pub fn new(path: PathBuf, name: String, modified: SystemTime, is_default: bool) -> Self {
        NoteEntry {
            path,
            name,
            modified,
            is_default,
        }
    }
}
