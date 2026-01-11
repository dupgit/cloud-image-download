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
        let count = reader.read(&mut buffer).inspect_err(|e| {
            error!("Error while reading file {filename} skipped: {e}");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper function to create a temporary file with given content
    fn create_temp_file(content: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        file.write_all(content).expect("Failed to write to temp file");
        file.flush().expect("Failed to flush temp file");
        file
    }

    #[test]
    fn test_verify_file_none_checksum() {
        let file = create_temp_file(b"test content");
        let result = CheckSums::None.verify_file(file.path().to_str().unwrap());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_verify_file_sha256_valid() {
        let content = b"Hello, World!";
        let file = create_temp_file(content);

        // SHA256 hash of "Hello, World!"
        let expected_hash = "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f";
        let checksum = CheckSums::Sha256(expected_hash.to_string());

        let result = checksum.verify_file(file.path().to_str().unwrap());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(true));
    }

    #[test]
    fn test_verify_file_sha256_invalid() {
        let content = b"Hello, World!";
        let file = create_temp_file(content);

        // Incorrect hash
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let checksum = CheckSums::Sha256(wrong_hash.to_string());

        let result = checksum.verify_file(file.path().to_str().unwrap());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(false));
    }

    #[test]
    fn test_verify_file_sha512_valid() {
        let content = b"Hello, World!";
        let file = create_temp_file(content);

        // SHA512 hash of "Hello, World!"
        let expected_hash = "374d794a95cdcfd8b35993185fef9ba368f160d8daf432d08ba9f1ed1e5abe6cc69291e0fa2fe0006a52570ef18c19def4e617c33ce52ef0a6e5fbe318cb0387";
        let checksum = CheckSums::Sha512(expected_hash.to_string());

        let result = checksum.verify_file(file.path().to_str().unwrap());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(true));
    }

    #[test]
    fn test_verify_file_sha512_invalid() {
        let content = b"Hello, World!";
        let file = create_temp_file(content);

        // Incorrect hash
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
        let checksum = CheckSums::Sha512(wrong_hash.to_string());

        let result = checksum.verify_file(file.path().to_str().unwrap());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(false));
    }

    #[test]
    fn test_verify_file_nonexistent() {
        let checksum = CheckSums::Sha256("dummy_hash".to_string());
        let result = checksum.verify_file("/nonexistent/file/path.txt");

        assert!(result.is_err());
    }

    #[test]
    fn test_verify_file_empty_file() {
        let file = create_temp_file(b"");

        // SHA256 hash of empty string
        let expected_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let checksum = CheckSums::Sha256(expected_hash.to_string());

        let result = checksum.verify_file(file.path().to_str().unwrap());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(true));
    }

    #[test]
    fn test_verify_file_large_content() {
        // Create a file larger than buffer size (16MB + some extra)
        let large_content = vec![b'A'; 17_000_000];
        let file = create_temp_file(&large_content);

        // SHA256 hash of 17MB of 'A' characters
        let expected_hash = "3e4d2911aa103ff4e2d19f5180d10b099469826f182f8ebc7abd292896ec3fa3";
        let checksum = CheckSums::Sha256(expected_hash.to_string());

        let result = checksum.verify_file(file.path().to_str().unwrap());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(true));
    }

    #[test]
    fn test_verify_with_hasher_directly() {
        let content = b"Test content";
        let file = create_temp_file(content);

        let expected_hash = "9d9595c5d94fb65b824f56e9999527dba9542481580d69feb89056aabaa0aa87";

        let result = verify_with_hasher(file.path().to_str().unwrap(), Sha256::new(), expected_hash);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(true));
    }
}
