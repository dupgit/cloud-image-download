use crate::cloud_image::CloudImage;
use log::{debug, error, info, warn};
use rusqlite::{Connection, params};
use std::error::Error;
use std::marker::{Send, Sync};
use std::path::PathBuf;
use std::process::exit;

/// This is a structure to keep the connection to
/// the sqlite database
pub struct DbImageHistory {
    pub conn: Connection,
}

unsafe impl Send for DbImageHistory {}
unsafe impl Sync for DbImageHistory {}

impl DbImageHistory {
    /// Opens the Sqlite database if possible and returns a
    /// `DbImageHistory` with a connection on success (exits
    /// the program otherwise).
    #[must_use]
    pub fn open(path: PathBuf) -> Self {
        info!("Opening the database ({})", path.display());
        // Converts PathBuf to &str for shellexpand's call
        let path_os_string = path.into_os_string();
        let Some(db_str) = path_os_string.as_os_str().to_str() else {
            error!("Failed to convert {} into a valid UTF-8 string", path_os_string.display());
            exit(1);
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
    /// and adds an index made of the two columns name and checksum if it
    /// does not already exists.
    pub fn create_db_image_history(&self) {
        info!("Creating 'cid_images' table if necessary");
        match self.conn.execute(
            "CREATE TABLE IF NOT EXISTS cid_images (name TEXT NOT NULL, checksum TEXT NOT NULL, date TEXT NOT NULL)",
            (),
        ) {
            Ok(_) => info!("Table 'cid_images' exists"),
            Err(e) => {
                error!("Error while creating 'cid_images': {e}");
                exit(1);
            }
        }

        info!("Creating 'index_name_checksum' index if necessary");
        match self.conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS index_ncd ON cid_images(name, checksum, date)", ()) {
            Ok(_) => info!("Index index_ncd exists"),
            Err(e) => {
                error!("Error while creating index_ncd index: {e}");
                exit(1);
            }
        }
    }

    /// Tells whether the image is already in the database (ie: it already
    /// has been downloaded)
    ///
    /// # Errors
    ///
    /// Will return an error when calling rusqlite `prepare()` method and
    /// sql cannot be converted to a C-compatible string or if the underlying
    /// `SQLite` call fails.
    pub fn is_image_in_db(&self, cloud_image: Option<&CloudImage>) -> Result<bool, Box<dyn Error>> {
        let Some(cloud_image) = cloud_image else {
            return Ok(false);
        };

        let image_name = &cloud_image.name;
        let checksum = cloud_image.checksum.to_string();
        let date = cloud_image.date.format("%Y-%m-%d %H:%M:%S").to_string();

        let mut stmt = self
            .conn
            .prepare("SELECT name, checksum, date FROM cid_images WHERE name=?1 AND checksum=?2 AND date=?3")?;

        debug!(
            "SELECT name, checksum, date FROM cid_images WHERE name={image_name} AND checksum={checksum} and date={date}"
        );

        let exists = stmt.exists(params![cloud_image.name, checksum, date])?;
        Ok(exists)
    }

    /// The image has been downloaded and verified correctly so
    /// we can save the image name, it's checksum and date to
    /// the database so that we won't download it again
    pub fn save_image_in_db(&self, cloud_image: &CloudImage) {
        let image_name = &cloud_image.name;
        let checksum = cloud_image.checksum.to_string();
        let date = cloud_image.date.format("%Y-%m-%d %H:%M:%S").to_string();

        match self.conn.execute(
            "INSERT INTO cid_images (name, checksum, date) VALUES (?1, ?2, ?3)",
            params![image_name, checksum, date],
        ) {
            Ok(inserted) => {
                if inserted == 1 {
                    info!("Inserted {inserted} row successfully into the database");
                } else {
                    warn!("Something strange happened: {inserted} row(s) has been inserted");
                }
            }
            Err(e) => {
                warn!("Error while inserting {image_name} and {checksum} into the database: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksums::CheckSums;
    use crate::website::Url;
    use chrono::NaiveDateTime;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE cid_images (name TEXT, checksum TEXT, date TEXT)", []).unwrap();
        conn
    }

    fn dummy_url() -> Url {
        Url::default()
    }

    #[test]
    fn none_input_returns_false() {
        let db = DbImageHistory {
            conn: setup_db(),
        };
        assert!(!db.is_image_in_db(None).unwrap());
    }

    #[test]
    fn test_db_matching_row_returns_true() {
        let conn = setup_db();
        let checksum = CheckSums::Sha256("9bc2838008fc212eb9b7c7fdf6ea26f572afdeeaa254fa973ef5cd528b87adcf".into());
        let date = NaiveDateTime::parse_from_str("2026-08-27 03:03:00", "%Y-%m-%d %H:%M:%S").unwrap();

        conn.execute(
            "INSERT INTO cid_images (name, checksum, date) VALUES (?1, ?2, ?3)",
            params!["foo.qcow2", checksum.to_string(), "2026-08-27 03:03:00"],
        )
        .unwrap();

        let db = DbImageHistory {
            conn,
        };
        let img = CloudImage::new(dummy_url(), checksum, "foo.qcow2".to_string(), date);
        assert!(db.is_image_in_db(Some(&img)).unwrap());
    }

    #[test]
    fn test_db_no_matching_row_returns_false() {
        let db = DbImageHistory {
            conn: setup_db(),
        };
        let checksum = CheckSums::Sha256("9bc2838008fc212eb9b7c7fdf6ea26f572afdeeaa254fa973ef5cd528b87adcf".into());
        let date = NaiveDateTime::parse_from_str("2026-08-30 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let img = CloudImage::new(dummy_url(), checksum, "foo.qcow2".to_string(), date);
        assert!(!db.is_image_in_db(Some(&img)).unwrap());
    }

    #[test]
    fn test_db_checksum_mismatch_returns_false() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO cid_images (name, checksum, date) VALUES (?1, ?2, ?3)",
            params![
                "foo.qcow2",
                "9cff5cbfd35c0c9070619c83ce9cad65d4eab845c9b83fb72c1b9209453d085b",
                "2026-08-27 03:03:00"
            ],
        )
        .unwrap();

        let db = DbImageHistory {
            conn,
        };
        let checksum = CheckSums::Sha256("9bc2838008fc212eb9b7c7fdf6ea26f572afdeeaa254fa973ef5cd528b87adcf".into());
        let date = NaiveDateTime::parse_from_str("2026-08-27 03:03:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let img = CloudImage::new(dummy_url(), checksum, "foo.qcow2".to_string(), date);
        assert!(!db.is_image_in_db(Some(&img)).unwrap());
    }
}
