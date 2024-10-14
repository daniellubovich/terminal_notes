use crate::config::Config;
use crate::note_entry::NoteEntry;
use crate::NotesProvider;
use crate::SortDir;
use crate::SortField;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::rc::Rc;

pub struct FileSystemNotesProvider<'a> {
    config: &'a Config,
}

impl<'a> FileSystemNotesProvider<'a> {
    pub fn new(config: &'a Config) -> FileSystemNotesProvider {
        FileSystemNotesProvider { config }
    }
}

impl<'a> NotesProvider for FileSystemNotesProvider<'a> {
    fn validate_default_note_exists(&self) -> Result<()> {
        if !Path::new(&self.config.get_notes_directory()).exists() {
            bail!(format!(
                "No {} folder exists. Please create it first.",
                self.config.get_notes_directory()
            ))
        }

        if !Path::new(&self.config.get_default_notes_path()).exists() {
            bail!(format!(
                "No default notes file {} exists. Please create it first.",
                self.config.get_default_notes_file()
            ))
        }

        Ok(())
    }

    fn note_exists(&self, path: &Path) -> bool {
        // This might be more complicated in other providers. E.g. a sqlite database might get a
        // path and deconstruct it into a name or ID to check for existence in the DB.
        path.exists()
    }

    fn delete_note(&self, note: &NoteEntry) -> Result<()> {
        fs::remove_file(&note.path)?;
        Ok(())
    }

    fn rename_note(&self, note: &NoteEntry, new_path: &Path) -> Result<bool> {
        match fs::rename(&note.path, new_path) {
            Ok(_) => Ok(true),
            Err(error) => Err(error.into()),
        }
    }

    fn create_note(&self, note: NoteEntry) -> Result<NoteEntry> {
        match fs::File::create(&note.path) {
            Ok(_) => Ok(note),
            Err(error) => Err(error).context("error creating note"),
        }
    }

    fn get_notes(&self, sort_field: &SortField, sort_dir: &SortDir) -> Vec<Rc<NoteEntry>> {
        let files = fs::read_dir(self.config.get_notes_directory()).unwrap();
        let mut file_entries: Vec<Rc<NoteEntry>> = files
            .filter(|entry| {
                // Filter out directories
                let file = entry.as_ref().unwrap();
                !file.metadata().unwrap().is_dir()
            })
            .map(|entry| {
                let file = entry.unwrap();
                let name = file.file_name().to_str().unwrap().to_owned();
                let path = file.path();
                let is_default = name == self.config.get_default_notes_file();
                Rc::new(NoteEntry::new(
                    path,
                    name,
                    file.metadata().unwrap().modified().unwrap(),
                    is_default,
                    file.metadata().unwrap().size(),
                ))
            })
            .collect();

        file_entries.sort_by(|a, b| match sort_field {
            SortField::Modified => {
                let a_cmp = a.modified;
                let b_cmp = b.modified;
                match sort_dir {
                    SortDir::Asc => a_cmp.cmp(&b_cmp),
                    SortDir::Desc => b_cmp.cmp(&a_cmp),
                }
            }
            SortField::Size => {
                let a_cmp = a.get_size();
                let b_cmp = b.get_size();
                match sort_dir {
                    SortDir::Asc => a_cmp.cmp(b_cmp),
                    SortDir::Desc => b_cmp.cmp(a_cmp),
                }
            }
            SortField::Name => {
                let a_cmp = &a.name;
                let b_cmp = &b.name;
                match sort_dir {
                    SortDir::Asc => a_cmp.cmp(b_cmp),
                    SortDir::Desc => b_cmp.cmp(a_cmp),
                }
            }
        });
        file_entries
    }
}
