/* Image list management */
use crate::checksums::CheckSums;
use crate::download::get_filename_destination;
use crate::image_history::DbImageHistory;
use crate::website::Url;
use chrono::NaiveDateTime;
use colored::Colorize;
use log::{error, info, warn};
use std::fmt;
use std::path::Path;

#[derive(Default, PartialEq, Debug)]
pub struct CloudImage {
    pub url: Url,
    pub name: String,
    pub checksum: CheckSums,
    pub date: NaiveDateTime,
}

impl CloudImage {
    /// Creates a new `CloudImage` structure with `url`,
    /// `checksum`, `name` and `date` fields
    #[must_use]
    pub fn new(url: Url, checksum: CheckSums, name: String, date: NaiveDateTime) -> Self {
        CloudImage {
            url,
            name,
            checksum,
            date,
        }
    }

    /// Normalizes its filename before verifying
    /// itself that its checksum it correct.
    //@todo: simplify and get it shorter
    #[must_use]
    pub fn verify(&self, destination: &Path, normalize: &Option<String>) -> bool {
        let Some(filename) = get_filename_destination(self, destination, normalize) else {
            return false;
        };
        match self.checksum.verify_file(&filename) {
            Ok(no_error) => match no_error {
                Some(success) => {
                    if success {
                        info!("{} Successfully verified {filename}", "🗸".green());
                        return true;
                    }
                    warn!("{} Verifying failed for {filename}", "𐄂".red());
                    false
                }
                None => {
                    // File has not been verified because it has not any associated hash
                    // so let it be correctly not verified and return true :-)
                    warn!("{} {filename} not verified.", "𐄂".yellow());
                    true
                }
            },
            Err(e) => {
                error!("Error verifying {filename}: {e}");
                false
            }
        }
    }

    pub fn is_in_db(&self, db: &DbImageHistory) -> bool {
        // We do not want to fail here and a Result that
        // is an Err means false by default
        db.is_image_in_db(Some(self)).unwrap_or_default()
    }
}

impl fmt::Display for CloudImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.checksum {
            CheckSums::None => writeln!(f, "\t-> {}", self.url.url),
            CheckSums::Sha256(checksum) | CheckSums::Sha512(checksum) => {
                writeln!(f, "\t-> {} with checksum {}", self.url.url, checksum)
            }
        }
    }
}
