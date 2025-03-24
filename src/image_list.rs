/* Image list management */
use crate::checksums::CheckSums;
use std::fmt;
// use log::{error, info, warn};
// use serde::Deserialize;

#[derive(Default, PartialEq, Debug)]
pub struct CloudImage {
    pub name: String,
    pub checksum: CheckSums,
}

impl CloudImage {
    pub fn new(name: String, checksum: CheckSums) -> Self {
        CloudImage {
            name,
            checksum,
        }
    }
}

impl fmt::Display for CloudImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.checksum {
            CheckSums::None => writeln!(f, "\t-> {}", self.name),
            CheckSums::Sha256(checksum) | CheckSums::Sha512(checksum) => {
                writeln!(f, "\t-> {} with checksum {}", self.name, checksum)
            }
        }
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

    // Extends the ImageList with another ImageList
    pub fn extend(&mut self, image_list: ImageList) -> &mut Self {
        self.list.extend(image_list.list);
        self
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
