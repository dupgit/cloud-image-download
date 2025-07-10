/* Image list management */
use crate::checksums::CheckSums;
use crate::download::get_filename_destination;
use crate::image_history::DbImageHistory;
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
    pub url: String,
    pub name: String,
    pub checksum: CheckSums,
    pub date: NaiveDateTime,
}

impl CloudImage {
    pub fn new(url: String, checksum: CheckSums, name: String, date: NaiveDateTime) -> Self {
        CloudImage {
            url,
            name,
            checksum,
            date,
        }
    }

    /// @todo: simplify and get it shorter
    pub fn verify(&self, destination: &Path, normalize: bool) -> bool {
        if let Some(filename) = get_filename_destination(&self.name, destination, normalize, self.date) {
            match verify_file(&filename, &self.checksum) {
                Ok(no_error) => match no_error {
                    Some(success) => {
                        if success {
                            info!("{} Successfully verified {filename}", "🗸".green());
                            return true;
                        } else {
                            warn!("{} Verifying failed for {filename}", "𐄂".red());
                            return false;
                        }
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
        db.is_image_in_db(Some(self)).unwrap_or_default()
    }
}

pub fn verify_file(filename: &str, checksum: &CheckSums) -> Result<Option<bool>, Box<dyn Error>> {
    let input = match File::open(filename) {
        Ok(input) => input,
        Err(e) => {
            error!("Error while opening {filename}: {e}");
            return Err(Box::new(e));
        }
    };

    let mut reader = BufReader::new(input);

    match checksum {
        CheckSums::None => {
            warn!("No checksum for file {filename}: nothing verified");
            Ok(None)
        }
        CheckSums::Sha256(hash) => {
            info!("Verifying {filename} sha256's checksum");
            let digest = {
                let mut hasher = Sha256::new();
                let mut buffer = vec![0; 16_777_216];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(count) => {
                            if count == 0 {
                                break;
                            }
                            hasher.update(&buffer[..count]);
                        }
                        Err(e) => {
                            error!("Error while reading file {filename} Skipped");
                            return Err(Box::new(e));
                        }
                    }
                }
                hasher.finalize()
            };
            if base16ct::lower::encode_string(&digest) == *hash {
                Ok(Some(true))
            } else {
                Ok(Some(false))
            }
        }
        CheckSums::Sha512(hash) => {
            info!("Verifying {filename} sha512's checksum");
            let digest = {
                let mut hasher = Sha512::new();
                let mut buffer = vec![0; 16_777_216];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(count) => {
                            if count == 0 {
                                break;
                            }
                            hasher.update(&buffer[..count]);
                        }
                        Err(e) => {
                            error!("Error while reading file {filename} Skipped");
                            return Err(Box::new(e));
                        }
                    }
                }
                hasher.finalize()
            };
            if base16ct::lower::encode_string(&digest) == *hash {
                Ok(Some(true))
            } else {
                Ok(Some(false))
            }
        }
    }
}

impl fmt::Display for CloudImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.checksum {
            CheckSums::None => writeln!(f, "\t-> {}", self.url),
            CheckSums::Sha256(checksum) | CheckSums::Sha512(checksum) => {
                writeln!(f, "\t-> {} with checksum {}", self.url, checksum)
            }
        }
    }
}
