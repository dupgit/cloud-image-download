use crate::checksums::CheckSums;
/* Image download history management */
//use crate::image_list::{CloudImage, ImageList};
use log::{error, info, warn};
use rusqlite::{Connection, params};
use std::error::Error;
use std::path::PathBuf;
use std::process::exit;

/// This is a structure to keep the connection to
/// the sqlite database
pub struct DbImageHistory {
    pub conn: Connection,
}

impl DbImageHistory {
    /// Opens the Sqlite database if possible and returns a
    /// DbImageHistory with a connection on success (exits
    /// the program otherwise).
    pub fn open(path: PathBuf) -> Self {
        info!("Opening the database");
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

        let conn = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                error!("Error while opening the database at '{db_path}': {e}");
                exit(1);
            }
        };
        info!("Database opened");

        DbImageHistory {
            conn,
        }
    }

    /// Create the table within the database if it does not already exists
    pub fn create_db_image_history(&self) {
        info!("Creating 'cid_images' table if necessary");
        match self.conn.execute(
            "CREATE TABLE IF NOT EXISTS cid_images (id INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL)",
            (),
        ) {
            Ok(_) => {
                info!("Table 'cid_images' exists");
                // "cid_images".to_string()
            }
            Err(e) => {
                error!("Error while creating 'cid_images': {e}");
                exit(1);
            }
        };
    }

    pub fn is_image_in_db(&self, image_name: &str, checksum: &CheckSums) -> Result<bool, Box<dyn Error>> {
        let checksum = match checksum {
            CheckSums::None => "",
            CheckSums::Sha256(checksum) => checksum,
            CheckSums::Sha512(checksum) => checksum,
        };

        let mut stmt = self.conn.prepare("SELECT name, checksum FROM cid_images WHERE name=(?1) AND checksum=(?2)")?;
        match stmt.execute(params![image_name, checksum]) {
            Ok(nb) => return Ok(nb == 1),
            Err(e) => warn!("Error while executing the query: {e}"),
        }

        Ok(false)
    }
}
