/* Image list management */
use crate::checksums::CheckSums;
use crate::download::get_filename_destination;
use crate::image_history::DbImageHistory;
use crate::website::Url;
use base16ct::lower;
use chrono::NaiveDateTime;
use colored::Colorize;
use log::{error, info, warn};
use sha2::{Digest, Sha256, Sha512};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};
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
        if let Some(filename) = get_filename_destination(self, destination, normalize) {
            match verify_file(&filename, &self.checksum) {
                Ok(no_error) => match no_error {
                    Some(success) => {
                        if success {
                            info!("{} Successfully verified {filename}", "🗸".green());
                            return true;
                        }
                        warn!("{} Verifying failed for {filename}", "𐄂".red());
                        return false;
                    }
                    None => {
                        // File has not been verified because it has not any associated hash
                        // so let it be correctly not verified and return true :-)
                        warn!("{} {filename} not verified.", "𐄂".red());
                        return true;
                    }
                },
                Err(e) => {
                    error!("Error verifying {filename}: {e}");
                    return false;
                }
            }
        }
        false
    }

    pub fn is_in_db(&self, db: &DbImageHistory) -> bool {
        // We do not want to fail here and a Result that
        // is an Err means false by default
        db.is_image_in_db(Some(self)).unwrap_or_default()
    }
}

/// Generic hash verification helper
///
/// # Errors
///
/// It will return errors when
///  - the file can not be opened
///  - the file can not be read
fn verify_with_hasher<D: Digest>(
    filename: &str,
    mut hasher: D,
    expected_hash: &str,
) -> Result<Option<bool>, Box<dyn Error>> {
    let input = File::open(filename).map_err(|e| {
        error!("Error while opening {filename}: {e}");
        e
    })?;

    let mut reader = BufReader::new(input);
    let mut buffer = vec![0; 16_777_216];

    loop {
        let count = reader.read(&mut buffer).map_err(|e| {
            error!("Error while reading file {filename}. Skipped");
            e
        })?;

        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    let digest = hasher.finalize();
    Ok(Some(lower::encode_string(&digest) == expected_hash))
}

/// Verifies a file's (named `filename`) checksum (contained in `checksum`)
///
/// # Errors
///
/// It will return errors when
///  - the file cannot be opened
///  - the file cannot be read
pub fn verify_file(filename: &str, checksum: &CheckSums) -> Result<Option<bool>, Box<dyn Error>> {
    match checksum {
        CheckSums::None => {
            warn!("No checksum for file {filename}: nothing verified");
            Ok(None)
        }
        CheckSums::Sha256(hash) => {
            info!("Verifying {filename} sha256's checksum");
            verify_with_hasher(filename, Sha256::new(), hash)
        }
        CheckSums::Sha512(hash) => {
            info!("Verifying {filename} sha512's checksum");
            verify_with_hasher(filename, Sha512::new(), hash)
        }
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
