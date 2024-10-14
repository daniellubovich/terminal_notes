use std::{path::PathBuf, time::SystemTime};

use crate::render::{Column, Columnar, Field};

const DATE_FORMAT: &str = "%b %m %I:%M";

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

impl Columnar for NoteEntry {
    fn get_value(&self, column: &Column) -> String {
        match column.get_field() {
            Field::Size => self.size.to_string(),
            Field::Name => {
                let default_indicator = "  [Default]".to_owned();
                if self.is_default {
                    format!("{}{}", self.name, default_indicator)
                } else {
                    self.name.to_string()
                }
            }
            Field::Modified => {
                let date: chrono::DateTime<chrono::Local> = self.modified.into();
                date.format(DATE_FORMAT).to_string()
            }
        }
    }
}
