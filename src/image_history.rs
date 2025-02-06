/* Image download history management */
use log::{error, info, warn};
use rusqlite::Connection;
use std::path::PathBuf;
use std::process::exit;

pub struct DbImageHistory {
    connection: Connection,
}

impl DbImageHistory {
    /// Opens the Sqlite database if possible and returns a
    /// DbImageHistory with a connection on success (exits
    /// the program otherwise).
    pub fn open(path: PathBuf) -> Self {
        // Converts PathBuf to &str for shellexpand's call
        let path_os_string = path.into_os_string();
        let db_str = match path_os_string.as_os_str().to_str() {
            Some(s) => s,
            None => {
                error!("Failed to convert {path_os_string:?} into a valid UTF-8 string");
                exit(1);
            }
        };

        // expands path taking into account tilde (~) and
        // environment variables
        let db_path = match shellexpand::full(db_str) {
            Ok(p) => p.into_owned(),
            Err(e) => {
                error!("Error expanding '{db_str}': {e}");
                exit(1);
            }
        };

        let connection = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                error!("Error while opening the database at '{db_path}': {e}");
                exit(1);
            }
        };

        DbImageHistory {
            connection,
        }
    }

    /// Create the database if it does not already exists
    pub fn create_db_image_history(self) {
        match self.connection.query_row("SELECT name FROM sqlite_schema WHERE type ='table' AND name LIKE 'cid_table';", [], |row| row.get::<usize, String>(0)) {
            Ok(n) => {
                info!("'cid_table' exists");
                n
            }
            Err(e) => {
                warn!("{e}");
                info!("Creating 'cid_table' table");
                let name = match self.connection.execute("CREATE TABLE cid_table (id INTEGER PRIMARY KEY, name TEXT NOT NULL, date TEXT NOT NULL, checksum TEXT NOT NULL)", ()) {
                    Ok(_) => {
                        info!("Created 'cid_table' table");
                        "cid_table".to_string()
                    }
                    Err(e) => {
                        error!("Error while creating 'cid_table': {e}");
                        exit(1);
                    }
                };
                name
            }
        };
    }
}
