use base16ct::lower;
use httpdirectory::httpdirectory::HttpDirectory;
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

pub enum ChecksumSource {
    /// A single file contains the checksums for all images.
    Combined {
        checksum_filename: String,
    },

    /// Each image has its own checksum file (`{image}.SHA256SUM`).
    PerImage,

    /// No usable checksum file was found.
    None,
}

impl ChecksumSource {
    /// Determines where lies the checksums : all in one file or one checksum file
    /// per image file or none if neither
    ///
    /// # Panics
    /// This function will panic if suddenly `.SHA256SUM` is not recognize as
    /// a valid regex anymore.
    #[must_use]
    pub fn determine_checksum_source(url_httpdir: &HttpDirectory) -> ChecksumSource {
        // Trying to find if we have a single file that contains all checksums
        // for the files to be downloaded.
        let one_file = url_httpdir.files().filtering(|e| e.name().is_some_and(are_all_checksums_in_one_file));

        if one_file.len() == 1 {
            match one_file.first().and_then(|e| e.name()) {
                Some(name) => {
                    return ChecksumSource::Combined {
                        checksum_filename: name.to_string(),
                    };
                }
                None => warn!(
                    "Found exactly one combined-checksum file candidate but it has no name; \
                     falling back to per-image checksum detection"
                ),
            }
        }

        // @todo: some assumption here is made that a checksum made for each
        //        file is a SHA256. May be we can add a filter like the one
        //        are_all_checksums_in_one_file to get SHA512SUM also ?
        // ".SHA256SUM" is a fixed, valid regex: filter_by_name cannot fail here.
        let per_image_checksum_file_count =
            url_httpdir.files().filter_by_name(".SHA256SUM").expect(".SHA256SUM is a known-valid regex").len();

        if per_image_checksum_file_count >= 1 {
            ChecksumSource::PerImage
        } else {
            ChecksumSource::None
        }
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

/// Returns the value that matched the regex (ie: the hash itself). This value
/// is normalized to lowercase (to ease comparisons)
fn get_checksum_from_line_with_regex(line: &str, size: u8) -> Option<String> {
    // Boundaries `(?:^|[^a-f0-9])` and `(?:[^a-f0-9]|$)` ensure the captured
    // run is exactly `size` hex chars — not a prefix/suffix of a longer
    // consecutive hex run.
    let regex = format!(r"(?:^|[^a-f0-9])([a-f0-9]{{{size}}})(?:[^a-f0-9]|$)");
    let re = Regex::new(&regex).expect("Something went wrong because '{regex}' is a valid regex");
    match re.captures(&line.to_ascii_lowercase()) {
        Some(value) => Some(value[1].to_string()),
        None => None,
    }
}

impl CheckSums {
    /// Builds a `CheckSums` structure with the checksum found in
    /// the line guessing its type from the filename or the line content
    fn build_checksums_from_line(line: &str, filename: &str) -> CheckSums {
        if filename.contains("SHA512SUMS") || line.contains("SHA512") {
            let Some(chksum) = get_checksum_from_line_with_regex(line, 128) else {
                return CheckSums::None;
            };
            info!("found sha512 checksum '{chksum}'");
            CheckSums::Sha512(chksum)
        } else if filename.contains("SHA256SUMS") || line.contains("SHA256") {
            let Some(chksum) = get_checksum_from_line_with_regex(line, 64) else {
                return CheckSums::None;
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
                write!(f, "{checksum}")
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
mod verify_file_tests {
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
    fn verify_file_none_checksum() {
        let file = create_temp_file(b"test content");
        let result = CheckSums::None.verify_file(file.path().to_str().unwrap());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn verify_file_sha256_valid() {
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
    fn verify_file_sha256_invalid() {
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
    fn verify_file_sha512_valid() {
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
    fn verify_file_sha512_invalid() {
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
    fn verify_file_nonexistent() {
        let checksum = CheckSums::Sha256("dummy_hash".to_string());
        let result = checksum.verify_file("/nonexistent/file/path.txt");

        assert!(result.is_err());
    }

    #[test]
    fn verify_file_empty_file() {
        let file = create_temp_file(b"");

        // SHA256 hash of empty string
        let expected_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let checksum = CheckSums::Sha256(expected_hash.to_string());

        let result = checksum.verify_file(file.path().to_str().unwrap());

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(true));
    }

    #[test]
    fn verify_file_large_content() {
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
    fn verify_with_hasher_directly() {
        let content = b"Test content";
        let file = create_temp_file(content);

        let expected_hash = "9d9595c5d94fb65b824f56e9999527dba9542481580d69feb89056aabaa0aa87";

        let result = verify_with_hasher(file.path().to_str().unwrap(), Sha256::new(), expected_hash);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(true));
    }
}

#[cfg(test)]
mod checksum_source_tests {
    use super::*;
    use httpdirectory::httpdirectory::HttpDirectory;
    use httpdirectory::httpdirectoryentry::HttpDirectoryEntry;

    fn prepare_httpdir(entries: Vec<HttpDirectoryEntry>) -> HttpDirectory {
        HttpDirectory::from_entries(entries)
    }

    #[test]
    fn combined_checksum_file_detected() {
        let httpdir = prepare_httpdir(vec![
            HttpDirectoryEntry::new("debian-12.qcow2", "2026-08-21 17:03", "500M", "debian-12.qcow2"),
            HttpDirectoryEntry::new("SHA256SUMS", "2026-08-21 17:03", "1K", "SHA256SUMS"),
        ]);

        match ChecksumSource::determine_checksum_source(&httpdir) {
            ChecksumSource::Combined {
                checksum_filename,
            } => {
                assert_eq!(checksum_filename, "SHA256SUMS");
            }
            _ => panic!("expected ChecksumSource::Combined"),
        }
    }

    #[test]
    fn per_image_checksum_file_detected() {
        let httpdir = prepare_httpdir(vec![
            HttpDirectoryEntry::new(
                "jammy-server-cloudimg-amd64.qcow2",
                "2026-08-29 11:19",
                "600M",
                "jammy-server-cloudimg-amd64.qcow2",
            ),
            HttpDirectoryEntry::new(
                "jammy-server-cloudimg-amd64.qcow2.SHA256SUM",
                "2026-08-29 11:19",
                "100",
                "jammy-server-cloudimg-amd64.qcow2.SHA256SUM",
            ),
        ]);

        assert!(matches!(ChecksumSource::determine_checksum_source(&httpdir), ChecksumSource::PerImage));
    }

    #[test]
    fn no_checksum_file_found() {
        let httpdir = prepare_httpdir(vec![HttpDirectoryEntry::new(
            "jammy-server-cloudimg-amd64.qcow2",
            "2026-08-29 11:19",
            "600M",
            "jammy-server-cloudimg-amd64.qcow2",
        )]);

        assert!(matches!(ChecksumSource::determine_checksum_source(&httpdir), ChecksumSource::None));
    }
}

#[cfg(test)]
mod checksum_pattern_tests {
    use super::*;

    #[test]
    fn fedora_style_suffix_matches() {
        assert!(are_all_checksums_in_one_file("Fedora-Cloud-44-1.7-x86_64-CHECKSUM"));
    }

    #[test]
    fn centos_style_exact_match() {
        assert!(are_all_checksums_in_one_file("CHECKSUM"));
    }

    #[test]
    fn ubuntu_style_exact_match() {
        assert!(are_all_checksums_in_one_file("SHA256SUMS"));
    }

    #[test]
    fn debian_style_exact_match() {
        assert!(are_all_checksums_in_one_file("SHA512SUMS"));
    }

    #[test]
    fn unrelated_filename_does_not_match() {
        assert!(!are_all_checksums_in_one_file("debian-12-genericcloud-amd64.qcow2"));
    }

    #[test]
    fn per_image_checksum_file_does_not_match_combined_pattern() {
        assert!(!are_all_checksums_in_one_file("jammy-server-cloudimg-amd64.img.SHA256SUM"));
    }
}

#[cfg(test)]
mod checksum_extractor_tests {
    use super::*;

    #[test]
    fn extracts_sha256_checksum_from_plain_line() {
        let hash = "9d9595c5d94fb65b824f56e9999527dba9542481580d69feb89056aabaa0aa87";
        let line = format!("{hash}  archive.iso");
        assert_eq!(get_checksum_from_line_with_regex(&line, 64), Some(hash.to_string()));
    }

    #[test]
    fn extracts_sha512_checksum_from_plain_line() {
        let hash = "a".repeat(128);
        let line = format!("{hash}  archive.iso");
        assert_eq!(get_checksum_from_line_with_regex(&line, 128), Some(hash));
    }

    #[test]
    fn returns_none_when_hex_string_too_short() {
        // Only 63 hex chars: must not match a size-64 pattern
        let line = "9d9595c5d94fb65b824f56e9999527dba9542481580d69feb89056aabaa0aa8 file";
        assert_eq!(get_checksum_from_line_with_regex(line, 64), None);
    }

    #[test]
    fn extracts_sha256_checksum_on_uppercase_64_char_hex() {
        let hash = "9D9595C5D94FB65B824F56E9999527DBA9542481580D69FEB89056AABAA0AA87";
        let line = format!("{hash}  file.iso");
        assert_eq!(get_checksum_from_line_with_regex(&line, 64), Some(hash.to_string().to_ascii_lowercase()));
    }

    #[test]
    fn extracts_sha256_checksum_on_mixedcase_64_char_hex() {
        let hash = "9D9595c5d94fb65b824F56E9999527dba9542481580d69feb89056aabAA0AA87";
        let line = format!("{hash}  file.iso");
        assert_eq!(get_checksum_from_line_with_regex(&line, 64), Some(hash.to_string().to_ascii_lowercase()));
    }

    #[test]
    fn returns_none_on_empty_line() {
        assert_eq!(get_checksum_from_line_with_regex("", 64), None);
    }

    #[test]
    fn rejects_hex_run_longer_than_requested_size() {
        let hash = "a".repeat(128); // 2 * 64, no separator
        let line = format!("{hash}  file.iso");
        assert_eq!(get_checksum_from_line_with_regex(&line, 64), None);
    }

    #[test]
    fn rejects_hex_run_one_char_longer_than_size() {
        let wrong_hash = "a".repeat(65);
        let line = format!("{wrong_hash}  file.iso");
        assert_eq!(get_checksum_from_line_with_regex(&line, 64), None);
    }

    #[test]
    fn still_extracts_exact_size_hex_run_with_delimiters() {
        let hash = "a".repeat(64);
        let line = format!("{hash}  file.iso");
        assert_eq!(get_checksum_from_line_with_regex(&line, 64), Some(hash));
    }

    #[test]
    fn detects_sha512_from_filename() {
        let hash = "b".repeat(128);
        let line = format!("{hash}  archive.tar.gz");
        let result = CheckSums::build_checksums_from_line(&line, "SHA512SUMS");
        assert_eq!(result, CheckSums::Sha512(hash));
    }

    #[test]
    fn detects_sha512_from_line_content_regardless_of_filename() {
        let hash = "c".repeat(128);
        let line = format!("SHA512 {hash} archive.tar.gz");
        let result = CheckSums::build_checksums_from_line(&line, "checksums.txt");
        assert_eq!(result, CheckSums::Sha512(hash));
    }

    #[test]
    fn detects_sha256_from_filename() {
        let hash = "d".repeat(64);
        let line = format!("{hash}  archive.tar.gz");
        let result = CheckSums::build_checksums_from_line(&line, "SHA256SUMS");
        assert_eq!(result, CheckSums::Sha256(hash));
    }

    #[test]
    fn detects_sha256_from_line_content_regardless_of_filename() {
        let hash = "e".repeat(64);
        let line = format!("SHA256 {hash} archive.tar.gz");
        let result = CheckSums::build_checksums_from_line(&line, "checksums.txt");
        assert_eq!(result, CheckSums::Sha256(hash));
    }

    #[test]
    fn returns_none_when_no_shatype_hint_present() {
        let hash = "f".repeat(64);
        let line = format!("{hash}  archive.tar.gz");
        let result = CheckSums::build_checksums_from_line(&line, "checksums.txt");
        assert_eq!(result, CheckSums::None);
    }

    #[test]
    fn returns_none_when_type_detected_but_hash_missing() {
        // Filename signals SHA256SUMS but the line has no valid hex payload.
        let line = "not a checksum line";
        let result = CheckSums::build_checksums_from_line(line, "SHA256SUMS");
        assert_eq!(result, CheckSums::None);

        // Filename signals SHA512SUMS but the line has no valid hex payload.
        let result = CheckSums::build_checksums_from_line(line, "SHA512SUMS");
        assert_eq!(result, CheckSums::None);
    }

    #[test]
    fn sha512_takes_priority_when_line_mentions_both_filenames_and_types() {
        // Filename says SHA256SUMS, but the line content explicitly says
        // SHA512 — current logic checks SHA512 conditions first, so it wins.
        // Pin this precedence so a refactor doesn't flip it silently.
        let hash = "1".repeat(128);
        let line = format!("SHA512 ({hash}) archive.tar.gz");
        let result = CheckSums::build_checksums_from_line(&line, "SHA256SUMS");
        assert_eq!(result, CheckSums::Sha512(hash));
    }
}
