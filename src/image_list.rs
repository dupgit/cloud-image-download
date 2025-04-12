/* Image list management */
use crate::checksums::CheckSums;
use crate::download::get_filename_destination;
use crate::image_history::DbImageHistory;
use colored::Colorize;
use log::{error, info, warn};
use regex::Regex;
use sha2::{Digest, Sha256, Sha512};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};
use std::{cmp::Ordering, path::PathBuf};

#[derive(Default, PartialEq, Debug)]
pub struct CloudImage {
    pub url: String,
    pub checksum: CheckSums,
}

impl CloudImage {
    pub fn new(url: String, checksum: CheckSums) -> Self {
        CloudImage {
            url,
            checksum,
        }
    }

    pub fn compare_dates_in_names(&self, other: &Self) -> Ordering {
        compare_str_by_date(&self.url, &other.url)
    }

    /// @todo: simplify and get it shorter
    pub fn verify(&self, destination: &PathBuf) -> bool {
        if let Some((_, filename)) = get_filename_destination(&self.url, destination) {
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
        match db.is_image_in_db(Some(self)) {
            Ok(in_db) => in_db,
            Err(_) => false,
        }
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

fn get_date_from_string(name: &str) -> Option<String> {
    let re = Regex::new(r"(?<name>[2][0-9]{3}[0-1][0-9][0-3][0-9])").unwrap();
    match re.captures(name) {
        Some(capture) => Some(capture["name"].to_string()),
        None => None,
    }
}

pub fn compare_str_by_date(a: &str, b: &str) -> Ordering {
    let date1 = get_date_from_string(a);
    let date2 = get_date_from_string(b);
    match (date1, date2) {
        (Some(d1), Some(d2)) => d1.cmp(&d2),
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

#[derive(Default, Debug)]
pub struct ImageList {
    pub list: Vec<CloudImage>,
}

impl ImageList {
    pub fn new() -> Self {
        ImageList::default()
    }

    /// Adds a new image into the list
    pub fn push(&mut self, cloudimage: CloudImage) -> &mut Self {
        self.list.push(cloudimage);
        self
    }

    /// Extends the ImageList with another ImageList
    pub fn extend(&mut self, image_list: ImageList) -> &mut Self {
        self.list.extend(image_list.list);
        self
    }

    pub fn sort_by_date(&mut self) {
        self.list.sort_by(|a, b| a.compare_dates_in_names(b));
    }

    pub fn only_keep_last_element(&mut self) {
        let len = self.list.len();
        if len >= 1 {
            self.list = vec![self.list.swap_remove(len - 1)];
        }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

impl fmt::Display for ImageList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for image in &self.list {
            write!(f, "{image}")?;
        }
        Ok(())
    }
}
