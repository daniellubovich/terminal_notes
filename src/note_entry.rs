use std::time::SystemTime;

pub struct NoteEntry {
    pub path: String,
    pub name: String,
    pub modified: SystemTime,
    pub is_default: bool,
}

impl NoteEntry {
    pub fn new(path: String, name: String, modified: SystemTime, is_default: bool) -> Self {
        NoteEntry {
            path,
            name,
            modified,
            is_default,
        }
    }
}
