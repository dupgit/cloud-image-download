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
