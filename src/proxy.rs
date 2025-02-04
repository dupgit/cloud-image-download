/* Proxy management */
use log::error;
use serde::Deserialize;
use std::env;

/// Structure to store proxies information. It may be gathered from
/// the environment variables `http_proxy` and `https_proxy` or from
/// the configuration file.
#[derive(Debug, Default, Deserialize)]
pub struct Proxies {
    pub http: Option<String>,
    pub https: Option<String>,
}

impl Proxies {
    // Gets an environnment variable as an Option<String> uses
    // both lower and upper case, in that order to get the variable
    // value.
    fn get_variable_value(&self, variable: &str) -> Option<String> {
        let mut value = String::new();

        match env::var(variable.to_lowercase()) {
            Ok(v) => value = v,
            Err(e) => error!("Error: {e}"),
        };
        if value.is_empty() {
            match env::var(variable.to_uppercase()) {
                Ok(v) => value = v,
                Err(e) => error!("Error: {e}"),
            };
        };

        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    pub fn new(&self) -> Self {
        let http = self.get_variable_value("http_proxy");
        let https = self.get_variable_value("https_proxy");
        Proxies {
            http,
            https,
        }
    }
}
