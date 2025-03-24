/* Image download history management */
use crate::image_list::{CloudImage, ImageList};
use log::{error, info, warn};
use rusqlite::Connection;
use std::error::Error;
use std::path::PathBuf;
use std::process::exit;

/// This is a structure to keep the connection to
/// the sqlite database
pub struct DbImageHistory {
    connection: Connection,
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

        let connection = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                error!("Error while opening the database at '{db_path}': {e}");
                exit(1);
            }
        };
        info!("Database opened");

        DbImageHistory {
            connection,
        }
    }

    /// Create the table within the database if it does not already exists
    pub fn create_db_image_history(self) {
        info!("Creating 'cid_table' table if necessary");
        match self.connection.execute(
            "CREATE TABLE IF NOT EXISTS cid_table (id INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL)",
            (),
        ) {
            Ok(_) => {
                info!("Table 'cid_table' exists");
                "cid_table".to_string()
            }
            Err(e) => {
                error!("Error while creating 'cid_table': {e}");
                exit(1);
            }
        };
    }

    fn get_all_image_list(&self) -> Result<ImageList, Box<dyn Error>> {
        info!("Getting all images from db");
        let mut stmt = match self.connection.prepare("SELECT name, checksum FROM 'cid_table'") {
            Ok(stm) => stm,
            Err(e) => {
                error!("Creating sqlite statement: {e}");
                return Err(Box::new(e));
            }
        };

        // @todo review the database that is totally wrong here
        // let rows = stmt.query_map([], |row| Ok(CloudImage::new(row.get(0)?, row.get(1)?)))?;

        // @todo: iter to get the values into a list that we will return
        let mut image_list = ImageList::default();

        /*
        for row in rows {
            image_list.push(row?);
        }
        */

        Ok(image_list)
    }

    /// Checks if the image image_cloud with name and checksum
    /// is already in the db_image_list (the list of all images in the db)
    fn is_image_in_list(&self, cloud_image: &CloudImage, db_image_list: &ImageList) -> bool {
        db_image_list.list.contains(cloud_image)
    }

    /// Filters the whole image_list and returns a new list with
    /// only elements that are not in the database
    pub fn filter_image_list(self, image_list: ImageList) -> ImageList {
        let db_image_list = match self.get_all_image_list() {
            Ok(imagel) => imagel,
            Err(e) => {
                error!("Error: {e} while getting image list from db");
                ImageList::default()
            }
        };

        info!("Filtering image_list");
        let list = image_list
            .list
            .into_iter()
            .filter(|cloud_image| self.is_image_in_list(cloud_image, &db_image_list))
            .collect();
        ImageList {
            list,
        }
    }
}
