/* Configuration management */
use crate::cli::Cli;
use crate::website::WebSite;
use config::Config;
use log::error;
use serde::Deserialize;
use std::process::exit;

/// Stores settings read from a configuration file.
#[derive(Debug, Deserialize)]
pub struct Settings {
    pub db_path: Option<String>,
    pub sites: Vec<WebSite>,
}

impl Settings {
    /// Deserializes (if possible) the whole configuration file that
    /// may have been specified in the command line
    #[must_use]
    pub fn from_config(cli: &Cli) -> Self {
        let config_filename = match shellexpand::full(&cli.config) {
            Ok(conf) => conf,
            Err(e) => {
                error!("Error expanding {}: {e}", cli.config);
                exit(1);
            }
        };

        let config = match Config::builder()
            .add_source(config::File::with_name(&config_filename).required(false))
            .add_source(config::Environment::with_prefix("CID"))
            .build()
        {
            Ok(conf) => conf,
            Err(e) => {
                error!("Error: {e}");
                exit(1);
            }
        };

        let mut settings = match config.try_deserialize::<Settings>() {
            Ok(settings) => settings,
            Err(e) => {
                error!("Error deserializing: {e}");
                exit(1);
            }
        };

        // To give the command line option the latest word
        if let Some(db_path) = &cli.db_path {
            settings.db_path = Some(db_path.to_string());
        }

        settings
    }
}

// Tests that the test configuration has been correctly parsed
#[test]
fn test_settings_from_config() {
    use clap_verbosity_flag::Verbosity;

    let cli = Cli {
        db_path: None,
        config: "test_data/cloud-image-download.toml".to_string(),
        verbose: Verbosity::new(0, 0),
        concurrent_downloads: 2,
        verify_skipped: false,
    };

    let settings = Settings::from_config(&cli);

    assert_eq!(settings.sites.len(), 4);
    assert_eq!(settings.db_path, Some("~/.cache/cid".to_string()));
}

// Tests cli precedence
#[test]
fn test_db_path_settings_from_config() {
    use clap_verbosity_flag::Verbosity;

    let cli = Cli {
        db_path: Some("/var/lib/cid".to_string()),
        config: "test_data/cloud-image-download.toml".to_string(),
        verbose: Verbosity::new(0, 0),
        concurrent_downloads: 2,
        verify_skipped: false,
    };

    let settings = Settings::from_config(&cli);
    assert_eq!(settings.db_path, Some("/var/lib/cid".to_string()));
}

// Tests default
#[test]
fn test_db_path_settings_default() {
    use clap_verbosity_flag::Verbosity;

    let cli = Cli {
        db_path: None,
        config: "test_data/no_db_path.toml".to_string(),
        verbose: Verbosity::new(0, 0),
        concurrent_downloads: 2,
        verify_skipped: false,
    };

    let settings = Settings::from_config(&cli);
    assert_eq!(settings.db_path, None);
}
