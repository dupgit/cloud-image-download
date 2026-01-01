use base16ct::lower;
use log::{debug, error, info, trace, warn};
use regex::Regex;
use sha2::{Digest, Sha256, Sha512};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};

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

const HASH_BUFFER_SIZE: usize = 16_777_216;

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
    let mut buffer = vec![0; HASH_BUFFER_SIZE];

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

impl CheckSums {
    /// Builds a `CheckSums` structure with the checksum found in
    /// the line
    fn build_checksums_from_line(line: &str, filename: &str) -> CheckSums {
        if filename.contains("SHA512SUMS") || line.contains("SHA512") {
            let re = Regex::new(r".*([a-f0-9]{128}+).*")
                .expect("Something went wrong because '.*([a-f0-9]{128}+).*' is a valid regex");
            let chksum: String = match re.captures(line) {
                Some(value) => value[1].to_string(),
                None => return CheckSums::None,
            };
            info!("found sha512 checksum '{chksum}'");
            CheckSums::Sha512(chksum)
        } else if filename.contains("SHA256SUMS") || line.contains("SHA256") {
            let re = Regex::new(r".*([a-f0-9]{64}+).*")
                .expect("Something went wrong because '.*([a-f0-9]{64}+).*' is a valid regex");
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
    #[must_use]
    pub fn get_image_checksum_from_checksums_buffer(
        name: &str,
        checksums: &Option<String>,
        filename: &str,
    ) -> CheckSums {
        match checksums {
            Some(buffer) => {
                for line in buffer.lines() {
                    if !line.is_empty() && !line.starts_with('#') {
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

    /// Verifies a file's (named `filename`) checksum (contained in `checksum`)
    ///
    /// # Errors
    ///
    /// It will return errors when
    ///  - the file cannot be opened
    ///  - the file cannot be read
    pub fn verify_file(&self, filename: &str) -> Result<Option<bool>, Box<dyn Error>> {
        match self {
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
}

impl fmt::Display for CheckSums {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            CheckSums::None => writeln!(f),
            CheckSums::Sha256(checksum) | CheckSums::Sha512(checksum) => {
                writeln!(f, "{checksum}")
            }
        }
    }
}

/// Tells if inner String indicates that we are
/// in presence of a checksum files that contains
/// all checksums for all downloadable images
#[must_use]
pub fn are_all_checksums_in_one_file(inner: &str) -> bool {
    // -CHECKSUM is used in Fedora sites
    // CHECKSUM is used in Centos sites
    // SHA256SUMS is used in Ubuntu sites
    // SHA512SUMS is used in Debian sites
    inner.contains("-CHECKSUM") || inner == "CHECKSUM" || inner == "SHA256SUMS" || inner == "SHA512SUMS"
}
