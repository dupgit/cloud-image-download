/* Image list management */
use crate::checksums::CheckSums;
use regex::Regex;
use std::cmp::Ordering;
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

    pub fn compare_dates_in_names(&self, other: &Self) -> Ordering {
        compare_str_by_date(&self.name, &other.name)
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

    // Extends the ImageList with another ImageList
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
}

impl fmt::Display for ImageList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for image in &self.list {
            write!(f, "{image}")?;
        }
        Ok(())
    }
}
