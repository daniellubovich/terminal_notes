use toml::Table;
use toml::Value;

fn _expand_homedir(path: String) -> String {
    if path.starts_with('~') {
        let home_dir =
            home::home_dir().expect("Could not evaluate home directory. That's not good.");
        path.replacen('~', home_dir.to_str().unwrap(), 1)
    } else {
        path
    }
}

pub struct Config {
    notes_directory: String,
    default_notes_file: String,
    default_file_extension: String,
}

impl Config {
    pub fn new(config: toml::Table) -> Self {
        let mut default_notes_dir = home::home_dir().unwrap();
        default_notes_dir.push(".notes/");
        let default_notes_dir = Value::String(default_notes_dir.to_str().unwrap().to_string());
        let notes_directory = config
            .get("notes_directory")
            .unwrap_or(&default_notes_dir)
            .as_str();

        let default_notes_file = Value::String("default_notes.txt".to_string());
        let default_notes_file = config
            .get("default_notes_file")
            .unwrap_or(&default_notes_file)
            .as_str();

        let default_file_extension = Value::String("txt".to_string());
        let default_file_extension = config
            .get("default_file_extension")
            .unwrap_or(&default_file_extension)
            .as_str();

        Config {
            notes_directory: _expand_homedir(notes_directory.unwrap().to_owned()),
            default_notes_file: _expand_homedir(default_notes_file.unwrap().to_owned()),
            default_file_extension: default_file_extension.unwrap().to_owned(),
        }
    }

    pub fn generate() -> Table {
        let mut table = Table::new();
        table.insert(
            String::from("notes_directory"),
            Value::String(String::from("~/.notes/")),
        );
        table.insert(
            String::from("default_file_extension"),
            Value::String(String::from(".txt")),
        );
        table.insert(
            String::from("default_notes_file"),
            Value::String(String::from("default_notes.txt")),
        );

        table
    }

    pub fn get_default_notes_path(&self) -> String {
        format!("{}{}", self.notes_directory, self.default_notes_file)
    }

    pub fn get_default_notes_file(&self) -> &str {
        &self.default_notes_file
    }

    pub fn get_notes_directory(&self) -> &str {
        &self.notes_directory
    }

    pub fn get_default_file_extension(&self) -> &str {
        &self.default_file_extension
    }
}
