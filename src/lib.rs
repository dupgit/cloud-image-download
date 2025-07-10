#![doc = include_str!("../docs/architecture.md")]
use const_format::formatcp;

/// Command line interface definition
pub mod cli;

/// All about website
pub mod website;

/// Configuration management
pub mod settings;

/// Image download history management
pub mod image_history;

/// Cloud image management
pub mod cloud_image;

/// Checksum extraction from downloaded files
pub mod checksums;

/// Downloading the images to the configured
/// destination
pub mod download;

/// Maximum concurrent requests that can be made
/// for a single website and maximum number of
/// websites that can be fetched concurrently
/// This can be set with --concurrent_downloads
/// command line option
pub const CONCURRENT_REQUESTS: usize = 4;

/// User Agent for the whole program
pub const CID_USER_AGENT: &str = formatcp!("cid/{}", env!("CARGO_PKG_VERSION"));
