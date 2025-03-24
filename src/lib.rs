#![doc = include_str!("../docs/architecture.md")]

/// Command line interface definition
pub mod cli;

/// All about website
pub mod website;

/// Configuration management
pub mod settings;

/// Proxy management
pub mod proxy;

/// Image download history management
pub mod image_history;

/// Image list management
pub mod image_list;

/// Checksum extraction from downloaded files
pub mod checksums;

/// Maximum concurrent requests that can be made
/// for a single website and maximum number of
/// websites that can be fetched concurrently
pub const CONCURRENT_REQUESTS: usize = 4;
