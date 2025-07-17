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
        info!("Opening the database");
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
        if let Some(cloud_image) = cloud_image {
            let image_name = &cloud_image.name;
            let checksum = cloud_image.checksum.to_string();
            let date = cloud_image.date.format("%Y-%m-%d %H:%M:%S").to_string();

            let mut stmt = self
                .conn
                .prepare("SELECT name, checksum, date FROM cid_images WHERE name=?1 AND checksum=?2 AND date=?3")?;
            debug!(
                "SELECT name, checksum, date FROM cid_images WHERE name={image_name} AND checksum={checksum} and date={date}"
            );
            match stmt.query(params![image_name, checksum, date]) {
                Ok(mut rows) => {
                    while let Some(row) = rows.next()? {
                        match (row.get::<usize, String>(0), row.get::<usize, String>(1), row.get::<usize, String>(2)) {
                            (Ok(name), Ok(sum), Ok(d)) => {
                                return Ok(image_name == &name && checksum == sum && date == d);
                            }
                            (Err(e), Ok(_), Ok(_)) | (Ok(_), Err(e), Ok(_)) | (Ok(_), Ok(_), Err(e)) => {
                                warn!("Error while getting parameter: {e}");
                            }
                            (Err(e), Err(f), Ok(_)) | (Err(e), Ok(_), Err(f)) | (Ok(_), Err(e), Err(f)) => {
                                warn!("Error while getting parameters: {e} and {f}");
                            }
                            (Err(e), Err(f), Err(g)) => warn!("Error while getting parameters: {e}, {f} and {g}"),
                        }
                    }
                }
                Err(e) => warn!("Error while executing the query: {e}"),
            }
        }

        Ok(false)
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
