/* Configuration management */
use crate::cli::Cli;
use crate::website::WebSite;
use config::Config;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::exit;

#[derive(Debug, Deserialize)]
pub struct WebSiteConfig {
    pub site: WebSite,
    pub destination: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub sites: Vec<WebSiteConfig>,
}

impl Settings {
    pub fn from_config(cli: &Cli) -> Self {
        let config_filename = match shellexpand::full(&cli.config) {
            Ok(conf) => conf,
            Err(e) => {
                eprintln!("Error expanding {}: {e}", cli.config);
                exit(1);
            }
        };

        let config = match Config::builder().add_source(config::File::with_name(&config_filename).required(false)).add_source(config::Environment::with_prefix("CID")).build() {
            Ok(conf) => conf,
            Err(e) => {
                eprintln!("Error: {e}");
                exit(1);
            }
        };

        match config.try_deserialize::<Settings>() {
            Ok(settings) => settings,
            Err(e) => {
                eprintln!("Error deserializing: {e}");
                exit(1);
            }
        }
    }
}

#[test]
fn test_settings_get() {
    use clap_verbosity_flag::Verbosity;
    let cli = Cli {
        config: "test_data/cloud-image-download.toml".to_string(),
        verbose: Verbosity::new(0, 0),
    };
    let settings = Settings::get(&cli);

    assert_eq!(settings.sites.len(), 3);
    println!("{settings:?}");
}
