use log::{debug, info, trace};
use regex::Regex;
use std::fmt;

/// An enum to allow different checksum types
/// For now there is only sha256 and sha512
/// all other cases is a `CheckSums::None`
#[derive(PartialEq, Default, Debug)]
pub enum CheckSums {
    Sha256(String),
    Sha512(String),
    #[default]
    None,
}

impl CheckSums {
    /// Builds a CheckSums structure with the checksum found in
    /// the line
    fn build_checksums_from_line(line: &str, filename: &str) -> CheckSums {
        if filename.contains("SHA512SUMS") || line.contains("SHA512") {
            let re = Regex::new(r".*([a-f0-9]{128}+).*").unwrap();
            let chksum: String = match re.captures(line) {
                Some(value) => value[1].to_string(),
                None => return CheckSums::None,
            };
            info!("found sha512 checksum '{chksum}'");
            CheckSums::Sha512(chksum)
        } else if filename.contains("SHA256SUMS") || line.contains("SHA256") {
            let re = Regex::new(r".*([a-f0-9]{64}+).*").unwrap();
            let chksum: String = match re.captures(line) {
                Some(value) => value[1].to_string(),
                None => return CheckSums::None,
            };
            info!("found sha256 checksum '{chksum}'");
            CheckSums::Sha256(chksum)
        } else {
            info!("no checksum found (not a SHA256 or SHA512 ?)");
            CheckSums::None
        }
    }

    /// retrieves the checksum of the image named `name` in the buffer
    /// `checksums` that is the content of a file containing at least
    /// one checksum. `filename` is the filename of that file containing
    /// all checksums. We decide with its name the kind of checksums
    /// it contains (sha256 or sha512) along with the content of the
    /// line that may also be helpful
    pub fn get_image_checksum_from_checksums_buffer(
        name: &str,
        checksums: &Option<String>,
        filename: &str,
    ) -> CheckSums {
        match checksums {
            Some(buffer) => {
                for line in buffer.lines() {
                    if !line.is_empty() && !line.starts_with("#") {
                        trace!("line: {line}");
                        if line.contains(name) {
                            debug!("line: {line}");
                            return CheckSums::build_checksums_from_line(line, filename);
                        }
                    }
                }
                info!("no checksum found");
            }
            None => info!("no checksum buffer to analyze"),
        }
        CheckSums::None
    }
}

impl fmt::Display for CheckSums {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            CheckSums::None => writeln!(f),
            CheckSums::Sha256(checksum) | CheckSums::Sha512(checksum) => {
                writeln!(f, "{}", checksum)
            }
        }
    }
}

/// Tells if inner String indicates that we are
/// in presence of a checksum files that contains
/// all checksums for all downloadable images
pub fn are_all_checksums_in_one_file(inner: &str) -> bool {
    // -CHECKSUM is used in Fedora sites
    // CHECKSUM is used in Centos sites
    // SHA256SUMS is used in Ubuntu sites
    // SHA512SUMS is used in Debian sites
    inner.contains("-CHECKSUM") || inner == "CHECKSUM" || inner == "SHA256SUMS" || inner == "SHA512SUMS"
}
