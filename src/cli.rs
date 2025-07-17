use crate::CONCURRENT_REQUESTS;
use clap::Parser;

#[derive(Parser, Debug)]
/// This program downloads cloud image files
/// from configured sites
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Uses an alternative configuration file
    #[arg(long, default_value = "/etc/cid.toml")]
    pub config: String,

    /// path to the database
    pub db_path: Option<String>,

    /// Maximum simultaneous downloads
    #[arg(long, default_value_t = CONCURRENT_REQUESTS)]
    pub concurrent_downloads: usize,

    /// Forces verification of skipped downloads
    #[arg(long, default_value_t = false)]
    pub verify_skipped: bool,

    // Verbosity level managed through clap_verbosity_flag crate
    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
}

impl Cli {
    /// Parsing the Cli and returning the structure filled accordingly
    /// to the command line options
    #[must_use]
    pub fn analyze() -> Self {
        Cli::parse()
    }
}
